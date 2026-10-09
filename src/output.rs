use crate::model::{DirState, Inventory, ScanScope, SweepError};

pub fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

pub fn canonical_digest<T: serde::Serialize>(value: &T) -> Result<String, SweepError> {
    let error = |e: serde_json::Error| {
        SweepError::new("json", "canonical_digest", Path::new(""), e.to_string())
    };
    let mut value = serde_json::to_value(value).map_err(error)?;
    if let Some(object) = value.as_object_mut() {
        object.remove("digest_sha256");
    }
    // serde_json's default Map is ordered by key; arrays retain their observed order.
    Ok(sha256(&serde_json::to_vec(&value).map_err(error)?))
}

pub fn read_artifact<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, SweepError> {
    read_artifact_bounded(path, 32 * 1024 * 1024)
}

pub fn read_artifact_bounded<T: serde::de::DeserializeOwned>(
    path: &Path,
    limit: u64,
) -> Result<T, SweepError> {
    let limit = limit.min(32 * 1024 * 1024);
    crate::residency::ensure_no_materialization()?;
    use rustix::fs::{Mode, OFlags, open};
    use std::io::Read;
    let error =
        |message: String| SweepError::new("invalid_artifact", "read_artifact", path, message);
    let fd = open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|e| error(e.to_string()))?;
    let mut file = fs::File::from(fd);
    let metadata = file.metadata().map_err(|e| error(e.to_string()))?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(error(format!(
            "expected regular artifact within {limit} byte limit"
        )));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| error(e.to_string()))?;
    if bytes.len() as u64 > limit {
        return Err(error("artifact size limit exceeded".into()));
    }
    serde_json::from_slice(&bytes).map_err(|e| error(e.to_string()))
}
use std::{
    fs,
    io::{self, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub fn inventory_json(inventory: &Inventory) -> Result<Vec<u8>, SweepError> {
    serde_json::to_vec_pretty(inventory)
        .map_err(|e| SweepError::new("json", "serialize", Path::new(""), e.to_string()))
}

fn checked_destination(
    destination: &Path,
    inventory: &Inventory,
) -> Result<(PathBuf, std::ffi::OsString, fs::Metadata), SweepError> {
    crate::residency::ensure_no_materialization()?;
    let error =
        |message: String| SweepError::new("unsafe_output", "write_new", destination, message);
    let absolute = std::path::absolute(destination).map_err(|e| error(e.to_string()))?;
    let parent = absolute
        .parent()
        .ok_or_else(|| error("output needs a parent".into()))?;
    let name = absolute
        .file_name()
        .ok_or_else(|| error("output needs a file name".into()))?;
    // Refuse symlink traversal instead of reinterpreting where an output was requested.
    let mut prefix = PathBuf::new();
    for component in parent.components() {
        if component == Component::ParentDir {
            return Err(error("parent traversal is refused".into()));
        }
        prefix.push(component);
        let meta = fs::symlink_metadata(&prefix).map_err(|e| error(e.to_string()))?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(error("output ancestor is not a plain directory".into()));
        }
        if matches!(inventory.scope, ScanScope::AllMounted)
            || matches!(&inventory.scope, ScanScope::Roots { paths } if !paths.is_empty())
        {
            // A mounted or root walk can stop before discovering the output's repository.
            // Inspect ancestor markers independently of that partial inventory.
            let has = |name: &str| -> Result<bool, SweepError> {
                match fs::symlink_metadata(prefix.join(name)) {
                    Ok(_) => Ok(true),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
                    Err(e) => Err(error(format!("cannot inspect output ancestry: {e}"))),
                }
            };
            if prefix.file_name() == Some(std::ffi::OsStr::new(".git"))
                || has(".git")?
                || (has("HEAD")? && (has("objects")? || has("commondir")? || has("config")?))
            {
                return Err(error(
                    "output ancestor contains a repository or administration marker".into(),
                ));
            }
        }
    }
    let parent = fs::canonicalize(parent).map_err(|e| error(e.to_string()))?;
    let target = parent.join(name);
    let parent_identity = fs::metadata(&parent).map_err(|e| error(e.to_string()))?;
    let mut protected = Vec::new();
    match &inventory.scope {
        ScanScope::Repo { path } => protected.push(path.to_path()?),
        ScanScope::Roots { paths } => {
            for path in paths {
                protected.push(path.to_path()?);
            }
        }
        ScanScope::AllMounted => {}
    }
    for family in &inventory.families {
        protected.push(family.common_dir.to_path()?);
        for worktree in &family.worktrees {
            protected.push(worktree.path.to_path()?);
        }
    }
    for path in protected {
        // Existing targets are canonicalized; absent registered paths retain absolute identity.
        let normalized = match fs::canonicalize(&path) {
            Ok(p) => p,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::path::absolute(&path).map_err(|e| error(e.to_string()))?
            }
            Err(e) => return Err(error(format!("cannot establish target boundary: {e}"))),
        };
        if target.starts_with(normalized) {
            return Err(error(
                "output is inside the inspected scope or administration".into(),
            ));
        }
    }
    if fs::symlink_metadata(&target).is_ok() {
        return Err(error("output already exists".into()));
    }
    Ok((parent, name.to_os_string(), parent_identity))
}

pub fn write_new(
    destination: &Path,
    bytes: &[u8],
    inventory: &Inventory,
) -> Result<(), SweepError> {
    let (parent, name, expected) = checked_destination(destination, inventory)?;
    publish(&parent, &name, bytes, &expected)
}

pub(crate) struct RunDirectory {
    pub path: PathBuf,
    directory: fs::File,
    parent: fs::File,
}

impl RunDirectory {
    pub fn create(base: &Path, inventory: &Inventory) -> Result<Self, SweepError> {
        Self::create_with_hook(base, inventory, |_| {})
    }

    fn create_with_hook(
        base: &Path,
        inventory: &Inventory,
        mut after_mkdir: impl FnMut(&Path),
    ) -> Result<Self, SweepError> {
        crate::residency::ensure_no_materialization()?;
        use rustix::fs::{Mode, OFlags, mkdirat, openat};
        use std::os::unix::fs::MetadataExt;
        static NEXT_RUN: AtomicU64 = AtomicU64::new(0);
        let error = |message: String| SweepError::new("unsafe_output", "create_run", base, message);
        let mut protected_identities: Vec<_> = inventory
            .families
            .iter()
            .flat_map(|family| {
                std::iter::once(family.identity.clone()).chain(
                    family
                        .worktrees
                        .iter()
                        .filter_map(|worktree| worktree.identity.clone()),
                )
            })
            .collect();
        let roots = match &inventory.scope {
            ScanScope::Repo { path } => vec![path],
            ScanScope::Roots { paths } => paths.iter().collect(),
            ScanScope::AllMounted => Vec::new(),
        };
        for root in roots {
            match fs::metadata(root.to_path()?) {
                Ok(metadata) => protected_identities.push(crate::model::Identity {
                    device: metadata.dev(),
                    inode: metadata.ino(),
                }),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(error(format!("cannot identify protected scope: {e}"))),
            }
        }
        for _ in 0..128 {
            let name = format!(
                "run-{}-{}-{}",
                crate::model::now_ms(),
                std::process::id(),
                NEXT_RUN.fetch_add(1, Ordering::Relaxed)
            );
            let (parent, name, expected) = checked_destination(&base.join(name), inventory)?;
            let directory = open_directory(&parent, &expected)?;
            match mkdirat(&directory, &name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
                Ok(()) => {}
                Err(rustix::io::Errno::EXIST) => continue,
                Err(e) => return Err(error(e.to_string())),
            }
            after_mkdir(&parent.join(&name));
            let child = openat(
                &directory,
                &name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|e| error(e.to_string()))?;
            let stat = rustix::fs::fstat(&child).map_err(|e| error(e.to_string()))?;
            if protected_identities.iter().any(|identity| {
                identity.device == stat.st_dev as u64 && identity.inode == stat.st_ino as u64
            }) {
                return Err(error(
                    "acquired run directory identifies a protected target".into(),
                ));
            }
            // mkdir/open is not atomic. Reject a repository or any pre-existing
            // contents substituted before descriptor acquisition. Check through
            // this same descriptor before the first write, never by pathname.
            let mut contents =
                rustix::fs::Dir::read_from(&child).map_err(|e| error(e.to_string()))?;
            for entry in &mut contents {
                let entry = entry.map_err(|e| error(e.to_string()))?;
                if !matches!(entry.file_name().to_bytes(), b"." | b"..") {
                    return Err(error(
                        "acquired run directory is not empty; possible substitution".into(),
                    ));
                }
            }
            return Ok(Self {
                path: parent.join(name),
                directory: fs::File::from(child),
                parent: fs::File::from(directory),
            });
        }
        Err(error("run directory collision budget exhausted".into()))
    }

    pub(crate) fn open_artifact(&self, name: &str) -> Result<fs::File, SweepError> {
        use rustix::fs::{Mode, OFlags, openat};
        let error =
            |message: String| SweepError::new("run_output", "write_run", &self.path, message);
        if ![
            "inventory.json",
            "review.json",
            "forge.json",
            "report.txt",
            "report.html",
            "run.json",
        ]
        .contains(&name)
        {
            return Err(error("unsupported run artifact name".into()));
        }
        let fd = openat(
            &self.directory,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|e| error(e.to_string()))?;
        Ok(fs::File::from(fd))
    }
    #[cfg(test)]
    pub fn write(&self, name: &str, bytes: &[u8]) -> Result<(), SweepError> {
        let mut file = self.open_artifact(name)?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| SweepError::new("run_output", "write_run", &self.path, e.to_string()))
    }
    pub(crate) fn directory_file(&self) -> &fs::File {
        &self.directory
    }
    pub(crate) fn parent_file(&self) -> &fs::File {
        &self.parent
    }

    pub(crate) fn verify(&self) -> Result<(), SweepError> {
        use std::os::unix::fs::MetadataExt;
        let error =
            |message: String| SweepError::new("run_output", "verify_run", &self.path, message);
        let expected = rustix::fs::fstat(&self.directory).map_err(|e| error(e.to_string()))?;
        let current = fs::symlink_metadata(&self.path).map_err(|e| error(e.to_string()))?;
        if !current.is_dir()
            || current.dev() != expected.st_dev as u64
            || current.ino() != expected.st_ino as u64
        {
            return Err(error(
                "run directory identity changed; artifacts may remain at the original directory"
                    .into(),
            ));
        }
        Ok(())
    }
}

fn open_directory(
    parent: &Path,
    expected: &fs::Metadata,
) -> Result<rustix::fd::OwnedFd, SweepError> {
    use rustix::fs::{Mode, OFlags, open, openat};
    let error =
        |message: String| SweepError::new("unsafe_output", "open_directory", parent, message);
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut directory = open("/", flags, Mode::empty()).map_err(|e| error(e.to_string()))?;
    for component in parent.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(part) => {
                directory = openat(&directory, part, flags, Mode::empty())
                    .map_err(|e| error(e.to_string()))?;
            }
            _ => return Err(error("ambiguous output ancestor".into())),
        }
    }
    use std::os::unix::fs::MetadataExt;
    let observed = rustix::fs::fstat(&directory).map_err(|e| error(e.to_string()))?;
    if observed.st_dev as u64 != expected.dev() || observed.st_ino as u64 != expected.ino() {
        return Err(error("output directory identity changed".into()));
    }
    Ok(directory)
}

fn publish(
    parent: &Path,
    name: &std::ffi::OsStr,
    bytes: &[u8],
    expected: &fs::Metadata,
) -> Result<(), SweepError> {
    use rustix::fs::{AtFlags, Mode, OFlags, linkat, openat, unlinkat};
    let target = parent.join(name);
    let error = |message: String| SweepError::new("unsafe_output", "publish", &target, message);
    let directory = open_directory(parent, expected)?;
    // All following operations use this directory handle. A path replacement cannot
    // redirect creation, publication, or cleanup into a different directory.
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let staging = format!(
        ".wt-sweep-output-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let fd = openat(
        &directory,
        &staging,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|e| error(e.to_string()))?;
    let mut file = fs::File::from(fd);
    let result = (|| {
        file.write_all(bytes).map_err(|e| error(e.to_string()))?;
        file.sync_all().map_err(|e| error(e.to_string()))?;
        linkat(&directory, &staging, &directory, name, AtFlags::empty())
            .map_err(|e| error(e.to_string()))
    })();
    let cleanup =
        unlinkat(&directory, &staging, AtFlags::empty()).map_err(|e| error(e.to_string()));
    result.and(cleanup)
}

fn render_inventory(inventory: &Inventory, text: &mut impl std::fmt::Write) {
    crate::presentation::write_discovery_performance(&inventory.coverage, text);
    if let Some(disk) = &inventory.disk_usage {
        crate::disk::render(disk, &inventory.families, text);
    }
    let _ = writeln!(text);
    let _ = writeln!(text, "STATE\tHEAD\tBRANCH\tPATH");
    for root in &inventory.coverage.roots {
        let _ = writeln!(text, "Root: {}", root.display());
    }
    for volume in &inventory.coverage.volumes {
        let _ = writeln!(
            text,
            "Volume: {} available={} kind={}",
            volume.path.display(),
            volume.available,
            crate::model::BytePath::from_bytes(volume.kind.as_bytes()).display()
        );
    }
    for marker in inventory
        .markers
        .iter()
        .filter(|marker| marker.origin == "git_registry_outside_roots")
    {
        let _ = writeln!(text, "Outside-root registration: {}", marker.path.display());
    }
    for family in &inventory.families {
        let _ = writeln!(
            text,
            "Family: {} identity={}:{}",
            family.common_dir.display(),
            family.identity.device,
            family.identity.inode
        );
        for worktree in &family.worktrees {
            let state = match &worktree.path_state {
                DirState::Present => "present",
                DirState::Absent => "absent",
                DirState::Inaccessible { .. } => "inaccessible",
                DirState::ReadError { .. } => "read_error",
                DirState::VolumeUnavailable { .. } => "volume_unavailable",
                DirState::Unknown { .. } => "unknown",
            };
            let branch = worktree
                .branch
                .as_ref()
                .map(|b| b.display())
                .unwrap_or_else(|| {
                    if worktree.detached {
                        "(detached)".into()
                    } else {
                        "(none)".into()
                    }
                });
            let _ = writeln!(
                text,
                "{state}\t{}\t{branch}\t{}",
                worktree.head.as_deref().unwrap_or("UNKNOWN"),
                worktree.path.display()
            );
        }
    }
    let _ = writeln!(
        text,
        "Traversal complete: {}; gaps: {}; temporal consistency: {}",
        inventory.coverage.traversal_complete,
        inventory.coverage.gaps.len(),
        inventory.coverage.temporal_consistency
    );
    for gap in &inventory.coverage.gaps {
        let _ = writeln!(
            text,
            "Gap: {}: {}",
            gap.path.display(),
            crate::model::BytePath::from_bytes(gap.reason.as_bytes()).display()
        );
    }
    if let Some(remaining) = &inventory.coverage.remaining {
        let _ = writeln!(
            text,
            "Remaining: reason={} known_pending_paths={} cover=requested_scope (conservative, may include visited paths; mounted scope includes unknown mounts)",
            remaining.reason.as_str(),
            remaining.known_pending_paths
        );
    }
    if let Some(omitted) = &inventory.coverage.publication {
        let _ = writeln!(
            text,
            "Publication omitted: local_inventories={} markers={} families={} worktrees={} cover=requested_scope_and_registered_worktrees",
            omitted.local_inventories, omitted.markers, omitted.families, omitted.worktrees
        );
    }
}
pub fn inventory_table(inventory: &Inventory) -> String {
    let mut text = String::new();
    render_inventory(inventory, &mut text);
    text
}
fn inventory_table_bounded(inventory: &Inventory, limit: usize) -> Result<String, SweepError> {
    let mut text = BoundedText::new(limit);
    render_inventory(inventory, &mut text);
    text.finish()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    #[test]
    fn run_refuses_a_repository_substituted_before_descriptor_acquisition() {
        let root = PathBuf::from(format!(
            "/private/tmp/wt-sweep-run-swap-{}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("owned fixture");
        let target = root.join("protected");
        fs::create_dir(&target).expect("protected directory");
        fs::create_dir(target.join(".git")).expect("administration marker");
        fs::write(target.join("sentinel"), b"preserve").expect("sentinel");
        let inventory = Inventory::new(ScanScope::Repo {
            path: crate::model::BytePath::from_path(&target),
        });
        let mut substituted = PathBuf::new();
        let result = RunDirectory::create_with_hook(&root, &inventory, |created| {
            fs::rename(created, root.join("original-run")).expect("move newly created directory");
            fs::rename(&target, created).expect("substitute protected directory");
            substituted = created.to_path_buf();
        });
        let refused = result.is_err();
        if let Ok(directory) = result {
            directory
                .write("report.txt", b"must not write here")
                .expect("demonstrate unsafe write");
        }
        let wrote = substituted.join("report.txt").exists();
        let sentinel = fs::read(substituted.join("sentinel")).expect("sentinel remains");
        fs::remove_dir_all(&root).expect("owned fixture cleanup");
        assert!(refused, "substituted nonempty directory was accepted");
        assert!(!wrote, "report was written inside the moved target");
        assert_eq!(sentinel, b"preserve");
    }
    #[test]
    fn publisher_refuses_a_different_directory_identity() {
        let root = PathBuf::from(format!(
            "/private/tmp/wt-sweep-output-identity-{}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("fixture");
        let parent = root.join("output");
        fs::create_dir(&parent).expect("parent");
        let expected = fs::metadata(&parent).expect("parent identity");
        fs::rename(&parent, root.join("previous")).expect("rename");
        fs::create_dir(&parent).expect("different directory");
        let result = publish(
            &parent,
            std::ffi::OsStr::new("report.json"),
            b"report",
            &expected,
        );
        let changed = parent.join("report.json").exists();
        fs::remove_dir_all(&root).expect("owned fixture cleanup");
        assert!(result.is_err());
        assert!(!changed);
    }

    #[test]
    fn publisher_refuses_a_parent_replaced_by_symlink() {
        let root = PathBuf::from(format!(
            "/private/tmp/wt-sweep-output-race-{}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("fixture");
        let parent = root.join("output");
        let protected = root.join("target");
        fs::create_dir(&parent).expect("output dir");
        fs::create_dir(&protected).expect("target dir");
        let expected = fs::metadata(&parent).expect("parent identity");
        fs::rename(&parent, root.join("previous")).expect("move ancestor");
        std::os::unix::fs::symlink(&protected, &parent).expect("substitute ancestor");
        let result = publish(
            &parent,
            std::ffi::OsStr::new("report.json"),
            b"report",
            &expected,
        );
        let changed = protected.join("report.json").exists();
        fs::remove_dir_all(&root).expect("owned fixture cleanup");
        assert!(result.is_err(), "ambiguous destination was accepted");
        assert!(!changed, "published inside substituted target");
    }
}

#[derive(Clone, Debug)]
pub struct PublicationLimits {
    pub json_bytes: usize,
    pub report_bytes: usize,
    pub metadata_reserve_bytes: usize,
}
impl Default for PublicationLimits {
    fn default() -> Self {
        Self {
            json_bytes: 32 * 1024 * 1024,
            report_bytes: 32 * 1024 * 1024,
            metadata_reserve_bytes: 16 * 1024,
        }
    }
}
pub struct PreparedInventory {
    pub inventory: Inventory,
    pub json: Vec<u8>,
    pub report: String,
    pub partial: bool,
}
fn publication_limit_error(operation: &str) -> SweepError {
    SweepError::new(
        "publication_limit",
        operation,
        Path::new(""),
        "artifact byte limit exceeded; use a narrower scan or reduce optional detail",
    )
}

pub struct BoundedText {
    text: String,
    limit: usize,
    overflow: bool,
}
impl BoundedText {
    pub fn new(limit: usize) -> Self {
        Self {
            text: String::new(),
            limit,
            overflow: false,
        }
    }
    pub fn finish(self) -> Result<String, SweepError> {
        if self.overflow {
            Err(publication_limit_error("serialize_report"))
        } else {
            Ok(self.text)
        }
    }
}
impl std::fmt::Write for BoundedText {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        if self.overflow || text.len() > self.limit.saturating_sub(self.text.len()) {
            self.overflow = true;
            return Err(std::fmt::Error);
        }
        self.text.push_str(text);
        Ok(())
    }
}

struct BoundedJson {
    limit: usize,
    length: usize,
    bytes: Option<Vec<u8>>,
}
impl io::Write for BoundedJson {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.length) {
            return Err(io::Error::other("serialized byte budget exceeded"));
        }
        self.length += bytes.len();
        if let Some(buffer) = &mut self.bytes {
            buffer.extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn serialize_bounded<T: serde::Serialize>(
    value: &T,
    writer: &mut BoundedJson,
) -> Result<(), SweepError> {
    serde_json::to_writer_pretty(&mut *writer, value)
        .map_err(|_| publication_limit_error("serialize_json"))?;
    writer
        .write_all(b"\n")
        .map_err(|_| publication_limit_error("serialize_json"))
}
pub fn bounded_json_size<T: serde::Serialize>(
    value: &T,
    limit: usize,
) -> Result<usize, SweepError> {
    let mut writer = BoundedJson {
        limit,
        length: 0,
        bytes: None,
    };
    serialize_bounded(value, &mut writer)?;
    Ok(writer.length)
}
pub fn bounded_json<T: serde::Serialize>(value: &T, limit: usize) -> Result<Vec<u8>, SweepError> {
    let mut writer = BoundedJson {
        limit,
        length: 0,
        bytes: Some(Vec::new()),
    };
    serialize_bounded(value, &mut writer)?;
    Ok(writer.bytes.unwrap_or_default())
}
pub(crate) fn observation_bytes<T: serde::Serialize>(value: &T) -> Result<usize, SweepError> {
    let mut writer = BoundedJson {
        limit: usize::MAX,
        length: 0,
        bytes: None,
    };
    serde_json::to_writer(&mut writer, value)
        .map_err(|e| SweepError::new("json", "count_observation", Path::new(""), e.to_string()))?;
    Ok(writer.length)
}
fn publication_omissions(inventory: &Inventory) -> crate::model::PublicationOmissions {
    inventory
        .coverage
        .publication
        .clone()
        .unwrap_or(crate::model::PublicationOmissions {
            local_inventories: 0,
            markers: 0,
            families: 0,
            worktrees: 0,
            cover: crate::model::PublicationCover::RequestedScopeAndRegisteredWorktrees,
        })
}
fn inventory_fits(inventory: &Inventory, limits: &PublicationLimits) -> bool {
    bounded_json_size(inventory, limits.json_bytes).is_ok()
        && inventory_table_bounded(inventory, limits.report_bytes).is_ok()
}
fn prepared(
    inventory: Inventory,
    limits: &PublicationLimits,
) -> Result<PreparedInventory, SweepError> {
    let json = bounded_json(&inventory, limits.json_bytes)?;
    let report = inventory_table_bounded(&inventory, limits.report_bytes)?;
    let partial = inventory.disk_usage.as_ref().is_some_and(|d| !d.complete)
        || !inventory.coverage.traversal_complete
        || !inventory.coverage.gaps.is_empty()
        || inventory.coverage.remaining.is_some()
        || inventory.coverage.publication.is_some();
    Ok(PreparedInventory {
        inventory,
        json,
        report,
        partial,
    })
}
fn compact_prefix(source: &Inventory, retained_units: usize) -> Inventory {
    let mut compact = source.clone();
    compact.families.clear();
    let mut remaining = retained_units;
    let mut kept_worktrees = 0usize;
    for family in &source.families {
        if remaining == 0 {
            break;
        }
        // A family with no rows still has a positive common-directory identity.
        let units = family.worktrees.len().max(1);
        let keep = units.min(remaining);
        let mut retained = family.clone();
        retained.worktrees.truncate(keep);
        kept_worktrees += retained.worktrees.len();
        compact.families.push(retained);
        remaining -= keep;
    }
    let mut omitted = publication_omissions(source);
    omitted.families = omitted
        .families
        .saturating_add(source.families.len() - compact.families.len());
    omitted.worktrees = omitted.worktrees.saturating_add(
        source
            .families
            .iter()
            .map(|f| f.worktrees.len())
            .sum::<usize>()
            - kept_worktrees,
    );
    compact.coverage.publication = Some(omitted);
    compact
}

pub fn prepare_inventory(
    inventory: &Inventory,
    limits: &PublicationLimits,
) -> Result<PreparedInventory, SweepError> {
    // The reserve protects bookkeeping only. Full mandatory coverage is checked
    // against the total JSON/report limits, rather than against the reserve.
    let bookkeeping = crate::model::PublicationOmissions {
        local_inventories: usize::MAX,
        markers: usize::MAX,
        families: usize::MAX,
        worktrees: usize::MAX,
        cover: crate::model::PublicationCover::RequestedScopeAndRegisteredWorktrees,
    };
    if limits.json_bytes == 0
        || limits.report_bytes == 0
        || limits.metadata_reserve_bytes > limits.json_bytes
        || bounded_json_size(&bookkeeping, limits.metadata_reserve_bytes).is_err()
    {
        return Err(SweepError::new(
            "invalid_limits",
            "prepare_inventory",
            Path::new(""),
            "publication limits cannot accommodate omission bookkeeping; use a narrower scan and valid metadata reserve",
        ));
    }
    if inventory_fits(inventory, limits) {
        return prepared(inventory.clone(), limits);
    }
    let mut publication = inventory.clone();
    if !publication.local_inventories.is_empty() {
        let mut omitted = publication_omissions(&publication);
        omitted.local_inventories = omitted
            .local_inventories
            .saturating_add(publication.local_inventories.len());
        publication.local_inventories.clear();
        publication.coverage.publication = Some(omitted);
        if inventory_fits(&publication, limits) {
            return prepared(publication, limits);
        }
    }
    if !publication.markers.is_empty() {
        let mut omitted = publication_omissions(&publication);
        omitted.markers = omitted.markers.saturating_add(publication.markers.len());
        publication.markers.clear();
        publication.coverage.publication = Some(omitted);
        if inventory_fits(&publication, limits) {
            return prepared(publication, limits);
        }
    }
    let mandatory = compact_prefix(&publication, 0);
    if !inventory_fits(&mandatory, limits) {
        return Err(publication_limit_error("mandatory_inventory_metadata"));
    }
    let units: usize = publication
        .families
        .iter()
        .map(|f| f.worktrees.len().max(1))
        .sum();
    let mut low = 0usize;
    let mut high = units;
    while low < high {
        let mid = low + (high - low).div_ceil(2);
        if inventory_fits(&compact_prefix(&publication, mid), limits) {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    prepared(compact_prefix(&publication, low), limits)
}

pub fn revalidate_boundaries(inventory: &Inventory) -> Result<Inventory, SweepError> {
    use std::os::unix::fs::MetadataExt;
    let error = |path: &Path, message: String| {
        SweepError::new("unsafe_output", "revalidate_boundaries", path, message)
    };
    let scope_paths: Vec<_> = match &inventory.scope {
        ScanScope::Repo { path } => vec![path],
        ScanScope::Roots { paths } => paths.iter().collect(),
        ScanScope::AllMounted => Vec::new(),
    };
    for path in scope_paths {
        let path = path.to_path()?;
        if !path.is_absolute() {
            return Err(error(&path, "saved relative scope has no verified original working directory; rescan using absolute roots".into()));
        }
    }
    if inventory
        .coverage
        .publication
        .as_ref()
        .is_some_and(|omitted| omitted.families > 0 || omitted.worktrees > 0)
    {
        return Err(error(Path::new(""), "saved inventory omitted historical family or worktree boundaries; rescan before filesystem publication".into()));
    }
    let mut boundaries = inventory.clone();
    if inventory.families.is_empty()
        && let ScanScope::Repo { path } = &inventory.scope
    {
        let path = path.to_path()?;
        if inventory
            .coverage
            .remaining
            .as_ref()
            .is_some_and(|r| r.reason == crate::model::DiscoveryStop::Interrupted)
        {
            return Err(error(&path, "interrupted repository inventory has no verified family boundaries; rescan before filesystem publication".into()));
        }
        let current = crate::git::scan_family(&path).map_err(|e| error(&path, format!("cannot revalidate requested repository boundaries: {e}; rescan before publication")))?;
        if current.families.is_empty() {
            return Err(error(
                &path,
                "requested repository has no verified family boundaries; rescan before publication"
                    .into(),
            ));
        }
        boundaries.families.extend(current.families);
    }
    for family in &inventory.families {
        let common = family.common_dir.to_path()?;
        if !common.is_absolute() {
            return Err(error(&common, "saved family has a relative common-directory boundary; rescan using absolute roots".into()));
        }
        let metadata = fs::symlink_metadata(&common).map_err(|e| {
            error(
                &common,
                format!(
                    "cannot revalidate original common directory: {e}; rescan before publication"
                ),
            )
        })?;
        if !metadata.is_dir()
            || metadata.dev() != family.identity.device
            || metadata.ino() != family.identity.inode
        {
            return Err(error(
                &common,
                "output common-directory identity changed; rescan before publication".into(),
            ));
        }
        let entrypoint = family
            .worktrees
            .first()
            .map(|tree| &tree.path)
            .unwrap_or(&family.common_dir)
            .to_path()?;
        if !entrypoint.is_absolute() {
            return Err(error(
                &entrypoint,
                "saved family has a relative worktree entrypoint; rescan using absolute roots"
                    .into(),
            ));
        }
        let current = crate::git::scan_family(&entrypoint).map_err(|e| {
            error(
                &common,
                format!(
                    "cannot revalidate output family boundaries: {e}; rescan before publication"
                ),
            )
        })?;
        if current.families.len() != 1
            || current.families[0].identity != family.identity
            || current.families[0].common_dir != family.common_dir
        {
            return Err(error(&common, "output common-directory path or physical identity changed; rescan before publication".into()));
        }
        boundaries.families.extend(current.families);
    }
    Ok(boundaries)
}
