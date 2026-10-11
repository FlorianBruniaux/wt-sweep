//! Reading aids and discovery diagnostics, projected from captured source only.
use super::{Html, gap_item, list};
use crate::model::{Coverage, Gap};
use std::fmt::{self, Write};

pub(super) fn icon(h: &mut Html, kind: &str) -> fmt::Result {
    let paths = match kind {
        "overview" => "M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z",
        "worktree-families" => "M3 7h7l2 2h9v11H3z M3 7V4h7l2 3",
        "disk" | "disk-priority" => {
            "M4 6c0-4 16-4 16 0s-16 4-16 0 M4 6v12c0 4 16 4 16 0V6 M4 12c0 4 16 4 16 0"
        }
        "coverage" => "M12 3 2 21h20z M12 9v5 M12 17h.01",
        "search" => "M10 3a7 7 0 1 0 0 14 7 7 0 0 0 0-14 M15 15l6 6",
        "keep" => "M12 3 3 7v6c0 4 9 8 9 8s9-4 9-8V7z M8 12l3 3 5-6",
        "verify" => "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18 M12 7v6 M12 17h.01",
        "cleanup_candidate" => "M4 4h16v16H4z M8 12l3 3 5-6",
        _ => "M3 4h7l2 2 2-2h7v16h-7l-2 2-2-2H3z M12 6v16",
    };
    h.raw("<svg class=\"ui-icon\" aria-hidden=\"true\" focusable=\"false\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.7\" stroke-linecap=\"round\" stroke-linejoin=\"round\"><path d=\"")?;
    h.raw(paths)?;
    h.raw("\"/></svg>")
}

pub(super) fn guide(h: &mut Html, reviewed: bool) -> fmt::Result {
    h.raw("<details class=\"reading-guide\" id=\"reading-guide\"><summary>")?;
    icon(h, "guide")?;
    h.raw("How to use this report</summary><div class=\"guide-body\"><p>")?;
    h.raw(if reviewed {
        "<strong>This report includes a Git review.</strong> Read the reasons for each verdict. keep means work or a retention reason was observed; verify means required checks are missing; cleanup_candidate means the current checks passed, with usage still to confirm."
    } else {
        "<strong>This report is an inventory.</strong> It answers where the recorded worktrees are and how much was measured. A Git review is needed to determine what to keep or inspect for cleanup. No cleanup verdict was collected here."
    })?;
    h.raw("</p><ol><li>Open <a href=\"#worktree-families\">Worktree list</a>. Search for a repository, branch or path.</li><li>Sort by measured size to prioritize inspection. An unknown size does not mean an empty folder.</li><li>Expand a row for its recorded evidence. Copy its path to inspect the folder.</li><li>")?;
    h.raw(if reviewed { "Check the verdict's reasons and missing checks before deciding your next step." } else { "Choose a repository and a confirmed integration ref, then run <code>review</code> for current Git evidence." })?;
    h.raw("</li></ol><p><a href=\"#coverage\">Scan limits</a> explains what could not be explored or collected. Application frameworks can appear there even when they contain no Git repository.</p><figure class=\"guide-diagram\">")?;
    // The generated asset contains only fixed teaching labels, never captured paths.
    // Its own title/role bind the image; no Mermaid runtime or external asset is loaded.
    h.raw(include_str!("report-flow.svg"))?;
    h.raw("<figcaption>Discovery finds registrations; a separate Git review establishes verdicts. This diagram explains the workflow, not completion of every step in this report.</figcaption></figure><p>Filters and search change the view only. Source counts and recorded disk measurements remain unchanged. Sweep does not delete anything.</p></div></details>")
}

fn group(gap: &Gap) -> usize {
    let reason = gap.reason.to_ascii_lowercase();
    // Presentation categories only. Original paths/reasons remain intact below.
    if reason.contains("symlink") || reason.contains("already visited") || reason.contains("alias")
    {
        0
    } else if reason.contains("permission")
        || reason.contains("operation not permitted")
        || reason.contains("access denied")
    {
        1
    } else {
        2
    }
}

pub(super) fn limits(h: &mut Html, coverage: &Coverage, not_collected: &[String]) -> fmt::Result {
    h.raw("<div data-scan-limits><p><strong>")?;
    h.raw(
        if coverage.traversal_complete && coverage.remaining.is_none() {
            "The recorded filesystem traversal finished within the requested scope."
        } else if coverage.remaining.is_some() {
            "The filesystem scan stopped before exploring the whole requested scope."
        } else {
            "Discovery or local collection is incomplete. The technical details identify the recorded gaps."
        },
    )?;
    h.raw("</strong></p><p>These are discovery and collection diagnostics, not cleanup verdicts. They can include filesystem incidents, unknown Git information or local observations that could not be collected. A repeated symbolic link is recorded instead of exploring the same directory again.</p>")?;
    if let Some(remaining) = &coverage.remaining {
        write!(
            h.out,
            "<p><strong>{} filesystem paths still pending.</strong> This is not a count of undiscovered worktrees. ",
            remaining.known_pending_paths
        )?;
        h.raw(match remaining.reason {
            crate::model::DiscoveryStop::DirectoryBudget => "The limit on directories was reached.",
            crate::model::DiscoveryStop::EntryBudget => {
                "The limit on directory entries was reached."
            }
            crate::model::DiscoveryStop::Deadline => "The time limit was reached.",
            crate::model::DiscoveryStop::Interrupted => "Discovery was interrupted.",
            crate::model::DiscoveryStop::ObservationBudget => "The observation limit was reached.",
        })?;
        h.raw("</p>")?;
    }
    let mut groups: [Vec<&Gap>; 3] = Default::default();
    for gap in &coverage.gaps {
        groups[group(gap)].push(gap);
    }
    write!(
        h.out,
        "<p>{} recorded scan incidents. Details below retain every original path and reason.</p><details class=\"scan-technical\"><summary>Technical scan details</summary>",
        coverage.gaps.len()
    )?;
    for (i, (id, label)) in [
        ("links", "Symbolic links and repeated paths"),
        ("access", "Access refusals"),
        ("other", "Other recorded incidents"),
    ]
    .into_iter()
    .enumerate()
    {
        let gaps = &groups[i];
        write!(
            h.out,
            "<details data-gap-group=\"{id}\" data-gap-count=\"{}\"><summary>{label} ({})</summary><ul class=\"coverage-list\">",
            gaps.len(),
            gaps.len()
        )?;
        for gap in gaps {
            gap_item(h, gap)?;
        }
        h.raw("</ul></details>")?;
    }
    if !coverage.discovery_intervals.is_empty() {
        h.raw("<details><summary>Discovery observation intervals</summary><p>Separate observations, not a snapshot. Git family data belongs to its recorded pass.</p><ul>")?;
        for interval in &coverage.discovery_intervals {
            write!(
                h.out,
                "<li>Unix ms {}..{}</li>",
                interval.started_unix_ms, interval.finished_unix_ms
            )?;
        }
        h.raw("</ul></details>")?;
    }
    h.raw("<p>Temporal consistency: ")?;
    h.text(&coverage.temporal_consistency)?;
    h.raw(". Observations were captured over time, not as an atomic snapshot.</p>")?;
    if let Some(omitted) = &coverage.publication {
        write!(
            h.out,
            "<p>Publication omitted: {} families, {} worktrees, {} local manifests, {} markers. Displayed totals count the published source only.</p>",
            omitted.families, omitted.worktrees, omitted.local_inventories, omitted.markers
        )?;
    }
    h.raw("<details><summary>Evidence not collected</summary>")?;
    list(h, not_collected)?;
    h.raw("</details>")?;
    h.json(coverage)?;
    h.raw("</details></div>")
}
