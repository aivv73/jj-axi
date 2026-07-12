use crate::toon::ToonValue;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Status {
    pub conflicted: bool,
}

impl Status {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![("conflicted", ToonValue::Bool(self.conflicted))])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffStat {
    pub changed_files: u64,
    pub added_lines: u64,
    pub removed_lines: u64,
}

impl DiffStat {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("changed_files", ToonValue::UInt(self.changed_files)),
            ("added_lines", ToonValue::UInt(self.added_lines)),
            ("removed_lines", ToonValue::UInt(self.removed_lines)),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogEntry {
    pub change_id: String,
    pub description: String,
    pub status: Status,
    pub commit_id: Option<String>,
    pub parent_commit_ids: Option<Vec<String>>,
}

impl LogEntry {
    pub fn to_toon_value(&self) -> ToonValue {
        let mut fields = vec![
            ("change_id", string(&self.change_id)),
            ("description", string(&self.description)),
            ("status", self.status.to_toon_value()),
        ];

        if let Some(commit_id) = &self.commit_id {
            fields.push(("commit_id", string(commit_id)));
        }
        if let Some(parent_commit_ids) = &self.parent_commit_ids {
            fields.push(("parent_commit_ids", string_array(parent_commit_ids)));
        }

        ToonValue::Object(fields)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentChange {
    pub change_id: String,
    pub description: String,
    pub status: Status,
}

impl CurrentChange {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("change_id", string(&self.change_id)),
            ("description", string(&self.description)),
            ("status", self.status.to_toon_value()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Change {
    pub change_id: String,
    pub description: String,
}

impl Change {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("change_id", string(&self.change_id)),
            ("description", string(&self.description)),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoryChange {
    pub change_id: String,
    pub description: String,
    pub status: Status,
}

impl HistoryChange {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("change_id", string(&self.change_id)),
            ("description", string(&self.description)),
            ("status", self.status.to_toon_value()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HunkRef {
    pub path: String,
    pub lines: String,
}

impl HunkRef {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("path", string(&self.path)),
            ("lines", string(&self.lines)),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewData {
    pub current_change: Change,
}

impl NewData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![(
            "current_change",
            self.current_change.to_toon_value(),
        )])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DescribeData {
    pub change: Change,
    pub changed: bool,
}

impl DescribeData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("change", self.change.to_toon_value()),
            ("changed", ToonValue::Bool(self.changed)),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckpointData {
    pub checkpoint: Change,
    pub current_change: Change,
}

impl CheckpointData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("checkpoint", self.checkpoint.to_toon_value()),
            ("current_change", self.current_change.to_toon_value()),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DescriptionAction {
    Updated,
    Unchanged,
}

impl DescriptionAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Updated => "updated",
            Self::Unchanged => "unchanged",
        }
    }

    fn to_toon_value(self) -> ToonValue {
        string(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalBookmarkAction {
    Created,
    Moved,
    Unchanged,
}

impl LocalBookmarkAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Moved => "moved",
            Self::Unchanged => "unchanged",
        }
    }

    fn to_toon_value(self) -> ToonValue {
        string(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteBookmarkAction {
    Created,
    Moved,
    Unchanged,
}

impl RemoteBookmarkAction {
    fn to_toon_value(self) -> ToonValue {
        string(match self {
            Self::Created => "created",
            Self::Moved => "moved",
            Self::Unchanged => "unchanged",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FinishPublication {
    Skipped,
    Complete {
        bookmark: String,
        remote: String,
        local_action: LocalBookmarkAction,
        remote_action: RemoteBookmarkAction,
    },
}

impl FinishPublication {
    fn to_toon_value(&self) -> ToonValue {
        match self {
            Self::Skipped => ToonValue::Object(vec![("status", string("skipped"))]),
            Self::Complete {
                bookmark,
                remote,
                local_action,
                remote_action,
            } => ToonValue::Object(vec![
                ("status", string("complete")),
                ("bookmark", string(bookmark)),
                ("remote", string(remote)),
                ("local_action", local_action.to_toon_value()),
                ("remote_action", remote_action.to_toon_value()),
            ]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinishData {
    pub change: Change,
    pub description_action: DescriptionAction,
    pub publication: FinishPublication,
}

impl FinishData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("change", self.change.to_toon_value()),
            (
                "description_action",
                self.description_action.to_toon_value(),
            ),
            ("publication", self.publication.to_toon_value()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Truncation {
    pub truncated: bool,
    pub limit_bytes: Option<u64>,
    pub returned_bytes: u64,
    pub omitted_bytes: u64,
}

impl Truncation {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("truncated", ToonValue::Bool(self.truncated)),
            (
                "limit_bytes",
                self.limit_bytes.map_or(ToonValue::Null, ToonValue::UInt),
            ),
            ("returned_bytes", ToonValue::UInt(self.returned_bytes)),
            ("omitted_bytes", ToonValue::UInt(self.omitted_bytes)),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Patch {
    pub body: String,
    pub truncation: Truncation,
}

impl Patch {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("format", string("unified-diff-v1")),
            ("body", string(&self.body)),
            ("truncation", self.truncation.to_toon_value()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectData {
    pub current_change: CurrentChange,
    pub diff_stat: DiffStat,
    pub conflict_count: u64,
    pub divergence_count: u64,
}

impl InspectData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("current_change", self.current_change.to_toon_value()),
            ("diff_stat", self.diff_stat.to_toon_value()),
            ("conflict_count", ToonValue::UInt(self.conflict_count)),
            ("divergence_count", ToonValue::UInt(self.divergence_count)),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogData {
    pub changes: Vec<LogEntry>,
    pub complete: bool,
}

impl LogData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            (
                "changes",
                ToonValue::Array(self.changes.iter().map(LogEntry::to_toon_value).collect()),
            ),
            ("complete", ToonValue::Bool(self.complete)),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShowData {
    pub change: Change,
    pub diff_stat: DiffStat,
    pub patch: Patch,
}

impl ShowData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("change", self.change.to_toon_value()),
            ("diff_stat", self.diff_stat.to_toon_value()),
            ("patch", self.patch.to_toon_value()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffData {
    pub target: DiffTarget,
    pub diff_stat: DiffStat,
    pub patch: Patch,
}

impl DiffData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("target", self.target.to_toon_value()),
            ("diff_stat", self.diff_stat.to_toon_value()),
            ("patch", self.patch.to_toon_value()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiffTarget {
    WorkingCopy,
    Change { change_id: String },
}

impl DiffTarget {
    pub fn to_toon_value(&self) -> ToonValue {
        match self {
            Self::WorkingCopy => ToonValue::Object(vec![("kind", string("working_copy"))]),
            Self::Change { change_id } => ToonValue::Object(vec![
                ("kind", string("change")),
                ("change_id", string(change_id)),
            ]),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SplitData {
    pub selected: HistoryChange,
    pub remaining: HistoryChange,
    pub hunks: Vec<HunkRef>,
}

impl SplitData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("selected", self.selected.to_toon_value()),
            ("remaining", self.remaining.to_toon_value()),
            ("hunks", hunk_array(&self.hunks)),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoveData {
    pub source: HistoryChange,
    pub destination: HistoryChange,
    pub hunks: Vec<HunkRef>,
}

impl MoveData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("source", self.source.to_toon_value()),
            ("destination", self.destination.to_toon_value()),
            ("hunks", hunk_array(&self.hunks)),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbsorbSourceAction {
    Unchanged,
    Rewritten,
    Abandoned,
}

impl AbsorbSourceAction {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::Rewritten => "rewritten",
            Self::Abandoned => "abandoned",
        }
    }

    fn to_toon_value(self) -> ToonValue {
        string(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbsorbMove {
    pub destination_change_id: String,
    pub hunks: Vec<HunkRef>,
}

impl AbsorbMove {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("destination_change_id", string(&self.destination_change_id)),
            ("hunks", hunk_array(&self.hunks)),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnmovedHunk {
    pub path: String,
    pub lines: String,
    pub reason: String,
}

impl UnmovedHunk {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("path", string(&self.path)),
            ("lines", string(&self.lines)),
            ("reason", string(&self.reason)),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkippedPath {
    pub path: String,
    pub reason: String,
}

impl SkippedPath {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("path", string(&self.path)),
            ("reason", string(&self.reason)),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbsorbData {
    pub dry_run: bool,
    pub changed: bool,
    pub source_change_id: String,
    pub source_action: AbsorbSourceAction,
    pub moves: Vec<AbsorbMove>,
    pub unmoved_hunks: Vec<UnmovedHunk>,
    pub skipped_paths: Vec<SkippedPath>,
}

impl AbsorbData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("dry_run", ToonValue::Bool(self.dry_run)),
            ("changed", ToonValue::Bool(self.changed)),
            ("source_change_id", string(&self.source_change_id)),
            ("source_action", self.source_action.to_toon_value()),
            (
                "moves",
                ToonValue::Array(self.moves.iter().map(AbsorbMove::to_toon_value).collect()),
            ),
            (
                "unmoved_hunks",
                ToonValue::Array(
                    self.unmoved_hunks
                        .iter()
                        .map(UnmovedHunk::to_toon_value)
                        .collect(),
                ),
            ),
            (
                "skipped_paths",
                ToonValue::Array(
                    self.skipped_paths
                        .iter()
                        .map(SkippedPath::to_toon_value)
                        .collect(),
                ),
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReorderData {
    pub changed: bool,
    pub sequence: Vec<HistoryChange>,
}

impl ReorderData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("changed", ToonValue::Bool(self.changed)),
            (
                "sequence",
                ToonValue::Array(
                    self.sequence
                        .iter()
                        .map(HistoryChange::to_toon_value)
                        .collect(),
                ),
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Response {
    pub kind: ResponseKind,
    pub data: ResponseData,
}

impl Response {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("schema_version", ToonValue::UInt(1)),
            ("kind", self.kind.to_toon_value()),
            ("data", self.data.to_toon_value()),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationKind {
    Mutation,
    Synchronization,
    Undo,
    Foundation,
    Unknown,
}

impl OperationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Mutation => "mutation",
            Self::Synchronization => "synchronization",
            Self::Undo => "undo",
            Self::Foundation => "foundation",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationEntry {
    pub operation_id: String,
    pub parent_operation_ids: Vec<String>,
    pub description: String,
    pub kind: OperationKind,
    pub undo_candidate: bool,
    pub current: bool,
}

impl OperationEntry {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("operation_id", string(&self.operation_id)),
            (
                "parent_operation_ids",
                string_array(&self.parent_operation_ids),
            ),
            ("description", string(&self.description)),
            ("kind", string(self.kind.as_str())),
            ("undo_candidate", ToonValue::Bool(self.undo_candidate)),
            ("current", ToonValue::Bool(self.current)),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationsData {
    pub operations: Vec<OperationEntry>,
}

impl OperationsData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![(
            "operations",
            ToonValue::Array(
                self.operations
                    .iter()
                    .map(OperationEntry::to_toon_value)
                    .collect(),
            ),
        )])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UndoAction {
    Restored,
    Unchanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UndoSelection {
    LatestMutation,
    Explicit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoTarget {
    pub operation_id: String,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoData {
    pub action: UndoAction,
    pub selection: UndoSelection,
    pub source_operation_ids: Vec<String>,
    pub target_operation: UndoTarget,
    pub result_operation_id: String,
    pub undone_count: u64,
    pub external_effects: Vec<String>,
}

impl UndoData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            (
                "action",
                string(match self.action {
                    UndoAction::Restored => "restored",
                    UndoAction::Unchanged => "unchanged",
                }),
            ),
            (
                "selection",
                string(match self.selection {
                    UndoSelection::LatestMutation => "latest_mutation",
                    UndoSelection::Explicit => "explicit",
                }),
            ),
            (
                "source_operation_ids",
                string_array(&self.source_operation_ids),
            ),
            (
                "target_operation",
                ToonValue::Object(vec![
                    ("operation_id", string(&self.target_operation.operation_id)),
                    ("description", string(&self.target_operation.description)),
                ]),
            ),
            ("result_operation_id", string(&self.result_operation_id)),
            ("undone_count", ToonValue::UInt(self.undone_count)),
            (
                "external_effects",
                ToonValue::Object(vec![
                    ("reverted", ToonValue::Bool(false)),
                    ("kinds", string_array(&self.external_effects)),
                ]),
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookmarkTargetState {
    pub present: bool,
    pub conflicted: bool,
    pub added_change_ids: Vec<String>,
    pub removed_change_ids: Vec<String>,
}

impl BookmarkTargetState {
    fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("present", ToonValue::Bool(self.present)),
            ("conflicted", ToonValue::Bool(self.conflicted)),
            ("added_change_ids", string_array(&self.added_change_ids)),
            ("removed_change_ids", string_array(&self.removed_change_ids)),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BookmarkComparisonStatus {
    Available,
    LocalMissing,
    RemoteMissing,
    LocalConflicted,
    RemoteConflicted,
}

impl BookmarkComparisonStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::LocalMissing => "local_missing",
            Self::RemoteMissing => "remote_missing",
            Self::LocalConflicted => "local_conflicted",
            Self::RemoteConflicted => "remote_conflicted",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookmarkRemoteState {
    pub remote: String,
    pub tracking: bool,
    pub target: BookmarkTargetState,
    pub comparison_status: BookmarkComparisonStatus,
    pub ahead: Option<u64>,
    pub behind: Option<u64>,
}

impl BookmarkRemoteState {
    fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("remote", string(&self.remote)),
            ("tracking", ToonValue::Bool(self.tracking)),
            ("target", self.target.to_toon_value()),
            ("comparison_status", string(self.comparison_status.as_str())),
            ("ahead", self.ahead.map_or(ToonValue::Null, ToonValue::UInt)),
            (
                "behind",
                self.behind.map_or(ToonValue::Null, ToonValue::UInt),
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookmarkEntry {
    pub name: String,
    pub local: BookmarkTargetState,
    pub remotes: Vec<BookmarkRemoteState>,
}

impl BookmarkEntry {
    fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("name", string(&self.name)),
            ("local", self.local.to_toon_value()),
            (
                "remotes",
                ToonValue::Array(
                    self.remotes
                        .iter()
                        .map(BookmarkRemoteState::to_toon_value)
                        .collect(),
                ),
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookmarkListData {
    pub bookmarks: Vec<BookmarkEntry>,
    pub truncated: bool,
    pub next_after: Option<String>,
}

impl BookmarkListData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("remote_data_source", string("local_tracking_state")),
            (
                "bookmarks",
                ToonValue::Array(
                    self.bookmarks
                        .iter()
                        .map(BookmarkEntry::to_toon_value)
                        .collect(),
                ),
            ),
            ("truncated", ToonValue::Bool(self.truncated)),
            (
                "next_after",
                self.next_after
                    .as_ref()
                    .map_or(ToonValue::Null, |name| string(name)),
            ),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookmarkSetData {
    pub name: String,
    pub target_change_id: String,
    pub target_commit_id: String,
    pub action: LocalBookmarkAction,
}

impl BookmarkSetData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("name", string(&self.name)),
            ("target_change_id", string(&self.target_change_id)),
            ("target_commit_id", string(&self.target_commit_id)),
            ("action", self.action.to_toon_value()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BookmarkPushData {
    pub name: String,
    pub target_change_id: String,
    pub target_commit_id: String,
    pub remote: String,
    pub action: RemoteBookmarkAction,
}

impl BookmarkPushData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("name", string(&self.name)),
            ("target_change_id", string(&self.target_change_id)),
            ("target_commit_id", string(&self.target_commit_id)),
            ("remote", string(&self.remote)),
            ("action", self.action.to_toon_value()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrChecks {
    pub total: u64,
    pub passed: u64,
    pub failed: u64,
    pub pending: u64,
    pub skipped: u64,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrStatusData {
    pub repository: String,
    pub number: u64,
    pub url: String,
    pub state: String,
    pub draft: bool,
    pub head_ref: String,
    pub head_commit_id: String,
    pub base_ref: String,
    pub mergeability: String,
    pub review: String,
    pub checks: PrChecks,
    pub ready_to_merge: bool,
    pub blocking_reasons: Vec<String>,
}

impl PrStatusData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("repository", string(&self.repository)),
            ("number", ToonValue::UInt(self.number)),
            ("url", string(&self.url)),
            ("state", string(&self.state)),
            ("draft", ToonValue::Bool(self.draft)),
            (
                "head",
                ToonValue::Object(vec![
                    ("ref", string(&self.head_ref)),
                    ("commit_id", string(&self.head_commit_id)),
                ]),
            ),
            (
                "base",
                ToonValue::Object(vec![("ref", string(&self.base_ref))]),
            ),
            ("mergeability", string(&self.mergeability)),
            ("review", string(&self.review)),
            (
                "checks",
                ToonValue::Object(vec![
                    ("total", ToonValue::UInt(self.checks.total)),
                    ("passed", ToonValue::UInt(self.checks.passed)),
                    ("failed", ToonValue::UInt(self.checks.failed)),
                    ("pending", ToonValue::UInt(self.checks.pending)),
                    ("skipped", ToonValue::UInt(self.checks.skipped)),
                    ("status", string(&self.checks.status)),
                ]),
            ),
            ("ready_to_merge", ToonValue::Bool(self.ready_to_merge)),
            ("blocking_reasons", string_array(&self.blocking_reasons)),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetupSkillAction {
    Created,
    Updated,
    Unchanged,
}

impl SetupSkillAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Unchanged => "unchanged",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetupSkillData {
    pub output_path: String,
    pub sha256: String,
    pub bytes: u64,
    pub action: SetupSkillAction,
}

impl SetupSkillData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            (
                "skill",
                ToonValue::Object(vec![
                    ("name", string("jj-axi")),
                    ("output_path", string(&self.output_path)),
                    ("sha256", string(&self.sha256)),
                    ("bytes", ToonValue::UInt(self.bytes)),
                ]),
            ),
            ("action", string(self.action.as_str())),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SquashData {
    pub source_change_id: String,
    pub destination: HistoryChange,
    pub rebased_descendant_count: u64,
    pub conflict_count: u64,
}

impl SquashData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            (
                "source",
                ToonValue::Object(vec![
                    ("change_id", string(&self.source_change_id)),
                    ("abandoned", ToonValue::Bool(true)),
                ]),
            ),
            ("destination", self.destination.to_toon_value()),
            (
                "rebased_descendant_count",
                ToonValue::UInt(self.rebased_descendant_count),
            ),
            ("conflict_count", ToonValue::UInt(self.conflict_count)),
        ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbandonAction {
    Abandoned,
    Unchanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbandonData {
    pub change_id: String,
    pub action: AbandonAction,
    pub affected_bookmarks: Vec<String>,
    pub rebased_descendant_count: u64,
    pub conflict_count: u64,
    pub current_change: Change,
}

impl AbandonData {
    pub fn to_toon_value(&self) -> ToonValue {
        ToonValue::Object(vec![
            ("change_id", string(&self.change_id)),
            (
                "action",
                string(match self.action {
                    AbandonAction::Abandoned => "abandoned",
                    AbandonAction::Unchanged => "unchanged",
                }),
            ),
            ("affected_bookmarks", string_array(&self.affected_bookmarks)),
            (
                "rebased_descendant_count",
                ToonValue::UInt(self.rebased_descendant_count),
            ),
            ("conflict_count", ToonValue::UInt(self.conflict_count)),
            ("current_change", self.current_change.to_toon_value()),
        ])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResponseKind {
    New,
    Describe,
    Checkpoint,
    Finish,
    Inspect,
    Log,
    Show,
    Diff,
    Split,
    Move,
    Absorb,
    Reorder,
    Operations,
    Undo,
    BookmarkList,
    BookmarkSet,
    BookmarkPush,
    PrStatus,
    SetupSkill,
    Squash,
    Abandon,
}

impl ResponseKind {
    pub fn to_toon_value(&self) -> ToonValue {
        string(match self {
            Self::New => "new",
            Self::Describe => "describe",
            Self::Checkpoint => "checkpoint",
            Self::Finish => "finish",
            Self::Inspect => "inspect",
            Self::Log => "log",
            Self::Show => "show",
            Self::Diff => "diff",
            Self::Split => "split",
            Self::Move => "move",
            Self::Absorb => "absorb",
            Self::Reorder => "reorder",
            Self::Operations => "operations",
            Self::Undo => "undo",
            Self::BookmarkList => "bookmark_list",
            Self::BookmarkSet => "bookmark_set",
            Self::BookmarkPush => "bookmark_push",
            Self::PrStatus => "pr_status",
            Self::SetupSkill => "setup_skill",
            Self::Squash => "squash",
            Self::Abandon => "abandon",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResponseData {
    New(NewData),
    Describe(DescribeData),
    Checkpoint(CheckpointData),
    Finish(FinishData),
    Inspect(InspectData),
    Log(LogData),
    Show(ShowData),
    Diff(DiffData),
    Split(SplitData),
    Move(MoveData),
    Absorb(AbsorbData),
    Reorder(ReorderData),
    Operations(OperationsData),
    Undo(UndoData),
    BookmarkList(BookmarkListData),
    BookmarkSet(BookmarkSetData),
    BookmarkPush(BookmarkPushData),
    PrStatus(PrStatusData),
    SetupSkill(SetupSkillData),
    Squash(SquashData),
    Abandon(AbandonData),
}

impl ResponseData {
    pub fn to_toon_value(&self) -> ToonValue {
        match self {
            Self::New(data) => data.to_toon_value(),
            Self::Describe(data) => data.to_toon_value(),
            Self::Checkpoint(data) => data.to_toon_value(),
            Self::Finish(data) => data.to_toon_value(),
            Self::Inspect(data) => data.to_toon_value(),
            Self::Log(data) => data.to_toon_value(),
            Self::Show(data) => data.to_toon_value(),
            Self::Diff(data) => data.to_toon_value(),
            Self::Split(data) => data.to_toon_value(),
            Self::Move(data) => data.to_toon_value(),
            Self::Absorb(data) => data.to_toon_value(),
            Self::Reorder(data) => data.to_toon_value(),
            Self::Operations(data) => data.to_toon_value(),
            Self::Undo(data) => data.to_toon_value(),
            Self::BookmarkList(data) => data.to_toon_value(),
            Self::BookmarkSet(data) => data.to_toon_value(),
            Self::BookmarkPush(data) => data.to_toon_value(),
            Self::PrStatus(data) => data.to_toon_value(),
            Self::SetupSkill(data) => data.to_toon_value(),
            Self::Squash(data) => data.to_toon_value(),
            Self::Abandon(data) => data.to_toon_value(),
        }
    }
}

fn string(value: &str) -> ToonValue {
    ToonValue::String(value.to_owned())
}

fn string_array(values: &[String]) -> ToonValue {
    ToonValue::Array(values.iter().map(|value| string(value)).collect())
}

fn hunk_array(values: &[HunkRef]) -> ToonValue {
    ToonValue::Array(values.iter().map(HunkRef::to_toon_value).collect())
}
