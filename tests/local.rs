#![cfg(unix)]
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use wt_sweep::git::{ControlledGit, GitRead, scan_family};
use wt_sweep::local::{LocalHooks, LocalLimits, collect, collect_with, content_digest};
use wt_sweep::model::{BytePath, Worktree};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/wt-sweep-local-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap_or_else(|e| panic!("fixture: {e}"));
        let fixture = Self(path);
        fixture.git(&["init".as_ref(), "-b".as_ref(), "main".as_ref()]);
        fixture
    }
    fn git(&self, args: &[&std::ffi::OsStr]) -> Vec<u8> {
        let output = Command::new("/usr/bin/git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("git: {e}"));
        assert!(output.status.success(), "{:?}", output.stderr);
        output.stdout
    }
    fn worktree(&self) -> Worktree {
        scan_family(&self.0)
            .unwrap_or_else(|e| panic!("scan: {e}"))
            .families[0]
            .worktrees[0]
            .clone()
    }
    fn committed_file(&self) {
        fs::write(self.0.join("tracked"), b"baseline").unwrap_or_else(|e| panic!("write: {e}"));
        self.git(&["add".as_ref(), "tracked".as_ref()]);
        self.git(&["commit".as_ref(), "-m".as_ref(), "initial".as_ref()]);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn bp(path: &str) -> BytePath {
    BytePath::from_path(Path::new(path))
}

#[test]
fn exact_local_files_include_tracked_untracked_ignored_symlinks_and_index() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("tracked"), b"tracked content")
        .unwrap_or_else(|e| panic!("write: {e}"));
    fs::write(fixture.0.join(".gitignore"), b"ignored\n").unwrap_or_else(|e| panic!("write: {e}"));
    fixture.git(&["add".as_ref(), "tracked".as_ref(), ".gitignore".as_ref()]);
    fixture.git(&["commit".as_ref(), "-m".as_ref(), "initial".as_ref()]);
    fs::write(fixture.0.join("untracked \nname"), b"untracked")
        .unwrap_or_else(|e| panic!("write: {e}"));
    fs::write(fixture.0.join("ignored"), b"ignored content")
        .unwrap_or_else(|e| panic!("write: {e}"));
    std::os::unix::fs::symlink("/outside-never-follow", fixture.0.join("link"))
        .unwrap_or_else(|e| panic!("symlink: {e}"));
    let inventory = collect(&fixture.worktree(), &LocalLimits::default())
        .unwrap_or_else(|e| panic!("collect: {e}"));
    let files = inventory
        .entries
        .iter()
        .filter(|e| e.kind != "directory" && e.path != bp(".git"))
        .map(|e| e.path.clone())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        files,
        [
            bp("tracked"),
            bp(".gitignore"),
            bp("untracked \nname"),
            bp("ignored"),
            bp("link")
        ]
        .into_iter()
        .collect()
    );
    for (name, category) in [
        ("tracked", "tracked"),
        ("untracked \nname", "untracked"),
        ("ignored", "ignored"),
    ] {
        let entry = inventory
            .entries
            .iter()
            .find(|e| e.path == bp(name))
            .unwrap_or_else(|| panic!("missing {name}"));
        assert_eq!(entry.category, category, "{:?}", inventory.gaps);
        assert_eq!(entry.content_sha256.as_ref().map(String::len), Some(64));
    }
    let link = inventory
        .entries
        .iter()
        .find(|e| e.path == bp("link"))
        .unwrap_or_else(|| panic!("link"));
    assert_eq!(link.kind, "symlink");
    assert_eq!(link.link_target, Some(bp("/outside-never-follow")));
    assert!(
        inventory
            .index_entries
            .iter()
            .any(|e| e.path == bp("tracked") && e.stage == 0 && e.oid.len() == 40)
    );
    assert!(inventory.administration_entries.iter().any(|e| e.path
        == BytePath::from_path(&fixture.0.join(".git/index"))
        && e.content_sha256.is_some()));
    assert_eq!(inventory.temporal_consistency, "not_guaranteed");
    assert_eq!(inventory.content_digest_sha256.len(), 64);
}

#[test]
fn controlled_git_cwd_remains_anchored_after_rename_and_path_substitution() {
    let mut fixture = Fixture::new();
    fixture.committed_file();
    let original = fixture.0.clone();
    let fd = rustix::fs::open(
        &original,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .unwrap_or_else(|e| panic!("open: {e}"));
    let renamed = original.with_extension("moved");
    fs::rename(&original, &renamed).unwrap_or_else(|e| panic!("rename: {e}"));
    fixture.0 = renamed;
    fs::create_dir(&original).unwrap_or_else(|e| panic!("replacement: {e}"));
    let replacement = Fixture(original.clone());
    replacement.git(&["init".as_ref(), "-b".as_ref(), "main".as_ref()]);
    fs::write(original.join("replacement"), b"different repository")
        .unwrap_or_else(|e| panic!("write: {e}"));
    replacement.git(&["add".as_ref(), "replacement".as_ref()]);
    let raw = ControlledGit::default()
        .read_from_directory(&fd, &original, GitRead::LocalRefs)
        .unwrap_or_else(|e| panic!("anchored Git: {e}"));
    assert!(
        raw.windows(b"refs/heads/main".len())
            .any(|b| b == b"refs/heads/main")
    );
    assert!(
        !raw.windows(b"replacement".len())
            .any(|b| b == b"replacement")
    );
    let ordinary =
        std::fs::File::open(fixture.0.join("tracked")).unwrap_or_else(|e| panic!("file: {e}"));
    let fd = std::os::fd::OwnedFd::from(ordinary);
    assert!(
        ControlledGit::default()
            .read_from_directory(&fd, &original, GitRead::LocalRefs)
            .is_err()
    );
}

#[test]
fn digest_binds_file_bytes_at_same_size_and_git_status_but_excludes_collection_times() {
    let fixture = Fixture::new();
    fixture.committed_file();
    let tree = fixture.worktree();
    fs::write(fixture.0.join("tracked"), b"changedA").unwrap_or_else(|e| panic!("write: {e}"));
    let first = collect(&tree, &LocalLimits::default()).unwrap_or_else(|e| panic!("collect: {e}"));
    let status = fixture.git(&["status".as_ref(), "--porcelain".as_ref()]);
    fs::write(fixture.0.join("tracked"), b"changedB").unwrap_or_else(|e| panic!("write: {e}"));
    let second = collect(&tree, &LocalLimits::default()).unwrap_or_else(|e| panic!("collect: {e}"));
    assert_eq!(
        status,
        fixture.git(&["status".as_ref(), "--porcelain".as_ref()])
    );
    let file = |inventory: &wt_sweep::model::LocalInventory| {
        inventory
            .entries
            .iter()
            .find(|e| e.path == bp("tracked"))
            .unwrap_or_else(|| panic!("tracked"))
            .clone()
    };
    assert_eq!(file(&first).size, file(&second).size);
    assert_ne!(file(&first).content_sha256, file(&second).content_sha256);
    assert_ne!(first.content_digest_sha256, second.content_digest_sha256);
    let mut timestamps = second.clone();
    timestamps.started_unix_ms = 0;
    timestamps.finished_unix_ms = u64::MAX;
    timestamps.content_digest_sha256 = "ignored".into();
    assert_eq!(
        second.content_digest_sha256,
        content_digest(&timestamps).unwrap_or_else(|e| panic!("digest: {e}"))
    );
}

#[test]
fn split_index_and_own_head_reflog_have_absolute_raw_hash_observations() {
    use std::os::unix::fs::MetadataExt;
    let fixture = Fixture::new();
    fixture.committed_file();
    fixture.git(&["update-index".as_ref(), "--split-index".as_ref()]);
    let tree = fixture.worktree();
    let paths = fs::read_dir(fixture.0.join(".git"))
        .unwrap_or_else(|e| panic!("admin: {e}"))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name().is_some_and(|name| {
                name == "index" || name.to_string_lossy().starts_with("sharedindex.")
            })
        })
        .collect::<Vec<_>>();
    let witness = || {
        paths
            .iter()
            .map(|p| {
                let metadata = fs::symlink_metadata(p).unwrap_or_else(|e| panic!("stat: {e}"));
                (
                    p.clone(),
                    metadata.dev(),
                    metadata.ino(),
                    metadata.mode(),
                    metadata.len(),
                    metadata.mtime(),
                    metadata.mtime_nsec(),
                    metadata.ctime(),
                    metadata.ctime_nsec(),
                )
            })
            .collect::<Vec<_>>()
    };
    let before = witness();
    let inventory =
        collect(&tree, &LocalLimits::default()).unwrap_or_else(|e| panic!("collect: {e}"));
    for category in ["index", "shared_index", "own_reflog"] {
        let entry = inventory
            .administration_entries
            .iter()
            .find(|e| e.category == category)
            .unwrap_or_else(|| panic!("missing {category}: {:?}", inventory.gaps));
        assert!(
            entry
                .path
                .to_path()
                .unwrap_or_else(|e| panic!("path: {e}"))
                .is_absolute()
        );
        assert_eq!(entry.content_sha256.as_ref().map(String::len), Some(64));
    }
    assert_eq!(
        before,
        witness(),
        "source split-index metadata was modified"
    );
    assert!(inventory.index_entries.is_empty());
    assert!(
        inventory
            .entries
            .iter()
            .find(|e| e.path == bp("tracked"))
            .is_some_and(|e| e.category == "unknown")
    );
    assert!(
        inventory
            .gaps
            .iter()
            .any(|g| g.reason.contains("split index"))
    );
    assert!(!inventory.traversal_complete);
}

#[test]
fn no_index_ignore_probe_is_literal_bounded_and_accepts_only_expected_no_match() {
    let fixture = Fixture::new();
    fixture.committed_file();
    fs::write(
        fixture.0.join(".gitignore"),
        b":foo\n:(glob)\\*\n:!tracked\n\\*\n",
    )
    .unwrap_or_else(|e| panic!("ignore: {e}"));
    for name in [":foo", ":(glob)*", ":!tracked", "*", "ordinary"] {
        fs::write(fixture.0.join(name), b"synthetic").unwrap_or_else(|e| panic!("write: {e}"));
    }
    let inventory = collect(&fixture.worktree(), &LocalLimits::default())
        .unwrap_or_else(|e| panic!("collect: {e}"));
    for name in [":foo", ":(glob)*", ":!tracked", "*"] {
        let entry = inventory
            .entries
            .iter()
            .find(|e| e.path == bp(name))
            .unwrap_or_else(|| panic!("missing {name}"));
        assert_eq!(entry.category, "ignored", "{name}: {:?}", inventory.gaps);
    }
    let git = ControlledGit::default();
    assert_eq!(
        git.read(
            &fixture.0,
            GitRead::IgnorePaths {
                paths: vec![bp("ordinary")]
            }
        )
        .unwrap_or_else(|e| panic!("no match: {e}")),
        Vec::<u8>::new()
    );
    assert!(
        git.read(
            &fixture.0,
            GitRead::IgnorePaths {
                paths: vec![bp("../outside")]
            }
        )
        .is_err()
    );
    assert!(
        git.read(
            &fixture.0,
            GitRead::IgnorePaths {
                paths: vec![bp("/absolute")]
            }
        )
        .is_err()
    );
    assert!(
        git.read(
            &fixture.0,
            GitRead::IgnorePaths {
                paths: vec![bp("a\0../outside")]
            }
        )
        .is_err()
    );
    fs::write(fixture.0.join(".gitignore"), b"*\n").unwrap_or_else(|e| panic!("ignore: {e}"));
    let paths = (0..10000)
        .map(|i| bp(&format!("{:06}{}", i, "x".repeat(100))))
        .collect::<Vec<_>>();
    let raw = git
        .read(&fixture.0, GitRead::IgnorePaths { paths })
        .unwrap_or_else(|e| panic!("multiplex: {e}"));
    assert_eq!(raw.iter().filter(|b| **b == 0).count(), 10000);
}

#[test]
fn raw_v3_accepts_valid_extended_flags_and_refuses_reserved_flags_with_valid_checksum() {
    use sha2::Digest;
    let fixture = Fixture::new();
    fixture.committed_file();
    fixture.git(&[
        "update-index".as_ref(),
        "--skip-worktree".as_ref(),
        "tracked".as_ref(),
    ]);
    let tree = fixture.worktree();
    let valid = collect(&tree, &LocalLimits::default()).unwrap_or_else(|e| panic!("valid v3: {e}"));
    assert!(valid.traversal_complete, "{:?}", valid.gaps);
    assert!(
        valid
            .index_entries
            .iter()
            .any(|entry| entry.path == bp("tracked") && entry.stage == 0)
    );
    let index = fixture.0.join(".git/index");
    let mut bytes = fs::read(&index).unwrap_or_else(|e| panic!("read index: {e}"));
    assert_eq!(&bytes[4..8], &3u32.to_be_bytes());
    assert_ne!(bytes[72] & 0x40, 0);
    bytes[75] |= 1;
    let end = bytes.len() - 20;
    let mut hash = sha1_checked::Sha1::new();
    hash.update(&bytes[..end]);
    let digest = hash.try_finalize();
    assert!(!digest.has_collision());
    bytes[end..].copy_from_slice(digest.hash());
    fs::write(&index, &bytes).unwrap_or_else(|e| panic!("mutate index: {e}"));
    let output = Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .arg("-C")
        .arg(&fixture.0)
        .args(["ls-files", "--stage", "-z"])
        .output()
        .unwrap_or_else(|e| panic!("Git refusal: {e}"));
    assert_eq!(output.status.code(), Some(128));
    let invalid =
        collect(&tree, &LocalLimits::default()).unwrap_or_else(|e| panic!("invalid v3: {e}"));
    assert!(
        !invalid.traversal_complete,
        "reserved flags wrongly accepted"
    );
    assert!(invalid.index_entries.is_empty());
    assert!(
        invalid
            .gaps
            .iter()
            .any(|gap| gap.reason.contains("extended flags"))
    );
}

#[test]
fn entry_budget_includes_index_local_and_administration_records_together() {
    let fixture = Fixture::new();
    fixture.committed_file();
    let tree = fixture.worktree();
    for max_entries in [1, 2, 3, 4] {
        let inventory = collect(
            &tree,
            &LocalLimits {
                max_entries,
                ..LocalLimits::default()
            },
        )
        .unwrap_or_else(|e| panic!("collect: {e}"));
        let count = inventory.entries.len()
            + inventory.index_entries.len()
            + inventory.administration_entries.len();
        assert!(
            count <= max_entries,
            "{count} records exceeded budget {max_entries}"
        );
        assert!(!inventory.traversal_complete);
        assert!(
            inventory
                .gaps
                .iter()
                .any(|g| g.reason.contains("entry limit"))
        );
    }
}

struct Refuse {
    target: BytePath,
    errno: i32,
}
impl LocalHooks for Refuse {
    fn before_entry(&self, path: &BytePath) -> std::io::Result<()> {
        if path == &self.target {
            Err(std::io::Error::from_raw_os_error(self.errno))
        } else {
            Ok(())
        }
    }
}
#[test]
fn permission_and_io_errors_remain_explicit_gaps_while_other_entries_succeed() {
    let fixture = Fixture::new();
    fixture.committed_file();
    fs::write(fixture.0.join("other"), b"resident").unwrap_or_else(|e| panic!("write: {e}"));
    for errno in [libc::EPERM, libc::EACCES, libc::EIO] {
        let inventory = collect_with(
            &fixture.worktree(),
            &LocalLimits::default(),
            &Refuse {
                target: bp("tracked"),
                errno,
            },
        )
        .unwrap_or_else(|e| panic!("collect: {e}"));
        assert!(!inventory.traversal_complete);
        assert!(
            inventory
                .gaps
                .iter()
                .any(|g| g.path == bp("tracked") && g.reason.contains("entry refused"))
        );
        assert!(
            inventory
                .entries
                .iter()
                .any(|e| e.path == bp("other") && e.content_sha256.is_some())
        );
    }
}

#[test]
fn fifo_never_blocks_and_is_retained_as_incomplete_special_entry() {
    let fixture = Fixture::new();
    fixture.committed_file();
    let status = Command::new("/usr/bin/mkfifo")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .arg(fixture.0.join("pipe"))
        .status()
        .unwrap_or_else(|e| panic!("fifo: {e}"));
    assert!(status.success());
    let inventory = collect(&fixture.worktree(), &LocalLimits::default())
        .unwrap_or_else(|e| panic!("collect: {e}"));
    let entry = inventory
        .entries
        .iter()
        .find(|e| e.path == bp("pipe"))
        .unwrap_or_else(|| panic!("fifo"));
    assert_eq!(entry.kind, "fifo");
    assert!(entry.content_sha256.is_none());
    assert!(
        inventory
            .gaps
            .iter()
            .any(|g| g.path == bp("pipe") && g.reason.contains("special file"))
    );
    assert!(!inventory.traversal_complete);
}

struct Dataless {
    opened: std::cell::Cell<usize>,
}
impl LocalHooks for Dataless {
    fn is_dataless(&self, path: &BytePath, _: &rustix::fs::Stat) -> bool {
        path == &bp("tracked")
    }
    fn after_file_open(&self, path: &BytePath) -> std::io::Result<()> {
        if path == &bp("tracked") {
            self.opened.set(self.opened.get() + 1);
        }
        Ok(())
    }
}
#[test]
fn dataless_is_refused_before_file_open_and_budget_exhaustion_is_not_complete() {
    let fixture = Fixture::new();
    fixture.committed_file();
    let tree = fixture.worktree();
    let hooks = Dataless {
        opened: std::cell::Cell::new(0),
    };
    let inventory = collect_with(&tree, &LocalLimits::default(), &hooks)
        .unwrap_or_else(|e| panic!("collect: {e}"));
    assert_eq!(hooks.opened.get(), 0);
    assert!(!inventory.traversal_complete);
    for limits in [
        LocalLimits {
            max_entries: 1,
            ..LocalLimits::default()
        },
        LocalLimits {
            max_file_bytes: 1,
            ..LocalLimits::default()
        },
        LocalLimits {
            max_total_bytes: 1,
            ..LocalLimits::default()
        },
        LocalLimits {
            timeout: std::time::Duration::ZERO,
            ..LocalLimits::default()
        },
    ] {
        let inventory = collect(&tree, &limits).unwrap_or_else(|e| panic!("collect: {e}"));
        assert!(!inventory.traversal_complete);
        assert!(!inventory.gaps.is_empty());
    }
}

struct SwapParent {
    root: PathBuf,
    outside: PathBuf,
    done: std::cell::Cell<bool>,
}
impl LocalHooks for SwapParent {
    fn before_entry(&self, path: &BytePath) -> std::io::Result<()> {
        if path == &bp("safe/file") && !self.done.replace(true) {
            fs::rename(self.root.join("safe"), self.root.join("renamed"))?;
            std::os::unix::fs::symlink(&self.outside, self.root.join("safe"))?;
        }
        Ok(())
    }
}
#[test]
fn directory_parent_replacement_cannot_redirect_an_anchored_content_read() {
    use sha2::{Digest, Sha256};
    let fixture = Fixture::new();
    fixture.committed_file();
    let outside = Fixture::new();
    fs::create_dir(fixture.0.join("safe")).unwrap_or_else(|e| panic!("mkdir: {e}"));
    fs::write(fixture.0.join("safe/file"), b"inside").unwrap_or_else(|e| panic!("write: {e}"));
    fs::write(outside.0.join("file"), b"outside synthetic sentinel")
        .unwrap_or_else(|e| panic!("write: {e}"));
    let inventory = collect_with(
        &fixture.worktree(),
        &LocalLimits::default(),
        &SwapParent {
            root: fixture.0.clone(),
            outside: outside.0.clone(),
            done: std::cell::Cell::new(false),
        },
    )
    .unwrap_or_else(|e| panic!("collect: {e}"));
    let entry = inventory
        .entries
        .iter()
        .find(|e| e.path == bp("safe/file"))
        .unwrap_or_else(|| panic!("anchored file"));
    assert_eq!(
        entry.content_sha256,
        Some(format!("{:x}", Sha256::digest(b"inside")))
    );
    assert!(!inventory.traversal_complete);
    assert!(
        inventory
            .gaps
            .iter()
            .any(|g| g.reason.contains("directory changed"))
    );
}
