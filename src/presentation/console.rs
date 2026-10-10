//! Terminal summaries derived from the recorded facts, without new collection.
use super::text;
use crate::{
    discover::{DiscoveryPhase, DiscoveryProgress},
    model::{BytePath, Coverage, LocalSummary, Review, ReviewItem, ReviewView},
    review::{ProgressEvent, ProgressObservation},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

fn counts(rows: &[&ReviewItem]) -> (usize, usize, usize) {
    let keep = rows.iter().filter(|i| i.classification == "keep").count();
    let candidates = rows
        .iter()
        .filter(|i| i.classification == "cleanup_candidate")
        .count();
    (
        keep,
        candidates,
        rows.len().saturating_sub(keep + candidates),
    )
}

fn local_complete(s: &LocalSummary) -> bool {
    s.head_complete
        && s.index_complete
        && s.disk_complete
        && s.gaps.is_empty()
        && s.temporal_consistency != "change_observed"
        && [
            &s.staged,
            &s.unstaged,
            &s.conflicts,
            &s.untracked,
            &s.ignored,
        ]
        .iter()
        .all(|c| c.complete)
}

fn local_collected(s: &LocalSummary) -> bool {
    s.worktree_identity.is_some()
        || s.git_dir.is_some()
        || s.head.is_some()
        || !s.sources.is_empty()
        || s.head_complete
        || s.index_complete
        || s.disk_complete
        || [
            &s.staged,
            &s.unstaged,
            &s.conflicts,
            &s.untracked,
            &s.ignored,
        ]
        .iter()
        .any(|c| c.complete || c.observed > 0)
}

/// Bounded in lines regardless of worktree count. Coverage always describes the source.
pub fn summary(review: &Review) -> String {
    render(review, &review.items.iter().collect::<Vec<_>>(), false)
}

pub fn view_summary(view: &ReviewView) -> String {
    let selected: BTreeSet<_> = view.selected_paths.iter().collect();
    let rows: Vec<_> = view
        .source
        .items
        .iter()
        .filter(|i| selected.contains(&i.path))
        .collect();
    render(&view.source, &rows, true)
}

fn render(review: &Review, rows: &[&ReviewItem], filtered: bool) -> String {
    let total = review.items.len();
    let (keep, candidates, verify) = counts(rows);
    let incomplete = review
        .items
        .iter()
        .filter(|i| {
            i.decision
                .as_ref()
                .is_none_or(|d| !d.missing_conditions.is_empty())
        })
        .count();
    let discovery_complete = review.coverage.traversal_complete
        && review.coverage.remaining.is_none()
        && review.coverage.publication.is_none()
        && review.coverage.gaps.is_empty();
    let mut output = String::from("\nWORKTREE SWEEP\n========================================\n");
    if filtered {
        let _ = writeln!(output, "Worktrees selected: {}/{total}", rows.len());
    } else {
        let _ = writeln!(output, "Worktrees: {total}");
    }
    let not_requested = review
        .items
        .iter()
        .filter(|item| {
            item.unknowns
                .iter()
                .any(|reason| reason == "Not selected for evidence collection (--worktree).")
        })
        .count();
    if not_requested > 0 {
        let _ = writeln!(
            output,
            "Targeted collection: {} worktree(s); {not_requested} not requested",
            total - not_requested
        );
    }
    let targets: BTreeSet<_> = review
        .items
        .iter()
        .flat_map(|i| &i.history)
        .map(|h| &h.target_ref)
        .collect();
    if !targets.is_empty() {
        let _ = writeln!(
            output,
            "Targets: {}{}",
            targets
                .iter()
                .take(3)
                .map(|t| text(&t.display(), 160))
                .collect::<Vec<_>>()
                .join(", "),
            if targets.len() > 3 {
                " (more in report)"
            } else {
                ""
            }
        );
    }
    if total == 0 {
        let _ = writeln!(output, "Decision checks: no worktrees to assess");
    } else if incomplete > 0 {
        let _ = writeln!(
            output,
            "Candidate evidence incomplete: {incomplete}/{total} worktrees (retained worktrees can have incomplete checks)"
        );
    } else {
        let _ = writeln!(
            output,
            "Candidate evidence complete: {total} recorded worktrees"
        );
    }
    let _ = writeln!(
        output,
        "Resolved verdicts: {}/{}; {} still need verification",
        keep + candidates,
        rows.len(),
        verify
    );
    if let Some(metrics) = &review.collection_metrics {
        let _ = writeln!(output, "Collection budget: {}", metrics.budget_profile);
    }
    let _ = writeln!(
        output,
        "\nRESULTS{}\n  Keep: {keep}\n  Cleanup candidates: {candidates}\n  Needs verification: {verify}",
        if filtered {
            " (selected worktrees)"
        } else {
            ""
        }
    );
    if filtered && rows.len() != total {
        let (k, c, v) = counts(&review.items.iter().collect::<Vec<_>>());
        let _ = writeln!(
            output,
            "  Source totals: {k} keep, {c} candidates, {v} to verify"
        );
    }
    crate::disk::render_summary(review.disk_usage.as_ref(), &mut output);
    let complete = review
        .items
        .iter()
        .filter(|i| i.local_summary.as_ref().is_some_and(local_complete))
        .count();
    let collected = review
        .items
        .iter()
        .filter(|i| i.local_summary.as_ref().is_some_and(local_collected))
        .count();
    let history_incomplete = review
        .items
        .iter()
        .filter(|i| {
            i.history.is_empty()
                || i.history_omitted_targets > 0
                || i.history.iter().any(|h| h.result == "unknown")
        })
        .count();
    let _ = writeln!(
        output,
        "\nCOVERAGE (all {total} recorded worktrees)\n  Discovery: {}\n  Source coverage gaps: {}\n  Local checks complete: {complete}\n  Local checks partial: {}\n  Local checks not collected: {}\n  Commit history incomplete or not collected: {history_incomplete}",
        if discovery_complete {
            "complete"
        } else {
            "incomplete"
        },
        review.coverage.gaps.len(),
        collected.saturating_sub(complete),
        total.saturating_sub(collected)
    );
    if let Some(remaining) = &review.coverage.remaining {
        let _ = writeln!(
            output,
            "  Discovery remaining: {} ({} known pending paths)",
            remaining.reason.as_str(),
            remaining.known_pending_paths
        );
    }
    if let Some(omitted) = &review.coverage.publication {
        let _ = writeln!(
            output,
            "  Unpublished worktrees: {}; families: {}; local manifests: {}; markers: {}",
            omitted.worktrees, omitted.families, omitted.local_inventories, omitted.markers
        );
    }
    let _ = writeln!(output, "\nCHECKS STILL NEEDED (unresolved worktrees)");
    let mut missing = false;
    for (code, description) in [
        ("C1", "Current identity, locks and Git operations"),
        ("C2", "Integration into the selected targets"),
        ("C3", "Complete, clean local files"),
        ("C4", "Private history preserved by named refs"),
        ("C5", "Final consistency checks"),
    ] {
        let n = review
            .items
            .iter()
            .filter(|i| i.classification == "verify")
            .filter(|i| {
                i.decision
                    .as_ref()
                    .is_none_or(|d| d.missing_conditions.iter().any(|c| c == code))
            })
            .count();
        if n > 0 {
            let _ = writeln!(output, "  {description}: {n}");
            missing = true;
        }
    }
    if !missing {
        let _ = writeln!(
            output,
            "  None for unresolved worktrees. Retention reasons remain in the detail."
        );
    }
    write_discovery_performance(&review.coverage, &mut output);
    write_collection_performance(review, &mut output);
    let limits = limits(review);
    if !limits.is_empty() {
        let _ = writeln!(output, "\nCOLLECTION LIMITS");
        for reason in &limits {
            let _ = writeln!(output, "  - {reason}");
        }
        if limits.iter().any(|reason| reason.starts_with("Local ")) {
            if not_requested > 0 {
                let _ = writeln!(
                    output,
                    "\nNEXT CHECK\n  Inspect the selected worktree's gaps in the report.\n  The focused run still has per-worktree object, byte and time caps."
                );
            } else {
                let _ = writeln!(
                    output,
                    "\nNEXT CHECK\n  Retry one worktree with --worktree ABSOLUTE_PATH.\n  Add --budget thorough for larger finite allowances. Object and byte caps still apply."
                );
            }
        }
    }
    let _ = writeln!(
        output,
        "\nUsage by agents or terminals: not checked.\nRead-only review; no removal authorized.\nIncomplete checks do not mean a worktree is clean."
    );
    output
}

fn rate(count: u64, elapsed_ms: u64, unit: &str) -> String {
    if elapsed_ms == 0 {
        return "rate unavailable (<1 ms)".into();
    }
    format!("{:.1} {unit}/s", count as f64 * 1000.0 / elapsed_ms as f64)
}

pub fn write_discovery_performance(coverage: &Coverage, output: &mut impl Write) {
    if !coverage.discovery_intervals.is_empty() {
        let _ = writeln!(
            output,
            "Discovery observation intervals: {} (not a snapshot)",
            coverage.discovery_intervals.len()
        );
        for interval in &coverage.discovery_intervals {
            let _ = writeln!(
                output,
                "  Unix ms {}..{}",
                interval.started_unix_ms, interval.finished_unix_ms
            );
        }
    }

    let Some(metrics) = &coverage.discovery_metrics else {
        return;
    };
    let complete = coverage.traversal_complete
        && coverage.gaps.is_empty()
        && coverage.remaining.is_none()
        && coverage.publication.is_none();
    let _ = writeln!(output, "\nDISCOVERY PERFORMANCE (source inventory)");
    let _ = writeln!(
        output,
        "  Elapsed: {:.3}s; metadata workers: {}",
        metrics.elapsed_ms as f64 / 1000.0,
        metrics.workers
    );
    let _ = writeln!(
        output,
        "  Directory read attempts: {}; entries listed: {}",
        metrics.directory_read_attempts, metrics.entries_listed
    );
    if metrics.directory_read_attempts > 0 {
        let _ = writeln!(
            output,
            "  Average discovery throughput: {}",
            rate(metrics.entries_listed as u64, metrics.elapsed_ms, "entries")
        );
    }
    let _ = writeln!(
        output,
        "  Coverage: {}; {} gaps. Timing excludes disk sizing, local analysis and publication.",
        if complete { "complete" } else { "incomplete" },
        coverage.gaps.len()
    );
}

fn phase_title(name: &str) -> &'static str {
    match name {
        "controls" => "Identity and protections",
        "history" => "Commit history",
        "local" => "Local files",
        "deep" => "Candidate object verification",
        "private_history" => "Private history",
        "revalidation" => "Final checks",
        "context" => "Optional context",
        _ => "Other checks",
    }
}

pub fn write_collection_performance(review: &Review, output: &mut impl Write) {
    let Some(metrics) = &review.collection_metrics else {
        return;
    };
    let _ = writeln!(output, "\nCOLLECTION TIME");
    let mut total = 0_u64;
    for phase in &metrics.phases {
        total = total.saturating_add(phase.elapsed_ms);
        let _ = write!(
            output,
            "  {}: {:.1}s",
            phase_title(&phase.name),
            phase.elapsed_ms as f64 / 1000.0
        );
        if let Some(paths) = phase.counters.get("paths_checked") {
            let _ = write!(
                output,
                "; {paths} paths checked ({})",
                rate(*paths, phase.elapsed_ms, "paths")
            );
        }
        if let Some(processes) = phase.counters.get("git_processes") {
            let _ = write!(output, "; {processes} Git processes");
        }
        let _ = writeln!(output);
    }
    let _ = writeln!(
        output,
        "  Recorded phases: {:.1}s (excludes discovery, disk size measurement and publication)",
        total as f64 / 1000.0
    );
    if total > 0
        && let Some(slowest) = metrics.phases.iter().max_by_key(|phase| phase.elapsed_ms)
    {
        let _ = writeln!(
            output,
            "  Slowest phase: {} ({:.1}% of recorded collection time)",
            phase_title(&slowest.name),
            slowest.elapsed_ms as f64 * 100.0 / total as f64
        );
    }
}

// Diagnostic wording only: never used to determine a verdict or completeness.
fn limits(review: &Review) -> BTreeSet<&'static str> {
    let mut limits = BTreeSet::new();
    let mut record = |reason: &str| {
        if reason.contains("history") && reason.contains("budget") {
            limits.insert(if reason.contains("private history") {
                "Private history collection budget reached"
            } else if reason.contains("traversal") {
                "History traversal limit reached"
            } else {
                "History collection budget reached"
            });
        }
        if reason.contains("local") && (reason.contains("budget") || reason.contains("deadline")) {
            limits.insert("Local collection budget reached");
        }
        if reason.contains("summary time budget")
            || reason.contains("summary cancelled or deadline")
        {
            limits.insert("Local time budget reached");
        }
        if reason.contains("summary object limit") {
            limits.insert("Local object limit reached");
        }
        if reason.contains("summary total byte limit")
            || reason.contains("summary object byte limit")
        {
            limits.insert("Local byte limit reached");
        }
        if reason.contains("Git timeout") || reason.contains("git_timeout") {
            limits.insert("Git timeout; some repository checks could not finish");
        }
    };
    if let Some(reason) = &review.history_metrics.stop_reason {
        record(reason);
    }
    for item in &review.items {
        for reason in &item.unknowns {
            record(reason);
        }
        for h in &item.history {
            if let Some(reason) = &h.stop_reason {
                record(reason);
            }
        }
        if let Some(s) = &item.local_summary {
            for gap in &s.gaps {
                record(&gap.reason);
            }
        }
        if let Some(s) = &item.worktree_state {
            for gap in &s.gaps {
                record(gap);
            }
        }
        if let Some(s) = &item.private_history {
            for gap in &s.gaps {
                record(gap);
            }
        }
    }
    limits
}

pub fn discovery_progress(event: &DiscoveryProgress) -> String {
    match event.phase {
        DiscoveryPhase::Start => "Discovering repositories...".into(),
        DiscoveryPhase::Progress => format!(
            "  Searching: {} repositories found; {} known paths pending; {:.1}s; {}",
            event.families,
            event
                .queued_paths
                .saturating_add(event.metadata_ready_paths),
            event.elapsed_ms as f64 / 1000.0,
            rate(event.entries as u64, event.elapsed_ms, "entries")
        ),
        DiscoveryPhase::Finished => format!(
            "Discovery finished: {} repositories found{}; {:.3}s",
            event.families,
            event
                .stop
                .as_ref()
                .map(|s| format!("; stopped: {}", s.as_str()))
                .unwrap_or_default(),
            event.elapsed_ms as f64 / 1000.0
        ),
    }
}

pub fn review_progress(event: &ProgressEvent) -> String {
    if let Some(observation) = &event.observation {
        return match observation {
            ProgressObservation::History {
                path,
                head,
                target_ref,
                target_oid,
                result,
                complete,
            } => {
                format!(
                    "Observed history: {} (provisional, {}); {} HEAD={} target={} {}{}",
                    text(result, 80),
                    if *complete { "complete" } else { "partial" },
                    text(&path.display(), 180),
                    head.as_deref()
                        .map(|v| text(v, 128))
                        .unwrap_or_else(|| "UNKNOWN".into()),
                    text(&target_ref.display(), 160),
                    target_oid
                        .as_deref()
                        .map(|v| text(v, 128))
                        .unwrap_or_else(|| "UNKNOWN".into()),
                    event
                        .stop_reason
                        .as_deref()
                        .map(|v| format!("; {}", text(v, 160)))
                        .unwrap_or_default()
                )
            }
            ProgressObservation::Local { path, summary } => format!(
                "Observed local: staged={} unstaged={} conflicts={} untracked={} ignored={} (provisional); {}",
                super::local_count(&summary.staged),
                super::local_count(&summary.unstaged),
                super::local_count(&summary.conflicts),
                super::local_count(&summary.untracked),
                super::local_count(&summary.ignored),
                text(&path.display(), 180)
            ),
        };
    }
    let noun = if event.processed == 1 {
        "worktree"
    } else {
        "worktrees"
    };
    match event.phase {
        "controls" => "Checking worktree identity, locks and Git operations...".into(),
        "history" => "Comparing commit history with the selected targets...".into(),
        "history_first_pass" => format!("  First history pass: {} {noun} visited", event.processed),
        "history_complete" => format!(
            "  History pass finished for {} {noun}; coverage follows in the recap",
            event.processed
        ),
        "local" => "Checking local files...".into(),
        "deep" => "Verifying commit, tree and file objects for possible candidates...".into(),
        "private_history" => "Checking private history and shared stashes...".into(),
        "history_revalidate" => "Rechecking identity and commit history...".into(),
        "private_revalidate" => "Rechecking private history...".into(),
        "context" => "Reading commit and changelog context...".into(),
        "complete" => format!(
            "Review finished: {} {noun}; preparing the report.\n",
            event.processed
        ),
        other => format!("Review phase: {}", text(other, 80)),
    }
}

/// Short console recap. The saved report still uses the detailed rendering above.
pub fn compact_summary(review: &Review) -> String {
    compact_summary_with_executable(review, &BytePath::from_bytes(b"wt-sweep"))
}

pub fn compact_summary_with_executable(review: &Review, executable: &BytePath) -> String {
    compact(review, &review.items.iter().collect::<Vec<_>>(), executable)
}

pub fn compact_view_summary(view: &ReviewView) -> String {
    compact_view_summary_with_executable(view, &BytePath::from_bytes(b"wt-sweep"))
}

pub fn compact_view_summary_with_executable(view: &ReviewView, executable: &BytePath) -> String {
    let selected: BTreeSet<_> = view.selected_paths.iter().collect();
    compact(
        &view.source,
        &view
            .source
            .items
            .iter()
            .filter(|item| selected.contains(&item.path))
            .collect::<Vec<_>>(),
        executable,
    )
}

fn decimal_bytes(value: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
    let mut amount = value as f64;
    let mut unit = 0;
    while amount >= 1000.0 && unit + 1 < units.len() {
        amount /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value} B")
    } else {
        format!("{amount:.2} {}", units[unit])
    }
}

fn missing_labels(item: &ReviewItem) -> String {
    let Some(decision) = &item.decision else {
        return "decision evidence".into();
    };
    decision
        .missing_conditions
        .iter()
        .map(|condition| match condition.as_str() {
            "C1" => "identity/protections",
            "C2" => "integration",
            "C3" => "local files",
            "C4" => "private history",
            "C5" => "final checks",
            _ => "other evidence",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A copyable command must preserve each argument, not the escaped display path.
/// Control/non-UTF-8 paths remain in the report but get no shell suggestion.
fn shell_argument(value: &BytePath) -> Option<String> {
    let bytes = value.bytes().ok()?;
    let value = std::str::from_utf8(&bytes).ok()?;
    if value.is_empty()
        || value.len() > 4096
        || value.chars().any(|c| {
            c.is_control()
                || ('\u{202a}'..='\u{202e}').contains(&c)
                || ('\u{2066}'..='\u{2069}').contains(&c)
        })
    {
        return None;
    }
    Some(format!("'{}'", value.replace('\'', "'\\''")))
}

fn focused_command(item: &ReviewItem, executable: &BytePath) -> Option<String> {
    let path = item.path.to_path().ok()?;
    if !path.is_absolute()
        || path
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        || item.history_omitted_targets > 0
    {
        return None;
    }
    let targets: BTreeSet<_> = item.history.iter().map(|h| &h.target_ref).collect();
    if targets.is_empty() || targets.len() > 8 {
        return None;
    }
    let path = shell_argument(&item.path)?;
    let mut command = format!(
        "{} review --repo {path} --worktree {path} --path-prefix {path}",
        shell_argument(executable)?
    );
    for target in targets {
        crate::review::full_ref(target).ok()?;
        let _ = write!(command, " --target {}", shell_argument(target)?);
    }
    command.push_str(" --budget thorough --summary");
    Some(command)
}

fn write_history(rows: &[&ReviewItem], out: &mut String) {
    let mut positive = 0;
    let mut negative = 0;
    for item in rows {
        let current = |h: &&crate::model::HistoryObservation| {
            item.head.is_some()
                && h.source_oid == item.head
                && h.target_oid.is_some()
                && h.stop_reason.is_none()
        };
        if item
            .history
            .iter()
            .filter(current)
            .any(|h| matches!(h.result.as_str(), "identical_commit" | "ancestor"))
        {
            positive += 1;
        } else if !item.history.is_empty()
            && item.history_omitted_targets == 0
            && item
                .history
                .iter()
                .all(|h| current(&h) && h.result == "not_ancestor")
        {
            negative += 1;
        }
    }
    let _ = writeln!(
        out,
        "\nCOMMIT HISTORY (selected worktrees, against their recorded targets)"
    );
    let _ = writeln!(out, "  - HEAD matches or is an ancestor: {positive}");
    let _ = writeln!(out, "  - Not an ancestor of any checked target: {negative}");
    let _ = writeln!(
        out,
        "  - Relationship unknown: {}",
        rows.len() - positive - negative
    );
    let _ = writeln!(
        out,
        "  Commit ancestry alone does not establish cleanup eligibility or squash/rebase integration."
    );
}

fn compact(review: &Review, rows: &[&ReviewItem], executable: &BytePath) -> String {
    let total = review.items.len();
    let selected = rows.len();
    let (keep, candidates, verify) = counts(rows);
    let mut out = format!(
        "\nWORKTREE SWEEP\n========================================\nWorktrees selected: {selected}/{total}\n"
    );
    let targets: BTreeSet<_> = rows
        .iter()
        .flat_map(|i| &i.history)
        .map(|h| &h.target_ref)
        .collect();
    if !targets.is_empty() {
        let _ = writeln!(
            out,
            "Targets: {}{}",
            targets
                .iter()
                .take(3)
                .map(|t| text(&t.display(), 80))
                .collect::<Vec<_>>()
                .join(", "),
            if targets.len() > 3 {
                " (more in report)"
            } else {
                ""
            }
        );
    }
    let not_requested = review
        .items
        .iter()
        .filter(|item| {
            item.unknowns
                .iter()
                .any(|r| r == "Not selected for evidence collection (--worktree).")
        })
        .count();
    if not_requested > 0 {
        let _ = writeln!(
            out,
            "Targeted collection: {} worktree(s); {not_requested} not requested",
            total - not_requested
        );
    }

    let _ = writeln!(
        out,
        "\nDISK USAGE ({}, unaffected by review filters)",
        if review
            .disk_usage
            .as_ref()
            .is_some_and(|u| u.requested_paths.is_some())
        {
            "explicit disk selection"
        } else {
            "all recorded roots"
        }
    );
    if let Some(disk) = &review.disk_usage {
        let _ = writeln!(
            out,
            "  - {} observed allocated{}",
            decimal_bytes(disk.unique_bytes.allocated),
            if disk.complete { "" } else { " (INCOMPLETE)" }
        );
        let complete = disk
            .measurements
            .iter()
            .filter(|r| r.requested && r.complete)
            .count();
        let requested = disk.measurements.iter().filter(|r| r.requested).count();
        let _ = writeln!(
            out,
            "  - {complete}/{requested} roots complete; logical: {}",
            decimal_bytes(disk.unique_bytes.logical)
        );
        let _ = writeln!(out, "  - Disk collection: {}", disk.scope_summary());
        let _ = writeln!(
            out,
            "  - Reclaimable space: not established. Observed allocation is not reclaimable space."
        );
        let _ = writeln!(
            out,
            "  Captured sizes, deduplicated by inode; APFS sharing and snapshots remain unaccounted for."
        );
    } else {
        let _ = writeln!(out, "  - Not measured; use scan or review --disk-usage.");
    }
    write_history(rows, &mut out);
    let _ = writeln!(
        out,
        "\nRESULTS (selected worktrees)\n  - Keep: {keep}\n  - Cleanup candidates: {candidates}\n  - Needs verification: {verify}"
    );
    if candidates == 0 && verify > 0 {
        let _ = writeln!(
            out,
            "  No candidate established; incomplete checks do not prove that cleanup is impossible."
        );
    }
    if selected != total {
        let (k, c, v) = counts(&review.items.iter().collect::<Vec<_>>());
        let _ = writeln!(
            out,
            "  Source totals: {k} keep, {c} candidates, {v} to verify"
        );
    }

    // Rows are ranked by observed bytes only. Missing values never become zero,
    // and overlapping per-row measurements are never added to the source total.
    let mut ranked = Vec::new();
    if let Some(disk) = &review.disk_usage {
        let measurements: BTreeMap<_, _> = disk
            .measurements
            .iter()
            .filter(|m| m.requested && m.kind == crate::disk::DiskKind::Worktree)
            .map(|m| (&m.path, m))
            .collect();
        for row in rows {
            if let Some(&measurement) = measurements.get(&row.path)
                && let Some(bytes) = &measurement.bytes
            {
                ranked.push((*row, measurement, bytes.allocated));
            }
        }
        ranked.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.path.cmp(&b.0.path)));
        let _ = writeln!(
            out,
            "\nLARGEST WORKTREES (selected, by observed size; partial sizes may under-rank)"
        );
        for (row, measurement, bytes) in ranked.iter().take(5) {
            let _ = writeln!(
                out,
                "  - {} [{}] | {}",
                decimal_bytes(*bytes),
                if measurement.complete {
                    "complete"
                } else {
                    "partial"
                },
                text(&row.classification, 40)
            );
            let _ = writeln!(out, "    {}", text(&row.path.display(), 300));
            if row.classification == "keep" {
                if let Some(reason) = row.decision.as_ref().and_then(|d| d.keep_reasons.first()) {
                    let _ = writeln!(out, "    Keep reason: {}", super::keep_reason(reason));
                }
            } else if row.classification == "verify" {
                let _ = writeln!(out, "    Missing: {}", missing_labels(row));
            }
        }
        let missing = rows.len() - ranked.len();
        if missing > 0 {
            let _ = writeln!(
                out,
                "  - {missing} selected worktree(s) without a size; see report for unmeasured or unrequested roots."
            );
        }
    }

    let coverage = &review.coverage;
    let discovery_complete = coverage.traversal_complete
        && coverage.gaps.is_empty()
        && coverage.remaining.is_none()
        && coverage.publication.is_none();
    let _ = writeln!(out, "\nCHECKS TO COMPLETE");
    let _ = writeln!(
        out,
        "  - Discovery: {}; {} source gaps",
        if discovery_complete {
            "complete"
        } else {
            "incomplete"
        },
        coverage.gaps.len()
    );
    if let Some(remaining) = &coverage.remaining {
        let _ = writeln!(
            out,
            "  - Discovery remaining: {}; {} known pending paths",
            remaining.reason.as_str(),
            remaining.known_pending_paths
        );
    }
    if let Some(omitted) = &coverage.publication {
        let _ = writeln!(
            out,
            "  - Unpublished worktrees: {}; families: {}; local manifests: {}; markers: {}",
            omitted.worktrees, omitted.families, omitted.local_inventories, omitted.markers
        );
    }
    let complete = rows
        .iter()
        .filter(|i| i.local_summary.as_ref().is_some_and(local_complete))
        .count();
    let collected = rows
        .iter()
        .filter(|i| i.local_summary.as_ref().is_some_and(local_collected))
        .count();
    let _ = writeln!(
        out,
        "  - Local checks (selected): {complete} complete, {} partial, {} not collected",
        collected.saturating_sub(complete),
        selected.saturating_sub(collected)
    );
    if verify > 0 {
        let missing: Vec<_> = [
            ("C1", "identity"),
            ("C2", "integration"),
            ("C3", "local files"),
            ("C4", "private history"),
            ("C5", "final checks"),
        ]
        .into_iter()
        .filter_map(|(code, label)| {
            let n = rows
                .iter()
                .filter(|i| {
                    i.classification == "verify"
                        && i.decision
                            .as_ref()
                            .is_none_or(|d| d.missing_conditions.iter().any(|c| c == code))
                })
                .count();
            (n > 0).then(|| format!("{label} {n}"))
        })
        .collect();
        if !missing.is_empty() {
            let _ = writeln!(
                out,
                "  - Missing checks (selected, unresolved): {}",
                missing.join(", ")
            );
        }
    }
    let hit_limits = limits(review);
    if !hit_limits.is_empty() {
        let _ = writeln!(out, "\nCOLLECTION LIMITS (source run)");
        for limit in hit_limits {
            let _ = writeln!(out, "  - {limit}");
        }
    }
    if verify > 0 || !discovery_complete {
        let _ = writeln!(out, "\nNEXT CHECK");
        if !discovery_complete {
            let _ = writeln!(
                out,
                "  - Inspect discovery gaps in report.txt before treating the inventory as complete."
            );
        }
        let next = ranked
            .iter()
            .map(|(row, _, _)| *row)
            .find(|r| r.classification == "verify")
            .or_else(|| {
                rows.iter()
                    .copied()
                    .filter(|r| r.classification == "verify")
                    .min_by_key(|r| &r.path)
            });
        if let Some(row) = next {
            let _ = writeln!(out, "  - Focus on: {}", text(&row.path.display(), 300));
            if let Some(command) = focused_command(row, executable) {
                let _ = writeln!(
                    out,
                    "  Recorded targets; recheck their suitability before running:\n    {command}"
                );
                let _ = writeln!(
                    out,
                    "  Larger finite allowances; completion is not guaranteed. Disk sizing is a separate check."
                );
            } else {
                let _ = writeln!(
                    out,
                    "  - Inspect its gaps and confirm the full target ref in the report. No safe copyable command available."
                );
            }
        }
    }
    let _ = writeln!(
        out,
        "\nUsage by agents or terminals: not checked. Read-only review; no removal authorized.\nDetailed evidence and timings: report.txt or --verbose."
    );
    out
}
