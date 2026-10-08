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
        "1,23 Ko",
        "8,19 Ko",
        "4,10 Ko",
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
        "8,19 Ko",
        &"b".repeat(64),
        &review.digest_sha256,
        &selected.digest_sha256,
    ] {
        assert!(report.contains(expected), "missing {expected}: {report}");
    }
    assert!(!report.contains("<script>alert(1)"));
    let scripts: Vec<&str> = report
        .split("<script>")
        .skip(1)
        .map(|part| part.split_once("</script>").unwrap().0)
        .collect();
    assert_eq!(
        scripts,
        vec![
            include_str!("../src/presentation/html/theme.js"),
            include_str!("../src/presentation/html/report.js"),
        ]
    );
    assert_eq!(report.matches("</script>").count(), 2);
    assert!(report.find("<script>").unwrap() < report.find("</head>").unwrap());
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
            "8,19 Ko",
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
    assert!(!report.contains("0 o</span>"));
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

#[test]
fn disk_sizes_use_decimal_units_and_preserve_exact_counts() {
    for (bytes, label) in [
        (0, "0 o"),
        (999, "999 o"),
        (1000, "1,00 Ko"),
        (1234, "1,23 Ko"),
        (999_994, "999,99 Ko"),
        (999_995, "1,00 Mo"),
        (1_000_000, "1,00 Mo"),
        (38_037_770_240, "38,04 Go"),
        (33_181_323_981, "33,18 Go"),
        (1_000_000_000_000, "1,00 To"),
        (u64::MAX, "18,45 Eo"),
    ] {
        let (mut inventory, _) = fixture();
        let common = inventory.families[0].common_dir.clone();
        let usage = inventory.disk_usage.as_mut().unwrap();
        usage.unique_bytes = DiskBytes {
            logical: bytes,
            allocated: bytes,
        };
        usage.measurements[0].bytes = Some(usage.unique_bytes.clone());
        let mut shared = usage.measurements[0].clone();
        shared.path = common;
        shared.kind = DiskKind::GitCommon;
        usage.measurements.push(shared);
        let before = serde_json::to_vec(&inventory).unwrap();
        let report = html::inventory(&inventory, 1_000_000).unwrap();
        let value = format!("<span class=\"byte-size\" title=\"{bytes} octets\">{label}</span>");
        assert_eq!(
            report.matches(&value).count(),
            12,
            "overview, shortlist, totals, worktree and shared Git measurements: {label}"
        );
        assert!(report.contains(&format!("data-allocated=\"{bytes}\"")));
        assert!(report.contains("Exact totals (octets)"));
        assert!(report.contains("1 Ko = 1000 octets"));
        assert!(report.contains("partial; totals cover observed entries only"));
        assert_eq!(serde_json::to_vec(&inventory).unwrap(), before);
    }
}

#[test]
fn copy_controls_preserve_path_bytes_and_exclude_branches_and_subjects() {
    let (mut inventory, review) = fixture();
    let unicode = bp("/synthetic/été & \"notes\"/résumé".as_bytes());
    let newline = bp(b"/synthetic/line\nbreak");
    let selected = view::build(&inventory, &review, Default::default(), "repo").unwrap();
    inventory.scope = ScanScope::Roots {
        paths: vec![unicode.clone(), newline.clone()],
    };
    let before = serde_json::to_vec(&selected).unwrap();
    for report in [
        html::inventory(&inventory, 1_000_000).unwrap(),
        html::review(&selected, 1_000_000).unwrap(),
    ] {
        for path in inventory
            .families
            .iter()
            .flat_map(|f| std::iter::once(&f.common_dir).chain(f.worktrees.iter().map(|w| &w.path)))
        {
            assert!(
                report.contains(&format!("data-copy-path=\"{}\"", path.hex)),
                "missing copy control for {}",
                path.hex
            );
        }
        for non_path in [
            inventory.families[0].worktrees[0].branch.as_ref().unwrap(),
            review.items[0].subject.as_ref().unwrap(),
        ] {
            assert!(!report.contains(&format!("data-copy-path=\"{}\"", non_path.hex)));
        }
        assert!(report.contains("id=\"copy-status\""));
        assert!(report.contains("<dialog id=\"copy-dialog\""));
        assert!(report.contains("readonly"));
    }
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    for path in [
        &unicode,
        &newline,
        &inventory.coverage.gaps[0].path,
        &inventory.disk_usage.as_ref().unwrap().measurements[0].gaps[0].path,
    ] {
        assert!(report.contains(&format!("data-copy-path=\"{}\"", path.hex)));
    }
    assert_eq!(serde_json::to_vec(&selected).unwrap(), before);
}

fn disk_priority_section(report: &str) -> &str {
    report
        .split_once("id=\"disk-priority\"")
        .unwrap()
        .1
        .split_once("<div class=\"notice\">")
        .unwrap()
        .0
}

#[test]
fn disk_priority_retains_source_scope_without_claiming_reclaimable_bytes() {
    let (mut inventory, mut review) = fixture();
    let usage = inventory.disk_usage.as_mut().unwrap();
    let original = usage.measurements[0].clone();
    for (path, bytes, complete, kind) in [
        (
            b"/synthetic/large-a".as_slice(),
            Some(6000),
            false,
            DiskKind::Worktree,
        ),
        (
            b"/synthetic/large-b".as_slice(),
            Some(6000),
            true,
            DiskKind::Worktree,
        ),
        (
            b"/synthetic/shared".as_slice(),
            Some(999999),
            true,
            DiskKind::GitCommon,
        ),
        (
            b"/synthetic/unknown".as_slice(),
            None,
            false,
            DiskKind::Worktree,
        ),
    ] {
        let mut measurement = original.clone();
        measurement.path = bp(path);
        measurement.bytes = bytes.map(|allocated| DiskBytes {
            allocated,
            logical: allocated,
        });
        measurement.complete = complete;
        measurement.kind = kind;
        usage.measurements.push(measurement);
    }
    review.disk_usage = inventory.disk_usage.clone();
    review.inventory_sha256 = canonical_digest(&inventory).unwrap();
    review.digest_sha256.clear();
    review.digest_sha256 = canonical_digest(&review).unwrap();
    let selected = view::build(
        &inventory,
        &review,
        ReviewFilters {
            path_prefix: Some(bp(b"/no-match")),
            ..Default::default()
        },
        "repo",
    )
    .unwrap();
    assert!(selected.selected_paths.is_empty());
    let before = serde_json::to_vec(&selected).unwrap();
    let report = html::review(&selected, 1_000_000).unwrap();
    let section = disk_priority_section(&report);
    assert!(
        section.contains("8,19 Ko"),
        "retain deduplicated source total"
    );
    assert!(
        !section.contains("12,00 Ko"),
        "never sum overlapping row allocations"
    );
    assert!(section.contains("Recoverable space</span><strong>Not established"));
    assert!(
        section.contains("1 candidate"),
        "source count survives empty view"
    );
    assert!(section.contains("2/5 roots fully measured"));
    assert!(section.contains("2 partial; 1 not measured"));
    assert!(section.contains("APFS"));
    assert!(section.contains("unaffected by filters"));
    assert!(
        !section.contains("/synthetic/shared"),
        "shared Git excluded from worktree shortlist"
    );
    assert!(
        !section.contains("/synthetic/unknown"),
        "unknown size is not ranked as zero"
    );
    assert!(
        section.find("/synthetic/large-a").unwrap() < section.find("/synthetic/large-b").unwrap()
    );
    assert!(
        section.find("/synthetic/large-b").unwrap() < section.find("/synthetic/a/project").unwrap()
    );
    assert!(section.contains("Partial observation"));
    assert!(
        report.find("id=\"disk-priority\"").unwrap()
            < report.find("No removal authorized.</strong>").unwrap()
    );
    assert_eq!(serde_json::to_vec(&selected).unwrap(), before);
}

#[test]
fn disk_priority_distinguishes_absent_measurements_from_measured_zero() {
    let (mut inventory, _) = fixture();
    for empty in [false, true] {
        let mut source = inventory.clone();
        if empty {
            source.disk_usage.as_mut().unwrap().measurements.clear();
        } else {
            source.disk_usage = None;
        }
        let report = html::inventory(&source, 1_000_000).unwrap();
        let section = disk_priority_section(&report);
        assert!(section.contains("Not measured"));
        assert!(section.contains("Not reviewed"));
        assert!(!section.contains("0 o</span>"));
    }
    let usage = inventory.disk_usage.as_mut().unwrap();
    usage.unique_bytes = DiskBytes::default();
    usage.measurements[0].bytes = Some(DiskBytes::default());
    usage.measurements[0].complete = true;
    usage.complete = true;
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    let section = disk_priority_section(&report);
    assert!(section.contains("0 o</span>"));
    assert!(section.contains("1/1 roots fully measured"));
    assert!(section.contains("Recoverable space</span><strong>Not established"));
}
