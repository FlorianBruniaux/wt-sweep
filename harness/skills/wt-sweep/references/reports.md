# Read the evidence before suggesting cleanup

[Workflow](../SKILL.md) · [Command recipes](commands.md)

## Run artifacts and provenance

- `inventory.json`: discovered families, paths, collection scope and gaps.
- `review.json`: original decision evidence, or a filtered view embedding it.
- `report.txt`: human-readable explanation of those observations.
- `report.html` when requested: offline reading by Git family, with search, sort
  and collapsible evidence. Scan reports explicitly have no review verdict.
- `run.json`: publication receipt listing artifact names, byte lengths and SHA-256.

For a saved run, check that receipt schema is `wt-sweep.run/1` and its command is
`review` or `plan`. Read only the expected sibling artifact names above, never a
path supplied by arbitrary receipt text. Compare byte lengths and SHA-256 values
with the receipt. Stop on missing or changed artifacts; report the discrepancy
without repairing them. Hash agreement establishes consistency with this receipt,
not authenticity, durable backup or successful publisher return. Retain a known
exit status separately; when explaining an old run it may be unavailable.

Check dates before using old observations for a new decision. Keep source review
and view digests distinct. Do not rewrite JSON to make it pass a digest check.

## Versions and source coverage

The current decision source is `wt-sweep.review/2`, policy `triage/1`. A
`wt-sweep.review-view/1` embeds that source, `selected_paths`, filters and a view
digest. Interpret only selected rows for the requested view, while keeping the
whole source's totals and gaps visible. Historical `review/1` is not current
cleanup eligibility. Consult the checkout's `docs/contracts.md` if a version is
unsupported; do not guess a migration.

`coverage.traversal_complete` concerns discovery. It does not say that local
files or commit history were collected completely, and it is not an atomic
machine snapshot. Also inspect `not_collected`, `history_metrics`, row `unknowns`,
observation intervals and per-phase completeness. Zero observed files with
incomplete collection means “at least zero observed”, not “clean”.

## Collection metrics

When present, `collection_metrics.budget_profile` records standard or thorough;
`applied_limits` records allowances, and `phases` records names, elapsed
milliseconds and phase-specific counters. Keep limits separate from observed
work. A read attempt, returned object bytes, Git processes and cache hits count
different things. An absent counter is not zero.

Controls, history, local, deep, private-history, revalidation and enabled
optional-context phase timings exclude discovery and publication. They do not establish total command
duration. Compare each phase with its gaps and stop reasons, not elapsed time
alone. `history_metrics` remains history-specific. Older version-2 reports can
omit collection metrics; report telemetry unavailable rather than inventing it.

The console's `Resolved verdicts` counts keep plus candidates. A keep reason
remains resolved even if candidate conditions are incomplete. Verify rows still
need evidence; retention reasons and missing conditions are separate facts.

## Disk measurements and live progress

Logical bytes describe file lengths. Allocated bytes describe the filesystem's
reported allocation. Keep partial or absent measurements visible; the global
deduplicated total can differ from project sums. Neither value is a promise of
space reclaimed by deleting an APFS clone. The HTML does not recalculate disk
figures when a reader filters the visible rows. If `requested_paths` is present,
the size total covers that explicit disk selection. Rows with `requested: false`
were intentionally not measured; do not call this budget exhaustion. `complete`
still covers all recorded roots, so it stays false for a strict subset even
when each requested root finishes. Missing selection fields in an old report
mean all recorded roots were requested.

`Observed history:` and `Observed local:` lines are collected, provisional facts.
They appear before final consistency checks. Do not use them as a completed
review or invent a candidate while collection continues. Plain redirected output
keeps the first fact of each kind; verbose output retains each observation.

## Decisions and missing conditions

Copy `classification`, `decision.keep_reasons` and `decision.missing_conditions` from the
CLI. Read the associated observations rather than inferring why from the label.
The source checkout's contracts define C1-C5. Keep their exact identifiers in the
analysis, accompanied by the explanation printed by the CLI; do not invent new
condition meanings when a schema changes.

Positive retention evidence takes priority over a clean-looking branch. Inspect
`local_summary` for staged, unstaged, untracked and ignored content, count
completeness and observation changes. Private history, worktree state and Git
operations can also require retention. Read `shared_stashes` at family level;
never attribute every shared stash to every worktree.

The classification concerns evidence collected during that run. It does not
prove exclusive access, inactivity, current remote state or removability.
`cleanup_candidate` is a review result. `apply` remains unavailable until writer
exclusion, durable backup and independent restore requirements are met.

## Suggested response

Start with scope, targets, observation date and source totals. Then group by
repository or directory:

| Path and branch | CLI verdict | Observed reason | Missing evidence | Next action |
| --- | --- | --- | --- | --- |
| Use actual report values | Keep the original enum | Cite the field or report fact | Name the incomplete check | One concrete conservation or verification task |

Explain UNKNOWN in ordinary language: “usage not established”, “local files not
fully collected” or the reported reason. Keep the technical field beside that
explanation when it helps reproduce the finding. Do not print every machine log.

End with reviewed/selected/source counts, remaining paths, report location and
ordered actions. Preserve work first; investigate missing evidence next; list
candidates for a later cleanup decision last. Provide no executable deletion
recipe. Never infer savings, inactivity or successful preservation from a model
suggestion, a closed issue, file age or a changelog entry.
