#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use wt_sweep::{git::scan_family, model::BytePath};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = PathBuf::from(format!(
            "/private/tmp/wt-sweep-private-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        let f = Self(p);
        f.git(&["init", "--template=", "-b", "main"]);
        fs::write(f.0.join("file"), "fixture\n").unwrap();
        f.git(&["add", "file"]);
        f.git(&["commit", "-m", "fixture"]);
        f
    }
    fn at(path: &Path, args: &[&str]) -> String {
        let mut child = Command::new("/usr/bin/git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("TMPDIR", "/private/tmp")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_ALLOW_PROTOCOL", "")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "gc.auto=0",
                "-c",
                "maintenance.auto=false",
                "-C",
            ])
            .arg(path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while child.try_wait().unwrap().is_none() {
            if start.elapsed() > Duration::from_secs(15) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("owned fixture Git exceeded deadline");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout)
            .unwrap()
            .trim_end_matches('\n')
            .into()
    }
    fn git(&self, args: &[&str]) -> String {
        Self::at(&self.0, args)
    }
    fn linked(&self, name: &str) -> PathBuf {
        let p = self.0.join(name);
        self.git(&["worktree", "add", "--detach", p.to_str().unwrap(), "main"]);
        p
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn clean_detached_head_is_covered_and_exclusive_old_reflog_root_is_not() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let exclusive = f.git(&[
        "commit-tree",
        &tree,
        "-p",
        &base,
        "-m",
        "exclusive same tree",
    ]);
    Fixture::at(&linked, &["reset", "--hard", &exclusive]);
    Fixture::at(&linked, &["reset", "--hard", &base]);
    let inventory = scan_family(&f.0).unwrap();
    let facts = wt_sweep::review::enrich(&inventory, &[], None).unwrap();
    let history = facts
        .items
        .iter()
        .find(|i| i.path == BytePath::from_path(&linked))
        .unwrap()
        .private_history
        .as_ref()
        .expect("private history was not collected");
    assert!(history.enumeration_complete, "{:?}", history.gaps);
    assert_eq!(
        history.roots.iter().find(|r| r.oid == base).unwrap().result,
        "covered"
    );
    assert_eq!(
        history
            .roots
            .iter()
            .find(|r| r.oid == exclusive)
            .unwrap()
            .result,
        "not_preserved_by_retained_refs"
    );
    assert_eq!(
        history
            .roots
            .iter()
            .find(|r| r.oid == base)
            .unwrap()
            .anchor
            .as_ref()
            .unwrap()
            .reference,
        BytePath::from_bytes(b"refs/heads/main")
    );
}
#[test]
fn shared_stashes_have_exact_membership_and_verified_tracked_and_untracked_paths() {
    let f = Fixture::new();
    f.linked("linked");
    let base = f.git(&["rev-parse", "HEAD"]);
    fs::write(f.0.join("file"), "first stash\n").unwrap();
    f.git(&["stash", "push", "-m", "first synthetic"]);
    let older = f.git(&["rev-parse", "refs/stash"]);
    fs::write(f.0.join("file"), "second stash\n").unwrap();
    fs::write(f.0.join("untracked\nfile"), "synthetic\n").unwrap();
    f.git(&[
        "stash",
        "push",
        "--include-untracked",
        "-m",
        "second synthetic",
    ]);
    let newest = f.git(&["rev-parse", "refs/stash"]);
    let facts = wt_sweep::review::enrich(&scan_family(&f.0).unwrap(), &[], None).unwrap();
    assert_eq!(facts.shared_stashes.len(), 1);
    let stashes = &facts.shared_stashes[0];
    assert!(stashes.enumeration_complete, "{:?}", stashes.gaps);
    assert_eq!(
        stashes
            .entries
            .iter()
            .map(|s| s.oid.as_str())
            .collect::<Vec<_>>(),
        vec![newest.as_str(), older.as_str()]
    );
    for stash in &stashes.entries {
        assert_eq!(stash.base_oid.as_deref(), Some(base.as_str()));
        assert!(stash.metadata_complete, "{:?}", stash.gaps);
        assert!(stash.files_complete, "{:?}", stash.gaps);
        assert!(stash.timestamp_unix.is_some());
        assert!(stash.message.is_some());
        assert!(stash.files.contains(&BytePath::from_bytes(b"file")));
    }
    assert!(
        stashes.entries[0]
            .files
            .contains(&BytePath::from_bytes(b"untracked\nfile"))
    );
}
use wt_sweep::{
    local::LocalHooks,
    review::private::{
        PrivateHistoryHooks, PrivateHistoryLimits, collect_with, collect_with_limits,
    },
};
struct FinalMutation {
    path: PathBuf,
    replacement: Vec<u8>,
}
impl LocalHooks for FinalMutation {}
impl PrivateHistoryHooks for FinalMutation {
    fn before_final_validation(&self) -> std::io::Result<()> {
        fs::write(&self.path, &self.replacement)
    }
}
#[test]
fn unrelated_anchor_failure_does_not_erase_a_fresh_covering_anchor() {
    let f = Fixture::new();
    f.git(&["branch", "unrelated"]);
    let linked = f.linked("linked");
    let hook = FinalMutation {
        path: f.0.join(".git/refs/heads/unrelated"),
        replacement: b"malformed\n".to_vec(),
    };
    let facts = collect_with(&scan_family(&f.0).unwrap(), &[], &Default::default(), &hook).unwrap();
    let h = &facts.histories[&BytePath::from_path(&linked)];
    assert!(!h.anchors_complete);
    assert_eq!(h.roots[0].result, "covered", "{:?}", h.gaps);
    assert_eq!(
        h.roots[0].anchor.as_ref().unwrap().reference,
        BytePath::from_bytes(b"refs/heads/main")
    );
}
#[test]
fn shared_stash_membership_is_invalidated_when_its_own_source_changes() {
    let f = Fixture::new();
    fs::write(f.0.join("file"), "stash\n").unwrap();
    f.git(&["stash", "push", "-m", "synthetic"]);
    let hook = FinalMutation {
        path: f.0.join(".git/logs/refs/stash"),
        replacement: b"malformed\n".to_vec(),
    };
    let facts = collect_with(&scan_family(&f.0).unwrap(), &[], &Default::default(), &hook).unwrap();
    let s = &facts.shared_stashes[0];
    assert!(!s.enumeration_complete);
    assert_eq!(s.temporal_consistency, "change_observed");
    assert_eq!(s.entries.len(), 1, "retain observed stash members");
    assert!(
        facts
            .histories
            .values()
            .all(|h| h.roots.iter().all(|r| r.result == "covered"))
    );
}
#[test]
fn private_live_and_orphan_own_logs_are_not_anchors_even_with_another_detached_head() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let other = f.linked("other");
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let live = f.git(&["commit-tree", &tree, "-p", &base, "-m", "private live"]);
    let orphan = f.git(&["commit-tree", &tree, "-p", &base, "-m", "orphan old"]);
    let dir = PathBuf::from(Fixture::at(&linked, &["rev-parse", "--absolute-git-dir"]));
    fs::create_dir_all(dir.join("refs/worktree")).unwrap();
    fs::write(dir.join("refs/worktree/exclusive"), format!("{live}\n")).unwrap();
    fs::create_dir_all(dir.join("logs/refs/worktree")).unwrap();
    let line = format!(
        "{orphan} {base} Fixture <fixture@example.invalid> 1700000000 +0000\torphan synthetic\n"
    );
    fs::write(dir.join("logs/refs/worktree/deleted"), &line).unwrap();
    fs::write(dir.join("logs/ORIG_HEAD"), &line).unwrap();
    Fixture::at(&other, &["reset", "--hard", &live]);
    let inventory = scan_family(&f.0).unwrap();
    let hostile = [BytePath::from_bytes(b"refs/worktree/exclusive")];
    let refusal = collect_with_limits(&inventory, &hostile, &Default::default())
        .err()
        .expect("private namespace must refuse as an explicit integration target");
    assert!(
        refusal.message.contains("target") || refusal.message.contains("ref"),
        "refusal must identify the target/ref contract: {refusal}"
    );
    let facts = collect_with_limits(&inventory, &[], &Default::default()).unwrap();
    let h = &facts.histories[&BytePath::from_path(&linked)];
    assert!(h.enumeration_complete, "{:?}", h.gaps);
    for oid in [live, orphan] {
        let root = h.roots.iter().find(|r| r.oid == oid).unwrap();
        assert_eq!(root.result, "not_preserved_by_retained_refs");
    }
    assert!(h.roots.iter().any(|r| {
        r.provenance
            .iter()
            .any(|p| p.source == BytePath::from_path(&dir.join("logs/ORIG_HEAD")))
    }));
}
#[test]
fn lightweight_commit_tag_covers_a_detached_root_and_annotated_tag_is_unknown() {
    let f = Fixture::new();
    f.git(&["checkout", "--detach"]);
    f.git(&["tag", "retained"]);
    f.git(&["branch", "-D", "main"]);
    let inventory = scan_family(&f.0).unwrap();
    let positive = collect_with_limits(&inventory, &[], &Default::default()).unwrap();
    let h = &positive.histories[&BytePath::from_path(&f.0)];
    assert_eq!(h.roots[0].result, "covered");
    assert_eq!(
        h.roots[0].anchor.as_ref().unwrap().reference,
        BytePath::from_bytes(b"refs/tags/retained")
    );
    f.git(&["tag", "-d", "retained"]);
    f.git(&["tag", "-a", "unsupported", "-m", "synthetic annotated"]);
    let negative =
        collect_with_limits(&scan_family(&f.0).unwrap(), &[], &Default::default()).unwrap();
    let h = &negative.histories[&BytePath::from_path(&f.0)];
    assert!(!h.anchors_complete);
    assert!(h.roots.iter().all(|r| r.result == "unknown"));
}
#[test]
fn complete_empty_anchor_set_is_a_real_negative() {
    let f = Fixture::new();
    f.git(&["checkout", "--detach"]);
    f.git(&["branch", "-D", "main"]);
    let facts = collect_with_limits(&scan_family(&f.0).unwrap(), &[], &Default::default()).unwrap();
    let h = &facts.histories[&BytePath::from_path(&f.0)];
    assert!(h.enumeration_complete && h.anchors_complete, "{:?}", h.gaps);
    assert!(!h.roots.is_empty());
    assert!(
        h.roots
            .iter()
            .all(|r| r.result == "not_preserved_by_retained_refs")
    );
}
struct Refusal {
    path: PathBuf,
}
impl LocalHooks for Refusal {
    fn before_entry(&self, p: &BytePath) -> std::io::Result<()> {
        if *p == BytePath::from_path(&self.path) {
            Err(std::io::Error::from_raw_os_error(13))
        } else {
            Ok(())
        }
    }
}
impl PrivateHistoryHooks for Refusal {}
#[test]
fn refused_source_is_unknown_while_verified_absence_and_content_hash_are_distinct() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let linked = f.linked("linked");
    let dir = PathBuf::from(Fixture::at(&linked, &["rev-parse", "--absolute-git-dir"]));
    let log = dir.join("logs/HEAD");
    let bytes = fs::read(&log).unwrap();
    let positive =
        collect_with_limits(&scan_family(&f.0).unwrap(), &[], &Default::default()).unwrap();
    let h = &positive.histories[&BytePath::from_path(&linked)];
    let source = h
        .sources
        .iter()
        .find(|s| s.path == BytePath::from_path(&log))
        .unwrap();
    assert_eq!(
        source.sha256.as_deref(),
        Some(format!("{:x}", Sha256::digest(&bytes)).as_str())
    );
    assert!(
        h.sources
            .iter()
            .any(|s| s.exists == Some(false) && s.complete)
    );
    let refused = collect_with(
        &scan_family(&f.0).unwrap(),
        &[],
        &Default::default(),
        &Refusal { path: log.clone() },
    )
    .unwrap();
    let h = &refused.histories[&BytePath::from_path(&linked)];
    assert!(!h.enumeration_complete);
    let source = h
        .sources
        .iter()
        .find(|s| s.path == BytePath::from_path(&log))
        .unwrap();
    assert_eq!(source.exists, None);
    assert!(!source.complete);
    assert!(h.gaps.iter().any(|g| g.contains("Permission denied")));
}
#[test]
fn private_same_size_ref_rewrite_invalidates_current_results_but_retains_observations() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let dir = PathBuf::from(Fixture::at(&linked, &["rev-parse", "--absolute-git-dir"]));
    fs::create_dir_all(dir.join("refs/worktree")).unwrap();
    let path = dir.join("refs/worktree/exclusive");
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let exclusive = f.git(&["commit-tree", &tree, "-p", &base, "-m", "exclusive"]);
    fs::write(&path, format!("{exclusive}\n")).unwrap();
    let facts = collect_with(
        &scan_family(&f.0).unwrap(),
        &[],
        &Default::default(),
        &FinalMutation {
            path,
            replacement: format!("{base}\n").into_bytes(),
        },
    )
    .unwrap();
    let h = &facts.histories[&BytePath::from_path(&linked)];
    assert!(!h.enumeration_complete);
    assert_eq!(h.temporal_consistency, "change_observed");
    let root = h.roots.iter().find(|r| r.oid == exclusive).unwrap();
    assert_eq!(root.result, "unknown");
    assert_eq!(
        root.observed_result.as_deref(),
        Some("not_preserved_by_retained_refs")
    );
}
#[test]
fn missing_and_noncommit_roots_do_not_hide_an_independent_known_negative() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let dir = PathBuf::from(Fixture::at(&linked, &["rev-parse", "--absolute-git-dir"]));
    fs::create_dir_all(dir.join("refs/worktree")).unwrap();
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let exclusive = f.git(&["commit-tree", &tree, "-p", &base, "-m", "exclusive"]);
    let missing = "1234567890123456789012345678901234567890";
    fs::write(dir.join("AUTO_MERGE"), format!("{tree}\n")).unwrap();
    fs::write(
        dir.join("refs/worktree/exclusive"),
        format!("{exclusive}\n"),
    )
    .unwrap();
    fs::write(dir.join("refs/worktree/missing"), format!("{missing}\n")).unwrap();
    let facts = collect_with_limits(&scan_family(&f.0).unwrap(), &[], &Default::default()).unwrap();
    let h = &facts.histories[&BytePath::from_path(&linked)];
    assert_eq!(
        h.roots.iter().find(|r| r.oid == exclusive).unwrap().result,
        "not_preserved_by_retained_refs",
        "{h:?}"
    );
    for oid in [tree.as_str(), missing] {
        assert_eq!(
            h.roots.iter().find(|r| r.oid == oid).unwrap().result,
            "unknown"
        );
    }
}
#[test]
fn symlink_fifo_and_oversized_private_sources_never_become_empty_complete_enumerations() {
    for kind in ["symlink", "fifo", "oversized"] {
        let f = Fixture::new();
        let linked = f.linked("linked");
        let dir = PathBuf::from(Fixture::at(&linked, &["rev-parse", "--absolute-git-dir"]));
        fs::create_dir_all(dir.join("refs/worktree")).unwrap();
        let path = dir.join("refs/worktree/refusal");
        match kind {
            "symlink" => {
                std::os::unix::fs::symlink(f.0.join(".git/refs/heads/main"), &path).unwrap()
            }
            "fifo" => {
                let out = Command::new("/usr/bin/mkfifo").arg(&path).status().unwrap();
                assert!(out.success());
            }
            _ => fs::write(&path, vec![b'x'; 1024]).unwrap(),
        }
        let limits = PrivateHistoryLimits {
            file_bytes: 512,
            ..Default::default()
        };
        let facts = collect_with_limits(&scan_family(&f.0).unwrap(), &[], &limits).unwrap();
        let h = &facts.histories[&BytePath::from_path(&linked)];
        assert!(!h.enumeration_complete, "{kind}");
        assert!(
            h.sources
                .iter()
                .any(|s| s.path == BytePath::from_path(&path) && !s.complete)
        );
    }
}
#[test]
fn malformed_reflog_retains_valid_independent_roots_already_observed() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let dir = PathBuf::from(Fixture::at(&linked, &["rev-parse", "--absolute-git-dir"]));
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let exclusive = f.git(&["commit-tree", &tree, "-p", &base, "-m", "exclusive"]);
    fs::write(dir.join("logs/ORIG_HEAD"),format!("{exclusive} {base} Fixture <fixture@example.invalid> 1700000000 +0000\tvalid\nmalformed\n")).unwrap();
    let facts = collect_with_limits(&scan_family(&f.0).unwrap(), &[], &Default::default()).unwrap();
    let h = &facts.histories[&BytePath::from_path(&linked)];
    assert!(!h.enumeration_complete);
    assert_eq!(
        h.roots
            .iter()
            .find(|r| r.oid == exclusive)
            .expect("valid reflog root discarded")
            .result,
        "not_preserved_by_retained_refs"
    );
}
#[test]
fn stash_path_cap_keeps_membership_and_observed_paths_separate_from_detail_completeness() {
    let f = Fixture::new();
    fs::write(f.0.join("file"), "changed\n").unwrap();
    fs::write(f.0.join("new"), "untracked\n").unwrap();
    f.git(&["stash", "push", "-u", "-m", "synthetic"]);
    let limits = PrivateHistoryLimits {
        stash_paths: 1,
        ..Default::default()
    };
    let facts = collect_with_limits(&scan_family(&f.0).unwrap(), &[], &limits).unwrap();
    let s = &facts.shared_stashes[0];
    assert!(s.enumeration_complete);
    assert_eq!(s.entries.len(), 1);
    assert!(s.entries[0].metadata_complete);
    assert!(!s.entries[0].files_complete);
    assert_eq!(
        s.entries[0].files.len(),
        1,
        "observed path discarded under cap"
    );
}
#[test]
fn output_and_root_caps_leave_explicit_incompleteness_and_never_exceed_output_limit() {
    let f = Fixture::new();
    let inventory = scan_family(&f.0).unwrap();
    let limits = PrivateHistoryLimits {
        output_bytes: 256,
        ..Default::default()
    };
    let facts = collect_with_limits(&inventory, &[], &limits).unwrap();
    let encoded = serde_json::to_vec(&(
        facts.histories.values().collect::<Vec<_>>(),
        &facts.shared_stashes,
    ))
    .unwrap();
    assert!(
        encoded.len() <= limits.output_bytes,
        "{} bytes exceeds {}",
        encoded.len(),
        limits.output_bytes
    );
    assert!(facts.histories.values().all(|h| !h.enumeration_complete));
    let invalid = PrivateHistoryLimits {
        object_reads: 0,
        ..Default::default()
    };
    assert!(collect_with_limits(&inventory, &[], &invalid).is_err());
}
#[test]
fn initial_private_source_refusal_keeps_independently_verified_head_coverage_current() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let dir = PathBuf::from(Fixture::at(&linked, &["rev-parse", "--absolute-git-dir"]));
    fs::create_dir_all(dir.join("refs/worktree")).unwrap();
    let path = dir.join("refs/worktree/refused");
    fs::write(&path, b"not readable in this fixture\n").unwrap();
    let facts = collect_with(
        &scan_family(&f.0).unwrap(),
        &[],
        &Default::default(),
        &Refusal { path },
    )
    .unwrap();
    let h = &facts.histories[&BytePath::from_path(&linked)];
    assert!(!h.enumeration_complete);
    let base = f.git(&["rev-parse", "HEAD"]);
    let root = h.roots.iter().find(|r| r.oid == base).unwrap();
    assert_eq!(root.result, "covered");
    assert!(root.observed_result.is_none());
}
#[test]
fn orphan_unknown_own_log_name_is_visible_and_cannot_certify_complete_enumeration() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let dir = PathBuf::from(Fixture::at(&linked, &["rev-parse", "--absolute-git-dir"]));
    fs::write(
        dir.join("logs/.unexpected"),
        b"synthetic unsupported own log\n",
    )
    .unwrap();
    let facts = collect_with_limits(&scan_family(&f.0).unwrap(), &[], &Default::default()).unwrap();
    let h = &facts.histories[&BytePath::from_path(&linked)];
    assert!(!h.enumeration_complete);
    assert!(
        h.sources
            .iter()
            .any(|s| s.path == BytePath::from_path(&dir.join("logs/.unexpected")))
    );
}
#[test]
fn root_and_graph_caps_preserve_positive_identity_without_inventing_a_negative() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let dir = PathBuf::from(Fixture::at(&linked, &["rev-parse", "--absolute-git-dir"]));
    fs::create_dir_all(dir.join("refs/worktree")).unwrap();
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let exclusive = f.git(&["commit-tree", &tree, "-p", &base, "-m", "exclusive"]);
    let child = f.git(&["commit-tree", &tree, "-p", &base, "-m", "retained child"]);
    f.git(&["branch", "descendant", &child]);
    fs::write(
        dir.join("refs/worktree/exclusive"),
        format!("{exclusive}\n"),
    )
    .unwrap();
    let inventory = scan_family(&f.0).unwrap();
    let limited = collect_with_limits(
        &inventory,
        &[],
        &PrivateHistoryLimits {
            roots: 1,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!limited.histories[&BytePath::from_path(&linked)].enumeration_complete);
    let shallow = collect_with_limits(
        &inventory,
        &[],
        &PrivateHistoryLimits {
            per_root_steps: 1,
            ..Default::default()
        },
    )
    .unwrap();
    let h = &shallow.histories[&BytePath::from_path(&linked)];
    assert_eq!(
        h.roots.iter().find(|r| r.oid == base).unwrap().result,
        "covered"
    );
    assert_eq!(
        h.roots.iter().find(|r| r.oid == exclusive).unwrap().result,
        "unknown"
    );
}
#[test]
fn unreadable_stash_object_keeps_complete_membership_and_independent_head_coverage() {
    let f = Fixture::new();
    fs::write(f.0.join("file"), "changed\n").unwrap();
    f.git(&["stash", "push", "-m", "synthetic"]);
    let oid = f.git(&["rev-parse", "refs/stash"]);
    let inventory = scan_family(&f.0).unwrap();
    fs::remove_file(f.0.join(".git/objects").join(&oid[..2]).join(&oid[2..])).unwrap();
    let facts = collect_with_limits(&inventory, &[], &Default::default()).unwrap();
    assert!(facts.shared_stashes[0].enumeration_complete);
    assert_eq!(facts.shared_stashes[0].entries[0].oid, oid);
    assert!(!facts.shared_stashes[0].entries[0].metadata_complete);
    assert!(
        facts
            .histories
            .values()
            .all(|h| h.roots.iter().all(|r| r.result == "covered"))
    );
}
#[test]
fn private_packed_records_are_explicitly_unsupported_instead_of_silently_omitted() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let exclusive = f.git(&["commit-tree", &tree, "-p", &base, "-m", "packed private"]);
    fs::write(
        f.0.join(".git/packed-refs"),
        format!("# pack-refs with: peeled fully-peeled sorted\n{exclusive} refs/worktree/packed\n"),
    )
    .unwrap();
    let facts = collect_with_limits(&scan_family(&f.0).unwrap(), &[], &Default::default()).unwrap();
    let h = &facts.histories[&BytePath::from_path(&linked)];
    assert!(!h.enumeration_complete);
    assert!(h.gaps.iter().any(|g| g.contains("packed private")));
}
#[test]
fn a_timeout_in_private_final_checks_invalidates_graph_and_deep_claims_and_continues_other_families()
 {
    let blocked = Fixture::new();
    let linked = blocked.linked("linked");
    let healthy = Fixture::new();
    let mut inventory = scan_family(&blocked.0).unwrap();
    inventory
        .families
        .extend(scan_family(&healthy.0).unwrap().families);
    let config = blocked.0.join(".git/config");
    let mut injected = false;
    let target_map = wt_sweep::model::TargetMap {
        schema: wt_sweep::model::TARGET_MAP_SCHEMA.into(),
        families: inventory
            .families
            .iter()
            .map(|family| wt_sweep::model::TargetMapFamily {
                common_dir: family.common_dir.clone(),
                identity: family.identity.clone(),
                targets: vec![BytePath::from_bytes(b"refs/heads/main")],
            })
            .collect(),
    };
    let facts = wt_sweep::review::enrich_with_target_input_and_controls(
        &inventory,
        wt_sweep::review::targets::TargetInput::Map(&target_map),
        None,
        &Default::default(),
        &Default::default(),
        &wt_sweep::local::NativeHooks,
        &mut |event| {
            if event.phase == "private_revalidate" {
                fs::remove_file(&config).unwrap();
                make_fifo(&config);
                injected = true;
            }
        },
    )
    .unwrap();
    assert!(injected);
    for path in [&blocked.0, &linked] {
        let item = facts
            .items
            .iter()
            .find(|i| i.path == BytePath::from_path(path))
            .unwrap();
        assert!(item.observations.is_empty());
        assert!(
            item.history
                .iter()
                .all(|h| h.result == "unknown" && h.observed_result.is_some())
        );
        assert!(
            item.local_summary.as_ref().unwrap().index_complete,
            "independent index interpretation erased"
        );
        let h = item.private_history.as_ref().unwrap();
        assert!(
            h.roots
                .iter()
                .all(|r| r.result == "unknown" && r.observed_result.as_deref() == Some("covered"))
        );
        assert!(
            h.gaps.iter().any(|g| g.contains("RefFormat")),
            "{:?}",
            h.gaps
        );
    }
    let healthy_item = facts
        .items
        .iter()
        .find(|i| i.path == BytePath::from_path(&healthy.0))
        .unwrap();
    assert!(
        healthy_item
            .private_history
            .as_ref()
            .unwrap()
            .roots
            .iter()
            .all(|r| r.result == "covered")
    );
    assert_eq!(healthy_item.history[0].result, "identical_commit");
    assert_eq!(facts.history_metrics.completed_targets, 1);
    assert_eq!(facts.history_metrics.incomplete_targets, 2);
}

fn make_fifo(path: &Path) {
    let mut child = Command::new("/usr/bin/mkfifo")
        .env_clear()
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if start.elapsed() > Duration::from_secs(15) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("owned FIFO fixture child exceeded deadline");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
struct ErrorTextChanged {
    path: PathBuf,
    final_only: bool,
    final_phase: std::sync::atomic::AtomicBool,
}
impl LocalHooks for ErrorTextChanged {
    fn before_entry(&self, p: &BytePath) -> std::io::Result<()> {
        if *p == BytePath::from_path(&self.path)
            && (!self.final_only || self.final_phase.load(Ordering::SeqCst))
        {
            Err(std::io::Error::other(
                "EIO: changed is only an error message",
            ))
        } else {
            Ok(())
        }
    }
}
impl PrivateHistoryHooks for ErrorTextChanged {
    fn before_final_validation(&self) -> std::io::Result<()> {
        self.final_phase.store(true, Ordering::SeqCst);
        Ok(())
    }
}
#[test]
fn changed_in_an_error_message_never_claims_observed_source_mutation() {
    for final_only in [false, true] {
        let f = Fixture::new();
        let linked = f.linked("linked");
        let dir = PathBuf::from(Fixture::at(&linked, &["rev-parse", "--absolute-git-dir"]));
        let hook = ErrorTextChanged {
            path: dir.join("logs/HEAD"),
            final_only,
            final_phase: std::sync::atomic::AtomicBool::new(false),
        };
        let facts =
            collect_with(&scan_family(&f.0).unwrap(), &[], &Default::default(), &hook).unwrap();
        let h = &facts.histories[&BytePath::from_path(&linked)];
        assert!(!h.enumeration_complete);
        assert_eq!(h.temporal_consistency, "not_guaranteed");
    }
}
#[test]
fn packed_stash_source_is_part_of_final_membership_validation() {
    let f = Fixture::new();
    fs::write(f.0.join("file"), "changed\n").unwrap();
    f.git(&["stash", "push", "-m", "synthetic"]);
    let stash = f.git(&["rev-parse", "refs/stash"]);
    let base = f.git(&["rev-parse", "HEAD"]);
    let packed = f.0.join(".git/packed-refs");
    fs::write(
        &packed,
        format!("# pack-refs with: peeled fully-peeled sorted\n{stash} refs/stash\n"),
    )
    .unwrap();
    fs::remove_file(f.0.join(".git/refs/stash")).unwrap();
    let facts = collect_with(
        &scan_family(&f.0).unwrap(),
        &[],
        &Default::default(),
        &FinalMutation {
            path: packed,
            replacement: format!(
                "# pack-refs with: peeled fully-peeled sorted\n{base} refs/stash\n"
            )
            .into_bytes(),
        },
    )
    .unwrap();
    assert!(!facts.shared_stashes[0].enumeration_complete);
    assert_eq!(facts.shared_stashes[0].entries[0].oid, stash);
}
#[test]
fn malformed_peeled_packed_record_blocks_a_complete_negative_but_keeps_a_valid_positive() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let dir = PathBuf::from(Fixture::at(&linked, &["rev-parse", "--absolute-git-dir"]));
    fs::create_dir_all(dir.join("refs/worktree")).unwrap();
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let exclusive = f.git(&["commit-tree", &tree, "-p", &base, "-m", "exclusive"]);
    fs::write(
        dir.join("refs/worktree/exclusive"),
        format!("{exclusive}\n"),
    )
    .unwrap();
    let inventory = scan_family(&f.0).unwrap();
    fs::write(f.0.join(".git/packed-refs"),format!("# pack-refs with: peeled fully-peeled sorted\n{base} refs/heads/retained\n^not-a-full-object-id\n")).unwrap();
    let facts = collect_with_limits(&inventory, &[], &Default::default()).unwrap();
    let h = &facts.histories[&BytePath::from_path(&linked)];
    assert!(!h.anchors_complete);
    assert_eq!(
        h.roots.iter().find(|r| r.oid == exclusive).unwrap().result,
        "unknown"
    );
    assert_eq!(
        h.roots.iter().find(|r| r.oid == base).unwrap().result,
        "covered"
    );
}
#[test]
fn distinct_empty_directory_sources_have_identity_bound_digests() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let dir = PathBuf::from(Fixture::at(&linked, &["rev-parse", "--absolute-git-dir"]));
    let a = dir.join("refs/bisect");
    let b = dir.join("refs/worktree");
    fs::create_dir_all(&a).unwrap();
    fs::create_dir_all(&b).unwrap();
    let facts = collect_with_limits(&scan_family(&f.0).unwrap(), &[], &Default::default()).unwrap();
    let h = &facts.histories[&BytePath::from_path(&linked)];
    let a = h
        .sources
        .iter()
        .find(|s| s.path == BytePath::from_path(&a))
        .unwrap();
    let b = h
        .sources
        .iter()
        .find(|s| s.path == BytePath::from_path(&b))
        .unwrap();
    assert!(a.complete && b.complete);
    assert_ne!(
        a.sha256, b.sha256,
        "empty directory identity was omitted from source digest"
    );
}

#[test]
fn packed_anchor_requires_unchanged_proven_absence_of_loose_override() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let exclusive = f.git(&["commit-tree", &tree, "-p", &base, "-m", "packed archive"]);
    Fixture::at(&linked, &["reset", "--hard", &exclusive]);
    fs::write(
        f.0.join(".git/packed-refs"),
        format!("{exclusive} refs/heads/archive\n"),
    )
    .unwrap();
    let facts = collect_with(
        &scan_family(&f.0).unwrap(),
        &[],
        &Default::default(),
        &FinalMutation {
            path: f.0.join(".git/refs/heads/archive"),
            replacement: format!("{base}\n").into_bytes(),
        },
    )
    .unwrap();
    let h = &facts.histories[&BytePath::from_path(&linked)];
    let root = h.roots.iter().find(|r| r.oid == exclusive).unwrap();
    assert_eq!(root.result, "unknown");
    assert_eq!(root.observed_result.as_deref(), Some("covered"));
    assert_eq!(
        h.roots.iter().find(|r| r.oid == base).unwrap().result,
        "covered"
    );
}
#[test]
fn unreadable_loose_override_cannot_select_old_packed_anchor() {
    for mode in ["symlink", "malformed"] {
        let f = Fixture::new();
        let linked = f.linked("linked");
        let base = f.git(&["rev-parse", "HEAD"]);
        let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
        let exclusive = f.git(&["commit-tree", &tree, "-p", &base, "-m", "archive"]);
        Fixture::at(&linked, &["reset", "--hard", &exclusive]);
        let inventory = scan_family(&f.0).unwrap();
        fs::write(
            f.0.join(".git/packed-refs"),
            format!("{exclusive} refs/heads/archive\n"),
        )
        .unwrap();
        if mode == "symlink" {
            std::os::unix::fs::symlink("missing", f.0.join(".git/refs/heads/archive")).unwrap();
        } else {
            fs::write(
                f.0.join(".git/refs/heads/archive"),
                b"not-a-full-object-id\n",
            )
            .unwrap();
        }
        let facts = collect_with_limits(&inventory, &[], &Default::default()).unwrap();
        let h = &facts.histories[&BytePath::from_path(&linked)];
        assert_eq!(
            h.roots.iter().find(|r| r.oid == exclusive).unwrap().result,
            "unknown"
        );
        assert_eq!(
            h.roots.iter().find(|r| r.oid == base).unwrap().result,
            "covered"
        );
    }
}

#[test]
fn incomplete_packed_source_never_certifies_empty_stashes() {
    for mode in ["symlink", "malformed", "truncated"] {
        let f = Fixture::new();
        let base = f.git(&["rev-parse", "HEAD"]);
        let inventory = scan_family(&f.0).unwrap();
        let path = f.0.join(".git/packed-refs");
        match mode {
            "symlink" => std::os::unix::fs::symlink("missing", &path).unwrap(),
            "malformed" => fs::write(&path, format!("# pack-refs with: peeled fully-peeled sorted\n{base} refs/heads/archive\n^not-a-full-object-id\n")).unwrap(),
            _ => fs::write(&path, format!("{base} refs/heads/archive")).unwrap(),
        }
        let facts = collect_with_limits(&inventory, &[], &Default::default()).unwrap();
        assert!(!facts.shared_stashes[0].enumeration_complete, "{mode}");
        assert!(facts.shared_stashes[0].entries.is_empty());
        let h = &facts.histories[&BytePath::from_path(&f.0)];
        let root = h.roots.iter().find(|r| r.oid == base).unwrap();
        if mode != "truncated" {
            assert_eq!(root.result, "covered", "{mode}: {h:?}");
        } else {
            assert!(
                root.result == "covered"
                    || root
                        .reason
                        .as_ref()
                        .is_some_and(|r| r.contains("unavailable"))
            );
        }
    }
}
#[test]
fn deep_intermediate_timeout_quarantines_family_before_private_collection() {
    let blocked = Fixture::new();
    let linked = blocked.0.join("linked-t05");
    blocked.git(&[
        "worktree",
        "add",
        "-b",
        "topic-t05",
        linked.to_str().unwrap(),
    ]);
    let healthy = Fixture::new();
    let base = blocked.git(&["rev-parse", "HEAD"]);
    let tree = blocked.git(&["rev-parse", "HEAD^{tree}"]);
    let exclusive = blocked.git(&[
        "commit-tree",
        &tree,
        "-p",
        &base,
        "-m",
        "blocked intermediate",
    ]);
    blocked.git(&["update-ref", "refs/heads/zz_blocked", &exclusive]);
    let mut inventory = scan_family(&blocked.0).unwrap();
    inventory
        .families
        .extend(scan_family(&healthy.0).unwrap().families);
    let path = blocked
        .0
        .join(".git/objects")
        .join(&exclusive[..2])
        .join(&exclusive[2..]);
    let raw = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    make_fifo(&path);
    let mut restored = false;
    let target_map = wt_sweep::model::TargetMap {
        schema: wt_sweep::model::TARGET_MAP_SCHEMA.into(),
        families: inventory
            .families
            .iter()
            .map(|family| wt_sweep::model::TargetMapFamily {
                common_dir: family.common_dir.clone(),
                identity: family.identity.clone(),
                targets: vec![BytePath::from_bytes(b"refs/heads/main")],
            })
            .collect(),
    };
    let facts = wt_sweep::review::enrich_with_target_input_and_controls(
        &inventory,
        wt_sweep::review::targets::TargetInput::Map(&target_map),
        None,
        &Default::default(),
        &Default::default(),
        &wt_sweep::local::NativeHooks,
        &mut |event| {
            if event.phase == "private_history" {
                fs::remove_file(&path).unwrap();
                fs::write(&path, &raw).unwrap();
                restored = true;
            }
        },
    )
    .unwrap();
    assert!(restored);
    let item = facts
        .items
        .iter()
        .find(|i| i.path == BytePath::from_path(&blocked.0))
        .unwrap();
    assert!(
        !item.private_history.as_ref().unwrap().enumeration_complete,
        "deep timeout was retried as a fresh private history"
    );
    assert!(item.observations.is_empty());
    assert!(item.local_summary.as_ref().unwrap().index_complete);
    let good = facts
        .items
        .iter()
        .find(|i| i.path == BytePath::from_path(&healthy.0))
        .unwrap();
    assert!(good.private_history.as_ref().unwrap().enumeration_complete);
    assert_eq!(good.history[0].result, "identical_commit");
}

#[test]
fn refused_packed_stash_source_retains_independent_roots_and_observed_members() {
    for keep_log in [false, true] {
        let f = Fixture::new();
        let base = f.git(&["rev-parse", "HEAD"]);
        fs::write(f.0.join("file"), "packed stash\n").unwrap();
        f.git(&["stash", "push", "-m", "packed synthetic"]);
        let stash = f.git(&["rev-parse", "refs/stash"]);
        let inventory = scan_family(&f.0).unwrap();
        fs::write(
            f.0.join(".git/packed-refs"),
            format!("{stash} refs/stash\n"),
        )
        .unwrap();
        fs::remove_file(f.0.join(".git/refs/stash")).unwrap();
        if !keep_log {
            fs::remove_file(f.0.join(".git/logs/refs/stash")).unwrap();
        }
        let facts = collect_with(
            &inventory,
            &[],
            &Default::default(),
            &Refusal {
                path: f.0.join(".git/packed-refs"),
            },
        )
        .unwrap();
        let stashes = &facts.shared_stashes[0];
        assert!(!stashes.enumeration_complete);
        assert!(stashes.gaps.iter().any(|g| g.contains("packed")));
        if keep_log {
            assert_eq!(stashes.entries[0].oid, stash);
        } else {
            assert!(stashes.entries.is_empty());
        }
        let h = &facts.histories[&BytePath::from_path(&f.0)];
        assert_eq!(
            h.roots.iter().find(|r| r.oid == base).unwrap().result,
            "covered"
        );
    }
}

struct AncestorSiblingMutation {
    ancestor: PathBuf,
    final_phase: std::cell::Cell<bool>,
    created: std::cell::Cell<u64>,
}
impl LocalHooks for AncestorSiblingMutation {
    fn is_dataless(&self, path: &BytePath, _stat: &rustix::fs::Stat) -> bool {
        if self.final_phase.get() && *path == BytePath::from_path(&self.ancestor) {
            let next = self.created.get() + 1;
            fs::create_dir(self.ancestor.join(format!("unrelated-neighbor-{next}"))).unwrap();
            self.created.set(next);
        }
        false
    }
}
impl PrivateHistoryHooks for AncestorSiblingMutation {
    fn before_final_validation(&self) -> std::io::Result<()> {
        self.final_phase.set(true);
        Ok(())
    }
}
#[test]
fn sibling_creation_between_ancestor_stat_and_open_is_not_source_drift() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let exclusive = f.git(&["commit-tree", &tree, "-p", &base, "-m", "retained negative"]);
    Fixture::at(&linked, &["reset", "--hard", &exclusive]);
    Fixture::at(&linked, &["reset", "--hard", &base]);
    let hook = AncestorSiblingMutation {
        ancestor: f.0.clone(),
        final_phase: std::cell::Cell::new(false),
        created: std::cell::Cell::new(0),
    };
    let facts = collect_with(&scan_family(&f.0).unwrap(), &[], &Default::default(), &hook).unwrap();
    assert!(hook.created.get() > 0);
    let h = &facts.histories[&BytePath::from_path(&linked)];
    assert_eq!(h.temporal_consistency, "not_guaranteed", "{:?}", h.gaps);
    assert!(h.enumeration_complete, "{:?}", h.gaps);
    assert_eq!(
        h.roots.iter().find(|r| r.oid == base).unwrap().result,
        "covered"
    );
    assert_eq!(
        h.roots.iter().find(|r| r.oid == exclusive).unwrap().result,
        "not_preserved_by_retained_refs"
    );
}

struct AncestorReplacement {
    ancestor: PathBuf,
    final_phase: std::cell::Cell<bool>,
    changed: std::cell::Cell<bool>,
    seen: std::cell::Cell<u64>,
    symlink: bool,
}
impl LocalHooks for AncestorReplacement {
    fn is_dataless(&self, path: &BytePath, _stat: &rustix::fs::Stat) -> bool {
        if self.final_phase.get()
            && !self.changed.get()
            && *path == BytePath::from_path(&self.ancestor)
        {
            let next = self.seen.get() + 1;
            self.seen.set(next);
            // The first call validates logs as a source. The next traverses it as an ancestor.
            if next == 1 {
                return false;
            }
            let saved = self.ancestor.with_file_name("saved-logs");
            fs::rename(&self.ancestor, &saved).unwrap();
            if self.symlink {
                std::os::unix::fs::symlink(&saved, &self.ancestor).unwrap();
            } else {
                fs::create_dir(&self.ancestor).unwrap();
            }
            self.changed.set(true);
        }
        false
    }
}
impl PrivateHistoryHooks for AncestorReplacement {
    fn before_final_validation(&self) -> std::io::Result<()> {
        self.final_phase.set(true);
        Ok(())
    }
}
#[test]
fn ancestor_replacement_and_symlink_swap_still_refuse_current_coverage() {
    for symlink in [false, true] {
        let f = Fixture::new();
        let linked = f.linked("linked");
        let dir = PathBuf::from(Fixture::at(&linked, &["rev-parse", "--absolute-git-dir"]));
        let hook = AncestorReplacement {
            ancestor: dir.join("logs"),
            final_phase: std::cell::Cell::new(false),
            changed: std::cell::Cell::new(false),
            seen: std::cell::Cell::new(0),
            symlink,
        };
        let facts =
            collect_with(&scan_family(&f.0).unwrap(), &[], &Default::default(), &hook).unwrap();
        assert!(hook.changed.get());
        let h = &facts.histories[&BytePath::from_path(&linked)];
        assert!(!h.enumeration_complete, "{:?}", h.gaps);
        assert!(
            h.roots
                .iter()
                .all(|r| r.result == "unknown" && r.observed_result.as_deref() == Some("covered"))
        );
        if !symlink {
            assert!(
                h.gaps.iter().any(|g| g.contains("source parent changed")),
                "{:?}",
                h.gaps
            );
            assert_eq!(h.temporal_consistency, "change_observed");
        }
    }
}

#[test]
fn private_root_is_preserved_by_retained_child_with_uppercase_parent() {
    let f = Fixture::new();
    let linked = f.linked("linked");
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let raw = f.0.join(".git/t09-raw-commit");
    fs::write(&raw, format!("tree {}\nparent {}\nauthor Fixture <fixture@example.invalid> 1 +0000\ncommitter Fixture <fixture@example.invalid> 1 +0000\n\nUppercase graph\n", tree.to_ascii_uppercase(), base.to_ascii_uppercase())).unwrap();
    let child = f.git(&["hash-object", "-t", "commit", "-w", raw.to_str().unwrap()]);
    f.git(&["update-ref", "refs/heads/main", &child]);
    f.git(&["fsck", "--no-reflogs", "--no-dangling"]);
    let facts = collect_with_limits(
        &scan_family(&f.0).unwrap(),
        &[],
        &PrivateHistoryLimits::default(),
    )
    .unwrap();
    let history = &facts.histories[&BytePath::from_path(&linked)];
    let root = history.roots.iter().find(|root| root.oid == base).unwrap();
    assert_eq!(
        root.result, "covered",
        "case alias missed retained graph coverage"
    );
    assert_eq!(root.anchor.as_ref().unwrap().oid, child);
}
