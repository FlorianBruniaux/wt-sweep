`report.html` is self-contained: styles and script are embedded, with no model or network resource. Open it directly in a browser. The summary and evidence remain readable without JavaScript.

[Open the captured synthetic example](/demo/report.html). This example predates the volume dashboard described below.

## Disk dashboard and worktree list

Available in `v0.1.0-alpha.2`. Both report types open with a compact summary,
**In brief: recommended actions** and the worktree list. The recommendations
follow the recorded scope and verdicts. Disk measurements and coverage details
start folded, below the list.

Review rows show **Keep**, **Needs checking** or **Possible cleanup** and the main
reason. Expand a row for the original verdict, complete object IDs and evidence.
Quick filters select a status, the size shortcut sorts measured entries, and
**Show all** restores the initial view. **Possible cleanup** still requires a usage
check and does not authorize removal.

With `--disk-usage`, the dashboard shows captured capacity, available space and measured allocation for each volume. Volumes stay separate, including APFS volumes sharing a container. Old inventories keep their sizes and explicitly report missing capacity; rendering never queries the current machine.

Inventory reports open the worktree list directly. Filter present, bare, absent or unverified entries, or sort the entire list. Review reports retain Git-family grouping and verdict filters. Cmd+K / Ctrl+K opens a local search over sections and recorded worktrees. The reading guide works offline, including its diagram; technical scan limits remain expandable.

The diagram has seven clickable blocks. Click one, or use Tab and Enter, to reach its report section. In an inventory, the review blocks explain which Git checks are missing. In a reviewed report, they open the worktree evidence. The links work without JavaScript; with JavaScript, they also reopen collapsed destinations on repeated clicks.

Filters do not change source counts, volume gauges or recorded evidence. Partial measurements stay partial, and measured allocation does not predict freed disk space.

## Save and open a report

Add `--open` to `review`, `scan` or `plan` to save HTML and ask macOS to open it after publication. Without `--run-dir`, the destination is `~/Desktop/Worktree Sweep/run-*/`; the Desktop must already exist. Use `--html` to save without opening, or an explicit `--run-dir` outside the inspected roots. Each run has its own directory.

## GitHub and GitLab links

In a review, expand **Inspect evidence**, then **Repository links** to open the recorded repository, branch or commit. These URLs come from recognized local remotes. A local tracking ref and its equality with HEAD do not establish the current server state or prove a push. The CLI does not contact the forge.

Associated PR/MR links are visible alongside their captured state; expand their details for title and provenance. They require a matching `--forge FILE` import. Self-hosted providers also need an associated import to identify them. Unknown or ambiguous remotes remain explicit. `--no-context` skips remote metadata; existing imported PR/MR associations still appear. Inventory-only scans do not collect these links.

Opening an external link contacts the chosen forge through your browser. It does not change a verdict. [Import format and association rules](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/forge.md#links-in-review-reports).

## Read in this order

1. Scope, observation intervals, coverage and source/selection totals.
2. Git families and each worktree’s verdict.
3. Retention reasons, missing decision conditions and expanded evidence.
4. Disk rows and their complete, partial or absent measurements.

Search, verdict filters and sort change visible rows without recalculating source totals or disk measurements. CLI selection and browser filtering remain separate.

## Disk figures

**Observed disk allocation** is the deduplicated allocation of measured roots. Volume capacity and availability are independent captured values. Cleanup counts appear only for a review; an inventory alone has no cleanup verdict. Recoverable space remains unestablished.

Sizes use decimal units and are rounded for display; exact bytes stay in tooltips and JSON. The largest observed worktrees list excludes shared Git and can omit larger, unmeasured roots. Do not add its overlapping sizes.

## Navigation and sharing

The sidebar labels provide explanations on hover, focus or click. You can open and close the detail sections using the keyboard. The theme follows your system, with an optional local saved choice. Copy path uses the recorded path identity and offers a manual fallback if the clipboard is refused. Non-UTF-8 or control-containing paths retain reversible byte evidence instead of a lossy text copy.

Keep `run.json`, JSON and text with HTML. Verify artifact sizes and SHA-256 against the receipt before sharing a saved result. A receipt is neither a backup nor proof of successful final disk synchronization. An older report may no longer describe the current worktree state.

The [dated landing captures](/assets/report-captures.json) and [new interactive demo provenance](/demo/provenance.json) are separate records.

## Local content comparison and assistant prompt

Alpha.3 adds `--compare-local` to saved `review` and `plan` runs. The report separates
local bytes matching explicit Git targets, bytes matching a chosen same-family
checkout, differences and paths not compared. Index and disk layers remain distinct;
ignored files are included. Keep the linked `local-comparison.json` with the run.
Equal bytes do not establish integration, a restorable backup or cleanup eligibility.

**Claude prompt** prepares a French prompt from all saved HTML entries or the rows
matching current filters. It includes recorded paths, verdicts, sizes and missing
checks. Review the text before copying it to your assistant; nothing is sent
automatically. A manual copy fallback is available if clipboard access is denied.
