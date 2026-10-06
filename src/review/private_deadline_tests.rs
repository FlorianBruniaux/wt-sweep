//! Actual shortened children distinguish private budgets from backend failure.
use super::*;
use std::{
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(format!(
            "/private/tmp/wt-sweep-private-deadline-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap_or_else(|e| panic!("{e}"));
        fs::create_dir(root.join("repo")).unwrap_or_else(|e| panic!("{e}"));
        let fixture = Self(root);
        fixture.git(&["init", "--template=", "-q", "-b", "main"]);
        fs::write(fixture.0.join("repo/file"), b"fixture\n").unwrap_or_else(|e| panic!("{e}"));
        fixture.git(&["add", "file"]);
        fixture.git(&["commit", "-q", "-m", "fixture"]);
        fixture.git(&[
            "worktree",
            "add",
            "--detach",
            fixture
                .0
                .join("linked")
                .to_str()
                .unwrap_or_else(|| panic!("owned path is not UTF-8")),
            "main",
        ]);
        let status = Command::new("/usr/bin/mkfifo")
            .arg(fixture.0.join("fifo"))
            .status()
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(status.success());
        fixture
    }
    fn git(&self, args: &[&str]) {
        let output = Command::new("/usr/bin/git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "core.fsmonitor=false",
                "-C",
            ])
            .arg(self.0.join("repo"))
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(output.status.success(), "{:?}", output.stderr);
    }
    fn block_backend(&self) {
        let config = self.0.join("repo/.git/config");
        let mut raw = fs::read(&config).unwrap_or_else(|e| panic!("{e}"));
        raw.extend_from_slice(
            format!("\n[include]\n path = {}\n", self.0.join("fifo").display()).as_bytes(),
        );
        fs::write(config, raw).unwrap_or_else(|e| panic!("{e}"));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn shortened_private_final_child_preserves_independent_c1_c3_but_leaves_c4_c5_unknown() {
    let f = Fixture::new();
    let inventory = crate::git::scan_family(&f.0.join("repo")).unwrap_or_else(|e| panic!("{e}"));
    let targets = [BytePath::from_bytes(b"refs/heads/main")];
    let mut facts =
        crate::review::enrich(&inventory, &targets, None).unwrap_or_else(|e| panic!("{e}"));
    let linked = BytePath::from_path(&f.0.join("linked"));
    let item = facts
        .items
        .iter()
        .find(|item| item.path == linked)
        .unwrap_or_else(|| panic!("linked item missing"));
    assert!(
        c1(item, &inventory.families[0]) && c2(item) && c3(item),
        "baseline independent evidence missing: {item:?}"
    );
    assert!(
        c4(item) && c5(item, &facts),
        "baseline private evidence missing: {item:?}"
    );
    let bound = FamilyTargets::bind_with(&inventory, TargetInput::Simple(&targets), &NativeHooks)
        .unwrap_or_else(|e| panic!("{e}"));
    let mut history = history::HistoryRun::collect(
        &inventory,
        &bound,
        &Default::default(),
        &BTreeMap::new(),
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let limits = PrivateHistoryLimits::default();
    let mut private =
        PrivateHistoryRun::collect(&inventory, &bound, &limits, &NativeHooks, &BTreeMap::new())
            .unwrap_or_else(|e| panic!("{e}"));
    f.block_backend();
    private.started = Instant::now() - (limits.timeout - Duration::from_millis(500));
    crate::git::take_child_read_journal();
    private
        .finish(&inventory, &BTreeMap::new())
        .unwrap_or_else(|e| panic!("{e}"));
    let reads = crate::git::take_child_read_journal();
    assert_eq!(
        reads.len(),
        1,
        "shortened phase must start one actual Git child and refuse dependent retries: {reads:?}"
    );
    assert!(matches!(reads[0].1, GitRead::RefFormat));
    assert!(
        private.stopped_families.is_empty(),
        "private phase budget quarantined family: {:?}",
        private.stopped_families
    );
    history
        .stopped_families
        .extend(private.stopped_families.clone());
    history.reconcile_stopped(&inventory, &mut facts.items);
    for item in &mut facts.items {
        item.private_history = private.output.histories.remove(&item.path);
    }
    facts.finished_unix_ms = now_ms();
    let item = facts
        .items
        .iter()
        .find(|item| item.path == linked)
        .unwrap_or_else(|| panic!("linked item missing"));
    assert!(
        c1(item, &inventory.families[0]) && c2(item) && c3(item),
        "independent evidence erased: {item:?}"
    );
    assert!(item.history.iter().any(|h| h.result == "identical_commit"));
    assert!(
        !c4(item) && !c5(item, &facts),
        "private phase limit became complete evidence: {item:?}"
    );
    assert!(
        item.private_history.as_ref().is_some_and(
            |h| !h.enumeration_complete && h.roots.iter().all(|r| r.result == "unknown")
        )
    );
}
#[test]
fn full_backend_timeout_still_quarantines_private_family_and_stops_dependent_reads() {
    let f = Fixture::new();
    let inventory = crate::git::scan_family(&f.0.join("repo")).unwrap_or_else(|e| panic!("{e}"));
    let bound = FamilyTargets::bind_with(&inventory, TargetInput::Simple(&[]), &NativeHooks)
        .unwrap_or_else(|e| panic!("{e}"));
    let mut private = PrivateHistoryRun::collect(
        &inventory,
        &bound,
        &Default::default(),
        &NativeHooks,
        &BTreeMap::new(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    f.block_backend();
    private.started = Instant::now();
    crate::git::take_child_read_journal();
    let family = &inventory.families[0];
    let error = private
        .read(family, &f.0.join("repo"), GitRead::RefFormat)
        .err()
        .unwrap_or_else(|| panic!("owned FIFO did not block backend"));
    assert_eq!(error.code, "git_timeout");
    assert!(matches!(
        private.stopped_families.get(&PrivateHistoryRun::id(family)),
        Some(GitRead::RefFormat)
    ));
    assert_eq!(crate::git::take_child_read_journal().len(), 1);
    assert!(
        private
            .read(family, &f.0.join("repo"), GitRead::GitDir)
            .is_err()
    );
    assert!(crate::git::take_child_read_journal().is_empty());
}
