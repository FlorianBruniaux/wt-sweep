#![allow(clippy::expect_used)]

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_wt-sweep"))
        .args(args)
        .output()
        .expect("bootstrap CLI starts")
}

#[test]
fn help_explains_readonly_inventory_and_apply_boundary() {
    let output = run(&["--help"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).expect("help is UTF-8");
    assert!(text.contains("Worktree Sweep"));
    assert!(text.contains("--repo"));
    assert!(text.contains("--format"));
    assert!(text.contains("apply is unavailable"));
}

#[test]
fn version_comes_from_the_package() {
    let output = run(&["--version"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        format!("wt-sweep {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
    );
}

#[test]
fn bare_scan_requires_an_explicit_scope() {
    let output = run(&["scan"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("scope"));
}

#[test]
fn unimplemented_product_commands_never_report_success() {
    for args in [
        vec!["scan", "--all", "--repo", "/path/that/must/not/be/read"],
        vec!["scan", "--repo", "/path/that/must/not/be/read"],
        vec!["plan", "--inventory", "missing.json"],
        vec!["apply", "--yes"],
        vec!["prune"],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(!output.stderr.is_empty(), "{args:?}");
    }
}

#[test]
fn extra_arguments_do_not_hide_behind_help_or_version() {
    for args in [
        ["--help", "apply"],
        ["--version", "--yes"],
        ["unknown", "--help"],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn command_help_is_specific_and_does_not_collect() {
    for command in ["scan", "plan", "review", "forge"] {
        let output = run(&[command, "--help"]);
        assert!(output.status.success(), "{command}: {:?}", output.stderr);
        assert!(output.stderr.is_empty());
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains(&format!("wt-sweep {command}")));
        assert!(text.contains("Usage:"));
    }
    for sub in ["commandes", "ingerer"] {
        let output = run(&["forge", sub, "--help"]);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert!(String::from_utf8_lossy(&output.stdout).contains(&format!("forge {sub}")));
    }
    for args in [
        vec!["review", "--help", "--all"],
        vec!["forge", "unknown", "--help"],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn combined_review_rejects_invalid_options_before_discovery() {
    for options in [
        vec!["--summary", "--format", "json"],
        vec![
            "--target",
            "refs/heads/main",
            "--target-map",
            "/not/read.json",
        ],
        vec!["--no-context", "--changelog-path", "CHANGELOG.md"],
        vec!["--local-state", "invalid"],
        vec!["--group-by", "invalid"],
        vec!["--workers", "0"],
    ] {
        let mut args = vec!["review", "--repo", "/path/that/must/not/be/read"];
        args.extend(options);
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(!diagnostic.contains("discovery:"), "{args:?}: {diagnostic}");
        assert!(
            !diagnostic.contains("unsupported arguments"),
            "command was not recognized"
        );
    }
}

#[test]
fn disk_budget_requires_measurement_and_a_known_profile_before_discovery() {
    for command in ["scan", "review"] {
        for (extra, message) in [
            (
                vec!["--disk-budget", "thorough"],
                "--disk-budget requires --disk-usage",
            ),
            (
                vec!["--disk-usage", "--disk-budget", "unbounded"],
                "--disk-budget must be standard or thorough",
            ),
        ] {
            let mut args = vec![command, "--repo", "/must/not/read"];
            args.extend(extra);
            let output = run(&args);
            assert_eq!(output.status.code(), Some(2));
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert!(diagnostic.contains(message), "{diagnostic}");
            assert!(!diagnostic.contains("Discovering repositories"));
            assert!(output.stdout.is_empty());
        }
    }
}

#[test]
fn thorough_disk_profile_is_recorded_on_an_empty_explicit_scope() {
    let output = run(&[
        "scan",
        "--root",
        "/does-not-exist-wt-sweep-size-fixture",
        "--disk-usage",
        "--disk-budget",
        "thorough",
        "--format",
        "json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(value["disk_usage"]["limits"]["max_entries"], 10_000_000);
    assert_eq!(value["coverage"]["traversal_complete"], false);
    assert!(
        !value["coverage"]["gaps"]
            .as_array()
            .expect("gaps")
            .is_empty()
    );
}

#[test]
fn disk_root_requires_explicit_disk_opt_in_and_absolute_path_before_discovery() {
    for command in ["scan", "review"] {
        for (extra, message) in [
            (
                vec!["--disk-root", "/unread"],
                "--disk-root requires --disk-usage",
            ),
            (
                vec!["--disk-usage", "--disk-root", "relative"],
                "--disk-root requires an absolute",
            ),
        ] {
            let mut args = vec![command, "--repo", "/must/not/read"];
            args.extend(extra);
            let output = run(&args);
            assert_eq!(output.status.code(), Some(2));
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(error.contains(message), "{error}");
            assert!(!error.contains("Discovering repositories"));
            assert!(output.stdout.is_empty());
        }
    }
}
