use wt_sweep::model::*;

fn report() -> Result<Review, SweepError> {
    let facts = Observations {
        context: None,
        inventory: Inventory::new(ScanScope::Repo {
            path: BytePath::from_bytes(b"/fixture"),
        }),
        shared_stashes: vec![],
        history_metrics: Default::default(),
        collection_metrics: None,
        items: vec![],
        forge: None,
        started_unix_ms: 10,
        finished_unix_ms: 30,
    };
    let mut report = wt_sweep::review::judge(&facts)?;
    for (path, class, reasons, missing) in [
        (
            b"/fixture/kept".as_slice(),
            "keep",
            vec!["local_ignored", "git_operation"],
            vec!["C3"],
        ),
        (
            b"/fixture/candidate".as_slice(),
            "cleanup_candidate",
            vec![],
            vec![],
        ),
        (
            b"/fixture/check\nnext".as_slice(),
            "verify",
            vec![],
            vec!["C1", "C4"],
        ),
    ] {
        report.items.push(ReviewItem {
            worktree_state: None,
            decision: Some(TriageDecision {
                keep_reasons: reasons.into_iter().map(str::to_owned).collect(),
                missing_conditions: missing.into_iter().map(str::to_owned).collect(),
            }),
            path: BytePath::from_bytes(path),
            private_history: None,
            history: vec![],
            history_omitted_targets: 0,
            local_summary: None,
            branch: None,
            head: Some("a".repeat(40)),
            subject: None,
            subject_truncated: false,
            classification: class.into(),
            observations: vec![],
            requests: vec![],
            risks: vec![],
            unknowns: vec!["forge not collected".into()],
        });
    }
    Ok(report)
}

#[test]
fn triage_summary_counts_decisions_without_calling_unknown_usage_inactive() -> Result<(), SweepError>
{
    let summary = wt_sweep::presentation::summary(&report()?);
    assert!(summary.lines().count() <= 60);
    assert!(
        summary.contains("Keep: 1")
            && summary.contains("Cleanup candidates: 1")
            && summary.contains("Needs verification: 1"),
        "{summary}"
    );
    assert!(
        summary.contains("Usage by agents or terminals: not checked"),
        "{summary}"
    );
    assert!(summary.contains("removal"), "{summary}");
    assert!(!summary.contains("Active:"), "{summary}");
    Ok(())
}

#[test]
fn detail_explains_each_reason_missing_condition_and_candidate_limits() -> Result<(), SweepError> {
    let detail = wt_sweep::presentation::detail(&report()?);
    for expected in [
        "Ignored content observed",
        "Git operation marker observed",
        "Next check C1:",
        "Next check C4:",
        "Usage UNKNOWN: confirm before removal",
        "authorizes no removal",
        "Decision evidence interval: 10..30",
        "ACLs, extended attributes",
        "/fixture/check\\nnext",
    ] {
        assert!(detail.contains(expected), "missing {expected}: {detail}");
    }
    assert!(!detail.contains("/fixture/check\nnext"));
    Ok(())
}

#[test]
fn detail_byte_limit_refuses_truncation_and_keeps_exact_boundary() -> Result<(), SweepError> {
    let report = report()?;
    let expected = wt_sweep::presentation::detail(&report);
    let before = serde_json::to_vec(&report).map_err(|e| {
        SweepError::new("test", "serialize", std::path::Path::new(""), e.to_string())
    })?;
    assert_eq!(
        wt_sweep::presentation::detail_bounded(&report, expected.len())?,
        expected
    );
    assert!(wt_sweep::presentation::detail_bounded(&report, expected.len() - 1).is_err());
    assert!(wt_sweep::presentation::detail_bounded(&report, 0).is_err());
    let after = serde_json::to_vec(&report).map_err(|e| {
        SweepError::new("test", "serialize", std::path::Path::new(""), e.to_string())
    })?;
    assert_eq!(before, after);
    Ok(())
}

#[test]
fn detail_and_summary_preserve_source_coverage_omissions() -> Result<(), SweepError> {
    let mut report = report()?;
    report.coverage.remaining = Some(DiscoveryRemainder {
        reason: DiscoveryStop::Interrupted,
        known_pending_paths: 7,
        cover: RemainingCover::RequestedScope,
    });
    report.coverage.publication = Some(PublicationOmissions {
        local_inventories: 4,
        markers: 2,
        families: 1,
        worktrees: 3,
        cover: PublicationCover::RequestedScopeAndRegisteredWorktrees,
    });
    let detail = wt_sweep::presentation::detail(&report);
    assert!(
        detail.contains(
            "Discovery remaining: interrupted; known pending=7; conservative cover=requested scope"
        ),
        "{detail}"
    );
    assert!(
        detail
            .contains("Publication omitted: 1 families, 3 worktrees, 4 local manifests, 2 markers"),
        "{detail}"
    );
    assert!(
        detail.contains("including registered worktrees outside roots"),
        "{detail}"
    );
    let summary = wt_sweep::presentation::summary(&report);
    assert!(summary.lines().count() <= 60);
    assert!(
        summary.contains("Discovery remaining: interrupted"),
        "{summary}"
    );
    assert!(summary.contains("Unpublished worktrees: 3"), "{summary}");
    Ok(())
}

#[test]
fn complete_discovery_does_not_hide_partial_or_uncollected_local_checks() -> Result<(), SweepError>
{
    let mut report = report()?;
    report.coverage.traversal_complete = true;
    report.items[0].local_summary = Some(LocalSummary {
        worktree_identity: Some(Identity {
            device: 1,
            inode: 2,
        }),
        staged: LocalCount {
            observed: 2,
            ..Default::default()
        },
        ..Default::default()
    });
    report.items[1].local_summary = Some(LocalSummary {
        gaps: vec![Gap {
            path: report.items[1].path.clone(),
            reason: "whole-review local summary budget exhausted".into(),
        }],
        ..Default::default()
    });
    let summary = wt_sweep::presentation::summary(&report);
    for expected in [
        "WORKTREE SWEEP",
        "RESULTS",
        "Discovery: complete",
        "Candidate evidence incomplete: 2/3 worktrees",
        "Local checks complete: 0",
        "Local checks partial: 1",
        "Local checks not collected: 2",
        "Local collection budget reached",
    ] {
        assert!(summary.contains(expected), "missing {expected}: {summary}");
    }
    Ok(())
}

#[test]
fn complete_local_counts_are_not_a_claim_of_clean_files_or_inactive_usage()
-> Result<(), Box<dyn std::error::Error>> {
    let mut report = report()?;
    let count = LocalCount {
        complete: true,
        ..Default::default()
    };
    report.items[0].local_summary = Some(LocalSummary {
        head_complete: true,
        index_complete: true,
        disk_complete: true,
        staged: LocalCount {
            observed: 2,
            ..count.clone()
        },
        unstaged: count.clone(),
        conflicts: count.clone(),
        untracked: count.clone(),
        ignored: count,
        ..Default::default()
    });
    let summary = wt_sweep::presentation::summary(&report);
    assert!(summary.contains("Local checks complete: 1"), "{summary}");
    assert!(summary.contains("Keep: 1"), "{summary}");
    assert!(
        summary.contains("Usage by agents or terminals: not checked"),
        "{summary}"
    );
    report.items[0]
        .local_summary
        .as_mut()
        .ok_or("missing fixture summary")?
        .temporal_consistency = "change_observed".into();
    assert!(wt_sweep::presentation::summary(&report).contains("Local checks complete: 0"));
    Ok(())
}

#[test]
fn repeated_limits_are_grouped_and_console_does_not_mutate_evidence()
-> Result<(), Box<dyn std::error::Error>> {
    let mut report = report()?;
    report.history_metrics.stop_reason =
        Some("per-target history traversal step budget reached".into());
    for item in &mut report.items {
        item.unknowns
            .push("required final validation quarantined after family Git timeout".into());
    }
    let before = serde_json::to_vec(&report)?;
    let summary = wt_sweep::presentation::summary(&report);
    assert_eq!(
        summary.matches("History traversal limit reached").count(),
        1,
        "{summary}"
    );
    assert_eq!(summary.matches("Git timeout").count(), 1, "{summary}");
    assert_eq!(serde_json::to_vec(&report)?, before);
    Ok(())
}

#[test]
fn local_object_and_time_caps_have_distinct_retry_guidance() -> Result<(), SweepError> {
    let mut report = report()?;
    report.items[0].local_summary = Some(LocalSummary {
        gaps: vec![Gap {
            path: report.items[0].path.clone(),
            reason: "HEAD blob unavailable: summary object limit reached".into(),
        }],
        ..Default::default()
    });
    report.items[1].local_summary = Some(LocalSummary {
        gaps: vec![Gap {
            path: report.items[1].path.clone(),
            reason: "summary time budget exhausted during Git read".into(),
        }],
        ..Default::default()
    });
    let before = serde_json::to_vec(&report).map_err(|e| {
        SweepError::new("test", "serialize", std::path::Path::new(""), e.to_string())
    })?;
    let summary = wt_sweep::presentation::summary(&report);
    assert!(summary.contains("Local object limit reached"), "{summary}");
    assert!(summary.contains("Local time budget reached"), "{summary}");
    assert!(summary.contains("--worktree ABSOLUTE_PATH"), "{summary}");
    assert!(
        summary.contains("Object and byte caps still apply"),
        "{summary}"
    );
    assert!(!summary.contains("Git timeout"), "{summary}");
    assert_eq!(
        before,
        serde_json::to_vec(&report).map_err(|e| SweepError::new(
            "test",
            "serialize",
            std::path::Path::new(""),
            e.to_string()
        ))?
    );
    Ok(())
}

#[test]
fn focused_run_names_private_budget_without_suggesting_identical_retry() -> Result<(), SweepError> {
    let mut report = report()?;
    report.items[0]
        .unknowns
        .push("private history Git process or time budget reached".into());
    report.items[0].local_summary = Some(LocalSummary {
        gaps: vec![Gap {
            path: report.items[0].path.clone(),
            reason: "HEAD blob unavailable: summary object limit reached".into(),
        }],
        ..Default::default()
    });
    for item in &mut report.items[1..] {
        item.unknowns
            .push("Not selected for evidence collection (--worktree).".into());
    }
    let summary = wt_sweep::presentation::summary(&report);
    assert!(
        summary.contains("Private history collection budget reached"),
        "{summary}"
    );
    assert!(
        summary.contains("Inspect the selected worktree's gaps"),
        "{summary}"
    );
    assert!(!summary.contains("Retry one worktree"), "{summary}");
    Ok(())
}

#[test]
fn recap_distinguishes_justified_retention_from_unresolved_checks() -> Result<(), SweepError> {
    let rendered = wt_sweep::presentation::summary(&report()?);
    assert!(rendered.contains("Resolved verdicts: 2/3"), "{rendered}");
    let checks = rendered
        .split("CHECKS STILL NEEDED (unresolved worktrees)")
        .nth(1)
        .unwrap_or("");
    assert!(
        checks.contains("Current identity, locks and Git operations: 1"),
        "{rendered}"
    );
    assert!(
        checks.contains("Private history preserved by named refs: 1"),
        "{rendered}"
    );
    assert!(
        !checks.contains("Complete, clean local files:"),
        "keep-only C3 leaked into unresolved checks: {rendered}"
    );
    Ok(())
}

#[test]
fn performance_recap_uses_measured_denominators_and_preserves_verdicts()
-> Result<(), Box<dyn std::error::Error>> {
    let original = report()?;
    let mut json = serde_json::to_value(&original)?;
    json["coverage"]["discovery_metrics"] = serde_json::json!({
        "elapsed_ms": 2000, "directory_read_attempts": 64, "entries_listed": 500, "workers": 4
    });
    json["collection_metrics"] = serde_json::json!({
        "budget_profile": "standard", "applied_limits": {}, "phases": [
            {"name": "history", "elapsed_ms": 1000, "counters": {}},
            {"name": "local", "elapsed_ms": 3000, "counters": {"paths_checked": 3000, "git_processes": 12}}
        ]
    });
    let mut measured: Review = serde_json::from_value(json)?;
    let summary = wt_sweep::presentation::summary(&measured);
    for expected in [
        "250.0 entries/s",
        "1000.0 paths/s",
        "12 Git processes",
        "Local files (75.0%",
        "Keep: 1",
        "Needs verification: 1",
    ] {
        assert!(summary.contains(expected), "missing {expected}: {summary}");
    }
    // Less than one clock millisecond has no stable rate, and absent counters are not zero.
    if let Some(metrics) = &mut measured.collection_metrics {
        metrics.phases[1].elapsed_ms = 0;
        metrics.phases[1].counters.remove("git_processes");
    }
    let summary = wt_sweep::presentation::summary(&measured);
    assert!(!summary.contains("1000.0 paths/s"));
    assert!(!summary.contains("0 Git processes"));
    assert!(!summary.contains("inf"));
    assert!(!summary.contains("NaN"));
    Ok(())
}

#[test]
fn compact_view_keeps_source_gaps_and_partial_disk_but_counts_selected_checks()
-> Result<(), SweepError> {
    let mut source = report()?;
    source.coverage.traversal_complete = true;
    source.coverage.remaining = Some(DiscoveryRemainder {
        reason: DiscoveryStop::Interrupted,
        known_pending_paths: 7,
        cover: RemainingCover::RequestedScope,
    });
    source.disk_usage = Some(wt_sweep::disk::DiskUsage {
        requested_paths: None,
        started_unix_ms: 10,
        finished_unix_ms: 20,
        elapsed_ms: 10,
        limits: Default::default(),
        complete: false,
        temporal_consistency: "not_atomic".into(),
        unique_bytes: wt_sweep::disk::DiskBytes {
            allocated: 1024,
            logical: 2048,
        },
        measurements: vec![wt_sweep::disk::DiskMeasurement {
            requested: true,
            path: BytePath::from_bytes(b"/fixture/partial"),
            kind: wt_sweep::disk::DiskKind::Worktree,
            identity: None,
            bytes: Some(wt_sweep::disk::DiskBytes {
                allocated: 1024,
                logical: 2048,
            }),
            complete: false,
            entries_checked: 1,
            excluded_registered_roots: 0,
            elapsed_ms: 10,
            gaps: vec![],
            gaps_omitted: 0,
        }],
    });
    let view = ReviewView {
        schema: "wt-sweep.review-view/1".into(),
        selected_paths: vec![source.items[0].path.clone()],
        source,
        row_bindings: vec![],
        group_by: "repo".into(),
        filters: Default::default(),
        digest_sha256: String::new(),
    };
    let before = serde_json::to_vec(&view).map_err(|e| {
        SweepError::new("test", "serialize", std::path::Path::new(""), e.to_string())
    })?;
    let summary = wt_sweep::presentation::compact_view_summary(&view);
    for expected in [
        "Worktrees selected: 1/3",
        "Keep: 1",
        "Cleanup candidates: 0",
        "Needs verification: 0",
        "Source totals: 1 keep, 1 candidates, 1 to verify",
        "1.02 KB observed allocated (INCOMPLETE)",
        "0/1 roots complete",
        "unaffected by review filters",
        "not reclaimable space",
        "Discovery: incomplete",
        "7 known pending paths",
        "Local checks (selected): 0 complete, 0 partial, 1 not collected",
        "no removal authorized",
    ] {
        assert!(summary.contains(expected), "missing {expected}: {summary}");
    }
    assert!(
        !summary.contains("Missing checks (selected, unresolved)"),
        "other rows' missing checks leaked into selected counts"
    );
    assert_eq!(
        before,
        serde_json::to_vec(&view).map_err(|e| SweepError::new(
            "test",
            "serialize",
            std::path::Path::new(""),
            e.to_string()
        ))?
    );
    Ok(())
}

#[test]
fn compact_recap_exposes_unresolved_conditions_without_claiming_cleanliness()
-> Result<(), SweepError> {
    let mut source = report()?;
    source.coverage.traversal_complete = true;
    source.items[2]
        .unknowns
        .push("whole-review local summary budget exhausted".into());
    let summary = wt_sweep::presentation::compact_summary(&source);
    assert!(
        summary.contains("identity 1, private history 1"),
        "{summary}"
    );
    assert!(
        summary.contains("Local collection budget reached"),
        "{summary}"
    );
    assert!(
        summary.contains("No safe copyable command available"),
        "{summary}"
    );
    assert!(
        summary.contains("Usage by agents or terminals: not checked"),
        "{summary}"
    );
    assert!(summary.lines().count() <= 80, "{summary}");
    Ok(())
}

#[test]
fn progress_history_exposes_collected_facts_without_a_verdict() {
    use wt_sweep::review::{ProgressEvent, ProgressObservation};
    let event = ProgressEvent {
        phase: "history_observation",
        processed: 1,
        stop_reason: Some("limit".into()),
        observation: Some(ProgressObservation::History {
            path: BytePath::from_bytes(b"/fixture/odd\nname"),
            head: Some("a".repeat(40)),
            target_ref: BytePath::from_bytes(b"refs/heads/main"),
            target_oid: Some("b".repeat(40)),
            result: "unknown".into(),
            complete: false,
        }),
    };
    let output = wt_sweep::presentation::review_progress(&event);
    assert!(
        output.starts_with("Observed history: unknown (provisional, partial)"),
        "{output}"
    );
    for value in ["a".repeat(40), "b".repeat(40), "refs/heads/main".into()] {
        assert!(output.contains(&value), "{output}");
    }
    assert!(!output.contains('\n'));
    assert!(!output.contains("cleanup_candidate"));
}

#[test]
fn progress_local_retains_partial_and_unknown_counts() {
    use wt_sweep::review::{ProgressEvent, ProgressObservation};
    let summary = LocalSummary {
        staged: LocalCount {
            observed: 2,
            complete: false,
            ..Default::default()
        },
        unstaged: LocalCount {
            observed: 0,
            complete: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let event = ProgressEvent {
        phase: "local_observation",
        processed: 1,
        stop_reason: None,
        observation: Some(ProgressObservation::Local {
            path: BytePath::from_bytes(b"/fixture/work"),
            summary: Box::new(summary),
        }),
    };
    let output = wt_sweep::presentation::review_progress(&event);
    assert!(
        output.starts_with("Observed local: staged=>=2 unstaged=0"),
        "{output}"
    );
    assert!(output.contains("untracked=UNKNOWN"), "{output}");
    assert!(output.contains("ignored=UNKNOWN"), "{output}");
    assert!(output.contains("provisional"));
    assert!(!output.contains("cleanup_candidate"));
}

fn history_fact(result: &str) -> HistoryObservation {
    HistoryObservation {
        source_oid: Some("a".repeat(40)),
        target_ref: BytePath::from_bytes(b"refs/heads/main"),
        target_oid: Some("b".repeat(40)),
        result: result.into(),
        observed_result: None,
        started_unix_ms: 10,
        finished_unix_ms: 20,
        stop_reason: None,
    }
}

fn recap_disk(rows: &[(&[u8], Option<u64>, bool)]) -> wt_sweep::disk::DiskUsage {
    use wt_sweep::disk::*;
    DiskUsage {
        requested_paths: None,
        started_unix_ms: 10,
        finished_unix_ms: 20,
        elapsed_ms: 10,
        limits: Default::default(),
        complete: false,
        temporal_consistency: "not_guaranteed".into(),
        // Deliberately not the sum of rows: their hardlinks can overlap.
        unique_bytes: DiskBytes {
            allocated: 2_500_000_000,
            logical: 2_000_000_000,
        },
        measurements: rows
            .iter()
            .map(|(path, bytes, complete)| DiskMeasurement {
                requested: true,
                path: BytePath::from_bytes(path),
                kind: DiskKind::Worktree,
                identity: None,
                bytes: bytes.map(|allocated| DiskBytes {
                    allocated,
                    logical: allocated,
                }),
                complete: *complete,
                entries_checked: 1,
                excluded_registered_roots: 0,
                elapsed_ms: 1,
                gaps: vec![],
                gaps_omitted: 0,
            })
            .collect(),
    }
}

#[test]
fn actionable_recap_separates_commit_relations_from_cleanup_decisions()
-> Result<(), Box<dyn std::error::Error>> {
    let mut source = report()?;
    source.items[0].history = vec![history_fact("ancestor"), history_fact("ancestor")];
    source.items[1].history = vec![history_fact("not_ancestor")];
    let mut provisional = history_fact("unknown");
    provisional.observed_result = Some("ancestor".into());
    source.items[2].history = vec![provisional];
    let before = serde_json::to_vec(&source)?;
    let text = wt_sweep::presentation::compact_summary(&source);
    assert!(text.contains("HEAD matches or is an ancestor: 1"), "{text}");
    assert!(
        text.contains("Not an ancestor of any checked target: 1"),
        "{text}"
    );
    assert!(text.contains("Relationship unknown: 1"), "{text}");
    assert!(text.contains("Cleanup candidates: 1"), "{text}");
    assert_eq!(before, serde_json::to_vec(&source)?);
    source.items[1].history_omitted_targets = 1;
    let text = wt_sweep::presentation::compact_summary(&source);
    assert!(
        text.contains("Relationship unknown: 2"),
        "an omitted target cannot be counted negative: {text}"
    );
    Ok(())
}

#[test]
fn actionable_recap_leads_with_observed_disk_and_ranks_only_selected_worktrees()
-> Result<(), Box<dyn std::error::Error>> {
    let mut source = report()?;
    source.items[2].path = BytePath::from_bytes(b"/fixture/largest");
    source.disk_usage = Some(recap_disk(&[
        (b"/fixture/kept", Some(1_000_000_000), true),
        (b"/fixture/largest", Some(2_000_000_000), false),
        (b"/fixture/unselected", Some(9_000_000_000), true),
        (b"/fixture/candidate", None, false),
    ]));
    let text = wt_sweep::presentation::compact_summary(&source);
    assert!(
        text.find("DISK USAGE").ok_or("missing DISK USAGE")?
            < text.find("RESULTS").ok_or("missing RESULTS")?
    );
    assert!(
        text.contains("2.50 GB observed allocated (INCOMPLETE)"),
        "{text}"
    );
    assert!(
        text.contains("Reclaimable space: not established"),
        "{text}"
    );
    let largest = text
        .split("LARGEST WORKTREES")
        .nth(1)
        .ok_or("missing ranked worktrees")?;
    assert!(
        largest
            .find("/fixture/largest")
            .ok_or("missing /fixture/largest")?
            < largest
                .find("/fixture/kept")
                .ok_or("missing /fixture/kept")?
    );
    assert!(largest.contains("2.00 GB [partial]"), "{largest}");
    assert!(!largest.contains("/fixture/unselected"));
    assert!(
        text.contains("1 selected worktree(s) without a size"),
        "{text}"
    );
    assert!(!text.contains("0 B /fixture/candidate"));
    Ok(())
}

#[test]
fn actionable_recap_retry_round_trips_shell_arguments_and_never_guesses_a_target()
-> Result<(), Box<dyn std::error::Error>> {
    let mut source = report()?;
    source.coverage.traversal_complete = true;
    let path = b"/fixture/it's $(printf INJECTED) work";
    source.items[2].path = BytePath::from_bytes(path);
    source.items[2].history = vec![history_fact("unknown")];
    let text = wt_sweep::presentation::compact_summary(&source);
    let command = text
        .lines()
        .find(|line| line.trim_start().starts_with("'wt-sweep' review "))
        .ok_or("missing exact focused command")?
        .trim();
    let output = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(format!("set -- {command}; printf '%s\\n' \"$@\""))
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let args = String::from_utf8(output.stdout)?;
    assert_eq!(
        args.lines().collect::<Vec<_>>(),
        vec![
            "wt-sweep",
            "review",
            "--repo",
            std::str::from_utf8(path)?,
            "--worktree",
            std::str::from_utf8(path)?,
            "--path-prefix",
            std::str::from_utf8(path)?,
            "--target",
            "refs/heads/main",
            "--budget",
            "thorough",
            "--summary",
        ]
    );
    source.items[2].history.clear();
    let text = wt_sweep::presentation::compact_summary(&source);
    assert!(
        !text.contains("'wt-sweep' review"),
        "must not guess target: {text}"
    );
    source.items[2].history = vec![history_fact("unknown")];
    source.items[2].path = BytePath::from_bytes(b"/fixture/unsafe\npath");
    let text = wt_sweep::presentation::compact_summary(&source);
    assert!(
        !text.contains("'wt-sweep' review"),
        "do not offer lossy/control-character command: {text}"
    );
    Ok(())
}
