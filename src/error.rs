use crate::model::{DescriptionAction, HunkRef, LocalBookmarkAction};
use crate::toon::ToonValue;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RewritabilityReason {
    Root,
    Immutable,
}

impl RewritabilityReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Immutable => "immutable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadinessReason {
    EmptyDescription,
    MissingIdentity,
    Conflicted,
    Private,
}

impl ReadinessReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::EmptyDescription => "empty_description",
            Self::MissingIdentity => "missing_identity",
            Self::Conflicted => "conflicted",
            Self::Private => "private",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteBookmarkRejectReason {
    RemoteConflicted,
    RemoteUntracked,
    LocalConflicted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationIncompleteStep {
    ColocatedGitSynchronization,
    WorkingCopyUpdate,
}

impl OperationIncompleteStep {
    fn as_str(self) -> &'static str {
        match self {
            Self::ColocatedGitSynchronization => "colocated_git_synchronization",
            Self::WorkingCopyUpdate => "working_copy_update",
        }
    }
}

impl RemoteBookmarkRejectReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::RemoteConflicted => "remote_conflicted",
            Self::RemoteUntracked => "remote_untracked",
            Self::LocalConflicted => "local_conflicted",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationRemoteState {
    NotUpdated,
    Updated,
    Unknown,
}

impl PublicationRemoteState {
    fn as_str(self) -> &'static str {
        match self {
            Self::NotUpdated => "not_updated",
            Self::Updated => "updated",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationFailureReason {
    LeaseRejected,
    RemoteRejected,
    TransportOrAuthentication,
    LocalTrackingUpdate,
    Backend,
}

impl PublicationFailureReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::LeaseRejected => "lease_rejected",
            Self::RemoteRejected => "remote_rejected",
            Self::TransportOrAuthentication => "transport_or_authentication",
            Self::LocalTrackingUpdate => "local_tracking_update",
            Self::Backend => "backend",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppError {
    InvalidArgument {
        argument: &'static str,
        constraint: &'static str,
    },
    RepositoryNotFound {
        path: String,
    },
    RepositoryUnavailable {
        operation: &'static str,
    },
    RevisionNotFound {
        revision: String,
    },
    RevisionAmbiguous {
        revision: String,
        candidates: Vec<String>,
    },
    BackendFailure {
        operation: &'static str,
    },
    ChangeNotRewritable {
        change_id: String,
        reason: RewritabilityReason,
    },
    ChangeNotReady {
        change_id: String,
        reasons: Vec<ReadinessReason>,
    },
    BookmarkMoveRejected {
        bookmark: String,
        change_id: String,
    },
    BookmarkNotFound {
        bookmark: String,
    },
    RemoteNotFound {
        remote: String,
    },
    RemoteBookmarkRejected {
        bookmark: String,
        remote: String,
        reason: RemoteBookmarkRejectReason,
    },
    OperationIncomplete {
        operation: &'static str,
        failed_step: OperationIncompleteStep,
    },
    FinishPartial {
        change_id: String,
        bookmark: String,
        remote: String,
        description_action: DescriptionAction,
        local_action: LocalBookmarkAction,
        remote_state: PublicationRemoteState,
        reason: PublicationFailureReason,
    },
    BookmarkPushPartial {
        bookmark: String,
        target_change_id: String,
        target_commit_id: String,
        remote: String,
        remote_state: PublicationRemoteState,
        reason: PublicationFailureReason,
    },
    InvalidHunkSelection {
        path: String,
        requested: String,
        reason: String,
        nearest_hunks: Vec<HunkRef>,
    },
    InvalidPartitionManifest {
        pointer: String,
        reason: String,
    },
    PartitionManifestTooLarge {
        limit_bytes: u64,
    },
    StalePartitionSource {
        change_id: String,
        expected_commit_id: String,
        current_commit_id: String,
    },
    DuplicatePartitionHunk {
        path: String,
        lines: String,
        first_part_ordinal: u64,
        second_part_ordinal: u64,
    },
    InvalidPartitionHunk {
        part_ordinal: u64,
        path: String,
        requested: String,
        reason: String,
        nearest_hunks: Vec<HunkRef>,
    },
    PartitionRemainderNotEmpty {
        hunk_count: u64,
        skipped_path_count: u64,
        hunks: Vec<HunkRef>,
        skipped_paths: Vec<(String, String)>,
    },
    PartitionWorkingCopyUnsafe {
        total_count: u64,
        complete: bool,
        workspaces: Vec<String>,
    },
    InvalidHistoryShape {
        operation: String,
        reason: String,
        change_ids: Vec<String>,
    },
    OperationHistoryDiverged {
        operation_ids: Vec<String>,
    },
    InvalidOperationId {
        operation_id: String,
    },
    OperationNotFound {
        operation_id: String,
    },
    OperationAmbiguous {
        operation_id: String,
        candidates: Vec<String>,
    },
    OperationNotAncestor {
        operation_id: String,
    },
    NothingToUndo,
    OperationTargetUnsafe {
        operation_id: String,
        reason: &'static str,
    },
    GithubRepositoryNotFound,
    GithubRepositoryAmbiguous {
        candidates: Vec<String>,
    },
    GithubCliNotFound,
    GithubAuthRequired,
    PullRequestNotFound {
        number: u64,
    },
    GithubRateLimited,
    GithubApiUnavailable {
        retryable: bool,
    },
    GithubResponseInvalid,
    GithubResponseTooLarge,
    SquashMessageRequired {
        source_change_id: String,
        destination_change_id: String,
    },
    SquashDestinationRequired {
        source_change_id: String,
    },
    SkillOutputConflict {
        path: String,
    },
    InvalidSkillOutput {
        path: String,
        reason: &'static str,
    },
    Internal,
}

impl AppError {
    pub fn to_toon_value(&self) -> ToonValue {
        match self {
            Self::InvalidArgument {
                argument,
                constraint,
            } => ToonValue::Object(vec![
                ("code", string("invalid_argument")),
                ("argument", string(argument)),
                ("constraint", string(constraint)),
            ]),
            Self::RepositoryNotFound { path } => ToonValue::Object(vec![
                ("code", string("repository_not_found")),
                ("path", string(path)),
            ]),
            Self::RepositoryUnavailable { operation } => ToonValue::Object(vec![
                ("code", string("repository_unavailable")),
                ("operation", string(operation)),
            ]),
            Self::RevisionNotFound { revision } => ToonValue::Object(vec![
                ("code", string("revision_not_found")),
                ("revision", string(revision)),
            ]),
            Self::RevisionAmbiguous {
                revision,
                candidates,
            } => ToonValue::Object(vec![
                ("code", string("revision_ambiguous")),
                ("revision", string(revision)),
                (
                    "candidates",
                    ToonValue::Array(
                        candidates
                            .iter()
                            .map(|candidate| string(candidate))
                            .collect(),
                    ),
                ),
            ]),
            Self::BackendFailure { operation } => ToonValue::Object(vec![
                ("code", string("backend_failure")),
                ("operation", string(operation)),
            ]),
            Self::ChangeNotRewritable { change_id, reason } => ToonValue::Object(vec![
                ("code", string("change_not_rewritable")),
                ("change_id", string(change_id)),
                ("reason", string(reason.as_str())),
            ]),
            Self::ChangeNotReady { change_id, reasons } => ToonValue::Object(vec![
                ("code", string("change_not_ready")),
                ("change_id", string(change_id)),
                (
                    "reasons",
                    ToonValue::Array(
                        reasons
                            .iter()
                            .map(|reason| string(reason.as_str()))
                            .collect(),
                    ),
                ),
            ]),
            Self::BookmarkMoveRejected {
                bookmark,
                change_id,
            } => ToonValue::Object(vec![
                ("code", string("bookmark_move_rejected")),
                ("bookmark", string(bookmark)),
                ("change_id", string(change_id)),
                ("reason", string("backwards_or_sideways")),
            ]),
            Self::BookmarkNotFound { bookmark } => ToonValue::Object(vec![
                ("code", string("bookmark_not_found")),
                ("bookmark", string(bookmark)),
            ]),
            Self::RemoteNotFound { remote } => ToonValue::Object(vec![
                ("code", string("remote_not_found")),
                ("remote", string(remote)),
            ]),
            Self::RemoteBookmarkRejected {
                bookmark,
                remote,
                reason,
            } => ToonValue::Object(vec![
                ("code", string("remote_bookmark_rejected")),
                ("bookmark", string(bookmark)),
                ("remote", string(remote)),
                ("reason", string(reason.as_str())),
            ]),
            Self::OperationIncomplete {
                operation,
                failed_step,
            } => ToonValue::Object(vec![
                ("code", string("operation_incomplete")),
                ("operation", string(operation)),
                ("failed_step", string(failed_step.as_str())),
                ("repository_state", string("updated")),
            ]),
            Self::FinishPartial {
                change_id,
                bookmark,
                remote,
                description_action,
                local_action,
                remote_state,
                reason,
            } => ToonValue::Object(vec![
                ("code", string("finish_partial")),
                ("change_id", string(change_id)),
                ("bookmark", string(bookmark)),
                ("remote", string(remote)),
                ("description_action", string(description_action.as_str())),
                ("local_action", string(local_action.as_str())),
                ("remote_state", string(remote_state.as_str())),
                ("reason", string(reason.as_str())),
            ]),
            Self::BookmarkPushPartial {
                bookmark,
                target_change_id,
                target_commit_id,
                remote,
                remote_state,
                reason,
            } => ToonValue::Object(vec![
                ("code", string("bookmark_push_partial")),
                ("bookmark", string(bookmark)),
                ("target_change_id", string(target_change_id)),
                ("target_commit_id", string(target_commit_id)),
                ("remote", string(remote)),
                ("remote_state", string(remote_state.as_str())),
                ("reason", string(reason.as_str())),
            ]),
            Self::InvalidHunkSelection {
                path,
                requested,
                reason,
                nearest_hunks,
            } => ToonValue::Object(vec![
                ("code", string("invalid_hunk_selection")),
                ("path", string(path)),
                ("requested", string(requested)),
                ("reason", string(reason)),
                (
                    "nearest_hunks",
                    ToonValue::Array(nearest_hunks.iter().map(HunkRef::to_toon_value).collect()),
                ),
            ]),
            Self::InvalidPartitionManifest { pointer, reason } => ToonValue::Object(vec![
                ("code", string("invalid_partition_manifest")),
                ("pointer", string(pointer)),
                ("reason", string(reason)),
            ]),
            Self::PartitionManifestTooLarge { limit_bytes } => ToonValue::Object(vec![
                ("code", string("partition_manifest_too_large")),
                ("limit_bytes", ToonValue::UInt(*limit_bytes)),
            ]),
            Self::StalePartitionSource {
                change_id,
                expected_commit_id,
                current_commit_id,
            } => ToonValue::Object(vec![
                ("code", string("stale_partition_source")),
                ("change_id", string(change_id)),
                ("expected_commit_id", string(expected_commit_id)),
                ("current_commit_id", string(current_commit_id)),
            ]),
            Self::DuplicatePartitionHunk {
                path,
                lines,
                first_part_ordinal,
                second_part_ordinal,
            } => ToonValue::Object(vec![
                ("code", string("duplicate_partition_hunk")),
                ("path", string(path)),
                ("lines", string(lines)),
                ("first_part_ordinal", ToonValue::UInt(*first_part_ordinal)),
                ("second_part_ordinal", ToonValue::UInt(*second_part_ordinal)),
            ]),
            Self::InvalidPartitionHunk {
                part_ordinal,
                path,
                requested,
                reason,
                nearest_hunks,
            } => ToonValue::Object(vec![
                ("code", string("invalid_hunk_selection")),
                ("part_ordinal", ToonValue::UInt(*part_ordinal)),
                ("path", string(path)),
                ("requested", string(requested)),
                ("reason", string(reason)),
                (
                    "nearest_hunks",
                    ToonValue::Array(nearest_hunks.iter().map(HunkRef::to_toon_value).collect()),
                ),
            ]),
            Self::PartitionRemainderNotEmpty {
                hunk_count,
                skipped_path_count,
                hunks,
                skipped_paths,
            } => ToonValue::Object(vec![
                ("code", string("partition_remainder_not_empty")),
                ("hunk_count", ToonValue::UInt(*hunk_count)),
                ("skipped_path_count", ToonValue::UInt(*skipped_path_count)),
                (
                    "hunks",
                    ToonValue::Array(hunks.iter().map(HunkRef::to_toon_value).collect()),
                ),
                (
                    "skipped_paths",
                    ToonValue::Array(
                        skipped_paths
                            .iter()
                            .map(|(path, reason)| {
                                ToonValue::Object(vec![
                                    ("path", string(path)),
                                    ("reason", string(reason)),
                                ])
                            })
                            .collect(),
                    ),
                ),
            ]),
            Self::PartitionWorkingCopyUnsafe {
                total_count,
                complete,
                workspaces,
            } => ToonValue::Object(vec![
                ("code", string("partition_working_copy_unsafe")),
                ("total_count", ToonValue::UInt(*total_count)),
                ("complete", ToonValue::Bool(*complete)),
                (
                    "workspaces",
                    ToonValue::Array(workspaces.iter().map(|name| string(name)).collect()),
                ),
            ]),
            Self::InvalidHistoryShape {
                operation,
                reason,
                change_ids,
            } => ToonValue::Object(vec![
                ("code", string("invalid_history_shape")),
                ("operation", string(operation)),
                ("reason", string(reason)),
                (
                    "change_ids",
                    ToonValue::Array(
                        change_ids
                            .iter()
                            .map(|change_id| string(change_id))
                            .collect(),
                    ),
                ),
            ]),
            Self::OperationHistoryDiverged { operation_ids } => ToonValue::Object(vec![
                ("code", string("operation_history_diverged")),
                (
                    "operation_ids",
                    ToonValue::Array(operation_ids.iter().map(|id| string(id)).collect()),
                ),
            ]),
            Self::InvalidOperationId { operation_id } => ToonValue::Object(vec![
                ("code", string("invalid_operation_id")),
                ("operation_id", string(operation_id)),
            ]),
            Self::OperationNotFound { operation_id } => ToonValue::Object(vec![
                ("code", string("operation_not_found")),
                ("operation_id", string(operation_id)),
            ]),
            Self::OperationAmbiguous {
                operation_id,
                candidates,
            } => ToonValue::Object(vec![
                ("code", string("operation_ambiguous")),
                ("operation_id", string(operation_id)),
                (
                    "candidates",
                    ToonValue::Array(candidates.iter().map(|id| string(id)).collect()),
                ),
            ]),
            Self::OperationNotAncestor { operation_id } => ToonValue::Object(vec![
                ("code", string("operation_not_ancestor")),
                ("operation_id", string(operation_id)),
            ]),
            Self::NothingToUndo => ToonValue::Object(vec![("code", string("nothing_to_undo"))]),
            Self::OperationTargetUnsafe {
                operation_id,
                reason,
            } => ToonValue::Object(vec![
                ("code", string("operation_target_unsafe")),
                ("operation_id", string(operation_id)),
                ("reason", string(reason)),
            ]),
            Self::GithubRepositoryNotFound => ToonValue::Object(vec![
                ("code", string("github_repository_not_found")),
                ("retryable", ToonValue::Bool(false)),
            ]),
            Self::GithubRepositoryAmbiguous { candidates } => ToonValue::Object(vec![
                ("code", string("github_repository_ambiguous")),
                ("retryable", ToonValue::Bool(false)),
                (
                    "candidates",
                    ToonValue::Array(candidates.iter().map(|value| string(value)).collect()),
                ),
            ]),
            Self::GithubCliNotFound => ToonValue::Object(vec![
                ("code", string("github_cli_not_found")),
                ("retryable", ToonValue::Bool(false)),
            ]),
            Self::GithubAuthRequired => ToonValue::Object(vec![
                ("code", string("github_auth_required")),
                ("retryable", ToonValue::Bool(false)),
            ]),
            Self::PullRequestNotFound { number } => ToonValue::Object(vec![
                ("code", string("pull_request_not_found")),
                ("number", ToonValue::UInt(*number)),
                ("retryable", ToonValue::Bool(false)),
            ]),
            Self::GithubRateLimited => ToonValue::Object(vec![
                ("code", string("github_rate_limited")),
                ("retryable", ToonValue::Bool(true)),
            ]),
            Self::GithubApiUnavailable { retryable } => ToonValue::Object(vec![
                ("code", string("github_api_unavailable")),
                ("retryable", ToonValue::Bool(*retryable)),
            ]),
            Self::GithubResponseInvalid => ToonValue::Object(vec![
                ("code", string("github_response_invalid")),
                ("retryable", ToonValue::Bool(false)),
            ]),
            Self::GithubResponseTooLarge => ToonValue::Object(vec![
                ("code", string("github_response_too_large")),
                ("retryable", ToonValue::Bool(false)),
            ]),
            Self::SquashMessageRequired {
                source_change_id,
                destination_change_id,
            } => ToonValue::Object(vec![
                ("code", string("squash_message_required")),
                ("source_change_id", string(source_change_id)),
                ("destination_change_id", string(destination_change_id)),
            ]),
            Self::SquashDestinationRequired { source_change_id } => ToonValue::Object(vec![
                ("code", string("squash_destination_required")),
                ("source_change_id", string(source_change_id)),
            ]),
            Self::SkillOutputConflict { path } => ToonValue::Object(vec![
                ("code", string("skill_output_conflict")),
                ("path", string(path)),
            ]),
            Self::InvalidSkillOutput { path, reason } => ToonValue::Object(vec![
                ("code", string("invalid_skill_output")),
                ("path", string(path)),
                ("reason", string(reason)),
            ]),
            Self::Internal => ToonValue::Object(vec![("code", string("internal"))]),
        }
    }
}

fn string(value: &str) -> ToonValue {
    ToonValue::String(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_incomplete_reports_the_actual_failed_step() {
        let error = AppError::OperationIncomplete {
            operation: "partition",
            failed_step: OperationIncompleteStep::ColocatedGitSynchronization,
        };

        let rendered = crate::toon::render(&error.to_toon_value());
        assert!(rendered.contains("failed_step: colocated_git_synchronization"));
        assert!(!rendered.contains("failed_step: working_copy_update"));
    }
}
