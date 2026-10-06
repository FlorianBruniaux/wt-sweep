//! Descriptions explain evidence; they never participate in judgement.
use crate::model::{BytePath, LocalCount, Review, ReviewItem, ReviewView};
use std::fmt::Write;

mod console;
pub use console::{discovery_progress, review_progress, summary, view_summary};

fn text(value: &str, limit: usize) -> String {
    let mut output = String::new();
    for character in value.chars().take(limit) {
        match character {
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\\' => output.push_str("\\\\"),
            c if c.is_control()
                || ('\u{202a}'..='\u{202e}').contains(&c)
                || ('\u{2066}'..='\u{2069}').contains(&c) =>
            {
                let _ = write!(output, "\\u{{{:x}}}", c as u32);
            }
            c => output.push(c),
        }
    }
    if value.chars().count() > limit {
        output.push_str("...");
    }
    output
}

fn local_count(count: &LocalCount) -> String {
    if count.complete {
        count.observed.to_string()
    } else if count.observed > 0 {
        format!(">={}", count.observed)
    } else {
        "UNKNOWN".into()
    }
}

fn completeness(complete: bool) -> &'static str {
    if complete { "complete" } else { "partial" }
}

fn private_result(result: &str) -> String {
    match result {
        "not_preserved_by_retained_refs" => "not preserved by retained refs".into(),
        other => text(other, 100),
    }
}

fn keep_reason(code: &str) -> &'static str {
    match code {
        "main_checkout" => "Main checkout; keep the repository entry point",
        "bare_repository" => "Bare repository; preserve the shared Git storage",
        "git_locked" => "Git registration locked; inspect its lock reason",
        "administrative_lock" => "Administrative lock observed; check the owning operation",
        "git_operation" => {
            "Git operation marker observed; inspect and finish or preserve that work"
        }
        "local_staged" => "Staged changes observed; preserve the index contents",
        "local_unstaged" => "Unstaged changes observed; preserve the local files",
        "local_conflicts" => "Index conflicts observed; preserve the unresolved versions",
        "local_untracked" => "Untracked content observed; inspect and preserve it",
        "local_ignored" => "Ignored content observed; inspect and preserve it",
        "open_request" => "Associated open PR/MR observed; review its current state",
        "head_not_integrated" => {
            "HEAD is not an ancestor of any completely examined accepted target"
        }
        "private_history_not_preserved" => {
            "Private history root is not covered by retained named refs"
        }
        _ => "Unrecognized reason; inspect the original evidence",
    }
}

fn next_check(condition: &str) -> &'static str {
    match condition {
        "C1" => "Rescan identity, Git administration, locks and operation markers",
        "C2" => "Select the intended target and complete commit, tree and blob verification",
        "C3" => "Inspect local gaps and collect all current HEAD, index and file comparisons",
        "C4" => "Inspect private refs and reflogs; establish named-ref coverage for every root",
        "C5" => "Run a fresh scan and review after resolving changed or incomplete checks",
        _ => "Inspect the original decision evidence",
    }
}

pub fn detail(review: &Review) -> String {
    let mut output = String::new();
    detail_rows(
        &mut output,
        review,
        &review.items.iter().collect::<Vec<_>>(),
        None,
    );
    output
}
fn detail_rows(
    output: &mut impl std::fmt::Write,
    review: &Review,
    items: &[&ReviewItem],
    groups: Option<&std::collections::BTreeMap<BytePath, BytePath>>,
) {
    let _ = writeln!(output, "WORKTREE\tSUBJECT\tBRANCH\tHEAD\tCLASSIFICATION");
    let contexts: std::collections::BTreeMap<_, _> = review
        .context
        .as_ref()
        .map(|context| context.items.iter().map(|row| (&row.path, row)).collect())
        .unwrap_or_default();
    let mut previous_group = None;
    for item in items {
        if let Some(group) = groups.and_then(|g| g.get(&item.path))
            && previous_group != Some(group)
        {
            let _ = writeln!(output, "Group: {}", group.display());
            previous_group = Some(group);
        }
        let _ = writeln!(
            output,
            "{}\t{}{}\t{}\t{}\t{}",
            item.path.display(),
            item.subject
                .as_ref()
                .map(BytePath::display)
                .unwrap_or_else(|| "UNKNOWN".into()),
            if item.subject_truncated {
                " [truncated]"
            } else {
                ""
            },
            item.branch
                .as_ref()
                .map(BytePath::display)
                .unwrap_or_else(|| "detached/unknown".into()),
            text(item.head.as_deref().unwrap_or("UNKNOWN"), 64),
            text(&item.classification, 100)
        );
        if let Some(state) = &item.worktree_state {
            let _ = writeln!(
                output,
                "  Current controls {}..{}: role={}; checks={}; family={}",
                state.started_unix_ms,
                state.finished_unix_ms,
                text(&state.role, 30),
                text(&state.checks, 40),
                state.common_dir.display()
            );
            for protection in &state.protections {
                let _ = writeln!(output, "    Administrative lock: {}", protection.display());
            }
            for operation in &state.operations {
                let _ = writeln!(output, "    Operation marker: {}", operation.display());
            }
            for gap in &state.gaps {
                let _ = writeln!(output, "    Controls UNKNOWN: {}", text(gap, 500));
            }
        }
        if let Some(decision) = &item.decision {
            for reason in &decision.keep_reasons {
                let _ = writeln!(
                    output,
                    "  Keep [{}]: {}",
                    text(reason, 100),
                    keep_reason(reason)
                );
            }
            for condition in &decision.missing_conditions {
                let _ = writeln!(
                    output,
                    "  Next check {}: {}",
                    text(condition, 30),
                    next_check(condition)
                );
            }
        }
        if item.classification == "cleanup_candidate" {
            let _ = writeln!(
                output,
                "  Usage UNKNOWN: confirm before removal; this report authorizes no removal."
            );
        }
        for history in &item.history {
            let _ = writeln!(
                output,
                "  History {} {} -> {} {} (commit graph only; {}..{})",
                text(&history.result, 100),
                text(history.source_oid.as_deref().unwrap_or("UNKNOWN"), 64),
                history.target_ref.display(),
                text(history.target_oid.as_deref().unwrap_or("UNKNOWN"), 64),
                history.started_unix_ms,
                history.finished_unix_ms
            );
            if let Some(previous) = &history.observed_result {
                let _ = writeln!(output, "    Earlier graph result: {}", text(previous, 100));
            }
            if let Some(reason) = &history.stop_reason {
                let _ = writeln!(output, "    History incomplete: {}", text(reason, 500));
            }
        }
        if item.history_omitted_targets > 0 {
            let _ = writeln!(
                output,
                "  {} history targets omitted; results unknown",
                item.history_omitted_targets
            );
        }
        if let Some(private) = &item.private_history {
            let _ = writeln!(
                output,
                "  Private history {}..{}: {} observed roots; enumeration={}; anchors={}; commit graph only",
                private.started_unix_ms,
                private.finished_unix_ms,
                private.roots.len(),
                completeness(private.enumeration_complete),
                completeness(private.anchors_complete)
            );
            for root in &private.roots {
                let _ = writeln!(
                    output,
                    "    Root {}: {}",
                    text(&root.oid, 64),
                    private_result(&root.result)
                );
                if let Some(previous) = &root.observed_result {
                    let _ = writeln!(
                        output,
                        "      Earlier private result: {}",
                        private_result(previous)
                    );
                }
                if let Some(anchor) = &root.anchor {
                    let label = if root.result == "covered" {
                        Some("Covered via")
                    } else if root.observed_result.as_deref() == Some("covered") {
                        Some("Earlier anchor")
                    } else {
                        None
                    };
                    if let Some(label) = label {
                        let _ = writeln!(
                            output,
                            "      {label}: {} {}",
                            anchor.reference.display(),
                            text(&anchor.oid, 64)
                        );
                    }
                }
                for provenance in &root.provenance {
                    let _ = writeln!(
                        output,
                        "      Source {}: {}",
                        text(&provenance.kind, 100),
                        provenance.source.display()
                    );
                }
                if let Some(reason) = &root.reason {
                    let _ = writeln!(output, "      {}", text(reason, 500));
                }
            }
            for gap in &private.gaps {
                let _ = writeln!(output, "    Private history UNKNOWN: {}", text(gap, 500));
            }
        } else {
            let _ = writeln!(output, "  Private history UNKNOWN: not collected");
        }
        if let Some(local) = &item.local_summary {
            let _ = writeln!(
                output,
                "  Local now {}..{}: staged={}; unstaged={}; conflicts={}; untracked={}; ignored={}",
                local.started_unix_ms,
                local.finished_unix_ms,
                local_count(&local.staged),
                local_count(&local.unstaged),
                local_count(&local.conflicts),
                local_count(&local.untracked),
                local_count(&local.ignored)
            );
            for (name, count) in [
                ("staged", &local.staged),
                ("unstaged", &local.unstaged),
                ("conflicts", &local.conflicts),
                ("untracked", &local.untracked),
                ("ignored", &local.ignored),
            ] {
                for sample in &count.samples {
                    let _ = writeln!(output, "    {name}: {}", sample.display());
                }
                if count.samples_omitted > 0 {
                    let _ = writeln!(
                        output,
                        "    {name}: {} samples omitted",
                        count.samples_omitted
                    );
                }
            }
            for gap in &local.gaps {
                let _ = writeln!(
                    output,
                    "    LOCAL UNKNOWN {}: {}",
                    gap.path.display(),
                    text(&gap.reason, 500)
                );
            }
            if local.details_truncated {
                let _ = writeln!(output, "    Local detail truncated by output limits");
            }
        }
        for request in &item.requests {
            let _ = writeln!(
                output,
                "  MR/PR {} {} {}: {} ({})",
                request.iid,
                text(&request.state, 100),
                text(request.project.as_deref().unwrap_or("UNKNOWN project"), 240),
                text(request.title.as_deref().unwrap_or("title unknown"), 240),
                text(request.url.as_deref().unwrap_or("URL unknown"), 2048)
            );
            let _ = writeln!(
                output,
                "    Recorded forge host={}; HEAD={}; updated_at={}",
                text(request.host.as_deref().unwrap_or("UNKNOWN"), 240),
                text(request.head_oid.as_deref().unwrap_or("UNKNOWN"), 64),
                text(request.updated_at.as_deref().unwrap_or("UNKNOWN"), 100)
            );
            if let Some(source) = review
                .forge
                .as_ref()
                .and_then(|forge| forge.sources.get(request.source_index))
            {
                let _ = writeln!(
                    output,
                    "    Forge source {} SHA-256 {}; receipt verified={}; collection={}..{}",
                    source.raw_path.display(),
                    text(&source.sha256, 64),
                    source.receipt_verified,
                    source
                        .started_unix_ms
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "UNKNOWN".into()),
                    source
                        .finished_unix_ms
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "UNKNOWN".into())
                );
                if let Some(path) = &source.receipt_path {
                    let _ = writeln!(
                        output,
                        "    Receipt {} SHA-256 {}",
                        path.display(),
                        text(source.receipt_sha256.as_deref().unwrap_or("UNKNOWN"), 64)
                    );
                }
                for gap in &source.gaps {
                    let _ = writeln!(output, "    Forge provenance UNKNOWN: {}", text(gap, 1000));
                }
            } else {
                let _ = writeln!(
                    output,
                    "    Forge source UNKNOWN: no matching imported source"
                );
            }
            let _ = writeln!(
                output,
                "    Imported observation; current server state is not established. An open request does not establish process activity."
            );
        }
        for local in review
            .local_inventory_refs
            .iter()
            .filter(|local| local.worktree == item.path)
        {
            let _ = writeln!(
                output,
                "  Local inventory at {}: {} entries, {} index entries, {} gaps; digest {} (not revalidated)",
                local.finished_unix_ms,
                local.entry_count,
                local.index_entry_count,
                local.gap_count,
                text(&local.content_digest_sha256, 64)
            );
            if local.gap_count > 0 || local.not_collected_count > 0 {
                let _ = writeln!(
                    output,
                    "    Historical coverage detail: read the input inventory matched by its digest"
                );
            }
        }
        for observation in &item.observations {
            let _ = writeln!(
                output,
                "  {} {} -> {} {} [{}]",
                text(&observation.relation, 100),
                observation.source_oid,
                observation.target_ref.display(),
                observation.target_oid,
                text(&observation.target_role, 100)
            );
        }
        for unknown in &item.unknowns {
            let _ = writeln!(output, "  UNKNOWN: {}", text(unknown, 1000));
        }
        for risk in &item.risks {
            let _ = writeln!(output, "  Risk: {}", text(risk, 1000));
        }
        if let Some(context) = contexts.get(&item.path) {
            context_detail(output, context);
        }
    }
    for family in &review.shared_stashes {
        let _ = writeln!(
            output,
            "Shared stashes for {} (family {}:{}; {}..{}): {} observed; list={}",
            family.common_dir.display(),
            family.identity.device,
            family.identity.inode,
            family.started_unix_ms,
            family.finished_unix_ms,
            family.entries.len(),
            completeness(family.enumeration_complete)
        );
        for stash in &family.entries {
            let _ = writeln!(
                output,
                "  Stash {}: base={}; timestamp={}; metadata={}; files={}",
                text(&stash.oid, 64),
                text(stash.base_oid.as_deref().unwrap_or("UNKNOWN"), 64),
                stash
                    .timestamp_unix
                    .map(|date| date.to_string())
                    .unwrap_or_else(|| "UNKNOWN".into()),
                completeness(stash.metadata_complete),
                completeness(stash.files_complete)
            );
            let _ = writeln!(
                output,
                "    Message: {}{}",
                stash
                    .message
                    .as_ref()
                    .map(BytePath::display)
                    .unwrap_or_else(|| "UNKNOWN".into()),
                if stash.message_truncated {
                    " [truncated]"
                } else {
                    ""
                }
            );
            for path in &stash.files {
                let _ = writeln!(output, "    Changed path: {}", path.display());
            }
            for gap in &stash.gaps {
                let _ = writeln!(output, "    Stash detail UNKNOWN: {}", text(gap, 500));
            }
        }
        for gap in &family.gaps {
            let _ = writeln!(output, "  Shared stash UNKNOWN: {}", text(gap, 500));
        }
    }
    if let Some(remaining) = &review.coverage.remaining {
        let _ = writeln!(
            output,
            "Discovery remaining: {}; known pending={}; conservative cover=requested scope from input inventory (may include visited paths)",
            remaining.reason.as_str(),
            remaining.known_pending_paths
        );
    }
    if let Some(omitted) = &review.coverage.publication {
        let _ = writeln!(
            output,
            "Publication omitted: {} families, {} worktrees, {} local manifests, {} markers; conservative cover=requested scope including registered worktrees outside roots",
            omitted.families, omitted.worktrees, omitted.local_inventories, omitted.markers
        );
    }
    if let Some(collection) = &review.collection_metrics {
        let _ = writeln!(
            output,
            "Collection budget: {}",
            text(&collection.budget_profile, 80)
        );
        for (name, value) in &collection.applied_limits {
            let _ = writeln!(output, "Applied limit {name}: {value}");
        }
        for phase in &collection.phases {
            let _ = writeln!(
                output,
                "Collection phase {}: {} ms; {:?}",
                text(&phase.name, 80),
                phase.elapsed_ms,
                phase.counters
            );
        }
    }
    let metrics = &review.history_metrics;
    let _ = writeln!(
        output,
        "History phase: {}/{} worktrees started; {} targets complete; {} unknown; {} object reads; {} Git children; {} ms",
        metrics.first_pass_items,
        metrics.items_total,
        metrics.completed_targets,
        metrics.incomplete_targets,
        metrics.object_read_attempts,
        metrics.spawned_processes,
        metrics.elapsed_ms
    );
    if let Some(reason) = &metrics.stop_reason {
        let _ = writeln!(output, "History stop: {}", text(reason, 500));
    }
    let _ = writeln!(
        output,
        "Decision evidence interval: {}..{}",
        review.started_unix_ms, review.finished_unix_ms
    );
    if let Some(context) = &review.context {
        let _ = writeln!(
            output,
            "Context collection interval: {}..{}; recorded Git OIDs only; no continuity after decision checks.",
            context.started_unix_ms, context.finished_unix_ms
        );
        let _ = writeln!(output, "Context rows omitted: {}", context.omitted_items);
        for gap in &context.gaps {
            let _ = writeln!(output, "Context UNKNOWN: {}", text(gap, 1000));
        }
        let _ = writeln!(
            output,
            "Commit membership does not establish semantic equivalence; squash, cherry-pick and revert require separate review."
        );
    } else {
        let _ = writeln!(output, "Context: not collected");
    }
    let _ = writeln!(
        output,
        "Conservation limits: ACLs, extended attributes and unreachable objects outside examined roots are not covered. Writer exclusion and durable backup are not established."
    );
    let _ = writeln!(output, "Review SHA-256: {}", review.digest_sha256);
}

pub fn view_detail(view: &ReviewView) -> Result<String, crate::model::SweepError> {
    let mut output = String::new();
    view_detail_into(&mut output, view)?;
    Ok(output)
}

fn view_detail_into(
    output: &mut impl std::fmt::Write,
    view: &ReviewView,
) -> Result<(), crate::model::SweepError> {
    let index: std::collections::BTreeMap<_, _> =
        view.source.items.iter().map(|i| (&i.path, i)).collect();
    let rows: Vec<_> = view
        .selected_paths
        .iter()
        .filter_map(|p| index.get(p).copied())
        .collect();
    let groups = crate::view::groups(view)?;
    let _ = output.write_str(&view_summary(view));
    let _ = writeln!(output, "View SHA-256: {}", view.digest_sha256);
    detail_rows(output, &view.source, &rows, Some(&groups));
    for gap in &view.source.coverage.gaps {
        let _ = writeln!(
            output,
            "Source coverage gap: {}: {}",
            gap.path.display(),
            text(&gap.reason, 1000)
        );
    }
    Ok(())
}

fn context_detail(output: &mut impl std::fmt::Write, context: &crate::model::WorktreeContext) {
    let _ = writeln!(
        output,
        "  Context for recorded HEAD {} in {} (family {}:{})",
        text(context.head_oid.as_deref().unwrap_or("UNKNOWN"), 64),
        context.common_dir.display(),
        context.family_identity.device,
        context.family_identity.inode
    );
    let commits = &context.uncovered;
    let count = if commits.complete {
        commits.observed.to_string()
    } else if commits.observed > 0 {
        format!(">={} (incomplete)", commits.observed)
    } else {
        "UNKNOWN".into()
    };
    let _ = writeln!(
        output,
        "    Commits outside accepted target union: {count}; samples={}; known samples omitted={}",
        commits.samples.len(),
        commits.samples_omitted
    );
    for target in &commits.targets {
        let _ = writeln!(
            output,
            "      Recorded target {} {}",
            target.reference.display(),
            text(target.oid.as_deref().unwrap_or("UNKNOWN"), 64)
        );
    }
    for sample in &commits.samples {
        let _ = writeln!(
            output,
            "      Commit {}: {}{}",
            text(&sample.oid, 64),
            sample
                .subject
                .as_ref()
                .map(BytePath::display)
                .unwrap_or_else(|| "UNKNOWN subject".into()),
            if sample.subject_truncated {
                " [truncated]"
            } else {
                ""
            }
        );
    }
    for gap in &commits.gaps {
        let _ = writeln!(output, "      Commit context UNKNOWN: {}", text(gap, 1000));
    }
    for changelog in &context.changelogs {
        let _ = writeln!(
            output,
            "    Changelog {} at {}: {}; root tree={}; blob={}; verified blob bytes={}",
            changelog.path.display(),
            text(&changelog.revision_oid, 64),
            text(&changelog.status, 30),
            text(changelog.tree_oid.as_deref().unwrap_or("UNKNOWN"), 64),
            text(changelog.blob_oid.as_deref().unwrap_or("UNKNOWN"), 64),
            changelog
                .blob_bytes
                .map(|v| v.to_string())
                .unwrap_or_else(|| "UNKNOWN".into())
        );
        if let Some(excerpt) = &changelog.excerpt {
            let _ = writeln!(
                output,
                "      Excerpt: {}{}",
                excerpt.display(),
                if changelog.excerpt_truncated {
                    " [truncated]"
                } else {
                    ""
                }
            );
        }
        for gap in &changelog.gaps {
            let _ = writeln!(output, "      Changelog UNKNOWN: {}", text(gap, 1000));
        }
    }
    for gap in &context.gaps {
        let _ = writeln!(output, "    Context UNKNOWN: {}", text(gap, 1000));
    }
}

/// Bounded CLI rendering. A report that does not fit is refused.
pub fn detail_bounded(review: &Review, limit: usize) -> Result<String, crate::model::SweepError> {
    let mut output = crate::output::BoundedText::new(limit);
    detail_rows(
        &mut output,
        review,
        &review.items.iter().collect::<Vec<_>>(),
        None,
    );
    output.finish()
}

pub fn view_detail_bounded(
    view: &ReviewView,
    limit: usize,
) -> Result<String, crate::model::SweepError> {
    let mut output = crate::output::BoundedText::new(limit);
    view_detail_into(&mut output, view)?;
    output.finish()
}
