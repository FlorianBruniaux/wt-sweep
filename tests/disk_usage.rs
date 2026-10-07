#![cfg(target_os = "macos")]
#![allow(clippy::expect_used)]
use std::os::unix::fs::{MetadataExt, symlink};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use wt_sweep::{
    discover::scan_scope,
    model::{BytePath, ScanScope},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = PathBuf::from(format!(
            "/private/tmp/wt-sweep-size-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).expect("owned fixture");
        Self(p)
    }
    fn git(&self, p: &Path, args: &[&str]) {
        let o = Command::new("/usr/bin/git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .arg("-C")
            .arg(p)
            .args(args)
            .output()
            .expect("git starts");
        assert!(o.status.success(), "{:?}", o.stderr);
    }
    fn repo(&self) -> PathBuf {
        let p = self.0.join("repo");
        fs::create_dir(&p).expect("repo");
        self.git(&p, &["init", "-b", "main"]);
        self.git(&p, &["commit", "--allow-empty", "-m", "fixture"]);
        p
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("owned fixture cleanup");
    }
}

#[test]
fn scan_disk_usage_is_opt_in_and_reports_ignored_sparse_and_symlink_bytes() {
    let f = Fixture::new();
    let p = f.repo();
    fs::write(p.join(".gitignore"), b"ignored/\n").expect("ignore");
    fs::create_dir(p.join("ignored")).expect("ignored");
    fs::write(p.join("ignored/data"), vec![42; 8192]).expect("ignored data");
    let sparse = fs::File::create(p.join(".sparse")).expect("sparse");
    sparse.set_len(8 * 1024 * 1024).expect("sparse length");
    fs::write(f.0.join("outside"), vec![7; 32768]).expect("outside sentinel");
    symlink(f.0.join("outside"), p.join("link")).expect("symlink");
    let run = |extra: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_wt-sweep"))
            .args(["scan", "--repo"])
            .arg(&p)
            .args(extra)
            .args(["--format", "json"])
            .output()
            .expect("CLI");
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        serde_json::from_slice::<serde_json::Value>(&o.stdout).expect("JSON")
    };
    assert!(run(&[]).get("disk_usage").is_none());
    let value = run(&["--disk-usage"]);
    let disk = &value["disk_usage"];
    assert_eq!(disk["complete"], true);
    let rows = disk["measurements"].as_array().expect("rows");
    assert_eq!(rows.len(), 2, "worktree and shared Git counted separately");
    let wt = rows
        .iter()
        .find(|r| r["kind"] == "worktree")
        .expect("worktree");
    let expected_logical: u64 = [".gitignore", "ignored/data", ".sparse", "link"]
        .iter()
        .map(|n| fs::symlink_metadata(p.join(n)).expect("metadata").len())
        .sum();
    let expected_blocks: u64 = [
        "",
        ".gitignore",
        "ignored",
        "ignored/data",
        ".sparse",
        "link",
    ]
    .iter()
    .map(|n| fs::symlink_metadata(p.join(n)).expect("metadata").blocks() * 512)
    .sum();
    assert_eq!(wt["bytes"]["logical"], expected_logical);
    assert_eq!(wt["bytes"]["allocated"], expected_blocks);
    assert_eq!(
        fs::read(f.0.join("outside")).expect("sentinel"),
        vec![7; 32768]
    );
    let report =
        wt_sweep::output::inventory_table(&serde_json::from_value(value).expect("inventory"));
    assert!(report.contains("DISK USAGE"));
    assert!(report.contains("not reclaimable space"));
}

#[test]
fn global_totals_deduplicate_hardlinks_and_nested_registered_worktrees() {
    let f = Fixture::new();
    let p = f.repo();
    let nested = p.join("nested");
    f.git(
        &p,
        &[
            "worktree",
            "add",
            "-b",
            "nested",
            nested.to_str().expect("fixture path"),
        ],
    );
    fs::write(p.join("data"), vec![1; 16384]).expect("data");
    fs::hard_link(p.join("data"), nested.join("same-data")).expect("hardlink");
    let mut inv = scan_scope(&ScanScope::Repo {
        path: BytePath::from_path(&p),
    })
    .expect("scan");
    wt_sweep::disk::collect(
        &mut inv,
        &wt_sweep::disk::DiskLimits::default(),
        &mut |_| {},
    )
    .expect("disk");
    let disk = inv.disk_usage.expect("disk usage");
    assert!(disk.complete);
    assert_eq!(disk.measurements.len(), 3);
    let sum: u64 = disk
        .measurements
        .iter()
        .map(|r| r.bytes.as_ref().expect("bytes").allocated)
        .sum();
    assert_eq!(
        sum - disk.unique_bytes.allocated,
        fs::metadata(p.join("data")).expect("file").blocks() * 512
    );
    let main = disk
        .measurements
        .iter()
        .find(|r| r.path == BytePath::from_path(&p))
        .expect("main");
    assert_eq!(
        main.bytes.as_ref().expect("bytes").logical,
        16384,
        "nested and common Git excluded"
    );
}

#[test]
fn exhausted_budget_and_missing_root_are_unknown_not_zero_sized_complete() {
    let f = Fixture::new();
    let p = f.repo();
    let mut inv = scan_scope(&ScanScope::Repo {
        path: BytePath::from_path(&p),
    })
    .expect("scan");
    let limits = wt_sweep::disk::DiskLimits {
        max_entries: 0,
        ..Default::default()
    };
    wt_sweep::disk::collect(&mut inv, &limits, &mut |_| {}).expect("disk");
    let disk = inv.disk_usage.as_ref().expect("disk usage");
    assert!(!disk.complete);
    assert!(
        disk.measurements
            .iter()
            .all(|r| !r.complete && !r.gaps.is_empty())
    );
    let moved = f.0.join("moved");
    fs::rename(&p, moved).expect("move owned fixture");
    wt_sweep::disk::collect(&mut inv, &Default::default(), &mut |_| {}).expect("disk");
    let disk = inv.disk_usage.expect("disk usage");
    assert!(!disk.complete);
    assert!(disk.measurements.iter().all(|r| r.bytes.is_none()));
}

#[test]
fn control_path_bytes_and_directory_links_are_preserved_without_following() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new();
    let p = f.repo();
    let name = std::ffi::OsString::from_vec(b"raw-\t\n".to_vec());
    fs::write(p.join(&name), b"payload").expect("raw name");
    fs::hard_link(p.join(&name), p.join("duplicate")).expect("local hardlink");
    symlink(&p, p.join("loop")).expect("cycle");
    let mut inv = scan_scope(&ScanScope::Repo {
        path: BytePath::from_path(&p),
    })
    .expect("scan");
    wt_sweep::disk::collect(&mut inv, &Default::default(), &mut |_| {}).expect("disk");
    let disk = inv.disk_usage.expect("disk");
    assert!(disk.complete);
    let row = disk
        .measurements
        .iter()
        .find(|r| r.path == BytePath::from_path(&p))
        .expect("root");
    assert_eq!(
        row.bytes.as_ref().expect("size").logical,
        7 + fs::symlink_metadata(p.join("loop")).expect("link").len()
    );
    assert_eq!(fs::read(p.join(name)).expect("preserved bytes"), b"payload");
}

#[test]
fn replaced_root_identity_is_refused_without_measuring_the_replacement() {
    let f = Fixture::new();
    let p = f.repo();
    let mut inv = scan_scope(&ScanScope::Repo {
        path: BytePath::from_path(&p),
    })
    .expect("scan");
    fs::rename(&p, f.0.join("original")).expect("move fixture");
    fs::create_dir(&p).expect("replacement");
    fs::write(p.join("sentinel"), b"do not account").expect("sentinel");
    wt_sweep::disk::collect(&mut inv, &Default::default(), &mut |_| {}).expect("disk");
    let disk = inv.disk_usage.expect("disk");
    let row = disk
        .measurements
        .iter()
        .find(|r| r.path == BytePath::from_path(&p))
        .expect("root");
    assert!(row.bytes.is_none());
    assert!(!row.complete);
    assert!(row.gaps[0].reason.contains("identity changed"));
    assert_eq!(disk.unique_bytes.logical, 0);
}

#[test]
fn depth_and_time_limits_remain_partial_in_publication() {
    use wt_sweep::{
        disk::DiskLimits,
        output::{PublicationLimits, prepare_inventory},
    };
    let f = Fixture::new();
    let p = f.repo();
    fs::create_dir_all(p.join("deep/child")).expect("nested");
    fs::write(p.join("deep/child/data"), b"hidden by depth limit").expect("data");
    let mut inv = scan_scope(&ScanScope::Repo {
        path: BytePath::from_path(&p),
    })
    .expect("scan");
    assert!(inv.coverage.traversal_complete);
    for limits in [
        DiskLimits {
            max_depth: 0,
            ..Default::default()
        },
        DiskLimits {
            timeout: std::time::Duration::ZERO,
            ..Default::default()
        },
    ] {
        wt_sweep::disk::collect(&mut inv, &limits, &mut |_| {}).expect("disk");
        let published =
            prepare_inventory(&inv, &PublicationLimits::default()).expect("publication");
        assert!(published.partial);
        assert!(published.report.contains("INCOMPLETE"));
        assert!(
            !published
                .inventory
                .disk_usage
                .expect("measurements retained")
                .complete
        );
        assert!(
            inv.coverage.traversal_complete,
            "disk gaps are separate from discovery coverage"
        );
    }
}

#[test]
fn bare_repository_is_only_counted_as_shared_git() {
    let f = Fixture::new();
    let p = f.0.join("bare");
    f.git(&f.0, &["init", "--bare", p.to_str().expect("path")]);
    let mut inv = scan_scope(&ScanScope::Repo {
        path: BytePath::from_path(&p),
    })
    .expect("scan");
    wt_sweep::disk::collect(&mut inv, &Default::default(), &mut |_| {}).expect("disk");
    let disk = inv.disk_usage.expect("disk");
    assert!(disk.complete);
    assert_eq!(disk.measurements.len(), 1);
    assert_eq!(
        disk.measurements[0].kind,
        wt_sweep::disk::DiskKind::GitCommon
    );
    assert!(disk.unique_bytes.logical > 0);
}

#[test]
fn review_publishes_native_disk_measurements_in_filtered_json_report_and_console() {
    let f = Fixture::new();
    let repo = f.repo();
    let linked = f.0.join("linked");
    f.git(
        &repo,
        &[
            "worktree",
            "add",
            "-b",
            "linked",
            linked.to_str().expect("path"),
        ],
    );
    fs::write(linked.join("payload"), vec![42; 8192]).expect("payload");
    let destination = f.0.join("reports");
    fs::create_dir(&destination).expect("destination");
    let output = Command::new(env!("CARGO_BIN_EXE_wt-sweep"))
        .args(["review", "--repo"])
        .arg(&repo)
        .args([
            "--target",
            "refs/heads/main",
            "--disk-usage",
            "--disk-budget",
            "thorough",
            "--no-context",
            "--summary",
            "--verbose",
            "--group-by",
            "directory",
            "--path-prefix",
        ])
        .arg(&linked)
        .arg("--run-dir")
        .arg(&destination)
        .output()
        .expect("review CLI");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run = fs::read_dir(&destination)
        .expect("run dir")
        .next()
        .expect("run")
        .expect("entry")
        .path();
    let inventory: serde_json::Value =
        serde_json::from_slice(&fs::read(run.join("inventory.json")).expect("inventory"))
            .expect("inventory JSON");
    let review: serde_json::Value =
        serde_json::from_slice(&fs::read(run.join("review.json")).expect("review"))
            .expect("review JSON");
    assert_eq!(
        review["selected_paths"].as_array().expect("selected").len(),
        1
    );
    assert_eq!(review["source"]["disk_usage"], inventory["disk_usage"]);
    let disk = &inventory["disk_usage"];
    assert_eq!(disk["complete"], true);
    assert_eq!(disk["measurements"].as_array().expect("disk rows").len(), 3);
    let expected_size = wt_sweep::disk::human_bytes(
        disk["unique_bytes"]["allocated"]
            .as_u64()
            .expect("allocated"),
    );
    let stdout = String::from_utf8(output.stdout).expect("console");
    let report = fs::read_to_string(run.join("report.txt")).expect("report");
    for text in [&stdout, &report] {
        assert!(text.contains("DISK USAGE (all recorded roots"), "{text}");
        assert!(
            text.contains(&format!(
                "Observed allocated, deduplicated: {expected_size}"
            )),
            "{text}"
        );
        assert!(text.contains("unaffected by review filters"), "{text}");
        assert!(text.contains("not reclaimable space"), "{text}");
    }
    for path in [&repo, &linked, &repo.join(".git")] {
        assert!(
            report.contains(&BytePath::from_path(path).display()),
            "disk path missing"
        );
    }
    assert!(report.contains("Shared Git:"));
    assert_eq!(
        report.matches("DISK USAGE (").count(),
        1,
        "one recap per filtered report"
    );
    let parsed: wt_sweep::model::ReviewView = serde_json::from_value(review).expect("view");
    assert_eq!(
        wt_sweep::output::canonical_digest(&parsed).expect("digest"),
        parsed.digest_sha256
    );
    assert_eq!(
        wt_sweep::output::canonical_digest(&parsed.source).expect("source digest"),
        parsed.source.digest_sha256
    );
}

#[test]
fn plan_keeps_captured_disk_sizes_after_files_change_and_reports_missing_measurements() {
    let f = Fixture::new();
    let repo = f.repo();
    let payload = repo.join("payload");
    fs::write(&payload, vec![1; 8192]).expect("payload");
    let mut inventory = scan_scope(&ScanScope::Repo {
        path: BytePath::from_path(&repo),
    })
    .expect("scan");
    wt_sweep::disk::collect(
        &mut inventory,
        &wt_sweep::disk::DiskLimits::default(),
        &mut |_| {},
    )
    .expect("disk");
    let captured = serde_json::to_value(inventory.disk_usage.as_ref().expect("disk usage"))
        .expect("disk JSON");
    fs::write(&payload, vec![1; 32768]).expect("change payload after capture");
    for has_disk in [true, false] {
        if !has_disk {
            inventory.disk_usage = None;
        }
        let input = f.0.join(format!("inventory-{has_disk}.json"));
        fs::write(
            &input,
            serde_json::to_vec(&inventory).expect("inventory JSON"),
        )
        .expect("input");
        let destination = f.0.join(format!("plans-{has_disk}"));
        fs::create_dir(&destination).expect("destination");
        let output = Command::new(env!("CARGO_BIN_EXE_wt-sweep"))
            .args(["plan", "--inventory"])
            .arg(&input)
            .args([
                "--target",
                "refs/heads/main",
                "--no-context",
                "--summary",
                "--verbose",
                "--run-dir",
            ])
            .arg(&destination)
            .output()
            .expect("plan CLI");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let run = fs::read_dir(&destination)
            .expect("run dir")
            .next()
            .expect("run")
            .expect("entry")
            .path();
        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(run.join("review.json")).expect("review"))
                .expect("JSON");
        let report = fs::read_to_string(run.join("report.txt")).expect("report");
        let stdout = String::from_utf8(output.stdout).expect("console");
        if has_disk {
            assert_eq!(
                json["disk_usage"], captured,
                "plan must carry the captured measurement, not recount live files"
            );
            for text in [&report, &stdout] {
                assert!(text.contains("Disk observation interval (Unix ms):"));
            }
        } else {
            assert!(json.get("disk_usage").is_none());
            for text in [&report, &stdout] {
                assert!(text.contains("Not measured; use scan or review --disk-usage."));
                assert!(!text.contains("Observed allocated, deduplicated:"));
            }
        }
    }
}
