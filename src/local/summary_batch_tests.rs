use super::*;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
// These deadline-sensitive payload fixtures must not compete with each other.
static PAYLOAD_FIXTURE: std::sync::Mutex<()> = std::sync::Mutex::new(());
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/wt-sweep-summary-batch-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap_or_else(|e| panic!("{e}"));
        let f = Self(path);
        f.git(&["init", "-q", "-b", "main"]);
        f
    }
    fn git(&self, args: &[&str]) -> Vec<u8> {
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
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(output.status.success(), "{:?}", output.stderr);
        output.stdout
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn distinct_blob_summary_bounds_actual_git_children() {
    let _guard = PAYLOAD_FIXTURE.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    for n in 0..1024 {
        fs::write(f.0.join(format!("file{n:04}")), format!("distinct {n}\n"))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    f.git(&["add", "."]);
    f.git(&["commit", "-q", "-m", "fixture"]);
    let worktree = crate::git::scan_family(&f.0)
        .unwrap_or_else(|e| panic!("{e}"))
        .families[0]
        .worktrees[0]
        .clone();
    crate::git::take_child_read_journal();
    let summary =
        collect_summary(&worktree, &SummaryLimits::default()).unwrap_or_else(|e| panic!("{e}"));
    let reads = crate::git::take_child_read_journal();
    eprintln!(
        "1024 distinct flat blobs: {} total Git children",
        reads.len()
    );
    assert!(
        reads.len() <= 24,
        "1024 distinct blobs spawned {} Git children",
        reads.len()
    );
    assert!(
        summary.head_complete && summary.index_complete && summary.disk_complete,
        "{summary:?}"
    );
    assert_eq!((summary.staged.observed, summary.unstaged.observed), (0, 0));
}

#[test]
fn medium_blobs_split_payload_batches_without_scalar_reads() {
    let _guard = PAYLOAD_FIXTURE.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    for n in 0..3 {
        fs::write(
            f.0.join(format!("file{n}")),
            vec![b'a' + n; 4 * 1024 * 1024],
        )
        .unwrap_or_else(|e| panic!("{e}"));
    }
    f.git(&["add", "."]);
    f.git(&["commit", "-q", "-m", "fixture"]);
    let worktree = crate::git::scan_family(&f.0)
        .unwrap_or_else(|e| panic!("{e}"))
        .families[0]
        .worktrees[0]
        .clone();
    crate::git::take_child_read_journal();
    let summary =
        collect_summary(&worktree, &SummaryLimits::default()).unwrap_or_else(|e| panic!("{e}"));
    let reads = crate::git::take_child_read_journal();
    assert!(
        summary.head_complete && summary.index_complete && summary.disk_complete,
        "{summary:?}"
    );
    assert_eq!(summary.unstaged.observed, 0);
    assert!(
        !reads
            .iter()
            .any(|(_, op)| matches!(op, GitRead::Blob { .. })),
        "medium blobs were scalar: {reads:?}"
    );
}
#[test]
fn blob_at_scalar_output_ceiling_remains_available() {
    let _guard = PAYLOAD_FIXTURE.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    fs::write(f.0.join("large"), vec![b'x'; 8 * 1024 * 1024]).unwrap_or_else(|e| panic!("{e}"));
    f.git(&["add", "."]);
    f.git(&["commit", "-q", "-m", "fixture"]);
    let worktree = crate::git::scan_family(&f.0)
        .unwrap_or_else(|e| panic!("{e}"))
        .families[0]
        .worktrees[0]
        .clone();
    let summary =
        collect_summary(&worktree, &SummaryLimits::default()).unwrap_or_else(|e| panic!("{e}"));
    assert!(
        summary.head_complete && summary.index_complete && summary.disk_complete,
        "{summary:?}"
    );
    assert!(summary.unstaged.complete);
    assert_eq!(summary.unstaged.observed, 0);
}

#[test]
fn object_budget_exhaustion_never_launches_scalar_fallbacks() {
    let _guard = PAYLOAD_FIXTURE.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    for n in 0..3 {
        fs::write(f.0.join(format!("file{n}")), format!("distinct {n}\n"))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    f.git(&["add", "."]);
    f.git(&["commit", "-q", "-m", "fixture"]);
    let worktree = crate::git::scan_family(&f.0)
        .unwrap_or_else(|e| panic!("{e}"))
        .families[0]
        .worktrees[0]
        .clone();
    crate::git::take_child_read_journal();
    let limits = SummaryLimits {
        max_objects: 4,
        ..SummaryLimits::default()
    };
    let summary = collect_summary(&worktree, &limits).unwrap_or_else(|e| panic!("{e}"));
    let reads = crate::git::take_child_read_journal();
    assert!(
        summary.index_complete && summary.staged.complete && summary.disk_complete,
        "{summary:?}"
    );
    assert!(
        !summary.head_complete && !summary.unstaged.complete,
        "{summary:?}"
    );
    assert!(!summary.gaps.is_empty());
    assert!(
        !reads
            .iter()
            .any(|(_, op)| matches!(op, GitRead::Blob { .. })),
        "object budget caused scalar fallback: {reads:?}"
    );
}

#[test]
fn distinct_directory_trees_bound_actual_git_children_and_stay_complete() {
    let _guard = PAYLOAD_FIXTURE.lock().unwrap_or_else(|e| e.into_inner());
    let f = Fixture::new();
    for n in 0..1024 {
        let dir = f.0.join(format!("dir{n:04}"));
        fs::create_dir(&dir).unwrap_or_else(|e| panic!("{e}"));
        fs::write(dir.join("file"), format!("distinct {n}\n")).unwrap_or_else(|e| panic!("{e}"));
    }
    f.git(&["add", "."]);
    f.git(&["commit", "-q", "-m", "fixture"]);
    let worktree = crate::git::scan_family(&f.0)
        .unwrap_or_else(|e| panic!("{e}"))
        .families[0]
        .worktrees[0]
        .clone();
    crate::git::take_child_read_journal();
    let summary =
        collect_summary(&worktree, &SummaryLimits::default()).unwrap_or_else(|e| panic!("{e}"));
    let reads = crate::git::take_child_read_journal();
    let object_reads = reads
        .iter()
        .filter(|(_, op)| {
            matches!(
                op,
                GitRead::Tree { .. }
                    | GitRead::Blob { .. }
                    | GitRead::Objects { .. }
                    | GitRead::ObjectInfo { .. }
            )
        })
        .count();
    assert!(
        object_reads <= 40,
        "1024 directory trees spawned {object_reads} object children"
    );
    assert!(
        summary.head_complete && summary.index_complete && summary.disk_complete,
        "{summary:?}"
    );
    assert!(summary.staged.complete && summary.unstaged.complete);
    assert_eq!((summary.staged.observed, summary.unstaged.observed), (0, 0));
}

#[test]
fn summary_metrics_count_payload_reads_and_same_run_digest_reuse() {
    let f = Fixture::new();
    for name in ["a", "b"] {
        fs::write(f.0.join(name), b"shared\n").unwrap_or_else(|e| panic!("{e}"));
    }
    f.git(&["add", "."]);
    f.git(&["commit", "-q", "-m", "fixture"]);
    let worktree = crate::git::scan_family(&f.0)
        .unwrap_or_else(|e| panic!("{e}"))
        .families[0]
        .worktrees[0]
        .clone();
    let payload_bytes = f.git(&["cat-file", "commit", "HEAD"]).len()
        + f.git(&["cat-file", "tree", "HEAD^{tree}"]).len()
        + 7;
    crate::git::take_child_read_journal();
    let outcome = collect_summary_for_review(&worktree, &SummaryLimits::default())
        .unwrap_or_else(|e| panic!("{e}"));
    let reads = crate::git::take_child_read_journal();
    assert!(
        outcome.summary.head_complete
            && outcome.summary.index_complete
            && outcome.summary.disk_complete
    );
    // One commit, one root tree and one shared blob are actual payload reads.
    assert_eq!(outcome.metrics.object_reads, 3);
    assert_eq!(outcome.metrics.object_bytes, payload_bytes as u64);
    assert_eq!(outcome.metrics.git_processes, reads.len() as u64);
    // Two HEAD lookups, two index lookups, two resident-content comparisons.
    assert_eq!(outcome.metrics.cache_hits, 6);
    assert_eq!(outcome.metrics.paths_checked, 2);
}

#[test]
fn thorough_profile_respects_lower_caps_cancellation_and_absolute_ceiling() {
    struct Cancel;
    impl LocalHooks for Cancel {
        fn cancelled(&self) -> bool {
            true
        }
    }
    let f = Fixture::new();
    fs::write(f.0.join("file"), b"resident\n").unwrap_or_else(|e| panic!("{e}"));
    f.git(&["add", "."]);
    f.git(&["commit", "-q", "-m", "fixture"]);
    let worktree = crate::git::scan_family(&f.0)
        .unwrap_or_else(|e| panic!("{e}"))
        .families[0]
        .worktrees[0]
        .clone();
    let limited = collect_summary(
        &worktree,
        &SummaryLimits {
            max_objects: 0,
            ..SummaryLimits::thorough()
        },
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(!limited.head_complete && !limited.staged.complete && !limited.unstaged.complete);
    assert!(!limited.gaps.is_empty());
    crate::git::take_child_read_journal();
    let cancelled = collect_outcome(&worktree, &SummaryLimits::thorough(), &Cancel)
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        !cancelled.summary.head_complete
            && !cancelled.summary.index_complete
            && !cancelled.summary.disk_complete
    );
    assert!(!cancelled.summary.gaps.is_empty());
    assert_eq!(cancelled.metrics.git_processes, 0);
    assert!(crate::git::take_child_read_journal().is_empty());
    for excessive in [
        SummaryLimits {
            max_objects: 65_537,
            ..SummaryLimits::thorough()
        },
        SummaryLimits {
            max_total_bytes: 512 * 1024 * 1024 + 1,
            ..SummaryLimits::thorough()
        },
        SummaryLimits {
            timeout: Duration::from_secs(121),
            ..SummaryLimits::thorough()
        },
    ] {
        let error = match collect_summary(&worktree, &excessive) {
            Err(error) => error,
            Ok(_) => panic!("above-ceiling summary was accepted"),
        };
        assert_eq!(error.code, "local_limits");
    }
}
