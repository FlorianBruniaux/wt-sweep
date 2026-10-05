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
    const LIMIT: u64 = 32 * 1024 * 1024;
    let metadata = file.metadata().map_err(|e| error(e.to_string()))?;
    if !metadata.is_file() || metadata.len() > LIMIT {
        return Err(error(
            "expected regular artifact within 32 MiB limit".into(),
        ));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| error(e.to_string()))?;
    if bytes.len() as u64 > LIMIT {
        return Err(error("artifact size limit exceeded".into()));
    }
    serde_json::from_slice(&bytes).map_err(|e| error(e.to_string()))
}
use std::{
    fs,
    io::Write,
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
        if matches!(inventory.scope, ScanScope::AllMounted) {
            // A global walk can stop before discovering the output's repository.
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
                    "global output ancestor contains a repository or administration marker".into(),
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
    directory: rustix::fd::OwnedFd,
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
                directory: child,
            });
        }
        Err(error("run directory collision budget exhausted".into()))
    }

    pub fn write(&self, name: &str, bytes: &[u8]) -> Result<(), SweepError> {
        use rustix::fs::{Mode, OFlags, openat};
        let error =
            |message: String| SweepError::new("run_output", "write_run", &self.path, message);
        if ![
            "inventory.json",
            "review.json",
            "forge.json",
            "report.txt",
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
        let mut file = fs::File::from(fd);
        file.write_all(bytes).map_err(|e| error(e.to_string()))?;
        file.sync_all().map_err(|e| error(e.to_string()))
    }

    pub fn verify_and_sync(&self) -> Result<(), SweepError> {
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
        rustix::fs::fsync(&self.directory).map_err(|e| error(e.to_string()))
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

pub fn inventory_table(inventory: &Inventory) -> String {
    use std::fmt::Write;
    let mut text = String::from("STATE\tHEAD\tBRANCH\tPATH\n");
    for family in &inventory.families {
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
    text
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
