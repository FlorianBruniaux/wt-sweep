#![allow(clippy::unwrap_used)]
use wt_sweep::{model::*, output::canonical_digest, view};
fn bp(value: &[u8]) -> BytePath {
    BytePath::from_bytes(value)
}
fn source() -> Result<(Inventory, Review), SweepError> {
    let mut inventory = Inventory::new(ScanScope::Roots {
        paths: vec![bp(b"/fixture")],
    });
    inventory.coverage.traversal_complete = false;
    inventory.coverage.gaps.push(Gap {
        path: bp(b"/fixture/inaccessible"),
        reason: "synthetic permission refusal".into(),
    });
    let paths: [&[u8]; 3] = [
        b"/fixture/a/one",
        b"/fixture/ab/two",
        b"/fixture/a/raw-\xff",
    ];
    for (n, path) in paths.iter().enumerate() {
        inventory.families.push(Family {
            common_dir: bp(format!("/fixture/admin-{n}").as_bytes()),
            identity: Identity {
                device: 1,
                inode: n as u64 + 1,
            },
            worktrees: vec![Worktree {
                path: bp(path),
                identity: None,
                head: Some("a".repeat(40)),
                branch: Some(bp(if n == 2 {
                    b"refs/heads/raw-\xff"
                } else {
                    b"refs/heads/main"
                })),
                detached: false,
                bare: false,
                locked: None,
                prunable: None,
                path_state: DirState::Present,
            }],
        });
    }
    let facts = Observations {
        context: None,
        inventory: inventory.clone(),
        shared_stashes: vec![],
        history_metrics: Default::default(),
        items: vec![],
        forge: None,
        started_unix_ms: 10,
        finished_unix_ms: 20,
    };
    let mut report = wt_sweep::review::judge(&facts)?;
    for (n, family) in inventory.families.iter().enumerate() {
        let tree = &family.worktrees[0];
        report.items.push(ReviewItem {
            worktree_state: None,
            decision: Some(TriageDecision {
                keep_reasons: vec![],
                missing_conditions: if n == 0 { vec![] } else { vec!["C3".into()] },
            }),
            path: tree.path.clone(),
            private_history: None,
            history: vec![],
            history_omitted_targets: 0,
            local_summary: Some(LocalSummary {
                ignored: LocalCount {
                    observed: usize::from(n == 2),
                    complete: false,
                    ..Default::default()
                },
                ..Default::default()
            }),
            branch: tree.branch.clone(),
            head: tree.head.clone(),
            subject: None,
            subject_truncated: false,
            classification: if n == 0 {
                "cleanup_candidate"
            } else if n == 1 {
                "verify"
            } else {
                "keep"
            }
            .into(),
            observations: vec![],
            requests: vec![],
            risks: vec![],
            unknowns: vec![],
        });
    }
    report.digest_sha256 = canonical_digest(&report)?;
    Ok((inventory, report))
}
#[test]
fn component_prefix_and_raw_branch_preserve_every_source_byte_and_both_digests()
-> Result<(), SweepError> {
    let (inventory, report) = source()?;
    let before = serde_json::to_vec(&report).unwrap();
    let filtered = view::build(
        &inventory,
        &report,
        ReviewFilters {
            path_prefix: Some(bp(b"/fixture/a")),
            ..Default::default()
        },
        "directory",
    )?;
    assert_eq!(
        filtered.selected_paths,
        vec![bp(b"/fixture/a/one"), bp(b"/fixture/a/raw-\xff")]
    );
    assert_eq!(serde_json::to_vec(&filtered.source).unwrap(), before);
    assert_eq!(canonical_digest(&filtered)?, filtered.digest_sha256);
    assert_eq!(canonical_digest(&filtered.source)?, report.digest_sha256);
    let raw = view::build(
        &inventory,
        &report,
        ReviewFilters {
            branch: Some(bp(b"refs/heads/raw-\xff")),
            ..Default::default()
        },
        "repo",
    )?;
    assert_eq!(raw.selected_paths, vec![bp(b"/fixture/a/raw-\xff")]);
    Ok(())
}
#[test]
fn empty_selection_human_report_keeps_full_counts_gaps_and_source_bindings()
-> Result<(), SweepError> {
    let (inventory, report) = source()?;
    let filtered = view::build(
        &inventory,
        &report,
        ReviewFilters {
            repository: Some(bp(b"/absent")),
            ..Default::default()
        },
        "repo",
    )?;
    assert!(filtered.selected_paths.is_empty());
    assert_eq!(filtered.row_bindings.len(), 3);
    let summary = wt_sweep::presentation::view_summary(&filtered);
    assert_eq!(summary.lines().count(), 5);
    for expected in [
        "0/3",
        "source totals: keep=1, candidates=1, verify=1",
        "Source coverage gaps: 1",
        "traversal complete: false",
        "Usage: UNKNOWN",
    ] {
        assert!(summary.contains(expected), "{summary}");
    }
    let detail = wt_sweep::presentation::view_detail(&filtered)?;
    assert!(
        detail.contains("/fixture/inaccessible: synthetic permission refusal"),
        "{detail}"
    );
    assert!(detail.contains(&report.digest_sha256));
    assert!(detail.contains(&filtered.digest_sha256));
    Ok(())
}
#[test]
fn typed_positive_counts_win_over_partial_zero_and_clean_uses_c3() -> Result<(), SweepError> {
    let (inventory, report) = source()?;
    for (state, path) in [
        ("clean", b"/fixture/a/one".as_slice()),
        ("unknown", b"/fixture/ab/two".as_slice()),
        ("dirty", b"/fixture/a/raw-\xff".as_slice()),
    ] {
        let filtered = view::build(
            &inventory,
            &report,
            ReviewFilters {
                local_state: Some(state.into()),
                ..Default::default()
            },
            "repo",
        )?;
        assert_eq!(filtered.selected_paths, vec![bp(path)], "{state}");
    }
    Ok(())
}
#[test]
fn source_tampering_and_ambiguous_bindings_refuse_instead_of_hiding_rows() -> Result<(), SweepError>
{
    let (mut inventory, mut report) = source()?;
    report.items[0].classification = "keep".into();
    assert!(view::build(&inventory, &report, Default::default(), "repo").is_err());
    report.digest_sha256 = canonical_digest(&report)?;
    inventory.families[1].worktrees[0].path = inventory.families[0].worktrees[0].path.clone();
    assert!(view::build(&inventory, &report, Default::default(), "repo").is_err());
    report.inventory_sha256 = canonical_digest(&inventory)?;
    report.digest_sha256 = canonical_digest(&report)?;
    assert!(view::build(&inventory, &report, Default::default(), "repo").is_err());
    Ok(())
}
#[test]
fn visible_group_order_is_raw_byte_order_and_filter_intersection_is_exact() -> Result<(), SweepError>
{
    let (inventory, report) = source()?;
    let filtered = view::build(
        &inventory,
        &report,
        ReviewFilters {
            repository: Some(bp(b"/fixture/a/raw-\xff")),
            verdict: Some("keep".into()),
            ..Default::default()
        },
        "directory",
    )?;
    assert_eq!(filtered.selected_paths, vec![bp(b"/fixture/a/raw-\xff")]);
    let detail = wt_sweep::presentation::view_detail(&filtered)?;
    assert!(detail.contains("Group: /fixture/a"));
    assert!(detail.contains("/fixture/a/raw-\\xff"));
    assert!(!detail.contains("/fixture/ab/two\t"));
    let empty = view::build(
        &inventory,
        &report,
        ReviewFilters {
            repository: Some(bp(b"/fixture/a/raw-\xff")),
            verdict: Some("verify".into()),
            ..Default::default()
        },
        "repo",
    )?;
    assert!(empty.selected_paths.is_empty());
    Ok(())
}

#[test]
fn filtered_detail_byte_limit_keeps_full_source_and_refuses_partial_text() -> Result<(), SweepError>
{
    let (inventory, report) = source()?;
    let view = view::build(&inventory, &report, ReviewFilters::default(), "repo")?;
    let before = serde_json::to_vec(&view).unwrap();
    let expected = wt_sweep::presentation::view_detail(&view)?;
    assert_eq!(
        wt_sweep::presentation::view_detail_bounded(&view, expected.len())?,
        expected
    );
    assert!(wt_sweep::presentation::view_detail_bounded(&view, expected.len() - 1).is_err());
    assert!(wt_sweep::presentation::view_detail_bounded(&view, 0).is_err());
    assert_eq!(serde_json::to_vec(&view).unwrap(), before);
    Ok(())
}

#[test]
fn empty_selection_preserves_source_coverage_omissions() -> Result<(), SweepError> {
    let (mut inventory, mut report) = source()?;
    inventory.coverage.publication = Some(PublicationOmissions {
        local_inventories: 2,
        markers: 1,
        families: 1,
        worktrees: 4,
        cover: PublicationCover::RequestedScopeAndRegisteredWorktrees,
    });
    report.coverage = inventory.coverage.clone();
    report.inventory_sha256 = canonical_digest(&inventory)?;
    report.digest_sha256 = canonical_digest(&report)?;
    let filtered = view::build(
        &inventory,
        &report,
        ReviewFilters {
            path_prefix: Some(bp(b"/missing")),
            ..Default::default()
        },
        "repo",
    )?;
    assert!(filtered.selected_paths.is_empty());
    let detail = wt_sweep::presentation::view_detail(&filtered)?;
    assert!(
        detail
            .contains("Publication omitted: 1 families, 4 worktrees, 2 local manifests, 1 markers"),
        "{detail}"
    );
    let summary = wt_sweep::presentation::view_summary(&filtered);
    assert_eq!(summary.lines().count(), 5);
    assert!(summary.contains("unpublished worktrees: 4"), "{summary}");
    Ok(())
}
