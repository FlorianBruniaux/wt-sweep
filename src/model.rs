use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const INVENTORY_SCHEMA: &str = "wt-sweep.inventory/1";

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
        if self.hex.len() % 2 != 0
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
    pub traversal_complete: bool,
    pub temporal_consistency: String,
    pub gaps: Vec<Gap>,
    #[serde(default)]
    pub roots: Vec<BytePath>,
    #[serde(default)]
    pub volumes: Vec<VolumeObservation>,
    #[serde(default)]
    pub aliases: Vec<PathAlias>,
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
                traversal_complete: false,
                temporal_consistency: "not_guaranteed".into(),
                gaps: Vec::new(),
                roots: Vec::new(),
                volumes: Vec::new(),
                aliases: Vec::new(),
            },
            markers: Vec::new(),
            not_collected: vec![
                "local file contents, ignored files, index and reflog inventory".into(),
                "restoration, writer exclusion and current remote state".into(),
            ],
            local_inventories: Vec::new(),
        }
    }
    pub fn finish(&mut self) {
        self.finished_unix_ms = now_ms();
    }
}

pub const FORGE_SCHEMA: &str = "wt-sweep.forge/1";
pub const REVIEW_SCHEMA: &str = "wt-sweep.review/1";

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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewItem {
    pub path: BytePath,
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
    pub inventory: Inventory,
    pub items: Vec<ReviewItem>,
    pub forge: Option<ForgeExport>,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub schema: String,
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
    pub local_inventories: Vec<LocalInventory>,
}
