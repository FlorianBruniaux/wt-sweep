use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const INVENTORY_SCHEMA: &str = "wt-sweep.inventory/1";
pub const TARGET_MAP_SCHEMA: &str = "wt-sweep.targets/1";
pub const REVIEW_VIEW_SCHEMA: &str = "wt-sweep.review-view/1";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BytePath {
    pub hex: String,
}

impl BytePath {
    pub fn from_path(path: &Path) -> Self {
        use std::os::unix::ffi::OsStrExt;
        Self::from_bytes(path.as_os_str().as_bytes())
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        use std::fmt::Write;
        let mut hex = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            let _ = write!(hex, "{byte:02x}");
        }
        Self { hex }
    }

    pub fn bytes(&self) -> Result<Vec<u8>, SweepError> {
        let invalid = || SweepError {
            code: "invalid_path".into(),
            operation: "decode_path".into(),
            path: None,
            message: "path must be canonical lowercase hexadecimal without NUL".into(),
        };
        if !self.hex.len().is_multiple_of(2)
            || !self
                .hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid());
        }
        let mut bytes = Vec::with_capacity(self.hex.len() / 2);
        for pair in self.hex.as_bytes().chunks_exact(2) {
            let digit = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
            let byte = digit(pair[0]) * 16 + digit(pair[1]);
            if byte == 0 {
                return Err(invalid());
            }
            bytes.push(byte);
        }
        Ok(bytes)
    }

    pub fn to_path(&self) -> Result<PathBuf, SweepError> {
        use std::os::unix::ffi::OsStringExt;
        self.bytes()
            .map(|bytes| std::ffi::OsString::from_vec(bytes).into())
    }

    pub fn display(&self) -> String {
        use std::fmt::Write;
        let Ok(bytes) = self.bytes() else {
            return "<invalid path>".into();
        };
        let mut display = String::new();
        for byte in bytes {
            match byte {
                b'\n' => display.push_str("\\n"),
                b'\r' => display.push_str("\\r"),
                b'\t' => display.push_str("\\t"),
                b'\\' => display.push_str("\\\\"),
                32..=126 => display.push(char::from(byte)),
                _ => {
                    let _ = write!(display, "\\x{byte:02x}");
                }
            }
        }
        display
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub device: u64,
    pub inode: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum DirState {
    Present,
    Absent,
    Inaccessible { reason: String },
    ReadError { reason: String },
    VolumeUnavailable { reason: String },
    Unknown { reason: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SweepError {
    pub code: String,
    pub operation: String,
    pub path: Option<BytePath>,
    pub message: String,
}

impl SweepError {
    pub fn new(
        code: impl Into<String>,
        operation: impl Into<String>,
        path: &Path,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            operation: operation.into(),
            path: Some(BytePath::from_path(path)),
            message: message.into(),
        }
    }
}
impl std::fmt::Display for SweepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}): {}", self.code, self.operation, self.message)
    }
}
impl std::error::Error for SweepError {}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Worktree {
    pub path: BytePath,
    pub identity: Option<Identity>,
    pub head: Option<String>,
    pub branch: Option<BytePath>,
    pub detached: bool,
    pub bare: bool,
    pub locked: Option<String>,
    pub prunable: Option<String>,
    pub path_state: DirState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Family {
    pub common_dir: BytePath,
    pub identity: Identity,
    pub worktrees: Vec<Worktree>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gap {
    pub path: BytePath,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    /// Separate discovery passes; these intervals do not form an atomic snapshot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub discovery_intervals: Vec<ObservationInterval>,
    /// Diagnostic telemetry from discovery, never a completeness or decision proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery_metrics: Option<DiscoveryMetrics>,
    pub traversal_complete: bool,
    pub temporal_consistency: String,
    pub gaps: Vec<Gap>,
    #[serde(default)]
    pub roots: Vec<BytePath>,
    #[serde(default)]
    pub volumes: Vec<VolumeObservation>,
    #[serde(default)]
    pub aliases: Vec<PathAlias>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remaining: Option<DiscoveryRemainder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication: Option<PublicationOmissions>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationInterval {
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
}

/// Monotonic wall time; listing counts include incomplete discovery, not verified files.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryMetrics {
    pub elapsed_ms: u64,
    pub directory_read_attempts: usize,
    pub entries_listed: usize,
    pub workers: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryStop {
    Interrupted,
    Deadline,
    EntryBudget,
    DirectoryBudget,
    ObservationBudget,
}

impl DiscoveryStop {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Interrupted => "interrupted",
            Self::Deadline => "deadline",
            Self::EntryBudget => "entry_budget",
            Self::DirectoryBudget => "directory_budget",
            Self::ObservationBudget => "observation_budget",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemainingCover {
    RequestedScope,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryRemainder {
    pub reason: DiscoveryStop,
    pub known_pending_paths: usize,
    pub cover: RemainingCover,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationCover {
    RequestedScopeAndRegisteredWorktrees,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicationOmissions {
    pub local_inventories: usize,
    pub markers: usize,
    pub families: usize,
    pub worktrees: usize,
    pub cover: PublicationCover,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeObservation {
    pub path: BytePath,
    pub available: bool,
    pub kind: String,
    #[serde(default)]
    pub native: Option<NativeMountDetails>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeMountDetails {
    pub source: BytePath,
    pub filesystem: BytePath,
    pub flags: u32,
    pub filesystem_type: u32,
    pub filesystem_subtype: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathAlias {
    pub path: BytePath,
    pub canonical: BytePath,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Marker {
    pub path: BytePath,
    pub kind: String,
    pub common_dir: Option<BytePath>,
    pub origin: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScanScope {
    Repo { path: BytePath },
    Roots { paths: Vec<BytePath> },
    AllMounted,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub schema: String,
    pub scope: ScanScope,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub families: Vec<Family>,
    pub coverage: Coverage,
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub not_collected: Vec<String>,
    #[serde(default)]
    pub local_inventories: Vec<LocalInventory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_usage: Option<crate::disk::DiskUsage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalEntry {
    pub path: BytePath,
    pub kind: String,
    pub category: String,
    pub identity: Identity,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub size: u64,
    pub links: u64,
    pub modified_sec: i64,
    pub modified_nsec: i64,
    pub changed_sec: i64,
    pub changed_nsec: i64,
    pub flags: u32,
    pub content_sha256: Option<String>,
    pub link_target: Option<BytePath>,
}

/// A positive count survives partial collection; zero is exact only when complete.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalCount {
    pub observed: usize,
    pub complete: bool,
    pub samples: Vec<BytePath>,
    pub samples_omitted: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalSource {
    pub kind: String,
    pub path: BytePath,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalInventoryRef {
    pub worktree: BytePath,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub content_digest_sha256: String,
    pub traversal_complete: bool,
    pub temporal_consistency: String,
    pub entry_count: usize,
    pub index_entry_count: usize,
    pub administration_entry_count: usize,
    pub gap_count: usize,
    pub not_collected_count: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalSummary {
    pub worktree_identity: Option<Identity>,
    pub git_dir: Option<BytePath>,
    pub head: Option<String>,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub staged: LocalCount,
    pub unstaged: LocalCount,
    pub conflicts: LocalCount,
    pub untracked: LocalCount,
    pub ignored: LocalCount,
    pub head_complete: bool,
    pub index_complete: bool,
    pub disk_complete: bool,
    pub temporal_consistency: String,
    pub sources: Vec<LocalSource>,
    pub gaps: Vec<Gap>,
    pub details_truncated: bool,
    pub digest_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexEntry {
    pub path: BytePath,
    pub mode: u32,
    pub oid: String,
    pub stage: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalInventory {
    pub worktree: BytePath,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub entries: Vec<LocalEntry>,
    pub index_entries: Vec<IndexEntry>,
    pub administration_entries: Vec<LocalEntry>,
    pub traversal_complete: bool,
    pub temporal_consistency: String,
    pub gaps: Vec<Gap>,
    pub not_collected: Vec<String>,
    pub content_digest_sha256: String,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

impl Inventory {
    pub fn new(scope: ScanScope) -> Self {
        let started_unix_ms = now_ms();
        Self {
            schema: INVENTORY_SCHEMA.into(),
            scope,
            started_unix_ms,
            finished_unix_ms: started_unix_ms,
            families: Vec::new(),
            coverage: Coverage {
                discovery_intervals: Vec::new(),
                discovery_metrics: None,
                traversal_complete: false,
                temporal_consistency: "not_guaranteed".into(),
                gaps: Vec::new(),
                roots: Vec::new(),
                volumes: Vec::new(),
                aliases: Vec::new(),
                remaining: None,
                publication: None,
            },
            markers: Vec::new(),
            not_collected: vec![
                "local file contents, ignored files, index and reflog inventory".into(),
                "restoration, writer exclusion and current remote state".into(),
            ],
            local_inventories: Vec::new(),
            disk_usage: None,
        }
    }
    pub fn finish(&mut self) {
        self.finished_unix_ms = now_ms();
    }
}

pub const FORGE_SCHEMA: &str = "wt-sweep.forge/1";
pub const REVIEW_SCHEMA: &str = "wt-sweep.review/2";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeRequest {
    pub provider: String,
    pub host: Option<String>,
    pub project: Option<String>,
    pub iid: u64,
    pub source_project: Option<String>,
    pub target_project: Option<String>,
    pub source_branch: Option<String>,
    pub target_branch: Option<String>,
    pub state: String,
    #[serde(default)]
    pub title: Option<String>,
    pub head_oid: Option<String>,
    pub merge_oid: Option<String>,
    pub updated_at: Option<String>,
    pub url: Option<String>,
    pub source_index: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeSource {
    pub raw_path: BytePath,
    pub sha256: String,
    pub receipt_path: Option<BytePath>,
    pub receipt_sha256: Option<String>,
    pub receipt_verified: bool,
    pub started_unix_ms: Option<u64>,
    pub finished_unix_ms: Option<u64>,
    pub execution: Option<serde_json::Value>,
    pub gaps: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeCoverage {
    pub pagination_complete: bool,
    pub temporal_consistency: String,
    pub absence_proven: bool,
    pub gaps: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeExport {
    pub schema: String,
    pub requests: Vec<ForgeRequest>,
    pub sources: Vec<ForgeSource>,
    pub coverage: ForgeCoverage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegrationObservation {
    pub source_oid: String,
    pub target_ref: BytePath,
    pub target_oid: String,
    pub relation: String,
    pub target_role: String,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
}

/// Commit-graph observations, distinct from verified tree/blob integration evidence.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryObservation {
    pub source_oid: Option<String>,
    pub target_ref: BytePath,
    pub target_oid: Option<String>,
    pub result: String,
    #[serde(default)]
    pub observed_result: Option<String>,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub stop_reason: Option<String>,
}

/// Counters cover the history phase only; returned bytes exclude failed I/O.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryMetrics {
    pub items_total: u64,
    pub first_pass_items: u64,
    pub completed_targets: u64,
    pub incomplete_targets: u64,
    pub object_read_attempts: u64,
    pub object_bytes: u64,
    pub traversal_steps: u64,
    pub cache_hits: u64,
    pub target_resolution_attempts: u64,
    pub spawned_processes: u64,
    pub elapsed_ms: u64,
    pub stop_reason: Option<String>,
}

/// Collection telemetry is diagnostic only; it never satisfies a decision condition.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionMetrics {
    pub budget_profile: String,
    pub applied_limits: std::collections::BTreeMap<String, u64>,
    pub phases: Vec<CollectionPhase>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CollectionPhase {
    pub name: String,
    pub elapsed_ms: u64,
    pub counters: std::collections::BTreeMap<String, u64>,
}

/// File hashes or directory membership digests from bounded administration reads.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistorySource {
    pub path: BytePath,
    pub kind: String,
    pub exists: Option<bool>,
    pub sha256: Option<String>,
    pub bytes: Option<u64>,
    pub complete: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootProvenance {
    pub source: BytePath,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetainedAnchor {
    pub reference: BytePath,
    pub oid: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateRoot {
    pub oid: String,
    pub provenance: Vec<RootProvenance>,
    pub result: String,
    pub anchor: Option<RetainedAnchor>,
    pub observed_result: Option<String>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrivateHistory {
    pub git_dir: Option<BytePath>,
    pub git_dir_identity: Option<Identity>,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub enumeration_complete: bool,
    pub anchors_complete: bool,
    pub temporal_consistency: String,
    pub sources: Vec<HistorySource>,
    pub roots: Vec<PrivateRoot>,
    pub gaps: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StashEntry {
    pub oid: String,
    pub base_oid: Option<String>,
    pub timestamp_unix: Option<i64>,
    pub message: Option<BytePath>,
    pub message_truncated: bool,
    pub files: Vec<BytePath>,
    pub metadata_complete: bool,
    pub files_complete: bool,
    pub gaps: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyStashes {
    pub common_dir: BytePath,
    pub identity: Identity,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub enumeration_complete: bool,
    pub temporal_consistency: String,
    pub sources: Vec<HistorySource>,
    pub entries: Vec<StashEntry>,
    pub gaps: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorktreeState {
    pub observed: Option<Worktree>,
    pub common_dir: BytePath,
    pub family_identity: Option<Identity>,
    pub git_dir: Option<BytePath>,
    pub git_dir_identity: Option<Identity>,
    pub role: String,
    #[serde(default)]
    pub protections: Vec<BytePath>,
    pub protection_complete: bool,
    pub operations: Vec<BytePath>,
    pub operations_complete: bool,
    pub checks: String,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub sources: Vec<HistorySource>,
    pub gaps: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriageDecision {
    pub keep_reasons: Vec<String>,
    pub missing_conditions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewItem {
    #[serde(default)]
    pub worktree_state: Option<WorktreeState>,
    #[serde(default)]
    pub decision: Option<TriageDecision>,
    pub path: BytePath,
    #[serde(default)]
    pub private_history: Option<PrivateHistory>,
    #[serde(default)]
    pub history: Vec<HistoryObservation>,
    #[serde(default)]
    pub history_omitted_targets: usize,
    #[serde(default)]
    pub local_summary: Option<LocalSummary>,
    #[serde(default)]
    pub branch: Option<BytePath>,
    #[serde(default)]
    pub head: Option<String>,
    #[serde(default)]
    pub subject: Option<BytePath>,
    #[serde(default)]
    pub subject_truncated: bool,
    pub classification: String,
    pub observations: Vec<IntegrationObservation>,
    pub requests: Vec<ForgeRequest>,
    pub risks: Vec<String>,
    pub unknowns: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Observations {
    pub context: Option<ReviewContext>,
    pub inventory: Inventory,
    pub shared_stashes: Vec<FamilyStashes>,
    pub history_metrics: HistoryMetrics,
    pub collection_metrics: Option<CollectionMetrics>,
    pub items: Vec<ReviewItem>,
    pub forge: Option<ForgeExport>,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    /// Captured size observations from the source inventory, never decision evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_usage: Option<crate::disk::DiskUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<ReviewContext>,
    pub schema: String,
    #[serde(default)]
    pub shared_stashes: Vec<FamilyStashes>,
    #[serde(default)]
    pub history_metrics: HistoryMetrics,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection_metrics: Option<CollectionMetrics>,
    pub policy_version: String,
    pub inventory_sha256: String,
    pub digest_sha256: String,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub coverage: Coverage,
    pub items: Vec<ReviewItem>,
    pub forge: Option<ForgeExport>,
    pub not_collected: Vec<String>,
    #[serde(default)]
    pub local_inventory_refs: Vec<LocalInventoryRef>,
}

/// Explicit integration targets, bound to one saved physical repository family.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetMap {
    pub schema: String,
    pub families: Vec<TargetMapFamily>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetMapFamily {
    pub common_dir: BytePath,
    pub identity: Identity,
    pub targets: Vec<BytePath>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewFilters {
    pub repository: Option<BytePath>,
    pub path_prefix: Option<BytePath>,
    pub branch: Option<BytePath>,
    pub local_state: Option<String>,
    pub verdict: Option<String>,
}
/// Historical registration for grouping; this does not establish current C1.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRowBinding {
    pub path: BytePath,
    pub common_dir: BytePath,
    pub identity: Identity,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewView {
    pub schema: String,
    pub source: Review,
    pub row_bindings: Vec<ReviewRowBinding>,
    pub selected_paths: Vec<BytePath>,
    pub group_by: String,
    pub filters: ReviewFilters,
    pub digest_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewContext {
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub items: Vec<WorktreeContext>,
    pub omitted_items: usize,
    pub gaps: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorktreeContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<RemoteContext>,
    pub path: BytePath,
    pub common_dir: BytePath,
    pub family_identity: Identity,
    pub head_oid: Option<String>,
    pub uncovered: UncoveredCommits,
    pub changelogs: Vec<ChangelogContext>,
    pub gaps: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextTarget {
    pub reference: BytePath,
    pub oid: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UncoveredCommits {
    pub targets: Vec<ContextTarget>,
    pub observed: usize,
    pub complete: bool,
    pub samples: Vec<CommitContext>,
    pub samples_omitted: usize,
    pub gaps: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitContext {
    pub oid: String,
    pub subject: Option<BytePath>,
    pub subject_truncated: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangelogContext {
    pub revision_oid: String,
    pub path: BytePath,
    pub tree_oid: Option<String>,
    pub blob_oid: Option<String>,
    pub status: String,
    pub blob_bytes: Option<usize>,
    pub excerpt: Option<BytePath>,
    pub excerpt_truncated: bool,
    pub gaps: Vec<String>,
}

/// Local remote metadata is descriptive only; the server was not contacted.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteContext {
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub remotes: Vec<RemoteLink>,
    pub gaps: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteLink {
    pub name: BytePath,
    pub provider: String,
    pub host: String,
    pub project: String,
    pub branch: Option<BytePath>,
    pub tracking_ref: Option<BytePath>,
    pub tracking_oid: Option<String>,
    pub head_matches_tracking: Option<bool>,
}
