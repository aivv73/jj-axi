use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ffi::OsString;
use std::io;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;

use chrono::Local;
use futures::{StreamExt as _, TryStreamExt as _};
use jj_cli::cli_util::{load_fileset_aliases, load_revset_aliases, update_working_copy};
use jj_cli::config::{ConfigEnv, config_from_environment, default_config_layers};
use jj_cli::description_util::join_message_paragraphs;
use jj_cli::git_util::is_colocated_git_workspace;
use jj_cli::revset_util::{parse_bookmark_name, parse_immutable_heads_expression};
use jj_cli::ui::Ui;
use jj_lib::absorb::{AbsorbSource, SelectedTrees, absorb_hunks, split_hunks_to_trees};
use jj_lib::backend::{ChangeId, CommitId, CopyId, TreeValue};
use jj_lib::commit::Commit;
use jj_lib::config::ConfigGetResultExt as _;
use jj_lib::conflicts::{
    ConflictMarkerStyle, ConflictMaterializeOptions, MaterializedTreeValue,
    materialize_merge_result_to_bytes, materialize_tree_value,
};
use jj_lib::diff::{ContentDiff, DiffHunkKind};
use jj_lib::git::{
    self, GitProgress, GitPushError, GitPushOptions, GitPushRefTargets, GitSettings,
    GitSidebandLineTerminator, GitSubprocessCallback,
};
use jj_lib::lock::FileLock;
use jj_lib::matchers::EverythingMatcher;
use jj_lib::merge::{Diff as JjDiff, Merge};
use jj_lib::merged_tree::{MergedTree, TreeDiffEntry};
use jj_lib::merged_tree_builder::MergedTreeBuilder;
use jj_lib::object_id::{HexPrefix, ObjectId, PrefixResolution};
use jj_lib::op_store::RefTarget;
use jj_lib::op_walk;
use jj_lib::ref_name::{RefNameBuf, RemoteName, RemoteNameBuf};
use jj_lib::refs::{LocalAndRemoteRef, RefPushAction, classify_ref_push_action};
use jj_lib::repo::{Repo as _, StoreFactories};
use jj_lib::repo_path::{RepoPath, RepoPathBuf, RepoPathUiConverter};
use jj_lib::revset::{
    self, ResolvedRevsetExpression, RevsetDiagnostics, RevsetExpression, RevsetExtensions,
    RevsetFilterPredicate, RevsetParseContext, RevsetResolutionError, RevsetStreamExt as _,
    RevsetWorkspaceContext, SymbolResolver,
};
use jj_lib::rewrite::{
    CommitRewriter, CommitWithSelection, RebaseOptions, merge_commit_trees, squash_commits,
};
use jj_lib::settings::UserSettings;
use jj_lib::signing::SignBehavior;
use jj_lib::transaction::Transaction;
use jj_lib::workspace::{
    DefaultWorkspaceLoaderFactory, Workspace, WorkspaceLoader, WorkspaceLoaderFactory,
    default_working_copy_factories,
};
use similar::{ChangeTag, TextDiff};

use crate::cli::{HunkRange, HunkSpec, LogField};
use crate::error::{
    AppError, PublicationFailureReason, PublicationRemoteState, ReadinessReason,
    RemoteBookmarkRejectReason, RewritabilityReason,
};
use crate::model::{
    AbsorbData, AbsorbMove, AbsorbSourceAction, BookmarkComparisonStatus, BookmarkEntry,
    BookmarkListData, BookmarkPushData, BookmarkRemoteState, BookmarkSetData, BookmarkTargetState,
    Change, CheckpointData, CurrentChange, DescribeData, DescriptionAction, DiffData, DiffStat,
    DiffTarget, FinishData, FinishPublication, HistoryChange, HunkRef, InspectData,
    LocalBookmarkAction, LogData, LogEntry, MoveData, NewData, OperationEntry, OperationKind,
    OperationsData, Patch, RemoteBookmarkAction, ReorderData, ShowData, SkippedPath, SplitData,
    Status, Truncation, UndoAction, UndoData, UndoSelection, UndoTarget, UnmovedHunk,
};

const DEFAULT_PATCH_LIMIT_BYTES: u64 = 16 * 1024;
const AXI_UNDO_PREFIX: &str = "jj-axi undo: restore to operation ";

fn is_push_operation(operation: &jj_lib::operation::Operation) -> bool {
    operation.metadata().description.starts_with("push ")
        || operation
            .metadata()
            .attributes
            .get("args")
            .is_some_and(|args| args.contains(" push"))
}

fn classify_operation(operation: &jj_lib::operation::Operation) -> OperationKind {
    let metadata = operation.metadata();
    if metadata.is_snapshot {
        OperationKind::Synchronization
    } else if metadata.description.starts_with(AXI_UNDO_PREFIX)
        || metadata
            .description
            .starts_with("undo: restore to operation ")
    {
        OperationKind::Undo
    } else if operation.parent_ids().is_empty()
        || (metadata.description.starts_with("add workspace '")
            && operation.parent_ids().len() == 1)
    {
        OperationKind::Foundation
    } else if metadata
        .attributes
        .get("args")
        .is_some_and(|args| args.starts_with("jj-axi "))
    {
        OperationKind::Mutation
    } else {
        OperationKind::Unknown
    }
}

/// The sole boundary between jj's version-pinned APIs and jj-axi DTOs.
///
/// `open()` deliberately asks the installed `jj` binary to synchronize the working copy before
/// loading through `jj-lib`. A snapshot is observable internally, but it is necessary for the
/// `inspect`/working-copy-diff contract to match standard `jj status` semantics rather than
/// reporting a stale recorded tree.
pub(crate) struct JjBridge {
    cwd: PathBuf,
    workspace: Workspace,
    repo: Arc<jj_lib::repo::ReadonlyRepo>,
}

async fn load_observational_workspace(
    cwd: &Path,
) -> Result<(Workspace, Vec<jj_lib::operation::Operation>), AppError> {
    let cwd = std::fs::canonicalize(cwd).map_err(|_| AppError::RepositoryUnavailable {
        operation: "resolve_working_directory",
    })?;
    let workspace_root = find_workspace_root(&cwd).ok_or_else(|| AppError::RepositoryNotFound {
        path: cwd.display().to_string(),
    })?;
    let loader_factory = DefaultWorkspaceLoaderFactory;
    let loader =
        loader_factory
            .create(&workspace_root)
            .map_err(|_| AppError::RepositoryUnavailable {
                operation: "discover_workspace",
            })?;
    let settings = load_settings(loader.as_ref())?;
    let workspace = loader
        .load(
            &settings,
            &StoreFactories::default(),
            &default_working_copy_factories(),
        )
        .map_err(|_| AppError::RepositoryUnavailable {
            operation: "load_workspace",
        })?;
    let repo_loader = workspace.repo_loader();
    let head_ids = repo_loader
        .op_heads_store()
        .get_op_heads()
        .await
        .map_err(|_| AppError::BackendFailure {
            operation: "read_operation_heads",
        })?;
    let mut heads = Vec::with_capacity(head_ids.len());
    for id in head_ids {
        heads.push(repo_loader.load_operation(&id).await.map_err(|_| {
            AppError::BackendFailure {
                operation: "read_operations",
            }
        })?);
    }
    Ok((workspace, heads))
}

impl JjBridge {
    pub(crate) async fn git_remote_urls(cwd: &Path) -> Result<Vec<String>, AppError> {
        let (workspace, heads) = load_observational_workspace(cwd).await?;
        let operation = heads.first().ok_or(AppError::BackendFailure {
            operation: "read_git_remotes",
        })?;
        let repo = workspace
            .repo_loader()
            .load_at(operation)
            .await
            .map_err(|_| AppError::BackendFailure {
                operation: "read_git_remotes",
            })?;
        let git_repo = git::get_git_repo(repo.store()).map_err(|_| AppError::BackendFailure {
            operation: "read_git_remotes",
        })?;
        let mut urls = Vec::new();
        for name in git_repo.remote_names() {
            if let Some(Ok(remote)) = git_repo.try_find_remote(name.as_ref()) {
                if let Some(url) = remote.url(gix::remote::Direction::Fetch) {
                    urls.push(url.to_bstring().to_string());
                }
            }
        }
        urls.sort();
        urls.dedup();
        Ok(urls)
    }

    /// Reads the operation DAG without snapshotting the working copy or reconciling heads.
    pub(crate) async fn operations(cwd: &Path, limit: usize) -> Result<OperationsData, AppError> {
        let (_workspace, heads) = load_observational_workspace(cwd).await?;
        let head_set: HashSet<_> = heads
            .iter()
            .map(|operation| operation.id().clone())
            .collect();
        let operations = op_walk::walk_ancestors(&heads)
            .take(limit)
            .map(|result| {
                let operation = result.map_err(|_| AppError::BackendFailure {
                    operation: "read_operations",
                })?;
                let kind = classify_operation(&operation);
                Ok(OperationEntry {
                    operation_id: operation.id().hex(),
                    parent_operation_ids: operation
                        .parent_ids()
                        .iter()
                        .map(ObjectId::hex)
                        .collect(),
                    description: operation.metadata().description.clone(),
                    kind,
                    undo_candidate: matches!(
                        kind,
                        OperationKind::Mutation | OperationKind::Unknown
                    ),
                    current: head_set.contains(operation.id()),
                })
            })
            .try_collect()
            .await?;
        Ok(OperationsData { operations })
    }

    /// Reads cached bookmark collaboration state without snapshotting or contacting remotes.
    pub(crate) async fn bookmark_list(
        cwd: &Path,
        limit: usize,
        after: Option<&str>,
        exact_name: Option<&str>,
    ) -> Result<BookmarkListData, AppError> {
        let (workspace, heads) = load_observational_workspace(cwd).await?;
        if heads.len() != 1 {
            let mut operation_ids: Vec<_> = heads.iter().map(|op| op.id().hex()).collect();
            operation_ids.sort();
            return Err(AppError::OperationHistoryDiverged { operation_ids });
        }
        let repo = workspace
            .repo_loader()
            .load_at(&heads[0])
            .await
            .map_err(|_| AppError::BackendFailure {
                operation: "read_bookmarks",
            })?;
        let view = repo.view();
        let mut names = BTreeSet::new();
        for (name, _) in view.local_bookmarks() {
            names.insert(name.as_str().to_owned());
        }
        for (symbol, remote_ref) in view.all_remote_bookmarks() {
            if remote_ref.is_tracked() {
                names.insert(symbol.name.as_str().to_owned());
            }
        }
        if let Some(exact_name) = exact_name {
            names.retain(|name| name == exact_name);
        }
        if let Some(after) = after {
            names.retain(|name| name.as_str() > after);
        }
        let mut page_names: Vec<_> = names.into_iter().take(limit.saturating_add(1)).collect();
        let truncated = page_names.len() > limit;
        if truncated {
            page_names.pop();
        }
        let remote_names =
            git::get_all_remote_names(repo.store()).map_err(|_| AppError::BackendFailure {
                operation: "read_bookmarks",
            })?;
        let mut bookmarks = Vec::new();
        for name in page_names {
            let ref_name = RefNameBuf::from(name.clone());
            let local_target = view.get_local_bookmark(&ref_name);
            let local = bookmark_target_state(repo.as_ref(), local_target).await?;
            let mut remotes = Vec::new();
            for remote in &remote_names {
                let remote_ref = view.get_remote_bookmark(ref_name.to_remote_symbol(remote));
                let remote_target = if remote_ref.is_tracked() {
                    &remote_ref.target
                } else {
                    RefTarget::absent_ref()
                };
                let target = bookmark_target_state(repo.as_ref(), remote_target).await?;
                let comparison_status = if local_target.is_absent() {
                    BookmarkComparisonStatus::LocalMissing
                } else if local_target.has_conflict() {
                    BookmarkComparisonStatus::LocalConflicted
                } else if remote_target.is_absent() {
                    BookmarkComparisonStatus::RemoteMissing
                } else if remote_target.has_conflict() {
                    BookmarkComparisonStatus::RemoteConflicted
                } else {
                    BookmarkComparisonStatus::Available
                };
                let (ahead, behind) = if comparison_status == BookmarkComparisonStatus::Available {
                    let local_id = local_target.as_normal().ok_or(AppError::Internal)?;
                    let remote_id = remote_target.as_normal().ok_or(AppError::Internal)?;
                    (
                        Some(commit_difference_count(repo.as_ref(), local_id, remote_id).await?),
                        Some(commit_difference_count(repo.as_ref(), remote_id, local_id).await?),
                    )
                } else {
                    (None, None)
                };
                remotes.push(BookmarkRemoteState {
                    remote: remote.as_str().to_owned(),
                    tracking: remote_ref.is_tracked(),
                    target,
                    comparison_status,
                    ahead,
                    behind,
                });
            }
            bookmarks.push(BookmarkEntry {
                name,
                local,
                remotes,
            });
        }
        let next_after = truncated
            .then(|| bookmarks.last().map(|bookmark| bookmark.name.clone()))
            .flatten();
        Ok(BookmarkListData {
            bookmarks,
            truncated,
            next_after,
        })
    }

    pub(crate) async fn open(cwd: &Path) -> Result<Self, AppError> {
        let cwd = std::fs::canonicalize(cwd).map_err(|_| AppError::RepositoryUnavailable {
            operation: "resolve_working_directory",
        })?;
        let workspace_root =
            find_workspace_root(&cwd).ok_or_else(|| AppError::RepositoryNotFound {
                path: cwd.display().to_string(),
            })?;

        synchronize_working_copy(&workspace_root)?;

        let loader_factory = DefaultWorkspaceLoaderFactory;
        let loader = loader_factory.create(&workspace_root).map_err(|_| {
            AppError::RepositoryUnavailable {
                operation: "discover_workspace",
            }
        })?;
        let settings = load_settings(loader.as_ref())?;
        let workspace = loader
            .load(
                &settings,
                &StoreFactories::default(),
                &default_working_copy_factories(),
            )
            .map_err(|_| AppError::RepositoryUnavailable {
                operation: "load_workspace",
            })?;
        let repo = workspace.repo_loader().load_at_head().await.map_err(|_| {
            AppError::RepositoryUnavailable {
                operation: "load_repository",
            }
        })?;

        Ok(Self {
            cwd,
            workspace,
            repo,
        })
    }

    pub(crate) async fn undo(
        &mut self,
        to: Option<&str>,
        mut source_ids: Vec<String>,
    ) -> Result<UndoData, AppError> {
        let source = self.repo.operation().clone();
        if source_ids.is_empty() {
            source_ids.push(source.id().hex());
        }
        let walk_head = if to.is_none() {
            if let Some(hex) = source.metadata().description.strip_prefix(AXI_UNDO_PREFIX) {
                let id = jj_lib::op_store::OperationId::try_from_hex(hex).ok_or(
                    AppError::BackendFailure {
                        operation: "read_undo_marker",
                    },
                )?;
                self.repo.loader().load_operation(&id).await.map_err(|_| {
                    AppError::BackendFailure {
                        operation: "read_undo_marker",
                    }
                })?
            } else {
                source.clone()
            }
        } else {
            source.clone()
        };
        let operations: Vec<_> = op_walk::walk_ancestors(std::slice::from_ref(&walk_head))
            .try_collect()
            .await
            .map_err(|_| AppError::BackendFailure {
                operation: "read_operations",
            })?;

        let (target, selection, undone_count) = if let Some(prefix) = to {
            if prefix.is_empty() || !prefix.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(AppError::InvalidOperationId {
                    operation_id: prefix.to_owned(),
                });
            }
            let hex_prefix =
                HexPrefix::try_from_hex(prefix).ok_or_else(|| AppError::InvalidOperationId {
                    operation_id: prefix.to_owned(),
                })?;
            let target_id = match self
                .repo
                .op_store()
                .resolve_operation_id_prefix(&hex_prefix)
                .await
                .map_err(|_| AppError::BackendFailure {
                    operation: "resolve_operation",
                })? {
                PrefixResolution::NoMatch => {
                    return Err(AppError::OperationNotFound {
                        operation_id: prefix.to_owned(),
                    });
                }
                PrefixResolution::AmbiguousMatch => {
                    let mut candidates: Vec<_> = operations
                        .iter()
                        .filter(|operation| operation.id().hex().starts_with(prefix))
                        .map(|operation| operation.id().hex())
                        .collect();
                    candidates.sort();
                    candidates.truncate(3);
                    return Err(AppError::OperationAmbiguous {
                        operation_id: prefix.to_owned(),
                        candidates,
                    });
                }
                PrefixResolution::SingleMatch(id) => id,
            };
            let target = operations
                .iter()
                .find(|operation| operation.id() == &target_id)
                .cloned()
                .ok_or_else(|| AppError::OperationNotAncestor {
                    operation_id: target_id.hex(),
                })?;
            let target_view = target.view().await.map_err(|_| AppError::BackendFailure {
                operation: "read_operations",
            })?;
            if target_view
                .store_view()
                .wc_commit_ids
                .get(self.workspace.workspace_name())
                .is_none()
            {
                return Err(AppError::OperationTargetUnsafe {
                    operation_id: target.id().hex(),
                    reason: "active_workspace_missing",
                });
            }
            let count = operations
                .iter()
                .take_while(|operation| operation.id() != target.id())
                .filter(|operation| {
                    matches!(
                        classify_operation(operation),
                        OperationKind::Mutation | OperationKind::Unknown
                    )
                })
                .count() as u64;
            (target, UndoSelection::Explicit, count)
        } else {
            let mutation = operations
                .iter()
                .find(|operation| {
                    matches!(
                        classify_operation(operation),
                        OperationKind::Mutation | OperationKind::Unknown
                    )
                })
                .ok_or(AppError::NothingToUndo)?;
            let parent_id = mutation
                .parent_ids()
                .first()
                .ok_or(AppError::BackendFailure {
                    operation: "undo_root",
                })?;
            let target = self
                .repo
                .loader()
                .load_operation(parent_id)
                .await
                .map_err(|_| AppError::BackendFailure {
                    operation: "read_operations",
                })?;
            (target, UndoSelection::LatestMutation, 1)
        };

        if target.id() == source.id() {
            return Ok(UndoData {
                action: UndoAction::Unchanged,
                selection,
                source_operation_ids: source_ids,
                target_operation: UndoTarget {
                    operation_id: target.id().hex(),
                    description: target.metadata().description.clone(),
                },
                result_operation_id: source.id().hex(),
                undone_count: 0,
                external_effects: vec![],
            });
        }

        let target_view = target
            .view()
            .await
            .map_err(|_| AppError::BackendFailure { operation: "undo" })?;
        let restored_view = target_view.store_view().clone();
        let current_wc = if selection == UndoSelection::LatestMutation {
            Some(self.current_commit().await?)
        } else {
            None
        };
        let target_wc_id = restored_view
            .wc_commit_ids
            .get(self.workspace.workspace_name())
            .cloned();
        let mut tx = self.start_transaction("undo");
        tx.repo_mut().set_view(restored_view);
        if let (Some(current_wc), Some(target_wc_id)) = (current_wc, target_wc_id) {
            let target_wc = tx
                .repo()
                .store()
                .get_commit_async(&target_wc_id)
                .await
                .map_err(|_| AppError::BackendFailure { operation: "undo" })?;
            let mut builder = tx.repo_mut().rewrite_commit(&target_wc).detach();
            builder.set_tree(current_wc.tree());
            builder
                .write(tx.repo_mut())
                .await
                .map_err(|_| AppError::BackendFailure { operation: "undo" })?;
        }
        let description = format!("{AXI_UNDO_PREFIX}{}", target.id().hex());
        self.finish_transaction(tx, "undo", description).await?;
        let result_id = self.repo.operation().id().hex();
        let external_effects = if operations
            .iter()
            .take_while(|operation| operation.id() != target.id())
            .any(is_push_operation)
        {
            vec!["git_push".to_owned()]
        } else {
            vec![]
        };
        Ok(UndoData {
            action: UndoAction::Restored,
            selection,
            source_operation_ids: source_ids,
            target_operation: UndoTarget {
                operation_id: target.id().hex(),
                description: target.metadata().description.clone(),
            },
            result_operation_id: result_id,
            undone_count,
            external_effects,
        })
    }

    pub(crate) async fn inspect(&self) -> Result<InspectData, AppError> {
        let current = self.current_commit().await?;
        let diff = self.diff_for_commit(&current, false, false).await?;
        let conflict_count = self.visible_conflict_count().await?;
        let divergence_count = self.visible_divergence_count().await?;

        Ok(InspectData {
            current_change: CurrentChange {
                change_id: current.change_id().to_string(),
                description: current.description().to_owned(),
                status: Status {
                    conflicted: current.has_conflict(),
                },
            },
            diff_stat: diff.stat,
            conflict_count,
            divergence_count,
        })
    }

    pub(crate) async fn log(
        &self,
        limit: usize,
        conflicted_only: bool,
        fields: &[LogField],
    ) -> Result<LogData, AppError> {
        let expression = RevsetExpression::visible_heads()
            .ancestors()
            .intersection(&RevsetExpression::root().negated());
        let revset =
            expression
                .evaluate(self.repo.as_ref())
                .map_err(|_| AppError::BackendFailure {
                    operation: "evaluate_log",
                })?;
        let mut stream = revset.stream();
        let mut changes = Vec::new();
        let mut complete = true;

        while let Some(commit_id) =
            stream
                .try_next()
                .await
                .map_err(|_| AppError::BackendFailure {
                    operation: "read_log",
                })?
        {
            let commit = self.commit_by_id(&commit_id).await?;
            if conflicted_only && !commit.has_conflict() {
                continue;
            }
            if changes.len() == limit {
                complete = false;
                break;
            }
            changes.push(LogEntry {
                change_id: commit.change_id().to_string(),
                description: commit.description().to_owned(),
                status: Status {
                    conflicted: commit.has_conflict(),
                },
                commit_id: fields
                    .contains(&LogField::CommitId)
                    .then(|| commit.id().to_string()),
                parent_commit_ids: fields.contains(&LogField::ParentCommitIds).then(|| {
                    commit
                        .parent_ids()
                        .iter()
                        .map(ToString::to_string)
                        .collect()
                }),
            });
        }

        Ok(LogData { changes, complete })
    }

    pub(crate) async fn show(&self, revision: &str, full: bool) -> Result<ShowData, AppError> {
        let commit = self.resolve_one(revision).await?;
        let diff = self.diff_for_commit(&commit, true, full).await?;
        let patch = diff.patch.ok_or(AppError::Internal)?;
        Ok(ShowData {
            change: Change {
                change_id: commit.change_id().to_string(),
                description: commit.description().to_owned(),
            },
            diff_stat: diff.stat,
            patch,
        })
    }

    pub(crate) async fn diff(
        &self,
        revision: Option<&str>,
        full: bool,
    ) -> Result<DiffData, AppError> {
        let (commit, target) = if let Some(revision) = revision {
            let commit = self.resolve_one(revision).await?;
            let target = DiffTarget::Change {
                change_id: commit.change_id().to_string(),
            };
            (commit, target)
        } else {
            (self.current_commit().await?, DiffTarget::WorkingCopy)
        };
        let diff = self.diff_for_commit(&commit, true, full).await?;
        Ok(DiffData {
            target,
            diff_stat: diff.stat,
            patch: diff.patch.ok_or(AppError::Internal)?,
        })
    }

    pub(crate) async fn create_change(
        &mut self,
        message: Option<&str>,
    ) -> Result<NewData, AppError> {
        let parent = self.current_commit().await?;
        let description = normalize_message(message.unwrap_or(""));
        let mut tx = self.start_transaction("new");
        let tree = merge_commit_trees(tx.repo(), std::slice::from_ref(&parent))
            .await
            .map_err(|_| AppError::BackendFailure { operation: "new" })?;
        let mut builder = tx
            .repo_mut()
            .new_commit(vec![parent.id().clone()], tree)
            .detach();
        builder.set_description(description);
        let new_commit = builder
            .write(tx.repo_mut())
            .await
            .map_err(|_| AppError::BackendFailure { operation: "new" })?;
        tx.repo_mut()
            .edit(self.workspace.workspace_name().to_owned(), &new_commit)
            .await
            .map_err(|_| AppError::BackendFailure { operation: "new" })?;
        self.finish_transaction(tx, "new", "new empty commit".to_owned())
            .await?;

        let current = self.current_commit().await?;
        Ok(NewData {
            current_change: change_from_commit(&current),
        })
    }

    pub(crate) async fn describe_change(
        &mut self,
        revision: &str,
        message: &str,
    ) -> Result<DescribeData, AppError> {
        let target = self.resolve_one(revision).await?;
        let description = normalize_message(message);
        let change_id = target.change_id().clone();
        if target.description() == description {
            return Ok(DescribeData {
                change: change_from_commit(&target),
                changed: false,
            });
        }
        self.ensure_rewritable(self.repo.as_ref(), &target).await?;

        let old_commit_id = target.id().clone();
        let mut tx = self.start_transaction("describe");
        Self::rewrite_description(&mut tx, &target, &description, "describe").await?;
        self.finish_transaction(
            tx,
            "describe",
            format!("describe commit {}", old_commit_id.hex()),
        )
        .await?;

        Ok(DescribeData {
            change: self.change_by_change_id(&change_id).await?,
            changed: true,
        })
    }

    pub(crate) async fn checkpoint(&mut self, message: &str) -> Result<CheckpointData, AppError> {
        let old_wc = self.current_commit().await?;
        self.ensure_rewritable(self.repo.as_ref(), &old_wc).await?;
        let checkpoint_change_id = old_wc.change_id().clone();
        let old_commit_id = old_wc.id().clone();
        let workspace_names = self.repo.view().workspaces_for_wc_commit_id(old_wc.id());
        let description = normalize_message(message);

        let mut tx = self.start_transaction("checkpoint");
        let mut builder = tx.repo_mut().rewrite_commit(&old_wc).detach();
        builder.set_description(description);
        let checkpoint =
            builder
                .write(tx.repo_mut())
                .await
                .map_err(|_| AppError::BackendFailure {
                    operation: "checkpoint",
                })?;
        let current = tx
            .repo_mut()
            .new_commit(vec![checkpoint.id().clone()], checkpoint.tree())
            .write()
            .await
            .map_err(|_| AppError::BackendFailure {
                operation: "checkpoint",
            })?;
        for workspace_name in workspace_names {
            tx.repo_mut()
                .edit(workspace_name, &current)
                .await
                .map_err(|_| AppError::BackendFailure {
                    operation: "checkpoint",
                })?;
        }
        self.finish_transaction(
            tx,
            "checkpoint",
            format!("checkpoint commit {}", old_commit_id.hex()),
        )
        .await?;

        let checkpoint = self.change_by_change_id(&checkpoint_change_id).await?;
        let current = self.current_commit().await?;
        Ok(CheckpointData {
            checkpoint,
            current_change: change_from_commit(&current),
        })
    }

    pub(crate) async fn push_bookmark(
        &mut self,
        bookmark: &str,
        requested_remote: Option<&str>,
    ) -> Result<BookmarkPushData, AppError> {
        let name = parse_bookmark_name(bookmark).map_err(|_| AppError::InvalidArgument {
            argument: "bookmark",
            constraint: "valid_bookmark_name",
        })?;
        let remote = self.select_publication_remote(requested_remote)?;
        let target_id = self
            .repo
            .view()
            .get_local_bookmark(&name)
            .as_normal()
            .cloned()
            .ok_or_else(|| {
                if self.repo.view().get_local_bookmark(&name).is_absent() {
                    AppError::BookmarkNotFound {
                        bookmark: name.as_str().to_owned(),
                    }
                } else {
                    AppError::RemoteBookmarkRejected {
                        bookmark: name.as_str().to_owned(),
                        remote: remote.as_str().to_owned(),
                        reason: RemoteBookmarkRejectReason::LocalConflicted,
                    }
                }
            })?;
        let target = self.commit_by_id(&target_id).await?;
        self.validate_publication(self.repo.as_ref(), &name, &remote, &target_id)
            .await?;
        let action = self
            .publish_bookmark(&name, &remote)
            .await
            .map_err(|failure| AppError::BookmarkPushPartial {
                bookmark: name.as_str().to_owned(),
                target_change_id: target.change_id().to_string(),
                target_commit_id: target.id().hex(),
                remote: remote.as_str().to_owned(),
                remote_state: failure.remote_state,
                reason: failure.reason,
            })?;
        let final_target_id = self
            .repo
            .view()
            .get_local_bookmark(&name)
            .as_normal()
            .cloned()
            .ok_or(AppError::Internal)?;
        let final_target = self.commit_by_id(&final_target_id).await?;
        Ok(BookmarkPushData {
            name: name.as_str().to_owned(),
            target_change_id: final_target.change_id().to_string(),
            target_commit_id: final_target.id().hex(),
            remote: remote.as_str().to_owned(),
            action,
        })
    }

    pub(crate) async fn set_bookmark(
        &mut self,
        bookmark: &str,
        revision: &str,
        allow_backwards: bool,
    ) -> Result<BookmarkSetData, AppError> {
        let name = parse_bookmark_name(bookmark).map_err(|_| AppError::InvalidArgument {
            argument: "bookmark",
            constraint: "valid_bookmark_name",
        })?;
        let target = self.resolve_one(revision).await?;
        let old_target = self.repo.view().get_local_bookmark(&name);
        let new_target = RefTarget::normal(target.id().clone());
        let action = if old_target.is_absent() {
            LocalBookmarkAction::Created
        } else if old_target == &new_target {
            LocalBookmarkAction::Unchanged
        } else if allow_backwards
            || old_target.added_ids().any(|old_id| {
                self.repo
                    .index()
                    .is_ancestor(old_id, target.id())
                    .unwrap_or(false)
            })
        {
            LocalBookmarkAction::Moved
        } else {
            return Err(AppError::BookmarkMoveRejected {
                bookmark: name.as_str().to_owned(),
                change_id: target.change_id().to_string(),
            });
        };

        if action != LocalBookmarkAction::Unchanged {
            let mut tx = self.start_transaction("bookmark set");
            tx.repo_mut().set_local_bookmark_target(&name, new_target);
            self.finish_transaction(
                tx,
                "bookmark_set",
                format!(
                    "set bookmark {} to commit {}",
                    name.as_str(),
                    target.id().hex()
                ),
            )
            .await?;
        }

        Ok(BookmarkSetData {
            name: name.as_str().to_owned(),
            target_change_id: target.change_id().to_string(),
            target_commit_id: target.id().hex(),
            action,
        })
    }

    pub(crate) async fn finish_change(
        &mut self,
        revision: &str,
        message: Option<&str>,
        bookmark: Option<&str>,
    ) -> Result<FinishData, AppError> {
        let target = self.resolve_one(revision).await?;
        let change_id = target.change_id().clone();
        let change_id_string = change_id.to_string();

        let bookmark_context = if let Some(bookmark) = bookmark {
            Some(self.prepare_bookmark(bookmark, &target).await?)
        } else {
            None
        };

        let description = message.map(normalize_message);
        let description_action = match &description {
            Some(description) if description != target.description() => {
                self.ensure_rewritable(self.repo.as_ref(), &target).await?;
                DescriptionAction::Updated
            }
            Some(_) | None => DescriptionAction::Unchanged,
        };

        let mut tx = self.start_transaction("finish");
        let final_target_id = if description_action == DescriptionAction::Updated {
            Self::rewrite_description(
                &mut tx,
                &target,
                description.as_deref().unwrap_or_default(),
                "finish",
            )
            .await?
        } else {
            target.id().clone()
        };

        let publication = if let Some(context) = bookmark_context {
            if context.old_local_target.has_conflict() {
                return Err(AppError::RemoteBookmarkRejected {
                    bookmark: context.name.as_str().to_owned(),
                    remote: context.remote.as_str().to_owned(),
                    reason: RemoteBookmarkRejectReason::LocalConflicted,
                });
            }
            let local_action = self.validate_bookmark_move(
                &context.name,
                &context.old_local_target,
                target.id(),
                &final_target_id,
                &change_id,
                tx.repo(),
            )?;
            tx.repo_mut().set_local_bookmark_target(
                &context.name,
                RefTarget::normal(final_target_id.clone()),
            );

            self.validate_publication(tx.repo(), &context.name, &context.remote, &final_target_id)
                .await?;

            self.finish_transaction(tx, "finish", format!("finish change {change_id_string}"))
                .await?;
            return self
                .push_finished_change(
                    change_id,
                    context.name,
                    context.remote,
                    description_action,
                    local_action,
                )
                .await;
        } else {
            self.ensure_ready(tx.repo(), Vec::new(), &final_target_id)
                .await?;
            self.finish_transaction(tx, "finish", format!("finish change {change_id_string}"))
                .await?;
            FinishPublication::Skipped
        };

        Ok(FinishData {
            change: self.change_by_change_id(&change_id).await?,
            description_action,
            publication,
        })
    }

    pub(crate) async fn split_change(
        &mut self,
        change: &str,
        hunks: &[HunkSpec],
        description: &str,
    ) -> Result<SplitData, AppError> {
        let target = self.resolve_one(change).await?;
        self.ensure_rewritable(self.repo.as_ref(), &target).await?;
        let selected_change_id = target.change_id().clone();
        let (selection, selected_hunks) = self.select_hunks(&target, hunks, "split").await?;

        let mut tx = self.start_transaction("split");
        let selected = {
            let mut builder = tx.repo_mut().rewrite_commit(&selection.commit).detach();
            builder
                .set_tree(selection.selected_tree.clone())
                .set_description(normalize_message(description));
            builder
                .write(tx.repo_mut())
                .await
                .map_err(|_| AppError::BackendFailure { operation: "split" })?
        };
        let remaining = {
            let mut builder = tx.repo_mut().rewrite_commit(&selection.commit).detach();
            builder
                .set_parents(vec![selected.id().clone()])
                .set_tree(selection.commit.tree());
            builder.clear_rewrite_source();
            builder.generate_new_change_id();
            builder
                .write(tx.repo_mut())
                .await
                .map_err(|_| AppError::BackendFailure { operation: "split" })?
        };

        tx.repo_mut()
            .transform_descendants(vec![target.id().clone()], async |mut rewriter| {
                rewriter.replace_parent(selected.id(), [remaining.id()]);
                rewriter.rebase().await?.write().await?;
                Ok(())
            })
            .await
            .map_err(|_| AppError::BackendFailure { operation: "split" })?;
        for (workspace_name, working_copy_commit) in tx.base_repo().clone().view().wc_commit_ids() {
            if working_copy_commit == target.id() {
                tx.repo_mut()
                    .edit(workspace_name.clone(), &remaining)
                    .await
                    .map_err(|_| AppError::BackendFailure { operation: "split" })?;
            }
        }

        let remaining_change_id = remaining.change_id().clone();
        self.finish_transaction(tx, "split", format!("split commit {}", target.id().hex()))
            .await?;

        Ok(SplitData {
            selected: self
                .history_change_by_change_id(&selected_change_id)
                .await?,
            remaining: self
                .history_change_by_change_id(&remaining_change_id)
                .await?,
            hunks: selected_hunks,
        })
    }

    pub(crate) async fn move_hunks(
        &mut self,
        from: &str,
        to: &str,
        hunks: &[HunkSpec],
    ) -> Result<MoveData, AppError> {
        let source = self.resolve_one(from).await?;
        let destination = self.resolve_one(to).await?;
        if source.id() == destination.id() || source.change_id() == destination.change_id() {
            return Err(AppError::InvalidHistoryShape {
                operation: "move".to_owned(),
                reason: "same_change".to_owned(),
                change_ids: vec![
                    source.change_id().to_string(),
                    destination.change_id().to_string(),
                ],
            });
        }
        self.ensure_rewritable(self.repo.as_ref(), &source).await?;
        self.ensure_rewritable(self.repo.as_ref(), &destination)
            .await?;
        let source_change_id = source.change_id().clone();
        let destination_change_id = destination.change_id().clone();
        let (selection, selected_hunks) = self.select_hunks(&source, hunks, "move").await?;

        let mut tx = self.start_transaction("move");
        let squashed = squash_commits(
            tx.repo_mut(),
            std::slice::from_ref(&selection),
            &destination,
            true,
        )
        .await
        .map_err(|_| AppError::BackendFailure { operation: "move" })?
        .ok_or(AppError::BackendFailure { operation: "move" })?;
        squashed
            .commit_builder
            .write()
            .await
            .map_err(|_| AppError::BackendFailure { operation: "move" })?;
        self.finish_transaction(tx, "move", format!("move hunks from {}", source.id().hex()))
            .await?;

        Ok(MoveData {
            source: self.history_change_by_change_id(&source_change_id).await?,
            destination: self
                .history_change_by_change_id(&destination_change_id)
                .await?,
            hunks: selected_hunks,
        })
    }

    pub(crate) async fn absorb(&mut self, dry_run: bool) -> Result<AbsorbData, AppError> {
        let source_commit = self.current_commit().await?;
        self.ensure_rewritable(self.repo.as_ref(), &source_commit)
            .await?;
        let source_change_id = source_commit.change_id().to_string();
        let source = AbsorbSource::from_commit(self.repo.as_ref(), source_commit.clone())
            .await
            .map_err(|_| AppError::BackendFailure {
                operation: "absorb",
            })?;
        let parent_tree = source_commit
            .parent_tree(self.repo.as_ref())
            .await
            .map_err(|_| AppError::BackendFailure {
                operation: "absorb",
            })?;
        let mutable_destinations =
            self.resolve_configured_expression(self.repo.as_ref(), "mutable()", "absorb")?;
        let SelectedTrees {
            target_commits,
            skipped_paths,
        } = split_hunks_to_trees(
            self.repo.as_ref(),
            &source,
            &mutable_destinations,
            &EverythingMatcher,
        )
        .await
        .map_err(|_| AppError::BackendFailure {
            operation: "absorb",
        })?;

        let mut destination_ids = target_commits.keys().cloned().collect::<Vec<_>>();
        destination_ids.sort();
        let mut destination_commits = BTreeMap::new();
        for destination_id in destination_ids {
            let destination = self.commit_by_id(&destination_id).await?;
            self.ensure_rewritable(self.repo.as_ref(), &destination)
                .await?;
            destination_commits.insert(destination_id, destination);
        }

        let source_analysis = self
            .analyze_text_hunks(&parent_tree, &source_commit.tree(), "absorb")
            .await?;
        let (rebuilt_targets, moves) = self
            .materialize_absorb_targets(target_commits, &destination_commits, &parent_tree)
            .await?;
        let moved_hunks = moves
            .iter()
            .flat_map(|movement| {
                movement
                    .hunks
                    .iter()
                    .map(|hunk| (hunk.path.clone(), hunk.lines.clone()))
            })
            .collect::<BTreeSet<_>>();
        let mut unmoved_hunks = source_analysis
            .hunks
            .into_iter()
            .filter(|hunk| !moved_hunks.contains(&(hunk.path.clone(), hunk.lines.clone())))
            .map(|hunk| UnmovedHunk {
                path: hunk.path,
                lines: hunk.lines,
                reason: "no_unambiguous_destination".to_owned(),
            })
            .collect::<Vec<_>>();
        unmoved_hunks.sort_by(compare_unmoved_hunks);

        let mut skipped_by_path = source_analysis
            .skipped_paths
            .into_iter()
            .map(|skipped| (skipped.path, skipped.reason))
            .collect::<BTreeMap<_, _>>();
        for (path, reason) in skipped_paths {
            skipped_by_path
                .entry(path.as_internal_file_string().to_owned())
                .or_insert_with(|| normalize_absorb_skip_reason(&reason).to_owned());
        }
        let skipped_paths = skipped_by_path
            .into_iter()
            .map(|(path, reason)| SkippedPath { path, reason })
            .collect::<Vec<_>>();

        if rebuilt_targets.is_empty() {
            return Ok(AbsorbData {
                dry_run,
                changed: false,
                source_change_id,
                source_action: AbsorbSourceAction::Unchanged,
                moves,
                unmoved_hunks,
                skipped_paths,
            });
        }

        let mut tx = self.start_transaction("absorb");
        let stats = absorb_hunks(tx.repo_mut(), &source, rebuilt_targets)
            .await
            .map_err(|_| AppError::BackendFailure {
                operation: "absorb",
            })?;
        let source_action = if stats.rewritten_source.is_some() {
            AbsorbSourceAction::Rewritten
        } else {
            AbsorbSourceAction::Abandoned
        };
        let data = AbsorbData {
            dry_run,
            changed: true,
            source_change_id,
            source_action,
            moves,
            unmoved_hunks,
            skipped_paths,
        };
        if dry_run {
            return Ok(data);
        }

        self.finish_transaction(
            tx,
            "absorb",
            format!(
                "absorb changes into {} commits",
                stats.rewritten_destinations.len()
            ),
        )
        .await?;
        Ok(data)
    }

    pub(crate) async fn reorder(&mut self, sequence: &[String]) -> Result<ReorderData, AppError> {
        let mut commits = Vec::with_capacity(sequence.len());
        for revision in sequence {
            commits.push(self.resolve_one(revision).await?);
        }
        let change_ids = commits
            .iter()
            .map(|commit| commit.change_id().to_string())
            .collect::<Vec<_>>();
        if commits.len() < 2 {
            return Err(AppError::InvalidArgument {
                argument: "sequence",
                constraint: "at_least_two_revisions_oldest_to_newest",
            });
        }
        let mut seen_commit_ids = HashSet::new();
        let mut seen_change_ids = HashSet::new();
        if commits.iter().any(|commit| {
            !seen_commit_ids.insert(commit.id().clone())
                || !seen_change_ids.insert(commit.change_id().clone())
        }) {
            return Err(AppError::InvalidHistoryShape {
                operation: "reorder".to_owned(),
                reason: "duplicate_change".to_owned(),
                change_ids,
            });
        }
        for commit in &commits {
            self.ensure_rewritable(self.repo.as_ref(), commit).await?;
        }
        if commits.iter().any(|commit| commit.parent_ids().len() != 1) {
            return Err(AppError::InvalidHistoryShape {
                operation: "reorder".to_owned(),
                reason: "merge_commit".to_owned(),
                change_ids,
            });
        }
        let current_order = if let Some(order) = linear_history_order(&commits) {
            order
        } else {
            let mut non_linear = false;
            'pairs: for (index, left) in commits.iter().enumerate() {
                for right in commits.iter().skip(index + 1) {
                    let left_before_right = self
                        .repo
                        .index()
                        .is_ancestor(left.id(), right.id())
                        .map_err(|_| AppError::BackendFailure {
                            operation: "reorder",
                        })?;
                    let right_before_left = self
                        .repo
                        .index()
                        .is_ancestor(right.id(), left.id())
                        .map_err(|_| AppError::BackendFailure {
                            operation: "reorder",
                        })?;
                    if !left_before_right && !right_before_left {
                        non_linear = true;
                        break 'pairs;
                    }
                }
            }
            return Err(AppError::InvalidHistoryShape {
                operation: "reorder".to_owned(),
                reason: if non_linear {
                    "non_linear"
                } else {
                    "non_contiguous"
                }
                .to_owned(),
                change_ids: change_ids.clone(),
            });
        };
        if current_order
            .iter()
            .map(Commit::id)
            .eq(commits.iter().map(Commit::id))
        {
            return Ok(ReorderData {
                changed: false,
                sequence: commits.iter().map(history_change_from_commit).collect(),
            });
        }

        let mut parent_id = current_order[0].parent_ids()[0].clone();
        let mut tx = self.start_transaction("reorder");
        let mut realized = Vec::with_capacity(commits.len());
        for commit in commits {
            let rewritten = CommitRewriter::new(tx.repo_mut(), commit, vec![parent_id.clone()])
                .rebase()
                .await
                .map_err(|_| AppError::BackendFailure {
                    operation: "reorder",
                })?
                .write()
                .await
                .map_err(|_| AppError::BackendFailure {
                    operation: "reorder",
                })?;
            parent_id = rewritten.id().clone();
            realized.push(rewritten);
        }
        self.finish_transaction(tx, "reorder", format!("reorder {} commits", sequence.len()))
            .await?;

        Ok(ReorderData {
            changed: true,
            sequence: realized.iter().map(history_change_from_commit).collect(),
        })
    }

    async fn select_hunks(
        &self,
        commit: &Commit,
        specs: &[HunkSpec],
        operation: &'static str,
    ) -> Result<(CommitWithSelection, Vec<HunkRef>), AppError> {
        if specs.is_empty() {
            return Err(AppError::InvalidHunkSelection {
                path: String::new(),
                requested: String::new(),
                reason: "range_not_hunk".to_owned(),
                nearest_hunks: Vec::new(),
            });
        }

        let parent_tree = commit
            .parent_tree(self.repo.as_ref())
            .await
            .map_err(|_| AppError::BackendFailure { operation })?;
        let mut requested_by_path = BTreeMap::<String, Vec<HunkRange>>::new();
        let mut seen = BTreeSet::new();
        for spec in specs {
            let requested = format_hunk_range(spec.lines);
            if !seen.insert((spec.path.clone(), requested.clone())) {
                return Err(AppError::InvalidHunkSelection {
                    path: spec.path.clone(),
                    requested,
                    reason: "duplicate".to_owned(),
                    nearest_hunks: Vec::new(),
                });
            }
            requested_by_path
                .entry(spec.path.clone())
                .or_default()
                .push(spec.lines);
        }

        let mut selected_files = Vec::new();
        let mut selected_hunks = Vec::new();
        let mut diff_stream = parent_tree.diff_stream(&commit.tree(), &EverythingMatcher);
        while let Some(TreeDiffEntry { path, values }) = diff_stream.next().await {
            let path_string = path.as_internal_file_string().to_owned();
            let Some(requested_ranges) = requested_by_path.remove(&path_string) else {
                continue;
            };
            let JjDiff {
                before: before_value,
                after: after_value,
            } = values.map_err(|_| AppError::BackendFailure { operation })?;
            let before_metadata = regular_file_metadata(&before_value).map_err(|reason| {
                invalid_hunk_selection(&path_string, requested_ranges[0], reason, Vec::new())
            })?;
            let after_metadata = regular_file_metadata(&after_value).map_err(|reason| {
                invalid_hunk_selection(&path_string, requested_ranges[0], reason, Vec::new())
            })?;
            if before_metadata
                .as_ref()
                .zip(after_metadata.as_ref())
                .is_some_and(|(before, after)| before.copy_id != after.copy_id)
            {
                return Err(invalid_hunk_selection(
                    &path_string,
                    requested_ranges[0],
                    "unsupported_content",
                    Vec::new(),
                ));
            }
            if before_metadata
                .as_ref()
                .zip(after_metadata.as_ref())
                .is_some_and(|(before, after)| before.executable != after.executable)
            {
                return Err(invalid_hunk_selection(
                    &path_string,
                    requested_ranges[0],
                    "metadata_change",
                    Vec::new(),
                ));
            }

            let before_content = self
                .materialize_content(before_value, &path, parent_tree.labels())
                .await
                .map_err(|_| AppError::BackendFailure { operation })?;
            let after_content = self
                .materialize_content(after_value, &path, commit.tree().labels())
                .await
                .map_err(|_| AppError::BackendFailure { operation })?;
            let Some(before_text) = before_content.text() else {
                return Err(invalid_hunk_selection(
                    &path_string,
                    requested_ranges[0],
                    "unsupported_content",
                    Vec::new(),
                ));
            };
            let Some(after_text) = after_content.text() else {
                return Err(invalid_hunk_selection(
                    &path_string,
                    requested_ranges[0],
                    "unsupported_content",
                    Vec::new(),
                ));
            };

            let mut candidates = line_hunks(&path_string, before_text, after_text);
            if candidates.is_empty()
                && before_metadata.is_some()
                && after_metadata.is_none()
                && before_text.is_empty()
                && after_text.is_empty()
            {
                candidates.push(LineHunk {
                    range: HunkRange::Deletion { at: 1 },
                    reference: HunkRef {
                        path: path_string.clone(),
                        lines: "1-0".to_owned(),
                    },
                });
            }
            if candidates.is_empty() {
                let reason = if before_content.mode() != after_content.mode() {
                    "metadata_change"
                } else {
                    "path_not_changed"
                };
                return Err(invalid_hunk_selection(
                    &path_string,
                    requested_ranges[0],
                    reason,
                    Vec::new(),
                ));
            }

            let mut selected_ranges = HashSet::new();
            for requested in requested_ranges {
                let Some(candidate) = candidates
                    .iter()
                    .find(|candidate| candidate.range == requested)
                else {
                    return Err(invalid_hunk_selection(
                        &path_string,
                        requested,
                        "range_not_hunk",
                        nearest_hunks(&path_string, requested, &candidates),
                    ));
                };
                selected_ranges.insert(requested);
                selected_hunks.push(candidate.clone());
            }

            let selected_text = selected_text_by_hunks(before_text, after_text, &selected_ranges);
            selected_files.push(SelectedTextFile {
                path,
                text: selected_text,
                before_metadata,
                after_metadata,
            });
        }

        if let Some((path, ranges)) = requested_by_path.into_iter().next() {
            return Err(invalid_hunk_selection(
                &path,
                ranges[0],
                "path_not_changed",
                Vec::new(),
            ));
        }

        let mut builder = MergedTreeBuilder::new(parent_tree.clone());
        for file in selected_files {
            let value = if file.text.is_empty() && file.after_metadata.is_none() {
                Merge::absent()
            } else {
                let metadata = file
                    .before_metadata
                    .or(file.after_metadata)
                    .ok_or(AppError::BackendFailure { operation })?;
                let mut contents = file.text.as_bytes();
                let id = self
                    .repo
                    .store()
                    .write_file(&file.path, &mut contents)
                    .await
                    .map_err(|_| AppError::BackendFailure { operation })?;
                Merge::normal(TreeValue::File {
                    id,
                    executable: metadata.executable,
                    copy_id: metadata.copy_id,
                })
            };
            builder.set_or_remove(file.path, value);
        }
        let selected_tree = builder
            .write_tree()
            .await
            .map_err(|_| AppError::BackendFailure { operation })?;
        selected_hunks.sort_by(compare_line_hunks);
        let selected_hunks = selected_hunks
            .into_iter()
            .map(|hunk| hunk.reference)
            .collect();
        Ok((
            CommitWithSelection {
                commit: commit.clone(),
                selected_tree,
                parent_tree,
            },
            selected_hunks,
        ))
    }

    async fn history_change_by_change_id(
        &self,
        change_id: &ChangeId,
    ) -> Result<HistoryChange, AppError> {
        let targets = self
            .repo
            .resolve_change_id(change_id)
            .map_err(|_| AppError::BackendFailure {
                operation: "read_commit",
            })?
            .and_then(|targets| targets.into_visible())
            .ok_or(AppError::BackendFailure {
                operation: "read_commit",
            })?;
        let [commit_id] = targets.as_slice() else {
            return Err(AppError::BackendFailure {
                operation: "read_commit",
            });
        };
        Ok(history_change_from_commit(
            &self.commit_by_id(commit_id).await?,
        ))
    }

    async fn analyze_text_hunks(
        &self,
        before: &MergedTree,
        after: &MergedTree,
        operation: &'static str,
    ) -> Result<TextHunkAnalysis, AppError> {
        let mut hunks = Vec::new();
        let mut skipped_paths = Vec::new();
        let mut diff_stream = before.diff_stream(after, &EverythingMatcher);
        while let Some(TreeDiffEntry { path, values }) = diff_stream.next().await {
            let path_string = path.as_internal_file_string().to_owned();
            let JjDiff {
                before: before_value,
                after: after_value,
            } = values.map_err(|_| AppError::BackendFailure { operation })?;
            let before_metadata = match regular_file_metadata(&before_value) {
                Ok(metadata) => metadata,
                Err(reason) => {
                    skipped_paths.push(SkippedPath {
                        path: path_string,
                        reason: reason.to_owned(),
                    });
                    continue;
                }
            };
            let after_metadata = match regular_file_metadata(&after_value) {
                Ok(metadata) => metadata,
                Err(reason) => {
                    skipped_paths.push(SkippedPath {
                        path: path_string,
                        reason: reason.to_owned(),
                    });
                    continue;
                }
            };
            if before_metadata
                .as_ref()
                .zip(after_metadata.as_ref())
                .is_some_and(|(before, after)| before.copy_id != after.copy_id)
            {
                skipped_paths.push(SkippedPath {
                    path: path_string,
                    reason: "unsupported_content".to_owned(),
                });
                continue;
            }
            let before_content = self
                .materialize_content(before_value, &path, before.labels())
                .await
                .map_err(|_| AppError::BackendFailure { operation })?;
            let after_content = self
                .materialize_content(after_value, &path, after.labels())
                .await
                .map_err(|_| AppError::BackendFailure { operation })?;
            let (Some(before_text), Some(after_text)) =
                (before_content.text(), after_content.text())
            else {
                skipped_paths.push(SkippedPath {
                    path: path_string,
                    reason: "unsupported_content".to_owned(),
                });
                continue;
            };
            let mut path_hunks = line_hunks(&path_string, before_text, after_text);
            if path_hunks.is_empty()
                && before_metadata.is_some()
                && after_metadata.is_none()
                && before_text.is_empty()
                && after_text.is_empty()
            {
                path_hunks.push(LineHunk {
                    range: HunkRange::Deletion { at: 1 },
                    reference: HunkRef {
                        path: path_string.clone(),
                        lines: "1-0".to_owned(),
                    },
                });
            }
            if path_hunks.is_empty() {
                skipped_paths.push(SkippedPath {
                    path: path_string,
                    reason: "unsupported_content".to_owned(),
                });
                continue;
            }
            hunks.extend(path_hunks.into_iter().map(|hunk| hunk.reference));
        }
        hunks.sort_by(compare_hunk_refs);
        skipped_paths.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.reason.cmp(&right.reason))
        });
        skipped_paths
            .dedup_by(|left, right| left.path == right.path && left.reason == right.reason);
        Ok(TextHunkAnalysis {
            hunks,
            skipped_paths,
        })
    }

    async fn materialize_absorb_targets(
        &self,
        target_commits: HashMap<CommitId, MergedTreeBuilder>,
        destination_commits: &BTreeMap<CommitId, Commit>,
        parent_tree: &MergedTree,
    ) -> Result<(HashMap<CommitId, MergedTreeBuilder>, Vec<AbsorbMove>), AppError> {
        let mut targets = target_commits.into_iter().collect::<Vec<_>>();
        targets.sort_by(|(left, _), (right, _)| left.cmp(right));
        let mut rebuilt_targets = HashMap::with_capacity(targets.len());
        let mut moves = Vec::with_capacity(targets.len());
        for (commit_id, builder) in targets {
            let tree = builder
                .write_tree()
                .await
                .map_err(|_| AppError::BackendFailure {
                    operation: "absorb",
                })?;
            let analysis = self
                .analyze_text_hunks(parent_tree, &tree, "absorb")
                .await?;
            let destination =
                destination_commits
                    .get(&commit_id)
                    .ok_or(AppError::BackendFailure {
                        operation: "absorb",
                    })?;
            if analysis.hunks.is_empty() {
                continue;
            }
            moves.push(AbsorbMove {
                destination_change_id: destination.change_id().to_string(),
                hunks: analysis.hunks,
            });
            rebuilt_targets.insert(commit_id, MergedTreeBuilder::new(tree));
        }
        moves.sort_by(|left, right| {
            left.destination_change_id
                .cmp(&right.destination_change_id)
                .then_with(|| compare_hunk_ref_slices(&left.hunks, &right.hunks))
        });
        Ok((rebuilt_targets, moves))
    }

    async fn current_commit(&self) -> Result<Commit, AppError> {
        let commit_id = self
            .repo
            .view()
            .get_wc_commit_id(self.workspace.workspace_name())
            .ok_or(AppError::RepositoryUnavailable {
                operation: "read_working_copy",
            })?;
        self.commit_by_id(commit_id).await
    }

    async fn commit_by_id(
        &self,
        commit_id: &jj_lib::backend::CommitId,
    ) -> Result<Commit, AppError> {
        self.repo
            .store()
            .get_commit_async(commit_id)
            .await
            .map_err(|_| AppError::BackendFailure {
                operation: "read_commit",
            })
    }

    async fn change_by_change_id(&self, change_id: &ChangeId) -> Result<Change, AppError> {
        let targets = self
            .repo
            .resolve_change_id(change_id)
            .map_err(|_| AppError::BackendFailure {
                operation: "read_commit",
            })?
            .and_then(|targets| targets.into_visible())
            .ok_or(AppError::BackendFailure {
                operation: "read_commit",
            })?;
        let [commit_id] = targets.as_slice() else {
            return Err(AppError::BackendFailure {
                operation: "read_commit",
            });
        };
        let commit = self.commit_by_id(commit_id).await?;
        Ok(change_from_commit(&commit))
    }

    fn start_transaction(&self, operation: &'static str) -> Transaction {
        let mut tx = self.repo.start_transaction();
        tx.set_workspace_name(self.workspace.workspace_name());
        tx.set_attribute("args".to_owned(), format!("jj-axi {operation}"));
        tx
    }

    async fn finish_transaction(
        &mut self,
        mut tx: Transaction,
        operation: &'static str,
        operation_description: String,
    ) -> Result<bool, AppError> {
        if !tx.repo().has_changes() {
            return Ok(false);
        }

        let immutable = self.resolve_immutable_expression(tx.base_repo().as_ref())?;
        tx.repo_mut()
            .rebase_descendants_with_options(
                &immutable,
                &RebaseOptions::default(),
                |_old_commit, _rebased_commit| {},
            )
            .await
            .map_err(|_| AppError::BackendFailure { operation })?;

        let old_wc_id = tx
            .base_repo()
            .view()
            .get_wc_commit_id(self.workspace.workspace_name())
            .cloned();
        let new_wc_id = tx
            .repo()
            .view()
            .get_wc_commit_id(self.workspace.workspace_name())
            .cloned()
            .ok_or(AppError::BackendFailure { operation })?;
        let old_wc = if let Some(commit_id) = old_wc_id {
            Some(
                tx.base_repo()
                    .store()
                    .get_commit_async(&commit_id)
                    .await
                    .map_err(|_| AppError::BackendFailure { operation })?,
            )
        } else {
            None
        };
        let new_wc = tx
            .repo()
            .store()
            .get_commit_async(&new_wc_id)
            .await
            .map_err(|_| AppError::BackendFailure { operation })?;

        let git_lock = if is_colocated_git_workspace(&self.workspace, tx.base_repo()) {
            let lock = FileLock::lock(self.workspace.repo_path().join("git_import_export.lock"))
                .map_err(|_| AppError::BackendFailure { operation })?;
            git::reset_head(tx.repo_mut(), &new_wc)
                .await
                .map_err(|_| AppError::BackendFailure { operation })?;
            let stats = git::export_refs(tx.repo_mut())
                .map_err(|_| AppError::BackendFailure { operation })?;
            if !stats.failed_bookmarks.is_empty() || !stats.failed_tags.is_empty() {
                return Err(AppError::BackendFailure { operation });
            }
            Some(lock)
        } else {
            None
        };

        let new_repo = tx
            .commit(operation_description)
            .await
            .map_err(|_| AppError::BackendFailure { operation })?;
        self.repo = new_repo;
        update_working_copy(&self.repo, &mut self.workspace, old_wc.as_ref(), &new_wc)
            .await
            .map_err(|_| AppError::OperationIncomplete { operation })?;
        drop(git_lock);
        Ok(true)
    }

    fn with_revset_context<T>(
        &self,
        f: impl FnOnce(&RevsetParseContext<'_>, &RevsetExtensions) -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let ui = Ui::null();
        let settings = self.workspace.settings();
        let fileset_aliases = load_fileset_aliases(&ui, settings.config()).map_err(|_| {
            AppError::RepositoryUnavailable {
                operation: "load_fileset_aliases",
            }
        })?;
        let revset_aliases = load_revset_aliases(&ui, settings.config()).map_err(|_| {
            AppError::RepositoryUnavailable {
                operation: "load_revset_aliases",
            }
        })?;
        let extensions = RevsetExtensions::default();
        let path_converter = RepoPathUiConverter::Fs {
            cwd: self.cwd.clone(),
            base: self.workspace.workspace_root().to_owned(),
        };
        let workspace_context = RevsetWorkspaceContext {
            path_converter: &path_converter,
            workspace_name: self.workspace.workspace_name(),
        };
        let context = RevsetParseContext {
            aliases_map: &revset_aliases,
            local_variables: Default::default(),
            user_email: settings.user_email(),
            date_pattern_context: Local::now().into(),
            default_ignored_remote: None,
            fileset_aliases_map: &fileset_aliases,
            extensions: &extensions,
            workspace: Some(workspace_context),
        };
        f(&context, &extensions)
    }

    fn resolve_immutable_expression(
        &self,
        repo: &dyn jj_lib::repo::Repo,
    ) -> Result<Arc<ResolvedRevsetExpression>, AppError> {
        self.with_revset_context(|context, extensions| {
            let mut diagnostics = RevsetDiagnostics::new();
            let expression = parse_immutable_heads_expression(&mut diagnostics, context)
                .map_err(|_| AppError::BackendFailure {
                    operation: "resolve_immutable",
                })?
                .ancestors();
            let resolver = SymbolResolver::new(repo, extensions.symbol_resolvers());
            expression
                .resolve_user_expression(repo, &resolver)
                .map_err(|_| AppError::BackendFailure {
                    operation: "resolve_immutable",
                })
        })
    }

    fn resolve_configured_expression(
        &self,
        repo: &dyn jj_lib::repo::Repo,
        expression: &str,
        operation: &'static str,
    ) -> Result<Arc<ResolvedRevsetExpression>, AppError> {
        self.with_revset_context(|context, extensions| {
            let mut diagnostics = RevsetDiagnostics::new();
            let expression = revset::parse(&mut diagnostics, expression, context)
                .map_err(|_| AppError::BackendFailure { operation })?;
            let resolver = SymbolResolver::new(repo, extensions.symbol_resolvers());
            expression
                .resolve_user_expression(repo, &resolver)
                .map_err(|_| AppError::BackendFailure { operation })
        })
    }

    async fn ensure_rewritable(
        &self,
        repo: &dyn jj_lib::repo::Repo,
        commit: &Commit,
    ) -> Result<(), AppError> {
        if commit.id() == repo.store().root_commit_id() {
            return Err(AppError::ChangeNotRewritable {
                change_id: commit.change_id().to_string(),
                reason: RewritabilityReason::Root,
            });
        }
        let immutable = self.resolve_immutable_expression(repo)?;
        let revset = immutable
            .evaluate(repo)
            .map_err(|_| AppError::BackendFailure {
                operation: "resolve_immutable",
            })?;
        if revset.containing_fn()(commit.id()).map_err(|_| AppError::BackendFailure {
            operation: "resolve_immutable",
        })? {
            return Err(AppError::ChangeNotRewritable {
                change_id: commit.change_id().to_string(),
                reason: RewritabilityReason::Immutable,
            });
        }
        Ok(())
    }

    async fn rewrite_description(
        tx: &mut Transaction,
        target: &Commit,
        description: &str,
        operation: &'static str,
    ) -> Result<CommitId, AppError> {
        let old_target_id = target.id().clone();
        let mut new_target_id = None;
        tx.repo_mut()
            .transform_descendants(vec![old_target_id.clone()], async |rewriter| {
                let old_commit_id = rewriter.old_commit().id().clone();
                if old_commit_id == old_target_id {
                    let commit = rewriter
                        .reparent()
                        .set_description(description)
                        .write()
                        .await?;
                    new_target_id = Some(commit.id().clone());
                } else {
                    rewriter.reparent().write().await?;
                }
                Ok(())
            })
            .await
            .map_err(|_| AppError::BackendFailure { operation })?;
        new_target_id.ok_or(AppError::BackendFailure { operation })
    }

    async fn prepare_bookmark(
        &self,
        bookmark: &str,
        target: &Commit,
    ) -> Result<BookmarkContext, AppError> {
        let name = parse_bookmark_name(bookmark).map_err(|_| AppError::InvalidArgument {
            argument: "bookmark",
            constraint: "valid_bookmark_name",
        })?;
        let remote = self.select_publication_remote(None)?;
        let old_local_target = self.repo.view().get_local_bookmark(&name).clone();

        if old_local_target.is_present()
            && !old_local_target.has_conflict()
            && old_local_target != RefTarget::normal(target.id().clone())
            && !old_local_target.added_ids().any(|old_target| {
                self.repo
                    .index()
                    .is_ancestor(old_target, target.id())
                    .unwrap_or(false)
            })
        {
            return Err(AppError::BookmarkMoveRejected {
                bookmark: name.as_str().to_owned(),
                change_id: target.change_id().to_string(),
            });
        }

        Ok(BookmarkContext {
            name,
            remote,
            old_local_target,
        })
    }

    fn select_publication_remote(
        &self,
        requested: Option<&str>,
    ) -> Result<RemoteNameBuf, AppError> {
        let remotes =
            git::get_all_remote_names(self.repo.store()).map_err(|_| AppError::BackendFailure {
                operation: "finish",
            })?;
        let remote = if let Some(requested) = requested {
            requested.into()
        } else if let Some(configured) = self
            .workspace
            .settings()
            .get_string("git.push")
            .optional()
            .map_err(|_| AppError::BackendFailure {
                operation: "finish",
            })?
        {
            configured.into()
        } else if let [remote] = remotes.as_slice() {
            remote.clone()
        } else {
            "origin".into()
        };
        if remotes.contains(&remote) {
            Ok(remote)
        } else {
            Err(AppError::RemoteNotFound {
                remote: remote.as_str().to_owned(),
            })
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_bookmark_move(
        &self,
        name: &RefNameBuf,
        old_local_target: &RefTarget,
        old_target_id: &CommitId,
        final_target_id: &CommitId,
        change_id: &ChangeId,
        repo: &dyn jj_lib::repo::Repo,
    ) -> Result<LocalBookmarkAction, AppError> {
        if old_local_target.is_absent() {
            return Ok(LocalBookmarkAction::Created);
        }
        if old_local_target == &RefTarget::normal(final_target_id.clone()) {
            return Ok(LocalBookmarkAction::Unchanged);
        }
        if old_local_target == &RefTarget::normal(old_target_id.clone()) {
            return Ok(LocalBookmarkAction::Moved);
        }
        let fast_forward = old_local_target.added_ids().any(|old_target| {
            repo.index()
                .is_ancestor(old_target, final_target_id)
                .unwrap_or(false)
        });
        if fast_forward {
            Ok(LocalBookmarkAction::Moved)
        } else {
            Err(AppError::BookmarkMoveRejected {
                bookmark: name.as_str().to_owned(),
                change_id: change_id.to_string(),
            })
        }
    }

    async fn validate_publication(
        &self,
        repo: &dyn jj_lib::repo::Repo,
        bookmark: &RefNameBuf,
        remote: &RemoteName,
        target_id: &CommitId,
    ) -> Result<(), AppError> {
        let known_heads = repo
            .view()
            .remote_bookmarks(remote)
            .flat_map(|(_, remote_ref)| remote_ref.target.added_ids().cloned())
            .collect();
        self.ensure_ready(repo, known_heads, target_id).await?;
        self.preflight_remote_bookmark(repo, bookmark, remote)
    }

    async fn ensure_ready(
        &self,
        repo: &dyn jj_lib::repo::Repo,
        known_heads: Vec<CommitId>,
        target_id: &CommitId,
    ) -> Result<(), AppError> {
        let immutable = self.resolve_immutable_expression(repo)?;
        let target = RevsetExpression::commits(vec![target_id.clone()]);
        let commits = RevsetExpression::commits(known_heads)
            .union(&immutable)
            .range(&target)
            .union(&target);
        let private_expression = self
            .workspace
            .settings()
            .get_string("git.private-commits")
            .map_err(|_| AppError::BackendFailure {
                operation: "finish",
            })?;
        let private = self.resolve_configured_expression(repo, &private_expression, "finish")?;
        let private_revset = private
            .evaluate(repo)
            .map_err(|_| AppError::BackendFailure {
                operation: "finish",
            })?;
        let is_private = private_revset.containing_fn();
        let revset = commits
            .evaluate(repo)
            .map_err(|_| AppError::BackendFailure {
                operation: "finish",
            })?;
        let mut stream = revset.stream().commits(repo.store());
        while let Some(commit) = stream
            .try_next()
            .await
            .map_err(|_| AppError::BackendFailure {
                operation: "finish",
            })?
        {
            let mut reasons = Vec::new();
            if commit.description().is_empty() {
                reasons.push(ReadinessReason::EmptyDescription);
            }
            if commit.author().name.is_empty()
                || commit.author().email.is_empty()
                || commit.committer().name.is_empty()
                || commit.committer().email.is_empty()
            {
                reasons.push(ReadinessReason::MissingIdentity);
            }
            if commit.has_conflict() {
                reasons.push(ReadinessReason::Conflicted);
            }
            if is_private(commit.id()).map_err(|_| AppError::BackendFailure {
                operation: "finish",
            })? {
                reasons.push(ReadinessReason::Private);
            }
            if !reasons.is_empty() {
                return Err(AppError::ChangeNotReady {
                    change_id: commit.change_id().to_string(),
                    reasons,
                });
            }
        }
        Ok(())
    }

    fn preflight_remote_bookmark(
        &self,
        repo: &dyn jj_lib::repo::Repo,
        bookmark: &RefNameBuf,
        remote: &RemoteName,
    ) -> Result<(), AppError> {
        let targets = LocalAndRemoteRef {
            local_target: repo.view().get_local_bookmark(bookmark),
            remote_ref: repo
                .view()
                .get_remote_bookmark(bookmark.to_remote_symbol(remote)),
        };
        let reason = match classify_ref_push_action(targets) {
            RefPushAction::AlreadyMatches | RefPushAction::Update(_) => return Ok(()),
            RefPushAction::LocalConflicted => RemoteBookmarkRejectReason::LocalConflicted,
            RefPushAction::RemoteConflicted => RemoteBookmarkRejectReason::RemoteConflicted,
            RefPushAction::RemoteUntracked => RemoteBookmarkRejectReason::RemoteUntracked,
        };
        Err(AppError::RemoteBookmarkRejected {
            bookmark: bookmark.as_str().to_owned(),
            remote: remote.as_str().to_owned(),
            reason,
        })
    }

    async fn push_finished_change(
        &mut self,
        change_id: ChangeId,
        bookmark: RefNameBuf,
        remote: RemoteNameBuf,
        description_action: DescriptionAction,
        local_action: LocalBookmarkAction,
    ) -> Result<FinishData, AppError> {
        let change_id_string = change_id.to_string();
        let result = self.publish_bookmark(&bookmark, &remote).await;
        match result {
            Ok(remote_action) => Ok(FinishData {
                change: self.change_by_change_id(&change_id).await?,
                description_action,
                publication: FinishPublication::Complete {
                    bookmark: bookmark.as_str().to_owned(),
                    remote: remote.as_str().to_owned(),
                    local_action,
                    remote_action,
                },
            }),
            Err(failure) => Err(AppError::FinishPartial {
                change_id: change_id_string,
                bookmark: bookmark.as_str().to_owned(),
                remote: remote.as_str().to_owned(),
                description_action,
                local_action,
                remote_state: failure.remote_state,
                reason: failure.reason,
            }),
        }
    }

    /// Publishes exactly one existing local bookmark and classifies every outcome without
    /// embedding any command-specific response schema.
    async fn publish_bookmark(
        &mut self,
        bookmark: &RefNameBuf,
        remote: &RemoteNameBuf,
    ) -> Result<RemoteBookmarkAction, PublicationFailure> {
        let local_target = self.repo.view().get_local_bookmark(bookmark).clone();
        let remote_ref = self
            .repo
            .view()
            .get_remote_bookmark(bookmark.to_remote_symbol(remote))
            .clone();
        let update = match classify_ref_push_action(LocalAndRemoteRef {
            local_target: &local_target,
            remote_ref: &remote_ref,
        }) {
            RefPushAction::AlreadyMatches => return Ok(RemoteBookmarkAction::Unchanged),
            RefPushAction::Update(update) => update,
            RefPushAction::LocalConflicted
            | RefPushAction::RemoteConflicted
            | RefPushAction::RemoteUntracked => {
                return Err(PublicationFailure::not_updated(
                    PublicationFailureReason::Backend,
                ));
            }
        };
        let remote_action = if update.before.is_none() {
            RemoteBookmarkAction::Created
        } else {
            RemoteBookmarkAction::Moved
        };

        let mut tx = self.start_transaction("push");
        let mut targets = GitPushRefTargets {
            bookmarks: vec![(bookmark.clone(), update)],
            tags: Vec::new(),
        };
        let sign_on_push = self
            .workspace
            .settings()
            .get_bool("git.sign-on-push")
            .map_err(|_| PublicationFailure::not_updated(PublicationFailureReason::Backend))?;
        if sign_on_push {
            self.sign_commits_before_push(&mut tx, remote, &mut targets)
                .await
                .map_err(|_| PublicationFailure::not_updated(PublicationFailureReason::Backend))?;
        }
        if let Some(signed_target) = targets
            .bookmarks
            .first()
            .and_then(|(_, update)| update.after.clone())
        {
            tx.repo_mut()
                .set_local_bookmark_target(bookmark, RefTarget::normal(signed_target));
        }

        let git_settings = GitSettings::from_settings(self.workspace.settings())
            .map_err(|_| PublicationFailure::not_updated(PublicationFailureReason::Backend))?;
        let mut subprocess_options = git_settings.to_subprocess_options();
        subprocess_options
            .environment
            .insert(OsString::from("GIT_TERMINAL_PROMPT"), OsString::from("0"));
        let mut callback = SilentGitSubprocessCallback;
        let push_result = git::push_refs(
            tx.repo_mut(),
            subprocess_options,
            remote,
            &targets,
            &mut callback,
            &GitPushOptions::default(),
        );
        let bookmark_string = bookmark.as_str();
        let remote_string = remote.as_str();

        match push_result {
            Ok(stats) if stats.all_ok() => {
                let operation_description =
                    format!("push bookmark {bookmark_string} to git remote {remote_string}");
                if let Err(error) = self
                    .finish_transaction(tx, "push", operation_description)
                    .await
                {
                    let reason = match error {
                        AppError::OperationIncomplete { .. } => {
                            PublicationFailureReason::LocalTrackingUpdate
                        }
                        _ => PublicationFailureReason::Backend,
                    };
                    return Err(PublicationFailure::updated(reason));
                }
                Ok(remote_action)
            }
            Ok(stats) if !stats.pushed.is_empty() => {
                let operation_description =
                    format!("push bookmark {bookmark_string} to git remote {remote_string}");
                let _ = self
                    .finish_transaction(tx, "push", operation_description)
                    .await;
                Err(PublicationFailure::updated(
                    PublicationFailureReason::LocalTrackingUpdate,
                ))
            }
            Ok(stats) => {
                let reason = if !stats.rejected.is_empty() {
                    PublicationFailureReason::LeaseRejected
                } else if !stats.remote_rejected.is_empty() {
                    PublicationFailureReason::RemoteRejected
                } else {
                    PublicationFailureReason::Backend
                };
                Err(PublicationFailure::not_updated(reason))
            }
            Err(GitPushError::Subprocess(_)) => Err(PublicationFailure::unknown(
                PublicationFailureReason::TransportOrAuthentication,
            )),
            Err(_) => Err(PublicationFailure::unknown(
                PublicationFailureReason::Backend,
            )),
        }
    }

    async fn sign_commits_before_push(
        &self,
        tx: &mut Transaction,
        remote: &RemoteName,
        targets: &mut GitPushRefTargets,
    ) -> Result<(), AppError> {
        let new_heads = targets
            .bookmarks
            .iter()
            .chain(&targets.tags)
            .filter_map(|(_, update)| update.after.clone())
            .collect();
        let old_heads = tx
            .base_repo()
            .view()
            .remote_bookmarks(remote)
            .flat_map(|(_, remote_ref)| remote_ref.target.added_ids().cloned())
            .collect();
        let immutable = self.resolve_immutable_expression(tx.base_repo().as_ref())?;
        let to_push = RevsetExpression::commits(old_heads)
            .union(&immutable)
            .range(&RevsetExpression::commits(new_heads));
        let mut sign_settings = self.workspace.settings().sign_settings();
        sign_settings.behavior = SignBehavior::Own;
        let revset = to_push
            .evaluate(tx.repo())
            .map_err(|_| AppError::BackendFailure { operation: "push" })?;
        let mut stream = revset.stream().commits(tx.repo().store());
        let mut commit_ids = BTreeSet::new();
        while let Some(commit) = stream
            .try_next()
            .await
            .map_err(|_| AppError::BackendFailure { operation: "push" })?
        {
            if !commit.is_signed() && sign_settings.should_sign(commit.store_commit()) {
                commit_ids.insert(commit.id().clone());
            }
        }
        drop(stream);
        drop(revset);
        if commit_ids.is_empty() {
            return Ok(());
        }

        let mut old_to_new = HashMap::new();
        tx.repo_mut()
            .transform_descendants(commit_ids.iter().cloned().collect(), async |rewriter| {
                let old_commit_id = rewriter.old_commit().id().clone();
                let commit = if commit_ids.contains(&old_commit_id) {
                    rewriter
                        .reparent()
                        .set_sign_behavior(SignBehavior::Own)
                        .write()
                        .await?
                } else {
                    rewriter.reparent().write().await?
                };
                old_to_new.insert(old_commit_id, commit.id().clone());
                Ok(())
            })
            .await
            .map_err(|_| AppError::BackendFailure { operation: "push" })?;

        for (_, update) in targets.bookmarks.iter_mut().chain(&mut targets.tags) {
            if let Some(after) = &mut update.after
                && let Some(new_id) = old_to_new.get(after)
            {
                *after = new_id.clone();
            }
        }
        Ok(())
    }

    async fn resolve_one(&self, revision: &str) -> Result<Commit, AppError> {
        let ui = Ui::null();
        let settings = self.workspace.settings();
        let fileset_aliases = load_fileset_aliases(&ui, settings.config()).map_err(|_| {
            AppError::RepositoryUnavailable {
                operation: "load_fileset_aliases",
            }
        })?;
        let revset_aliases = load_revset_aliases(&ui, settings.config()).map_err(|_| {
            AppError::RepositoryUnavailable {
                operation: "load_revset_aliases",
            }
        })?;
        let extensions = RevsetExtensions::default();
        let path_converter = RepoPathUiConverter::Fs {
            cwd: self.cwd.clone(),
            base: self.workspace.workspace_root().to_owned(),
        };
        let workspace_context = RevsetWorkspaceContext {
            path_converter: &path_converter,
            workspace_name: self.workspace.workspace_name(),
        };
        let context = RevsetParseContext {
            aliases_map: &revset_aliases,
            local_variables: Default::default(),
            user_email: settings.user_email(),
            date_pattern_context: Local::now().into(),
            default_ignored_remote: None,
            fileset_aliases_map: &fileset_aliases,
            extensions: &extensions,
            workspace: Some(workspace_context),
        };
        let mut diagnostics = RevsetDiagnostics::new();
        let expression = revset::parse(&mut diagnostics, revision, &context).map_err(|_| {
            AppError::InvalidArgument {
                argument: "change",
                constraint: "valid_revset",
            }
        })?;
        let resolver = SymbolResolver::new(self.repo.as_ref(), extensions.symbol_resolvers());
        let resolved = expression
            .resolve_user_expression(self.repo.as_ref(), &resolver)
            .map_err(|error| self.map_resolution_error(revision, error))?;
        let revset =
            resolved
                .evaluate(self.repo.as_ref())
                .map_err(|_| AppError::BackendFailure {
                    operation: "evaluate_revision",
                })?;
        let ids = revset.stream().try_collect::<Vec<_>>().await.map_err(|_| {
            AppError::BackendFailure {
                operation: "read_revision",
            }
        })?;
        match ids.as_slice() {
            [] => Err(AppError::RevisionNotFound {
                revision: revision.to_owned(),
            }),
            [commit_id] => self.commit_by_id(commit_id).await,
            _ => Err(AppError::RevisionAmbiguous {
                revision: revision.to_owned(),
                candidates: ids.iter().map(ToString::to_string).collect(),
            }),
        }
    }

    fn map_resolution_error(&self, revision: &str, error: RevsetResolutionError) -> AppError {
        match error {
            RevsetResolutionError::NoSuchRevision { .. }
            | RevsetResolutionError::WorkspaceMissingWorkingCopy { .. }
            | RevsetResolutionError::EmptyString => AppError::RevisionNotFound {
                revision: revision.to_owned(),
            },
            RevsetResolutionError::DivergentChangeId {
                visible_targets, ..
            } => AppError::RevisionAmbiguous {
                revision: revision.to_owned(),
                candidates: visible_targets
                    .into_iter()
                    .map(|(_, commit_id)| commit_id.to_string())
                    .collect(),
            },
            RevsetResolutionError::ConflictedRef { targets, .. } => AppError::RevisionAmbiguous {
                revision: revision.to_owned(),
                candidates: targets
                    .into_iter()
                    .map(|commit_id| commit_id.to_string())
                    .collect(),
            },
            RevsetResolutionError::AmbiguousCommitIdPrefix(_)
            | RevsetResolutionError::AmbiguousChangeIdPrefix(_) => AppError::RevisionAmbiguous {
                revision: revision.to_owned(),
                candidates: Vec::new(),
            },
            RevsetResolutionError::Backend(_) => AppError::BackendFailure {
                operation: "resolve_revision",
            },
            RevsetResolutionError::Other(_) => AppError::InvalidArgument {
                argument: "change",
                constraint: "resolvable_revision",
            },
        }
    }

    async fn visible_conflict_count(&self) -> Result<u64, AppError> {
        let expression = RevsetExpression::filter(RevsetFilterPredicate::HasConflict);
        let revset =
            expression
                .evaluate(self.repo.as_ref())
                .map_err(|_| AppError::BackendFailure {
                    operation: "evaluate_conflicts",
                })?;
        let count = revset
            .stream()
            .try_fold(0_u64, |count, _| async move { Ok(count.saturating_add(1)) })
            .await
            .map_err(|_| AppError::BackendFailure {
                operation: "read_conflicts",
            })?;
        Ok(count)
    }

    async fn visible_divergence_count(&self) -> Result<u64, AppError> {
        let revset = RevsetExpression::divergent()
            .evaluate(self.repo.as_ref())
            .map_err(|_| AppError::BackendFailure {
                operation: "evaluate_divergence",
            })?;
        let mut stream = revset.stream();
        let mut changes = BTreeSet::new();
        while let Some(commit_id) =
            stream
                .try_next()
                .await
                .map_err(|_| AppError::BackendFailure {
                    operation: "read_divergence",
                })?
        {
            let commit = self.commit_by_id(&commit_id).await?;
            changes.insert(commit.change_id().to_string());
        }
        Ok(changes.len().try_into().unwrap_or(u64::MAX))
    }

    async fn diff_for_commit(
        &self,
        commit: &Commit,
        include_patch: bool,
        full: bool,
    ) -> Result<ComputedDiff, AppError> {
        let before =
            commit
                .parent_tree(self.repo.as_ref())
                .await
                .map_err(|_| AppError::BackendFailure {
                    operation: "load_parent_tree",
                })?;
        let after = commit.tree();
        self.compute_diff(&before, &after, include_patch, full)
            .await
    }

    async fn compute_diff(
        &self,
        before: &MergedTree,
        after: &MergedTree,
        include_patch: bool,
        full: bool,
    ) -> Result<ComputedDiff, AppError> {
        let mut stream = before.diff_stream(after, &EverythingMatcher);
        let mut stat = DiffStat {
            changed_files: 0,
            added_lines: 0,
            removed_lines: 0,
        };
        let mut sections = Vec::new();

        while let Some(TreeDiffEntry { path, values }) = stream.next().await {
            let JjDiff {
                before: before_value,
                after: after_value,
            } = values.map_err(|_| AppError::BackendFailure {
                operation: "read_tree_diff",
            })?;
            let before_content = self
                .materialize_content(before_value, &path, before.labels())
                .await?;
            let after_content = self
                .materialize_content(after_value, &path, after.labels())
                .await?;
            stat.changed_files = stat.changed_files.saturating_add(1);

            if let (Some(before_text), Some(after_text)) =
                (before_content.text(), after_content.text())
            {
                let (added, removed) = line_changes(before_text, after_text);
                stat.added_lines = stat.added_lines.saturating_add(added);
                stat.removed_lines = stat.removed_lines.saturating_add(removed);
            }
            if include_patch {
                sections.push(render_file_patch(&path, &before_content, &after_content));
            }
        }

        let patch = include_patch.then(|| truncate_patch(sections, full));
        Ok(ComputedDiff { stat, patch })
    }

    async fn materialize_content(
        &self,
        value: jj_lib::merge::MergedTreeValue,
        path: &RepoPath,
        labels: &jj_lib::conflict_labels::ConflictLabels,
    ) -> Result<PatchContent, AppError> {
        let value = materialize_tree_value(self.repo.store(), path, value, labels)
            .await
            .map_err(|_| AppError::BackendFailure {
                operation: "read_diff_content",
            })?;
        match value {
            MaterializedTreeValue::Absent => Ok(PatchContent::Absent),
            MaterializedTreeValue::File(mut file) => {
                let mode = Some(if file.executable { "100755" } else { "100644" });
                let bytes = file
                    .read_all(path)
                    .await
                    .map_err(|_| AppError::BackendFailure {
                        operation: "read_diff_content",
                    })?;
                Ok(PatchContent::from_bytes(bytes, mode))
            }
            MaterializedTreeValue::FileConflict(conflict) => {
                let bytes = materialize_merge_result_to_bytes(
                    &conflict.contents,
                    &conflict.labels,
                    &ConflictMaterializeOptions {
                        marker_style: ConflictMarkerStyle::Diff,
                        marker_len: None,
                        merge: self.repo.store().merge_options().clone(),
                    },
                )
                .to_vec();
                let mode = conflict
                    .executable
                    .map(|executable| if executable { "100755" } else { "100644" });
                Ok(PatchContent::from_bytes(bytes, mode))
            }
            MaterializedTreeValue::Symlink { target, .. } => Ok(PatchContent::Text {
                body: target,
                mode: Some("120000"),
            }),
            MaterializedTreeValue::GitSubmodule(commit_id) => Ok(PatchContent::Text {
                body: format!("Subproject commit {commit_id}\n"),
                mode: Some("160000"),
            }),
            MaterializedTreeValue::AccessDenied(_) => Err(AppError::BackendFailure {
                operation: "read_diff_content",
            }),
            MaterializedTreeValue::OtherConflict { .. } | MaterializedTreeValue::Tree(_) => {
                Ok(PatchContent::Binary { mode: None })
            }
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
struct RegularFileMetadata {
    executable: bool,
    copy_id: CopyId,
}

struct SelectedTextFile {
    path: RepoPathBuf,
    text: String,
    before_metadata: Option<RegularFileMetadata>,
    after_metadata: Option<RegularFileMetadata>,
}

#[derive(Clone)]
struct LineHunk {
    range: HunkRange,
    reference: HunkRef,
}

struct TextHunkAnalysis {
    hunks: Vec<HunkRef>,
    skipped_paths: Vec<SkippedPath>,
}

fn regular_file_metadata(
    value: &jj_lib::merge::MergedTreeValue,
) -> Result<Option<RegularFileMetadata>, &'static str> {
    match value.as_resolved() {
        None => Err("conflict"),
        Some(None) => Ok(None),
        Some(Some(TreeValue::File {
            executable,
            copy_id,
            ..
        })) => Ok(Some(RegularFileMetadata {
            executable: *executable,
            copy_id: copy_id.clone(),
        })),
        Some(Some(TreeValue::Symlink(_))) => Err("symlink"),
        Some(Some(TreeValue::GitSubmodule(_))) => Err("submodule"),
        Some(Some(TreeValue::Tree(_))) => Err("unsupported_content"),
    }
}

fn format_hunk_range(range: HunkRange) -> String {
    match range {
        HunkRange::Lines { start, end } if start == end => start.to_string(),
        HunkRange::Lines { start, end } => format!("{start}-{end}"),
        HunkRange::Deletion { at } => format!("{at}-0"),
    }
}

fn line_hunks(path: &str, before: &str, after: &str) -> Vec<LineHunk> {
    let diff = ContentDiff::by_line([before, after]);
    let mut hunks = Vec::new();
    for hunk in diff.hunk_ranges() {
        if hunk.kind != DiffHunkKind::Different {
            continue;
        }
        let [_, after_range] = hunk.ranges.as_slice() else {
            continue;
        };
        let range = post_image_hunk_range(after, after_range);
        hunks.push(LineHunk {
            reference: HunkRef {
                path: path.to_owned(),
                lines: format_hunk_range(range),
            },
            range,
        });
    }
    hunks
}

fn post_image_hunk_range(after: &str, after_range: &Range<usize>) -> HunkRange {
    if after_range.is_empty() {
        return HunkRange::Deletion {
            at: line_at_byte_offset(after, after_range.start),
        };
    }
    let start = line_at_byte_offset(after, after_range.start);
    let count = line_count(&after[after_range.clone()]);
    let end = start.saturating_add(u32::try_from(count.saturating_sub(1)).unwrap_or(u32::MAX));
    HunkRange::Lines { start, end }
}

fn line_at_byte_offset(text: &str, offset: usize) -> u32 {
    let line = if offset == text.len() {
        line_count(text).saturating_add(1)
    } else {
        text.as_bytes()[..offset]
            .iter()
            .filter(|&&byte| byte == b'\n')
            .count()
            .saturating_add(1)
    };
    u32::try_from(line).unwrap_or(u32::MAX)
}

fn line_count(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.as_bytes()
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        .saturating_add(usize::from(!text.ends_with('\n')))
}

fn selected_text_by_hunks(
    before: &str,
    after: &str,
    selected_ranges: &HashSet<HunkRange>,
) -> String {
    let diff = ContentDiff::by_line([before, after]);
    let mut selected = String::with_capacity(before.len().max(after.len()));
    for hunk in diff.hunk_ranges() {
        let [before_range, after_range] = hunk.ranges.as_slice() else {
            continue;
        };
        match hunk.kind {
            DiffHunkKind::Matching => selected.push_str(&after[after_range.clone()]),
            DiffHunkKind::Different => {
                if selected_ranges.contains(&post_image_hunk_range(after, after_range)) {
                    selected.push_str(&after[after_range.clone()]);
                } else {
                    selected.push_str(&before[before_range.clone()]);
                }
            }
        }
    }
    selected
}

fn invalid_hunk_selection(
    path: &str,
    requested: HunkRange,
    reason: &str,
    nearest_hunks: Vec<HunkRef>,
) -> AppError {
    AppError::InvalidHunkSelection {
        path: path.to_owned(),
        requested: format_hunk_range(requested),
        reason: reason.to_owned(),
        nearest_hunks,
    }
}

fn nearest_hunks(path: &str, requested: HunkRange, candidates: &[LineHunk]) -> Vec<HunkRef> {
    let requested_start = hunk_range_start(requested);
    let mut candidates = candidates.to_vec();
    candidates.sort_by(|left, right| {
        requested_start
            .abs_diff(hunk_range_start(left.range))
            .cmp(&requested_start.abs_diff(hunk_range_start(right.range)))
            .then_with(|| hunk_range_key(left.range).cmp(&hunk_range_key(right.range)))
            .then_with(|| left.reference.path.cmp(&right.reference.path))
    });
    candidates
        .into_iter()
        .take(3)
        .map(|candidate| HunkRef {
            path: if candidate.reference.path.is_empty() {
                path.to_owned()
            } else {
                candidate.reference.path
            },
            lines: candidate.reference.lines,
        })
        .collect()
}

fn hunk_range_start(range: HunkRange) -> u32 {
    match range {
        HunkRange::Lines { start, .. } | HunkRange::Deletion { at: start } => start,
    }
}

fn hunk_range_key(range: HunkRange) -> (u32, u32) {
    match range {
        HunkRange::Lines { start, end } => (start, end),
        HunkRange::Deletion { at } => (at, 0),
    }
}

fn compare_line_hunks(left: &LineHunk, right: &LineHunk) -> std::cmp::Ordering {
    left.reference
        .path
        .cmp(&right.reference.path)
        .then_with(|| hunk_range_key(left.range).cmp(&hunk_range_key(right.range)))
}

async fn commit_difference_count(
    repo: &dyn jj_lib::repo::Repo,
    head: &CommitId,
    excluded_head: &CommitId,
) -> Result<u64, AppError> {
    let included = RevsetExpression::commits(vec![head.clone()]).ancestors();
    let excluded = RevsetExpression::commits(vec![excluded_head.clone()]).ancestors();
    let expression = included
        .intersection(&excluded.negated())
        .intersection(&RevsetExpression::root().negated());
    let revset = expression
        .evaluate(repo)
        .map_err(|_| AppError::BackendFailure {
            operation: "compare_bookmarks",
        })?;
    let mut stream = revset.stream();
    let mut count = 0;
    while stream
        .try_next()
        .await
        .map_err(|_| AppError::BackendFailure {
            operation: "compare_bookmarks",
        })?
        .is_some()
    {
        count += 1;
    }
    Ok(count)
}

async fn bookmark_target_state(
    repo: &dyn jj_lib::repo::Repo,
    target: &RefTarget,
) -> Result<BookmarkTargetState, AppError> {
    let mut added_change_ids = Vec::new();
    for commit_id in target.added_ids() {
        let commit = repo
            .store()
            .get_commit_async(commit_id)
            .await
            .map_err(|_| AppError::BackendFailure {
                operation: "read_bookmarks",
            })?;
        added_change_ids.push(commit.change_id().to_string());
    }
    let mut removed_change_ids = Vec::new();
    for commit_id in target.removed_ids() {
        let commit = repo
            .store()
            .get_commit_async(commit_id)
            .await
            .map_err(|_| AppError::BackendFailure {
                operation: "read_bookmarks",
            })?;
        removed_change_ids.push(commit.change_id().to_string());
    }
    added_change_ids.sort();
    removed_change_ids.sort();
    Ok(BookmarkTargetState {
        present: target.is_present(),
        conflicted: target.has_conflict(),
        added_change_ids,
        removed_change_ids,
    })
}

fn compare_hunk_refs(left: &HunkRef, right: &HunkRef) -> std::cmp::Ordering {
    left.path
        .cmp(&right.path)
        .then_with(|| hunk_ref_key(&left.lines).cmp(&hunk_ref_key(&right.lines)))
}

fn compare_hunk_ref_slices(left: &[HunkRef], right: &[HunkRef]) -> std::cmp::Ordering {
    for (left_hunk, right_hunk) in left.iter().zip(right) {
        let ordering = compare_hunk_refs(left_hunk, right_hunk);
        if ordering != std::cmp::Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

fn compare_unmoved_hunks(left: &UnmovedHunk, right: &UnmovedHunk) -> std::cmp::Ordering {
    left.path
        .cmp(&right.path)
        .then_with(|| hunk_ref_key(&left.lines).cmp(&hunk_ref_key(&right.lines)))
        .then_with(|| left.reason.cmp(&right.reason))
}

fn hunk_ref_key(lines: &str) -> (u32, u32) {
    if let Some((start, end)) = lines.split_once('-') {
        return (
            start.parse().unwrap_or(u32::MAX),
            end.parse().unwrap_or(u32::MAX),
        );
    }
    let line = lines.parse().unwrap_or(u32::MAX);
    (line, line)
}

fn normalize_absorb_skip_reason(reason: &str) -> &'static str {
    let reason = reason.to_ascii_lowercase();
    if reason.contains("access") && reason.contains("denied") {
        "access_denied"
    } else if reason.contains("conflict") {
        "conflict"
    } else if reason.contains("symlink") {
        "symlink"
    } else if reason.contains("submodule") {
        "submodule"
    } else {
        "unsupported_content"
    }
}

fn linear_history_order(commits: &[Commit]) -> Option<Vec<Commit>> {
    let selected_ids = commits
        .iter()
        .map(|commit| commit.id().clone())
        .collect::<HashSet<_>>();
    let commits_by_id = commits
        .iter()
        .map(|commit| (commit.id().clone(), commit.clone()))
        .collect::<HashMap<_, _>>();
    let mut children = HashMap::<CommitId, Vec<CommitId>>::new();
    let mut roots = Vec::new();
    for commit in commits {
        let parent_id = &commit.parent_ids()[0];
        if selected_ids.contains(parent_id) {
            children
                .entry(parent_id.clone())
                .or_default()
                .push(commit.id().clone());
        } else {
            roots.push(commit.id().clone());
        }
    }
    if roots.len() != 1 || children.values().any(|children| children.len() > 1) {
        return None;
    }
    let mut ordered = Vec::with_capacity(commits.len());
    let mut current = roots.pop()?;
    loop {
        ordered.push(commits_by_id.get(&current)?.clone());
        let Some(next) = children.remove(&current) else {
            break;
        };
        let [next] = next.as_slice() else {
            return None;
        };
        current = next.clone();
    }
    (ordered.len() == commits.len()).then_some(ordered)
}

fn history_change_from_commit(commit: &Commit) -> HistoryChange {
    HistoryChange {
        change_id: commit.change_id().to_string(),
        description: commit.description().to_owned(),
        status: Status {
            conflicted: commit.has_conflict(),
        },
    }
}

struct BookmarkContext {
    name: RefNameBuf,
    remote: RemoteNameBuf,
    old_local_target: RefTarget,
}

struct PublicationFailure {
    remote_state: PublicationRemoteState,
    reason: PublicationFailureReason,
}

impl PublicationFailure {
    fn not_updated(reason: PublicationFailureReason) -> Self {
        Self {
            remote_state: PublicationRemoteState::NotUpdated,
            reason,
        }
    }

    fn updated(reason: PublicationFailureReason) -> Self {
        Self {
            remote_state: PublicationRemoteState::Updated,
            reason,
        }
    }

    fn unknown(reason: PublicationFailureReason) -> Self {
        Self {
            remote_state: PublicationRemoteState::Unknown,
            reason,
        }
    }
}

struct SilentGitSubprocessCallback;

impl GitSubprocessCallback for SilentGitSubprocessCallback {
    fn needs_progress(&self) -> bool {
        false
    }

    fn progress(&mut self, _progress: &GitProgress) -> io::Result<()> {
        Ok(())
    }

    fn local_sideband(
        &mut self,
        _message: &[u8],
        _term: Option<GitSidebandLineTerminator>,
    ) -> io::Result<()> {
        Ok(())
    }

    fn remote_sideband(
        &mut self,
        _message: &[u8],
        _term: Option<GitSidebandLineTerminator>,
    ) -> io::Result<()> {
        Ok(())
    }
}

fn normalize_message(message: &str) -> String {
    join_message_paragraphs(&[message.to_owned()])
}

fn change_from_commit(commit: &Commit) -> Change {
    Change {
        change_id: commit.change_id().to_string(),
        description: commit.description().to_owned(),
    }
}

fn find_workspace_root(cwd: &Path) -> Option<PathBuf> {
    cwd.ancestors()
        .find(|candidate| candidate.join(".jj").is_dir())
        .map(Path::to_owned)
}

fn synchronize_working_copy(workspace_root: &Path) -> Result<(), AppError> {
    let status = Command::new("jj")
        .args(["--quiet", "--no-pager", "status"])
        .current_dir(workspace_root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|_| AppError::RepositoryUnavailable {
            operation: "synchronize_working_copy",
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::RepositoryUnavailable {
            operation: "synchronize_working_copy",
        })
    }
}

fn load_settings(loader: &dyn WorkspaceLoader) -> Result<UserSettings, AppError> {
    let ui = Ui::null();
    let mut config = config_from_environment(default_config_layers());
    let mut config_env = ConfigEnv::from_environment();
    config_env
        .reload_system_config(&mut config)
        .map_err(|_| AppError::RepositoryUnavailable {
            operation: "load_system_config",
        })?;
    config_env
        .reload_user_config(&mut config)
        .map_err(|_| AppError::RepositoryUnavailable {
            operation: "load_user_config",
        })?;
    config_env.reset_repo_path(loader.repo_path());
    config_env
        .reload_repo_config(&ui, &mut config)
        .map_err(|_| AppError::RepositoryUnavailable {
            operation: "load_repository_config",
        })?;
    config_env.reset_workspace_path(loader.workspace_root());
    config_env
        .reload_workspace_config(&ui, &mut config)
        .map_err(|_| AppError::RepositoryUnavailable {
            operation: "load_workspace_config",
        })?;
    let config =
        config_env
            .resolve_config(&config)
            .map_err(|_| AppError::RepositoryUnavailable {
                operation: "resolve_config",
            })?;
    UserSettings::from_config(config).map_err(|_| AppError::RepositoryUnavailable {
        operation: "load_settings",
    })
}

struct ComputedDiff {
    stat: DiffStat,
    patch: Option<Patch>,
}

enum PatchContent {
    Absent,
    Text {
        body: String,
        mode: Option<&'static str>,
    },
    Binary {
        mode: Option<&'static str>,
    },
}

impl PatchContent {
    fn from_bytes(bytes: Vec<u8>, mode: Option<&'static str>) -> Self {
        if bytes.contains(&0) {
            Self::Binary { mode }
        } else {
            match String::from_utf8(bytes) {
                Ok(body) => Self::Text { body, mode },
                Err(_) => Self::Binary { mode },
            }
        }
    }

    fn text(&self) -> Option<&str> {
        match self {
            Self::Absent => Some(""),
            Self::Text { body, .. } => Some(body),
            Self::Binary { .. } => None,
        }
    }

    fn mode(&self) -> Option<&'static str> {
        match self {
            Self::Absent => None,
            Self::Text { mode, .. } | Self::Binary { mode } => *mode,
        }
    }

    fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }
}

fn line_changes(before: &str, after: &str) -> (u64, u64) {
    let diff = TextDiff::from_lines(before, after);
    diff.iter_all_changes()
        .fold((0_u64, 0_u64), |(added, removed), change| {
            match change.tag() {
                ChangeTag::Insert => (added.saturating_add(1), removed),
                ChangeTag::Delete => (added, removed.saturating_add(1)),
                ChangeTag::Equal => (added, removed),
            }
        })
}

fn render_file_patch(path: &RepoPath, before: &PatchContent, after: &PatchContent) -> String {
    let path = path.as_internal_file_string();
    let before_label = if before.is_absent() {
        "/dev/null".to_owned()
    } else {
        format!("a/{path}")
    };
    let after_label = if after.is_absent() {
        "/dev/null".to_owned()
    } else {
        format!("b/{path}")
    };
    let mut section = format!("diff --git a/{path} b/{path}\n");
    match (before.mode(), after.mode()) {
        (None, Some(mode)) => section.push_str(&format!("new file mode {mode}\n")),
        (Some(mode), None) => section.push_str(&format!("deleted file mode {mode}\n")),
        (Some(before_mode), Some(after_mode)) if before_mode != after_mode => {
            section.push_str(&format!("old mode {before_mode}\nnew mode {after_mode}\n"));
        }
        _ => {}
    }
    match (before.text(), after.text()) {
        (Some(before_text), Some(after_text)) => {
            let diff = TextDiff::from_lines(before_text, after_text);
            section.push_str(
                &diff
                    .unified_diff()
                    .context_radius(3)
                    .header(&before_label, &after_label)
                    .to_string(),
            );
        }
        _ => section.push_str(&format!(
            "Binary files {before_label} and {after_label} differ\n"
        )),
    }
    section
}

fn truncate_patch(sections: Vec<String>, full: bool) -> Patch {
    let full_bytes = sections.iter().fold(0_u64, |total, section| {
        total.saturating_add(section.len() as u64)
    });
    if full {
        let body = sections.concat();
        return Patch {
            body,
            truncation: Truncation {
                truncated: false,
                limit_bytes: None,
                returned_bytes: full_bytes,
                omitted_bytes: 0,
            },
        };
    }

    let mut body = String::new();
    for section in sections {
        if body
            .len()
            .checked_add(section.len())
            .is_none_or(|size| size > DEFAULT_PATCH_LIMIT_BYTES as usize)
        {
            break;
        }
        body.push_str(&section);
    }
    let returned_bytes = body.len() as u64;
    Patch {
        body,
        truncation: Truncation {
            truncated: returned_bytes < full_bytes,
            limit_bytes: Some(DEFAULT_PATCH_LIMIT_BYTES),
            returned_bytes,
            omitted_bytes: full_bytes.saturating_sub(returned_bytes),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jj_lib::op_store::{RemoteRef, RemoteRefState};

    fn commit_id(hex: &'static str) -> CommitId {
        CommitId::from_hex(hex)
    }

    #[test]
    fn classify_local_conflict() {
        let local = RefTarget::from_legacy_form([], [commit_id("11"), commit_id("22")]);
        let remote = RemoteRef {
            target: RefTarget::absent(),
            state: RemoteRefState::Tracked,
        };
        assert_eq!(
            classify_ref_push_action(LocalAndRemoteRef {
                local_target: &local,
                remote_ref: &remote,
            }),
            RefPushAction::LocalConflicted
        );
    }

    #[test]
    fn classify_remote_conflict() {
        let local = RefTarget::normal(commit_id("11"));
        let remote = RemoteRef {
            target: RefTarget::from_legacy_form([], [commit_id("22"), commit_id("33")]),
            state: RemoteRefState::Tracked,
        };
        assert_eq!(
            classify_ref_push_action(LocalAndRemoteRef {
                local_target: &local,
                remote_ref: &remote,
            }),
            RefPushAction::RemoteConflicted
        );
    }

    #[test]
    fn classify_remote_untracked() {
        let local = RefTarget::normal(commit_id("11"));
        let remote = RemoteRef {
            target: RefTarget::normal(commit_id("22")),
            state: RemoteRefState::New,
        };
        assert_eq!(
            classify_ref_push_action(LocalAndRemoteRef {
                local_target: &local,
                remote_ref: &remote,
            }),
            RefPushAction::RemoteUntracked
        );
    }

    #[test]
    fn line_hunks_use_context_free_post_image_ranges() {
        let hunks = line_hunks("file.txt", "one\ntwo\nthree\n", "one\nTWO\nthree\nfour\n");

        assert_eq!(
            hunks
                .iter()
                .map(|hunk| hunk.reference.lines.as_str())
                .collect::<Vec<_>>(),
            vec!["2", "4"]
        );
    }

    #[test]
    fn eof_deletion_uses_zero_width_post_image_boundary() {
        let hunks = line_hunks("file.txt", "one\ntwo", "one\n");

        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].reference.lines, "2-0");
        assert_eq!(hunks[0].range, HunkRange::Deletion { at: 2 });
    }

    #[test]
    fn selected_text_keeps_unselected_hunks_from_left() {
        let before = "one\ntwo\nthree\n";
        let after = "one\nTWO\nthree\nfour\n";
        let selected_ranges = HashSet::from([HunkRange::Lines { start: 2, end: 2 }]);

        assert_eq!(
            selected_text_by_hunks(before, after, &selected_ranges),
            "one\nTWO\nthree\n"
        );
    }

    #[test]
    fn line_hunks_keep_adjacent_post_image_ranges_distinct() {
        let hunks = line_hunks(
            "file.txt",
            "one\ntwo\nthree\nfour\n",
            "one\nTWO\nthree\nFOUR\n",
        );

        assert_eq!(
            hunks
                .iter()
                .map(|hunk| hunk.reference.lines.as_str())
                .collect::<Vec<_>>(),
            vec!["2", "4"]
        );
    }

    #[test]
    fn nearest_hunks_are_sorted_and_bounded() {
        let candidates = line_hunks(
            "file.txt",
            "one\ntwo\nthree\nfour\nfive\nsix\nseven\n",
            "one\nTWO\nthree\nFOUR\nfive\nSIX\nseven\n",
        );
        let nearest = nearest_hunks(
            "file.txt",
            HunkRange::Lines { start: 5, end: 5 },
            &candidates,
        );

        assert_eq!(
            nearest
                .iter()
                .map(|hunk| hunk.lines.as_str())
                .collect::<Vec<_>>(),
            vec!["4", "6", "2"]
        );
        assert!(nearest.len() <= 3);
    }
}
