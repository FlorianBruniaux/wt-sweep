#![cfg(unix)]
use sha2::Digest;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use wt_sweep::{
    git::scan_family,
    local::{SummaryLimits, collect_summary},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(format: &str) -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/wt-sweep-summary-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap_or_else(|e| panic!("fixture: {e}"));
        let f = Self(path);
        f.git(&["init", "-b", "main", &format!("--object-format={format}")]);
        f
    }
    fn git(&self, args: &[&str]) -> Vec<u8> {
        let out = Command::new("/usr/bin/git")
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
        assert!(out.status.success(), "{:?}", out.stderr);
        out.stdout
    }
    fn commit(&self) {
        self.git(&["add", "."]);
        self.git(&["commit", "-m", "fixture"]);
    }
    fn worktree(&self) -> wt_sweep::model::Worktree {
        scan_family(&self.0)
            .unwrap_or_else(|e| panic!("scan: {e}"))
            .families[0]
            .worktrees[0]
            .clone()
    }
    fn summary(&self) -> wt_sweep::model::LocalSummary {
        collect_summary(&self.worktree(), &SummaryLimits::default())
            .unwrap_or_else(|e| panic!("summary: {e}"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn nested_tracked_file_is_exactly_clean_in_both_object_formats() {
    for format in ["sha1", "sha256"] {
        let f = Fixture::new(format);
        fs::create_dir(f.0.join("src")).unwrap_or_else(|e| panic!("mkdir: {e}"));
        fs::write(f.0.join("src/main.rs"), b"tracked\n").unwrap_or_else(|e| panic!("write: {e}"));
        f.commit();
        let before = fs::read(f.0.join(".git/index")).unwrap_or_else(|e| panic!("read: {e}"));
        let s = f.summary();
        assert!(
            s.head_complete && s.index_complete && s.disk_complete,
            "{s:?}"
        );
        for c in [
            &s.staged,
            &s.unstaged,
            &s.conflicts,
            &s.untracked,
            &s.ignored,
        ] {
            assert!(c.complete);
            assert_eq!(c.observed, 0);
        }
        assert_eq!(
            s.head.as_ref().map(String::len),
            Some(if format == "sha1" { 40 } else { 64 })
        );
        assert_eq!(s.sources.len(), 2);
        assert!(s.gaps.is_empty());
        assert_eq!(s.digest_sha256.len(), 64);
        assert_eq!(
            fs::read(f.0.join(".git/index")).unwrap_or_else(|e| panic!("read: {e}")),
            before
        );
    }
}
#[test]
fn three_versions_are_compared_separately_and_deletions_count() {
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"HEAD").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    fs::write(f.0.join("file"), b"INDEX").unwrap_or_else(|e| panic!("{e}"));
    f.git(&["add", "file"]);
    fs::write(f.0.join("file"), b"DISK").unwrap_or_else(|e| panic!("{e}"));
    let s = f.summary();
    assert_eq!((s.staged.observed, s.unstaged.observed), (1, 1));
    assert!(s.staged.complete && s.unstaged.complete);
    fs::remove_file(f.0.join("file")).unwrap_or_else(|e| panic!("{e}"));
    let s = f.summary();
    assert_eq!(s.unstaged.observed, 1);
    f.git(&["add", "-u"]);
    let s = f.summary();
    assert_eq!((s.staged.observed, s.unstaged.observed), (1, 0));
    assert!(s.unstaged.complete);
}
#[test]
fn malformed_and_unsupported_indexes_never_report_complete_empty_index() {
    for action in ["checksum", "v4", "split", "absent", "intent"] {
        let f = Fixture::new("sha1");
        fs::write(f.0.join("file"), b"base").unwrap_or_else(|e| panic!("{e}"));
        f.commit();
        match action {
            "checksum" => {
                let p = f.0.join(".git/index");
                let mut raw = fs::read(&p).unwrap_or_else(|e| panic!("{e}"));
                raw[20] ^= 1;
                fs::write(p, raw).unwrap_or_else(|e| panic!("{e}"));
            }
            "v4" => {
                f.git(&["update-index", "--index-version=4"]);
            }
            "split" => {
                f.git(&["update-index", "--split-index"]);
            }
            "absent" => {
                fs::remove_file(f.0.join(".git/index")).unwrap_or_else(|e| panic!("{e}"));
            }
            _ => {
                fs::write(f.0.join("new"), b"intent").unwrap_or_else(|e| panic!("{e}"));
                f.git(&["add", "-N", "new"]);
            }
        }
        let s = f.summary();
        assert!(
            !s.index_complete && !s.staged.complete && !s.unstaged.complete,
            "{action}: {s:?}"
        );
        assert!(!s.gaps.is_empty());
    }
}
#[test]
fn positive_ignored_presence_survives_content_refusal() {
    struct Refuse;
    impl wt_sweep::local::LocalHooks for Refuse {
        fn after_file_open(&self, p: &wt_sweep::model::BytePath) -> std::io::Result<()> {
            if p.display() == "src/ignored" {
                Err(std::io::Error::from_raw_os_error(libc::EIO))
            } else {
                Ok(())
            }
        }
    }
    let f = Fixture::new("sha1");
    fs::create_dir(f.0.join("src")).unwrap_or_else(|e| panic!("{e}"));
    fs::write(f.0.join("src/tracked"), b"base").unwrap_or_else(|e| panic!("{e}"));
    fs::write(f.0.join(".gitignore"), b"src/ignored\n").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    fs::write(f.0.join("src/ignored"), b"kept").unwrap_or_else(|e| panic!("{e}"));
    let s =
        wt_sweep::local::collect_summary_with(&f.worktree(), &SummaryLimits::default(), &Refuse)
            .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(s.ignored.observed, 1);
    assert_eq!(s.untracked.observed, 0);
    assert!(!s.disk_complete);
    assert!(!s.gaps.is_empty());
}
#[test]
fn ordinary_index_25001_shared_blobs_is_clean_and_summary_stays_compact() {
    let f = Fixture::new("sha1");
    fs::create_dir(f.0.join("src")).unwrap_or_else(|e| panic!("{e}"));
    for n in 0..25001 {
        fs::write(f.0.join(format!("src/file{n:05}")), b"shared\n")
            .unwrap_or_else(|e| panic!("{e}"));
    }
    f.commit();
    let s = f.summary();
    assert!(
        s.head_complete && s.index_complete && s.disk_complete,
        "{s:?}"
    );
    assert_eq!(
        (s.staged.observed, s.unstaged.observed, s.untracked.observed),
        (0, 0, 0)
    );
    assert!(s.staged.complete && s.unstaged.complete);
    assert!(
        serde_json::to_vec(&s)
            .unwrap_or_else(|e| panic!("{e}"))
            .len()
            < 65536
    );
}
#[test]
fn distinct_flat_blobs_are_complete_and_detect_staged_and_unstaged_changes() {
    for format in ["sha1", "sha256"] {
        let f = Fixture::new(format);
        for n in 0..1024 {
            fs::write(f.0.join(format!("file{n:04}")), format!("distinct {n}\n"))
                .unwrap_or_else(|e| panic!("{e}"));
        }
        f.commit();
        let clean = f.summary();
        assert!(
            clean.head_complete && clean.index_complete && clean.disk_complete,
            "{format}: {clean:?}"
        );
        for count in [
            &clean.staged,
            &clean.unstaged,
            &clean.conflicts,
            &clean.untracked,
            &clean.ignored,
        ] {
            assert!(count.complete, "{format}: {clean:?}");
            assert_eq!(count.observed, 0);
        }
        fs::write(f.0.join("file0000"), b"staged change\n").unwrap_or_else(|e| panic!("{e}"));
        f.git(&["add", "file0000"]);
        fs::write(f.0.join("file0001"), b"disk change\n").unwrap_or_else(|e| panic!("{e}"));
        let dirty = f.summary();
        assert!(
            dirty.head_complete && dirty.index_complete && dirty.disk_complete,
            "{format}: {dirty:?}"
        );
        assert!(
            dirty.staged.complete && dirty.unstaged.complete,
            "{format}: {dirty:?}"
        );
        assert_eq!((dirty.staged.observed, dirty.unstaged.observed), (1, 1));
        assert!(dirty.gaps.is_empty(), "{format}: {dirty:?}");
    }
}
#[test]
fn executable_symlink_and_index_flags_do_not_hide_resident_changes() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    for flag in ["--assume-unchanged", "--skip-worktree"] {
        let f = Fixture::new("sha1");
        fs::write(f.0.join("file"), b"same").unwrap_or_else(|e| panic!("{e}"));
        f.commit();
        f.git(&["update-index", flag, "file"]);
        fs::write(f.0.join("file"), b"diff").unwrap_or_else(|e| panic!("{e}"));
        let s = f.summary();
        assert_eq!(s.unstaged.observed, 1);
        assert!(s.index_complete && s.unstaged.complete, "{s:?}");
    }
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"same").unwrap_or_else(|e| panic!("{e}"));
    symlink("literal", f.0.join("link")).unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    fs::set_permissions(f.0.join("file"), fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|e| panic!("{e}"));
    fs::remove_file(f.0.join("link")).unwrap_or_else(|e| panic!("{e}"));
    fs::write(f.0.join("link"), b"literal").unwrap_or_else(|e| panic!("{e}"));
    let s = f.summary();
    assert_eq!(s.unstaged.observed, 2);
    assert!(s.unstaged.complete, "{s:?}");
}
#[test]
fn missing_staged_blob_preserves_index_and_staged_count() {
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"HEAD").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    fs::write(f.0.join("file"), b"STAGED").unwrap_or_else(|e| panic!("{e}"));
    f.git(&["add", "file"]);
    let oid = String::from_utf8(f.git(&["rev-parse", ":file"])).unwrap_or_else(|e| panic!("{e}"));
    let oid = oid.trim();
    fs::remove_file(f.0.join(format!(".git/objects/{}/{}", &oid[..2], &oid[2..])))
        .unwrap_or_else(|e| panic!("{e}"));
    let s = f.summary();
    assert!(s.index_complete && s.head_complete);
    assert_eq!(s.staged.observed, 1);
    assert!(s.staged.complete);
    assert!(!s.unstaged.complete);
    assert!(!s.gaps.is_empty());
}
#[test]
fn missing_blob_batch_preserves_other_blobs_and_observed_disk_changes() {
    for format in ["sha1", "sha256"] {
        let f = Fixture::new(format);
        for n in 0..128 {
            fs::write(f.0.join(format!("file{n:03}")), format!("distinct {n}\n"))
                .unwrap_or_else(|e| panic!("{e}"));
        }
        f.commit();
        let oid =
            String::from_utf8(f.git(&["rev-parse", ":file000"])).unwrap_or_else(|e| panic!("{e}"));
        let oid = oid.trim();
        fs::remove_file(f.0.join(format!(".git/objects/{}/{}", &oid[..2], &oid[2..])))
            .unwrap_or_else(|e| panic!("{e}"));
        fs::write(f.0.join("file127"), b"changed disk bytes\n").unwrap_or_else(|e| panic!("{e}"));
        let s = f.summary();
        assert!(
            s.index_complete && s.staged.complete && s.disk_complete,
            "{format}: {s:?}"
        );
        assert!(!s.head_complete && !s.unstaged.complete, "{format}: {s:?}");
        assert_eq!((s.staged.observed, s.unstaged.observed), (0, 1));
        assert!(!s.gaps.is_empty());
    }
}
#[test]
fn cancelled_or_small_budgets_cannot_claim_exact_zero() {
    struct Cancel;
    impl wt_sweep::local::LocalHooks for Cancel {
        fn cancelled(&self) -> bool {
            true
        }
    }
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"baseline").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    let s =
        wt_sweep::local::collect_summary_with(&f.worktree(), &SummaryLimits::default(), &Cancel)
            .unwrap_or_else(|e| panic!("{e}"));
    assert!(!s.head_complete && !s.index_complete && !s.disk_complete);
    assert!(!s.gaps.is_empty());
    for limits in [
        SummaryLimits {
            max_objects: 0,
            ..SummaryLimits::default()
        },
        SummaryLimits {
            max_paths: 0,
            ..SummaryLimits::default()
        },
        SummaryLimits {
            max_total_bytes: 16,
            ..SummaryLimits::default()
        },
        SummaryLimits {
            max_index_entries: 0,
            ..SummaryLimits::default()
        },
    ] {
        let s = collect_summary(&f.worktree(), &limits).unwrap_or_else(|e| panic!("{e}"));
        assert!(!s.head_complete || !s.index_complete || !s.disk_complete);
        assert!(!s.gaps.is_empty());
    }
}
#[test]
fn conflict_stages_count_one_distinct_path() {
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"base\n").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    f.git(&["checkout", "-b", "other"]);
    fs::write(f.0.join("file"), b"other\n").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("file"), b"main\n").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    let out = Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .arg("-C")
        .arg(&f.0)
        .args(["merge", "other"])
        .output()
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(!out.status.success());
    let s = f.summary();
    assert_eq!(s.conflicts.observed, 1);
    assert!(s.conflicts.complete && s.index_complete);
}
#[test]
fn same_size_and_mtime_bytes_are_still_compared() {
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"aaaa").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    let metadata = fs::metadata(f.0.join("file")).unwrap_or_else(|e| panic!("{e}"));
    fs::write(f.0.join("file"), b"bbbb").unwrap_or_else(|e| panic!("{e}"));
    let file = fs::OpenOptions::new()
        .write(true)
        .open(f.0.join("file"))
        .unwrap_or_else(|e| panic!("{e}"));
    file.set_times(
        fs::FileTimes::new().set_modified(metadata.modified().unwrap_or_else(|e| panic!("{e}"))),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let s = f.summary();
    assert_eq!(s.unstaged.observed, 1);
    assert!(s.unstaged.complete);
}
#[test]
fn detected_file_mutation_marks_temporal_drift() {
    struct Change(PathBuf);
    impl wt_sweep::local::LocalHooks for Change {
        fn after_file_open(&self, p: &wt_sweep::model::BytePath) -> std::io::Result<()> {
            if p.display() == "file" {
                fs::write(self.0.join("file"), b"changed!")?;
            }
            Ok(())
        }
    }
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"baseline").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    let s = wt_sweep::local::collect_summary_with(
        &f.worktree(),
        &SummaryLimits::default(),
        &Change(f.0.clone()),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(!s.disk_complete && !s.unstaged.complete);
    assert_eq!(s.temporal_consistency, "change_observed");
}
#[test]
fn path_memory_budget_is_explicitly_incomplete() {
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"base").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    let s = collect_summary(
        &f.worktree(),
        &SummaryLimits {
            max_path_bytes: 0,
            ..SummaryLimits::default()
        },
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(!s.head_complete && !s.disk_complete, "{s:?}");
    assert!(!s.gaps.is_empty());
}
#[test]
fn dataless_administration_marker_is_refused_before_git() {
    struct Dataless;
    impl wt_sweep::local::LocalHooks for Dataless {
        fn is_dataless(&self, p: &wt_sweep::model::BytePath, _: &rustix::fs::Stat) -> bool {
            p.to_path()
                .is_ok_and(|p| p.file_name().is_some_and(|n| n == ".git"))
        }
    }
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"base").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    let s =
        wt_sweep::local::collect_summary_with(&f.worktree(), &SummaryLimits::default(), &Dataless)
            .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        s.head.is_none() && !s.index_complete && !s.disk_complete,
        "{s:?}"
    );
    assert!(!s.gaps.is_empty());
}
#[test]
fn access_errors_keep_positive_untracked_counts_and_bound_diagnostics() {
    struct Refuse(i32);
    impl wt_sweep::local::LocalHooks for Refuse {
        fn before_entry(&self, p: &wt_sweep::model::BytePath) -> std::io::Result<()> {
            if p.display().starts_with("untracked") {
                Err(std::io::Error::from_raw_os_error(self.0))
            } else {
                Ok(())
            }
        }
    }
    for errno in [libc::EACCES, libc::EPERM, libc::EIO] {
        let f = Fixture::new("sha1");
        fs::write(f.0.join("file"), b"base").unwrap_or_else(|e| panic!("{e}"));
        f.commit();
        for n in 0..40 {
            fs::write(f.0.join(format!("untracked{n:02}")), b"resident")
                .unwrap_or_else(|e| panic!("{e}"));
        }
        let s = wt_sweep::local::collect_summary_with(
            &f.worktree(),
            &SummaryLimits::default(),
            &Refuse(errno),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(s.untracked.observed, 40);
        assert!(!s.untracked.complete && !s.disk_complete);
        assert!(s.gaps.len() <= 32);
        assert_eq!(s.untracked.samples.len(), 8);
        assert_eq!(s.untracked.samples_omitted, 32);
        assert!(s.details_truncated);
    }
}
#[test]
fn type_or_mode_change_between_metadata_and_content_invalidates_disk() {
    use std::os::unix::fs::PermissionsExt;
    struct Change(PathBuf);
    impl wt_sweep::local::LocalHooks for Change {
        fn before_entry(&self, p: &wt_sweep::model::BytePath) -> std::io::Result<()> {
            if p.display() == "file" {
                fs::set_permissions(self.0.join("file"), fs::Permissions::from_mode(0o755))?;
            }
            Ok(())
        }
    }
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"same").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    let s = wt_sweep::local::collect_summary_with(
        &f.worktree(),
        &SummaryLimits::default(),
        &Change(f.0.clone()),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(!s.disk_complete && !s.unstaged.complete, "{s:?}");
    assert_eq!(s.temporal_consistency, "change_observed");
}
#[test]
fn split_index_refusal_preserves_shared_index_metadata_and_bytes() {
    use std::os::unix::fs::MetadataExt;
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"base").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    f.git(&["update-index", "--split-index"]);
    let snapshot = || {
        fs::read_dir(f.0.join(".git"))
            .unwrap_or_else(|e| panic!("{e}"))
            .map(|e| e.unwrap_or_else(|e| panic!("{e}")).path())
            .filter(|p| {
                p.file_name().is_some_and(|n| {
                    n == "index" || n.as_encoded_bytes().starts_with(b"sharedindex.")
                })
            })
            .map(|p| {
                let m = fs::metadata(&p).unwrap_or_else(|e| panic!("{e}"));
                (
                    p.clone(),
                    (
                        fs::read(&p).unwrap_or_else(|e| panic!("{e}")),
                        m.ino(),
                        m.mode(),
                        m.mtime(),
                        m.mtime_nsec(),
                        m.ctime(),
                        m.ctime_nsec(),
                    ),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let before = snapshot();
    assert!(before.len() >= 2);
    let s = f.summary();
    assert!(!s.index_complete);
    assert_eq!(before, snapshot());
}
#[test]
fn gitlink_index_without_resident_directory_has_gap_and_incomplete_comparison() {
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"base").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    let head = String::from_utf8(f.git(&["rev-parse", "HEAD"])).unwrap_or_else(|e| panic!("{e}"));
    f.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{},module", head.trim()),
    ]);
    let s = f.summary();
    assert!(!s.index_complete && !s.unstaged.complete, "{s:?}");
    assert!(s.gaps.iter().any(|g| g.reason.contains("gitlink")));
}
#[test]
fn ignored_presence_survives_cancelled_hash_and_paths_keep_literal_bytes() {
    use std::sync::atomic::AtomicBool;
    struct CancelAfterOpen(AtomicBool);
    impl wt_sweep::local::LocalHooks for CancelAfterOpen {
        fn after_file_open(&self, p: &wt_sweep::model::BytePath) -> std::io::Result<()> {
            if p.bytes().is_ok_and(|b| b == b"src/ignored\nfile") {
                self.0.store(true, Ordering::Relaxed);
            }
            Ok(())
        }
        fn cancelled(&self) -> bool {
            self.0.load(Ordering::Relaxed)
        }
    }
    let f = Fixture::new("sha1");
    fs::create_dir(f.0.join("src")).unwrap_or_else(|e| panic!("{e}"));
    fs::write(f.0.join("src/tracked"), b"base").unwrap_or_else(|e| panic!("{e}"));
    fs::write(f.0.join(".gitignore"), b"src/ignored*\n").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    fs::write(f.0.join("src/ignored\nfile"), b"resident").unwrap_or_else(|e| panic!("{e}"));
    let s = wt_sweep::local::collect_summary_with(
        &f.worktree(),
        &SummaryLimits::default(),
        &CancelAfterOpen(AtomicBool::new(false)),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(s.ignored.observed, 1);
    assert!(!s.ignored.complete && !s.disk_complete);
    assert_eq!(
        s.ignored.samples[0]
            .bytes()
            .unwrap_or_else(|e| panic!("{e}")),
        b"src/ignored\nfile"
    );
}
#[test]
fn raw_required_extension_is_refused_despite_a_valid_checksum() {
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"base").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    let p = f.0.join(".git/index");
    let mut raw = fs::read(&p).unwrap_or_else(|e| panic!("{e}"));
    raw.truncate(raw.len() - 20);
    raw.extend_from_slice(b"abcd\0\0\0\0");
    let mut hash = sha1_checked::Sha1::new();
    hash.update(&raw);
    let result = hash.try_finalize();
    assert!(!result.has_collision());
    raw.extend_from_slice(result.hash());
    fs::write(p, raw).unwrap_or_else(|e| panic!("{e}"));
    let s = f.summary();
    assert!(!s.index_complete && !s.staged.complete && !s.unstaged.complete);
    assert!(s.gaps.iter().any(|g| g.reason.contains("extension")));
}
#[test]
fn corrupt_blob_bytes_never_pass_full_oid_verification_in_either_format() {
    for format in ["sha1", "sha256"] {
        let f = Fixture::new(format);
        fs::write(f.0.join("file"), b"original").unwrap_or_else(|e| panic!("{e}"));
        f.commit();
        let original =
            String::from_utf8(f.git(&["rev-parse", ":file"])).unwrap_or_else(|e| panic!("{e}"));
        fs::write(f.0.join("other"), b"tampered").unwrap_or_else(|e| panic!("{e}"));
        let other = String::from_utf8(f.git(&["hash-object", "-w", "other"]))
            .unwrap_or_else(|e| panic!("{e}"));
        let object = |oid: &str| {
            let oid = oid.trim();
            f.0.join(format!(".git/objects/{}/{}", &oid[..2], &oid[2..]))
        };
        let bytes = fs::read(object(&other)).unwrap_or_else(|e| panic!("{e}"));
        fs::remove_file(object(&original)).unwrap_or_else(|e| panic!("{e}"));
        fs::write(object(&original), bytes).unwrap_or_else(|e| panic!("{e}"));
        let s = f.summary();
        assert!(!s.head_complete && !s.unstaged.complete, "{s:?}");
        assert!(s.index_complete);
        assert!(s.gaps.iter().any(|g| g.reason.contains("OID")), "{s:?}");
    }
}
#[test]
fn revalidation_access_error_invalidates_coverage_without_fabricating_drift() {
    use std::sync::atomic::AtomicUsize;
    struct RefuseFinal {
        root: wt_sweep::model::BytePath,
        reads: AtomicUsize,
    }
    impl wt_sweep::local::LocalHooks for RefuseFinal {
        fn before_entry(&self, p: &wt_sweep::model::BytePath) -> std::io::Result<()> {
            if p == &self.root && self.reads.fetch_add(1, Ordering::Relaxed) > 0 {
                return Err(std::io::Error::from_raw_os_error(libc::EACCES));
            }
            Ok(())
        }
    }
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"base").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    let w = f.worktree();
    let s = wt_sweep::local::collect_summary_with(
        &w,
        &SummaryLimits::default(),
        &RefuseFinal {
            root: w.path.clone(),
            reads: AtomicUsize::new(0),
        },
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        !s.head_complete && !s.index_complete && !s.disk_complete,
        "{s:?}"
    );
    assert_eq!(s.temporal_consistency, "not_guaranteed");
    assert!(!s.gaps.is_empty());
}
#[test]
fn verified_conflict_positive_survives_unrelated_gitlink_refusal() {
    let f = Fixture::new("sha1");
    fs::write(f.0.join("file"), b"base\n").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    f.git(&["checkout", "-b", "other"]);
    fs::write(f.0.join("file"), b"other\n").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    f.git(&["checkout", "main"]);
    fs::write(f.0.join("file"), b"main\n").unwrap_or_else(|e| panic!("{e}"));
    f.commit();
    let status = Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .arg("-C")
        .arg(&f.0)
        .args(["merge", "other"])
        .output()
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(!status.status.success());
    assert!(!f.git(&["ls-files", "-u"]).is_empty());
    let oid = String::from_utf8(f.git(&["rev-parse", "HEAD"])).unwrap_or_else(|e| panic!("{e}"));
    f.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{},module", oid.trim()),
    ]);
    let s = f.summary();
    assert_eq!(s.conflicts.observed, 1, "{s:?}");
    assert!(!s.conflicts.complete && !s.index_complete);
    assert_eq!(
        s.conflicts.samples[0]
            .bytes()
            .unwrap_or_else(|e| panic!("{e}")),
        b"file"
    );
    assert!(s.gaps.iter().any(|g| g.reason.contains("gitlink")));
    // Unverified raw bytes must not become a positive conflict observation.
    let path = f.0.join(".git/index");
    let mut raw = fs::read(&path).unwrap_or_else(|e| panic!("{e}"));
    let last = raw.len() - 1;
    raw[last] ^= 1;
    fs::write(path, raw).unwrap_or_else(|e| panic!("{e}"));
    let refused = f.summary();
    assert_eq!(refused.conflicts.observed, 0);
    assert!(!refused.conflicts.complete && !refused.index_complete);
    assert!(refused.gaps.iter().any(|g| g.reason.contains("checksum")));
}

#[test]
fn unavailable_directory_tree_preserves_unknown_head_in_both_formats() {
    for format in ["sha1", "sha256"] {
        for fault in ["missing", "corrupt"] {
            let f = Fixture::new(format);
            for n in 0..2 {
                let dir = f.0.join(format!("dir{n}"));
                fs::create_dir(&dir).unwrap_or_else(|e| panic!("{e}"));
                fs::write(dir.join("file"), format!("distinct {n}\n"))
                    .unwrap_or_else(|e| panic!("{e}"));
            }
            f.commit();
            let object_path = |oid: Vec<u8>| {
                let oid = String::from_utf8(oid).unwrap_or_else(|e| panic!("{e}"));
                let oid = oid.trim();
                f.0.join(format!(".git/objects/{}/{}", &oid[..2], &oid[2..]))
            };
            let first = object_path(f.git(&["rev-parse", "HEAD:dir0"]));
            if fault == "missing" {
                fs::remove_file(first).unwrap_or_else(|e| panic!("{e}"));
            } else {
                let other = object_path(f.git(&["rev-parse", "HEAD:dir1"]));
                fs::remove_file(&first).unwrap_or_else(|e| panic!("{e}"));
                fs::write(first, fs::read(other).unwrap_or_else(|e| panic!("{e}")))
                    .unwrap_or_else(|e| panic!("{e}"));
            }
            let summary = f.summary();
            assert!(
                !summary.head_complete && !summary.staged.complete,
                "{format} {fault}: {summary:?}"
            );
            assert!(
                summary.index_complete && summary.disk_complete && summary.unstaged.complete,
                "{format} {fault}: {summary:?}"
            );
            assert_eq!(summary.unstaged.observed, 0);
            assert!(
                summary
                    .gaps
                    .iter()
                    .any(|gap| gap.reason.contains("HEAD tree unavailable")),
                "{summary:?}"
            );
        }
    }
}
#[test]
fn directory_tree_frontier_keeps_depth_and_path_limits_incomplete() {
    let f = Fixture::new("sha1");
    for n in 0..2 {
        let dir = f.0.join(format!("dir{n}"));
        fs::create_dir(&dir).unwrap_or_else(|e| panic!("{e}"));
        fs::write(dir.join("file"), format!("distinct {n}\n")).unwrap_or_else(|e| panic!("{e}"));
    }
    f.commit();
    for limits in [
        SummaryLimits {
            max_depth: 0,
            ..SummaryLimits::default()
        },
        SummaryLimits {
            max_paths: 2,
            ..SummaryLimits::default()
        },
    ] {
        let summary = collect_summary(&f.worktree(), &limits).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            !summary.head_complete && !summary.staged.complete,
            "{summary:?}"
        );
        assert!(!summary.gaps.is_empty());
    }
}

#[test]
fn expanded_profile_covers_more_than_4096_distinct_blobs_without_hiding_changes() {
    let f = Fixture::new("sha1");
    for n in 0..4097 {
        fs::write(f.0.join(format!("file{n:04}")), format!("distinct {n}\n"))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    f.commit();
    let worktree = f.worktree();
    let limits = SummaryLimits::thorough();
    let standard = collect_summary(&worktree, &SummaryLimits::default())
        .unwrap_or_else(|e| panic!("standard: {e}"));
    assert!(!standard.head_complete && !standard.unstaged.complete);
    assert!(
        standard
            .gaps
            .iter()
            .any(|gap| gap.reason.contains("object limit"))
    );
    let summary =
        collect_summary(&worktree, &limits).unwrap_or_else(|e| panic!("expanded clean: {e}"));
    assert!(
        summary.head_complete && summary.index_complete && summary.disk_complete,
        "{summary:?}"
    );
    for count in [
        &summary.staged,
        &summary.unstaged,
        &summary.conflicts,
        &summary.untracked,
        &summary.ignored,
    ] {
        assert!(count.complete);
        assert_eq!(count.observed, 0);
    }
    assert!(summary.gaps.is_empty());
    fs::write(f.0.join("file0000"), b"staged replacement\n").unwrap_or_else(|e| panic!("{e}"));
    f.git(&["add", "file0000"]);
    fs::write(f.0.join("file0001"), b"resident replacement\n").unwrap_or_else(|e| panic!("{e}"));
    let dirty =
        collect_summary(&worktree, &limits).unwrap_or_else(|e| panic!("expanded dirty: {e}"));
    assert!(
        dirty.staged.complete && dirty.unstaged.complete,
        "{dirty:?}"
    );
    assert_eq!((dirty.staged.observed, dirty.unstaged.observed), (1, 1));
    let staged_oid =
        String::from_utf8(f.git(&["rev-parse", ":file0000"])).unwrap_or_else(|e| panic!("{e}"));
    let staged_oid = staged_oid.trim();
    fs::remove_file(f.0.join(format!(
        ".git/objects/{}/{}",
        &staged_oid[..2],
        &staged_oid[2..]
    )))
    .unwrap_or_else(|e| panic!("{e}"));
    let missing = collect_summary(&worktree, &limits)
        .unwrap_or_else(|e| panic!("expanded missing staged blob: {e}"));
    assert!(
        missing.index_complete && missing.staged.complete && missing.head_complete,
        "{missing:?}"
    );
    assert_eq!(missing.staged.observed, 1);
    assert!(!missing.unstaged.complete);
    assert!(!missing.gaps.is_empty());
    let index_path = f.0.join(".git/index");
    let mut raw = fs::read(&index_path).unwrap_or_else(|e| panic!("{e}"));
    raw[20] ^= 1;
    fs::write(index_path, raw).unwrap_or_else(|e| panic!("{e}"));
    let corrupt =
        collect_summary(&worktree, &limits).unwrap_or_else(|e| panic!("expanded corrupt: {e}"));
    assert!(
        !corrupt.index_complete && !corrupt.staged.complete && !corrupt.unstaged.complete,
        "{corrupt:?}"
    );
    assert!(!corrupt.gaps.is_empty());
}
