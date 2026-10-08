#![allow(clippy::unwrap_used)]
use wt_sweep::{
    disk::{DiskBytes, DiskKind, DiskLimits, DiskMeasurement, DiskUsage},
    model::*,
    output::canonical_digest,
    presentation::html,
    view,
};

fn bp(bytes: &[u8]) -> BytePath {
    BytePath::from_bytes(bytes)
}
fn fixture() -> (Inventory, Review) {
    let mut inventory = Inventory::new(ScanScope::Roots {
        paths: vec![bp(b"/synthetic")],
    });
    inventory.started_unix_ms = 100;
    inventory.finished_unix_ms = 200;
    inventory.coverage.gaps = vec![Gap {
        path: bp(b"/synthetic/refused"),
        reason: "permission <denied>".into(),
    }];
    inventory.coverage.publication = Some(PublicationOmissions {
        local_inventories: 2,
        markers: 3,
        families: 4,
        worktrees: 5,
        cover: PublicationCover::RequestedScopeAndRegisteredWorktrees,
    });
    for (n, path) in [
        b"/synthetic/a/project".as_slice(),
        b"/synthetic/b/project\xff<&\"'\n",
    ]
    .into_iter()
    .enumerate()
    {
        inventory.families.push(Family {
            common_dir: bp(
                format!("/synthetic/{}/project/.git", if n == 0 { "a" } else { "b" }).as_bytes(),
            ),
            identity: Identity {
                device: 7,
                inode: n as u64 + 10,
            },
            worktrees: vec![Worktree {
                path: bp(path),
                identity: None,
                head: Some(if n == 0 {
                    "a".repeat(40)
                } else {
                    "b".repeat(64)
                }),
                branch: if n == 0 {
                    Some(bp(b"refs/heads/topic<&\""))
                } else {
                    None
                },
                detached: n == 1,
                bare: false,
                locked: None,
                prunable: None,
                path_state: DirState::Present,
            }],
        });
    }
    inventory.disk_usage = Some(DiskUsage {
        started_unix_ms: 101,
        finished_unix_ms: 150,
        elapsed_ms: 49,
        limits: DiskLimits::default(),
        complete: false,
        temporal_consistency: "not_guaranteed".into(),
        unique_bytes: DiskBytes {
            logical: 1234,
            allocated: 8192,
        },
        measurements: vec![DiskMeasurement {
            path: inventory.families[0].worktrees[0].path.clone(),
            kind: DiskKind::Worktree,
            identity: None,
            bytes: Some(DiskBytes {
                logical: 1234,
                allocated: 4096,
            }),
            complete: false,
            entries_checked: 3,
            excluded_registered_roots: 0,
            elapsed_ms: 49,
            gaps: vec![Gap {
                path: bp(b"/synthetic/a/ignored"),
                reason: "disk refused".into(),
            }],
            gaps_omitted: 6,
        }],
    });
    let observations = Observations {
        context: None,
        inventory: inventory.clone(),
        shared_stashes: vec![],
        history_metrics: Default::default(),
        collection_metrics: None,
        items: vec![],
        forge: None,
        started_unix_ms: 201,
        finished_unix_ms: 300,
    };
    let mut review = wt_sweep::review::judge(&observations).unwrap();
    review.items = inventory
        .families
        .iter()
        .enumerate()
        .map(|(n, family)| {
            let tree = &family.worktrees[0];
            ReviewItem {
                path: tree.path.clone(),
                branch: tree.branch.clone(),
                head: tree.head.clone(),
                subject: Some(bp(b"subject </script><script>alert(1)</script>")),
                subject_truncated: n == 1,
                classification: if n == 0 { "cleanup_candidate" } else { "keep" }.into(),
                worktree_state: Some(WorktreeState {
                    observed: Some(tree.clone()),
                    common_dir: family.common_dir.clone(),
                    family_identity: Some(family.identity.clone()),
                    git_dir: None,
                    git_dir_identity: None,
                    role: "linked".into(),
                    protections: vec![],
                    protection_complete: false,
                    operations: vec![],
                    operations_complete: false,
                    checks: "partial".into(),
                    started_unix_ms: 201,
                    finished_unix_ms: 300,
                    sources: vec![],
                    gaps: vec!["controls absent".into()],
                }),
                decision: Some(TriageDecision {
                    keep_reasons: if n == 1 {
                        vec!["local_staged".into()]
                    } else {
                        vec![]
                    },
                    missing_conditions: if n == 1 {
                        vec!["C2".into(), "C5".into()]
                    } else {
                        vec![]
                    },
                }),
                private_history: None,
                history: vec![],
                history_omitted_targets: 0,
                local_summary: None,
                observations: vec![],
                requests: vec![],
                risks: vec!["risk </script>\u{202e}hello".into()],
                unknowns: vec!["unknown & unfinished".into()],
            }
        })
        .collect();
    review.digest_sha256 = canonical_digest(&review).unwrap();
    (inventory, review)
}

#[test]
fn inventory_retains_distinct_families_bytes_sizes_and_untriaged_status() {
    let (inventory, _) = fixture();
    let before = serde_json::to_vec(&inventory).unwrap();
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    for expected in [
        "/synthetic/a/project/.git",
        "/synthetic/b/project/.git",
        "project\\xff&lt;&amp;&quot;&#39;\\n",
        "refs/heads/topic&lt;&amp;&quot;",
        "detached",
        "Not triaged",
        "1234 B",
        "8192 B",
        "4096 B",
        "partial",
        "disk refused",
        "Additional disk gaps: 6",
        "permission &lt;denied&gt;",
        "5 worktrees",
        "Unix ms",
        "not_guaranteed",
    ] {
        assert!(report.contains(expected), "missing {expected}: {report}");
    }
    assert_eq!(report.matches("data-worktree=\"").count(), 2);
    assert_eq!(serde_json::to_vec(&inventory).unwrap(), before);
}

#[test]
fn review_preserves_source_and_view_totals_and_exact_decisions_without_script_injection() {
    let (inventory, review) = fixture();
    let selected = view::build(&inventory, &review, Default::default(), "repo").unwrap();
    let before = serde_json::to_vec(&selected).unwrap();
    let report = html::review(&selected, 1_000_000).unwrap();
    for expected in [
        "cleanup_candidate",
        "keep",
        "local_staged",
        "C2",
        "C5",
        "controls absent",
        "unknown &amp; unfinished",
        "&lt;/script&gt;&lt;script&gt;alert(1)&lt;/script&gt;",
        "\\u{202e}",
        "No removal authorized",
        "8192 B",
        &"b".repeat(64),
        &review.digest_sha256,
        &selected.digest_sha256,
    ] {
        assert!(report.contains(expected), "missing {expected}: {report}");
    }
    assert!(!report.contains("<script>alert(1)"));
    assert_eq!(report.matches("<script>").count(), 1);
    assert_eq!(report.matches("data-worktree=\"").count(), 2);
    assert_eq!(serde_json::to_vec(&selected).unwrap(), before);
}

#[test]
fn filtered_and_empty_views_keep_original_counts_coverage_and_deduplicated_total() {
    let (inventory, review) = fixture();
    for (verdict, rows) in [("keep", 1), ("verify", 0)] {
        let selected = view::build(
            &inventory,
            &review,
            ReviewFilters {
                verdict: Some(verdict.into()),
                ..Default::default()
            },
            "repo",
        )
        .unwrap();
        let report = html::review(&selected, 1_000_000).unwrap();
        assert_eq!(report.matches("data-worktree=\"").count(), rows);
        for expected in [
            "Source totals",
            "Selected totals",
            "8192 B",
            "permission &lt;denied&gt;",
            "5 worktrees",
            "1 cleanup_candidate",
            "1 keep",
        ] {
            assert!(report.contains(expected), "missing {expected}: {report}");
        }
        if rows == 0 {
            assert!(report.contains("No worktrees selected"));
        }
    }
}

#[test]
fn output_limit_is_exact_and_refuses_a_partial_document() {
    let (inventory, review) = fixture();
    let selected = view::build(&inventory, &review, Default::default(), "repo").unwrap();
    let full = html::review(&selected, 1_000_000).unwrap();
    assert!(!full.is_empty(), "renderer must produce a document");
    assert_eq!(html::review(&selected, full.len()).unwrap(), full);
    assert_eq!(
        html::review(&selected, full.len() - 1).unwrap_err().code,
        "publication_limit"
    );
    assert!(html::inventory(&inventory, 0).is_err());
    let mut large = inventory;
    large.coverage.gaps[0].reason = "<&".repeat(1_000_000);
    assert_eq!(
        html::inventory(&large, 4096).unwrap_err().code,
        "publication_limit"
    );
}

#[test]
fn missing_measurements_and_current_branch_uncertainty_stay_visible() {
    let (mut inventory, _) = fixture();
    inventory.disk_usage = None;
    inventory.families[0].worktrees[0].branch = None;
    inventory.families[0].worktrees[0].detached = false;
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    assert!(report.contains("Not measured"));
    assert!(report.contains("Branch UNKNOWN"));
    assert!(!report.contains("0 B"));
}

#[test]
fn family_index_links_to_each_distinct_family_without_using_hostile_names_as_ids() {
    let (inventory, review) = fixture();
    let selected = view::build(&inventory, &review, Default::default(), "repo").unwrap();
    for report in [
        html::inventory(&inventory, 1_000_000).unwrap(),
        html::review(&selected, 1_000_000).unwrap(),
    ] {
        assert!(report.contains("href=\"#family-0\""));
        assert!(report.contains("href=\"#family-1\""));
        assert!(report.contains("id=\"family-0\""));
        assert!(report.contains("id=\"family-1\""));
    }
}

#[test]
fn invalid_hex_paths_and_ambiguous_or_missing_selected_rows_are_refused() {
    let (mut inventory, report) = fixture();
    let mut selected = view::build(&inventory, &report, Default::default(), "repo").unwrap();
    selected
        .selected_paths
        .push(selected.selected_paths[0].clone());
    assert_eq!(
        html::review(&selected, 1_000_000).unwrap_err().code,
        "invalid_review_view"
    );
    selected.selected_paths.pop();
    selected.selected_paths[0] = bp(b"/missing");
    assert_eq!(
        html::review(&selected, 1_000_000).unwrap_err().code,
        "invalid_review_view"
    );
    for bad in ["00", "abcdF", "' onclick='alert(1)"] {
        inventory.families[0].worktrees[0].path.hex = bad.into();
        assert_eq!(
            html::inventory(&inventory, 1_000_000).unwrap_err().code,
            "invalid_path"
        );
    }
}

#[test]
fn observations_display_the_exact_utc_interval_without_replacing_original_timestamps() {
    let (mut inventory, _) = fixture();
    inventory.started_unix_ms = 1_709_251_199_123;
    inventory.finished_unix_ms = 1_709_251_200_456;
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    assert!(report.contains("2024-02-29 23:59:59.123 UTC"));
    assert!(report.contains("2024-03-01 00:00:00.456 UTC"));
    assert!(report.contains("1709251199123..1709251200456"));
}

#[test]
fn recorded_evidence_remains_valid_copyable_json_with_hostile_unicode_and_newlines() {
    let (inventory, review) = fixture();
    let selected = view::build(&inventory, &review, Default::default(), "repo").unwrap();
    let report = html::review(&selected, 1_000_000).unwrap();
    let mut records = 0;
    for block in report.split("<pre>").skip(1) {
        let encoded = block.split("</pre>").next().unwrap();
        let json = encoded
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&amp;", "&");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        if let Some(risks) = parsed.get("risks") {
            assert_eq!(risks, &serde_json::json!(["risk </script>\u{202e}hello"]));
            records += 1;
        }
    }
    assert_eq!(
        records, 2,
        "all selected worktree evidence must remain valid JSON"
    );
}

#[test]
fn local_manifest_summaries_retain_per_worktree_gaps_omissions_and_completeness() {
    let (mut inventory, _) = fixture();
    inventory.coverage.gaps.push(Gap {
        path: inventory.families[0].worktrees[0].path.clone(),
        reason: "local inventory is incomplete; see local_inventories gaps".into(),
    });
    inventory.local_inventories = vec![
        LocalInventory {
            worktree: bp(b"/synthetic/manifest-\xff<&"),
            started_unix_ms: 401,
            finished_unix_ms: 499,
            entries: vec![],
            index_entries: vec![],
            administration_entries: vec![],
            traversal_complete: false,
            temporal_consistency: "manifest_interval_not_atomic".into(),
            gaps: vec![Gap {
                path: bp(b"/synthetic/manifest-\xff/ignored<&"),
                reason: "unique manifest gap </summary><script>alert(7)</script>".into(),
            }],
            not_collected: vec!["unique local omission ACLs & attributes".into()],
            content_digest_sha256: "c".repeat(64),
        },
        LocalInventory {
            worktree: bp(b"/synthetic/complete-manifest"),
            started_unix_ms: 501,
            finished_unix_ms: 599,
            entries: vec![],
            index_entries: vec![],
            administration_entries: vec![],
            traversal_complete: true,
            temporal_consistency: "not_guaranteed".into(),
            gaps: vec![],
            not_collected: vec![],
            content_digest_sha256: "d".repeat(64),
        },
    ];
    let before = serde_json::to_vec(&inventory).unwrap();
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    for expected in [
        "Local manifest: /synthetic/manifest-\\xff&lt;&amp;",
        "Local manifest: /synthetic/complete-manifest",
        "Traversal: incomplete",
        "Traversal: complete",
        "401..499",
        "501..599",
        "manifest_interval_not_atomic",
        "/synthetic/manifest-\\xff/ignored&lt;&amp;",
        "unique manifest gap &lt;/summary&gt;&lt;script&gt;alert(7)&lt;/script&gt;",
        "unique local omission ACLs &amp; attributes",
        &"c".repeat(64),
        &"d".repeat(64),
        "not a backup",
        "Local bytes have not been revalidated",
    ] {
        assert!(report.contains(expected), "missing {expected}: {report}");
    }
    assert!(!report.contains("<script>alert(7)"));
    assert_eq!(serde_json::to_vec(&inventory).unwrap(), before);
    assert_eq!(
        html::inventory(&inventory, report.len() - 1)
            .unwrap_err()
            .code,
        "publication_limit"
    );
}
