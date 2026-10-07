#![cfg(unix)]
use std::{fs, path::PathBuf, process::Command};
use wt_sweep::{
    git::scan_family,
    model::BytePath,
    review::{ProgressObservation, ReviewLimits, enrich_with_limits_and_progress, judge},
};

struct Fixture(PathBuf);
impl Fixture {
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
            .unwrap_or_else(|error| panic!("git: {error}"));
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
fn collected_worktree_facts_arrive_before_deep_checks_and_final_judgement() {
    let f = Fixture(PathBuf::from(format!(
        "/private/tmp/wt-sweep-progress-{}",
        std::process::id()
    )));
    fs::create_dir(&f.0).unwrap_or_else(|error| panic!("fixture: {error}"));
    f.git(&["init", "-q", "-b", "main"]);
    fs::write(f.0.join("tracked"), b"fixture\n").unwrap_or_else(|error| panic!("write: {error}"));
    f.git(&["add", "tracked"]);
    f.git(&["commit", "-q", "-m", "fixture"]);
    let linked = f.0.join("linked");
    let linked_name = linked.to_str().unwrap_or_else(|| panic!("fixture path"));
    f.git(&["worktree", "add", "--quiet", "--detach", linked_name]);
    // Keep the linked worktree out of the main's disk walk.
    let outside = f.0.with_extension("linked");
    f.git(&[
        "worktree",
        "move",
        linked_name,
        outside.to_str().unwrap_or_else(|| panic!("fixture path")),
    ]);
    struct Remove(PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _remove = Remove(outside.clone());
    let inventory = scan_family(&f.0).unwrap_or_else(|error| panic!("scan: {error}"));
    let target = BytePath::from_bytes(b"refs/heads/main");
    let mut events = Vec::new();
    let facts = enrich_with_limits_and_progress(
        &inventory,
        std::slice::from_ref(&target),
        None,
        &ReviewLimits::default(),
        &mut |event| events.push(event),
    )
    .unwrap_or_else(|error| panic!("collect: {error}"));
    let history_done = events
        .iter()
        .position(|event| event.phase == "history_complete")
        .unwrap_or_else(|| panic!("history complete event missing"));
    let deep_started = events
        .iter()
        .position(|event| event.phase == "deep")
        .unwrap_or_else(|| panic!("deep phase event missing"));
    for worktree in &inventory.families[0].worktrees {
        assert!(events[..history_done].iter().any(|event| matches!(event.observation.as_ref(),
            Some(ProgressObservation::History { path, head, target_ref, target_oid, result, complete })
            if path == &worktree.path && head == &worktree.head && target_ref == &target
                && target_oid == &worktree.head && result == "identical_commit" && *complete)),
            "actual history fact missing before completion for {:?}", worktree.path);
        assert!(
            events[..deep_started]
                .iter()
                .any(|event| matches!(event.observation.as_ref(),
            Some(ProgressObservation::Local { path, summary })
            if path == &worktree.path && summary.index_complete && summary.disk_complete
                && summary.staged.complete && summary.unstaged.complete)),
            "actual local fact missing before deep reads for {:?}",
            worktree.path
        );
    }
    let review = judge(&facts).unwrap_or_else(|error| panic!("judge: {error}"));
    assert!(
        review
            .items
            .iter()
            .any(|item| item.path == BytePath::from_path(&outside)
                && item.classification == "cleanup_candidate"),
        "positive final candidate required"
    );
    let missing = BytePath::from_bytes(b"refs/heads/not-present");
    let mut incomplete_events = Vec::new();
    let incomplete = enrich_with_limits_and_progress(
        &inventory,
        std::slice::from_ref(&missing),
        None,
        &ReviewLimits::default(),
        &mut |event| incomplete_events.push(event),
    )
    .unwrap_or_else(|error| panic!("collect missing target: {error}"));
    assert!(
        incomplete_events
            .iter()
            .any(|event| matches!(event.observation.as_ref(),
        Some(ProgressObservation::History { target_ref, target_oid, result, complete, .. })
        if target_ref == &missing && target_oid.is_none() && result == "unknown" && !complete)
                && event.stop_reason.is_some()),
        "refusal must remain a precise unknown observation"
    );
    let refused =
        judge(&incomplete).unwrap_or_else(|error| panic!("judge missing target: {error}"));
    assert!(
        refused
            .items
            .iter()
            .all(|item| item.classification != "cleanup_candidate"),
        "partial progress must not establish a candidate"
    );
}
