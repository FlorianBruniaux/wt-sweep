//! Descriptions explain evidence; they never participate in judgement.
use crate::model::{BytePath, Review};
use std::fmt::Write;

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

pub fn detail(review: &Review) -> String {
    let mut output = String::from("WORKTREE\tSUBJECT\tBRANCH\tHEAD\tCLASSIFICATION\n");
    for item in &review.items {
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
        }
        for local in review
            .local_inventories
            .iter()
            .filter(|local| local.worktree == item.path)
        {
            let _ = writeln!(
                output,
                "  Local inventory at {}: {} entries, {} index entries, {} gaps; digest {} (not revalidated)",
                local.finished_unix_ms,
                local.entries.len(),
                local.index_entries.len(),
                local.gaps.len(),
                text(&local.content_digest_sha256, 64)
            );
            for gap in &local.gaps {
                let _ = writeln!(
                    output,
                    "    UNKNOWN {}: {}",
                    gap.path.display(),
                    text(&gap.reason, 500)
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
    }
    let _ = writeln!(output, "Review SHA-256: {}", review.digest_sha256);
    output
}

/// Exactly five lines even when a repository contains thousands of worktrees.
pub fn summary(review: &Review) -> String {
    let active = review
        .items
        .iter()
        .filter(|i| i.classification == "active")
        .count();
    let integrated = review
        .items
        .iter()
        .filter(|i| i.classification == "integration_observed")
        .count();
    let triage = review.items.len().saturating_sub(active + integrated);
    let unknown = review
        .items
        .iter()
        .filter(|i| !i.unknowns.is_empty())
        .count();
    format!(
        "Worktrees: {}\nActive: {active}; integration observed: {integrated}; to triage: {triage}\nUnknowns: {unknown} worktrees; coverage gaps: {}\nRead-only review; local preservation and removal are not established. Detail: plan without --summary or report.txt in --run-dir.\nReview SHA-256: {}\n",
        review.items.len(),
        review.coverage.gaps.len(),
        review.digest_sha256
    )
}
