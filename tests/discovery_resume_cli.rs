#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "wt-sweep-resume-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(fs::canonicalize(p).unwrap())
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_wt-sweep"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
    fn root(&self) -> String {
        fs::create_dir_all(self.0.join("source/one/two")).unwrap();
        self.0.join("source").to_str().unwrap().into()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn checkpoint_round_trip_publishes_intervals_and_a_ready_continuation() {
    let f = Fixture::new();
    let root = f.root();
    let first = f.run(&[
        "scan",
        "--root",
        &root,
        "--discovery-directories",
        "1",
        "--checkpoint-out",
        "first.json",
        "--run-dir",
        ".",
        "--html",
    ]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let text = String::from_utf8_lossy(&first.stdout);
    assert!(
        text.contains("--resume") && text.contains("first.json"),
        "{text}"
    );
    let checkpoint = fs::read(f.0.join("first.json")).unwrap();
    let second = f.run(&[
        "scan",
        "--resume",
        "first.json",
        "--checkpoint-out",
        "second.json",
        "--run-dir",
        ".",
        "--html",
        "--format",
        "json",
    ]);
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let inventory: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(inventory["coverage"]["traversal_complete"], true);
    assert_eq!(
        inventory["coverage"]["discovery_intervals"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        inventory["coverage"]["temporal_consistency"],
        "multiple_observation_intervals_no_snapshot"
    );
    assert_eq!(fs::read(f.0.join("first.json")).unwrap(), checkpoint);
    let mut reports = 0;
    for entry in fs::read_dir(&f.0).unwrap().flatten() {
        if entry.file_name().to_string_lossy().starts_with("run-") {
            let html = fs::read_to_string(entry.path().join("report.html")).unwrap();
            assert!(html.contains("Discovery observation intervals"));
            let report = fs::read_to_string(entry.path().join("report.txt")).unwrap();
            assert!(report.contains("Discovery observation intervals"));
            reports += 1;
        }
    }
    assert_eq!(reports, 2);
}
#[test]
fn checkpoint_output_never_overwrites_or_writes_under_inspected_roots() {
    let f = Fixture::new();
    let root = f.root();
    fs::write(f.0.join("exists.json"), b"retain me").unwrap();
    for destination in ["exists.json", "source/checkpoint.json"] {
        let result = f.run(&["scan", "--root", &root, "--checkpoint-out", destination]);
        assert_eq!(result.status.code(), Some(2));
        assert!(!String::from_utf8_lossy(&result.stderr).contains("unsupported option"));
    }
    assert_eq!(fs::read(f.0.join("exists.json")).unwrap(), b"retain me");
    assert!(!f.0.join("source/checkpoint.json").exists());
}
#[test]
fn invalid_resume_flags_and_tampered_state_fail_before_discovery() {
    let f = Fixture::new();
    let root = f.root();
    for args in [
        vec!["scan", "--resume", "absent.json"],
        vec![
            "scan",
            "--repo",
            "/must/not/read",
            "--checkpoint-out",
            "checkpoint.json",
        ],
        vec!["scan", "--all", "--checkpoint-out", "checkpoint.json"],
        vec!["scan", "--root", &root, "--discovery-directories", "0"],
        vec!["scan", "--root", &root, "--discovery-seconds", "601"],
    ] {
        let result = f.run(&args);
        assert_eq!(result.status.code(), Some(2));
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(!stderr.contains("unsupported option"), "{stderr}");
        assert!(!stderr.contains("Discovering repositories"), "{stderr}");
    }
    assert!(
        f.run(&["scan", "--root", &root, "--checkpoint-out", "first.json"])
            .status
            .success()
    );
    let mut cp: serde_json::Value =
        serde_json::from_slice(&fs::read(f.0.join("first.json")).unwrap()).unwrap();
    cp["sha256"] = serde_json::json!("00");
    fs::write(f.0.join("tampered.json"), serde_json::to_vec(&cp).unwrap()).unwrap();
    let result = f.run(&[
        "scan",
        "--resume",
        "tampered.json",
        "--checkpoint-out",
        "second.json",
    ]);
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("digest mismatch"));
    assert!(!f.0.join("second.json").exists());
}
