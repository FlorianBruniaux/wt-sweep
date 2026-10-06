#![allow(clippy::unwrap_used)]
use super::*;
use std::{fs, process::Command};

struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn git(path: &Path, args: &[&str]) -> String {
    let mut command = Command::new("/usr/bin/git");
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", path)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .env("GIT_ALLOW_PROTOCOL", "")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
            "-C",
        ])
        .arg(path)
        .args(args);
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() > Duration::from_secs(10) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("owned Git fixture timed out");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    String::from_utf8(output.stdout).unwrap().trim().into()
}
#[test]
fn deep_verifies_distinct_blobs_in_bounded_batches_and_refuses_corruption() {
    let f = Fixture(std::env::temp_dir().join(format!(
        "wt-sweep-deep-batch-{}-{}",
        std::process::id(),
        now_ms()
    )));
    fs::create_dir(&f.0).unwrap();
    git(&f.0, &["init", "-q", "-b", "main"]);
    for n in 0..256 {
        fs::write(
            f.0.join(format!("file-{n}")),
            format!("distinct payload {n}\n"),
        )
        .unwrap();
    }
    git(&f.0, &["add", "."]);
    git(&f.0, &["commit", "-qm", "Fixture"]);
    let tree = git(&f.0, &["rev-parse", "HEAD^{tree}"]);
    let common = f.0.join(".git");
    let mut reader = ObjectReader::new();
    inspect_tree(&mut reader, &common, &tree).unwrap();
    assert!(
        reader.git.spawned_processes() <= 5,
        "{} children for 256 blobs",
        reader.git.spawned_processes()
    );
    let bad = git(&f.0, &["rev-parse", "HEAD:file-17"]);
    let bad_path = common.join("objects").join(&bad[..2]).join(&bad[2..]);
    fs::remove_file(&bad_path).unwrap();
    fs::write(bad_path, b"corrupt object bytes").unwrap();
    let mut fresh = ObjectReader::new();
    assert!(
        inspect_tree(&mut fresh, &common, &tree).is_err(),
        "fresh reader accepted corrupt blob"
    );
}

#[test]
fn deep_process_metrics_cover_both_families_and_failed_reads() {
    let f = Fixture(std::env::temp_dir().join(format!(
        "wt-sweep-deep-metrics-{}-{}",
        std::process::id(),
        now_ms()
    )));
    fs::create_dir(&f.0).unwrap();
    let mut inventories = Vec::new();
    for n in 0..2 {
        let repo = f.0.join(format!("repo-{n}"));
        fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        fs::write(repo.join("file"), format!("unique {n}\n")).unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-qm", "Fixture"]);
        inventories.push(crate::git::scan_family(&repo).unwrap());
    }
    let mut inventory = inventories.remove(0);
    inventory.families.extend(inventories.remove(0).families);
    let second = &inventory.families[1];
    let oid = second.worktrees[0].head.as_ref().unwrap();
    let failed_object = second
        .common_dir
        .to_path()
        .unwrap()
        .join("objects")
        .join(&oid[..2])
        .join(&oid[2..]);
    let mut expected = 0;
    let mut observed_families = BTreeSet::new();
    let mut failed_read_seen = false;
    let facts = enrich_with_options(
        &inventory,
        TargetInput::Simple(&[]),
        None,
        &ReviewOptions {
            context: ContextOptions {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        },
        &crate::local::NativeHooks,
        &mut |event| {
            if event.phase == "deep" {
                crate::git::take_child_read_journal();
                fs::remove_file(&failed_object).unwrap();
                fs::write(&failed_object, b"corrupt owned commit").unwrap();
            }
            if event.phase == "private_history" {
                let journal = crate::git::take_child_read_journal();
                expected = journal.len() as u64;
                for (path, op) in journal {
                    observed_families.insert(path.clone());
                    failed_read_seen |=
                        path == second.common_dir && matches!(op, GitRead::Commit { .. });
                }
            }
        },
    )
    .unwrap();
    assert_eq!(observed_families.len(), 2);
    assert!(failed_read_seen);
    assert!(expected >= 4);
    let phases = facts.collection_metrics.unwrap().phases;
    let metric = phases.iter().find(|p| p.name == "deep").unwrap();
    assert_eq!(metric.counters["git_processes"], expected);
}

#[test]
fn invalid_public_phase_budget_is_refused_before_collection() {
    let inventory = Inventory::new(ScanScope::Repo {
        path: BytePath::from_bytes(b"/never-inspected"),
    });
    for timeout in [Duration::ZERO, Duration::from_secs(901), Duration::MAX] {
        let result = enrich_with_options(
            &inventory,
            TargetInput::Simple(&[]),
            None,
            &ReviewOptions {
                local_timeout: timeout,
                ..Default::default()
            },
            &crate::local::NativeHooks,
            &mut |_| panic!("invalid budget started collection"),
        );
        assert!(result.is_err());
    }
}
