`report.html` is self-contained: styles and script are embedded, with no model or network resource. Open it directly in a browser. The summary and evidence remain readable without JavaScript.

[Open the synthetic example](/demo/report.html).

## Read in this order

1. Scope, observation intervals, coverage and source/selection totals.
2. Git families and each worktree’s verdict.
3. Retention reasons, missing decision conditions and expanded evidence.
4. Disk rows and their complete, partial or absent measurements.

Search, verdict filters and sort change visible rows without recalculating source totals or disk measurements. CLI selection and browser filtering remain separate.

## Disk figures

**Observed disk footprint** is the deduplicated allocation of requested measured roots. **Recoverable space** remains unestablished. **Cleanup review** describes the source review, or says that no review was performed for an inventory-only report.

Sizes use decimal units and are rounded for display; exact bytes stay in tooltips and JSON. The largest observed worktrees list excludes shared Git and can omit larger, unmeasured roots. Do not add its overlapping sizes.

## Navigation and sharing

You can open and close the detail sections using the keyboard. The theme follows your system, with an optional local saved choice. Copy path uses the recorded path identity and offers a manual fallback if the clipboard is refused. Non-UTF-8 or control-containing paths retain reversible byte evidence instead of a lossy text copy.

Keep `run.json`, JSON and text with HTML. Verify artifact sizes and SHA-256 against the receipt before sharing a saved result. A receipt is neither a backup nor proof of successful final disk synchronization. An older report may no longer describe the current worktree state.

The [dated landing captures](/assets/report-captures.json) and [new interactive demo provenance](/demo/provenance.json) are separate records.
