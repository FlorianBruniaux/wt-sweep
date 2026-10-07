# CLI reference

[Documentation index](README.md) · [Getting started](getting-started.md) · [JSON contracts](contracts.md)

Run `wt-sweep --help` for global help and `wt-sweep review --help` for the combined workflow. The executable is
`./target/release/wt-sweep` after a release build. Options are separate arguments,
for example `--workers 4`. `scan`, `plan`, `review`, `forge`, `forge commandes` and `forge ingerer` accept `--help` or `-h` without collecting data.

## Commands

```text
wt-sweep review (--repo PATH | --root PATH... | --all)
  [--workers 1..8] [--disk-usage] [--disk-budget standard|thorough]
  [all plan options except --inventory]

wt-sweep scan (--repo PATH | --root PATH... | --all)
  [--workers 1..8] [--local] [--disk-usage] [--disk-budget standard|thorough] [--format json|table]
  [--out FILE | --run-dir BASE]

wt-sweep plan --inventory FILE [--target FULL_REF... | --target-map FILE]
  [--budget standard|thorough] [--worktree ABSOLUTE_PATH] [--filter-repo PATH] [--path-prefix PATH] [--branch FULL_REF]
  [--local-state clean|dirty|unknown] [--verdict keep|cleanup_candidate|verify]
  [--group-by repo|directory] [--forge FILE]
  [--changelog-path RELATIVE... | --no-context] [--summary] [--format json|table]
  [--out FILE | --run-dir BASE]

wt-sweep forge commandes
wt-sweep forge ingerer --raw FILE [--receipt FILE]
  [--format json|table] [--out FILE | --run-dir BASE]

wt-sweep --help | -h
wt-sweep --version | -V
```

The `forge` subcommand names are currently French: `commandes` prints command
templates; `ingerer` imports an export. English aliases are not implemented.
Running with no arguments prints help and does not scan anything.

## Review in one command

```sh
wt-sweep review --repo /absolute/path/to/repo \
  --target refs/heads/main --summary --run-dir /absolute/path/to/reports
```

The reports directory must already exist outside inspected repositories. Select
the actual target ref for your repository. `review` accepts exactly one scope:
`--repo PATH`, repeated `--root PATH`, or `--all`. It accepts `--workers 1..8`
and all plan options listed below, except `--inventory`. `--local` belongs to
`scan`; review already collects current local state during judgement.

This command reuses discovery, enrichment and judgement from `scan` and `plan`.
It creates one run containing `inventory.json`, `review.json`, `report.txt` and
`run.json`. The receipt records the size and SHA-256 of all three data artifacts.
The review binds the saved inventory digest. Filters retain the complete review
source and coverage in a `wt-sweep.review-view/1` document.

The input inventory is never compacted during this workflow. If it or a review
artifact exceeds the 32 MiB limit, narrow the scope and use a new output. This is
an explicit refusal, not a truncated successful review. With stdout or `--out`,
only the review is emitted; use `--run-dir` to preserve the input inventory too.

A historical `wt-sweep.review/1` file is a report, not an inventory. Passing it to
`plan --inventory` explains that it cannot establish current C5 verification and
requires a fresh scan or `review`. Existing `scan` and `plan` commands remain
available for inspection and replay. An interruption returns 130, including when
publication cannot complete; inspect diagnostics before assuming a run was saved.
If interruption occurs before the repository family is identified, an explicit
target cannot be bound. The command can return 130 with a target-binding refusal
and no JSON. The terminal discovery event names the interrupted scope; use a new
run to retry. `scan` remains available to emit discovery observations separately.

## scan

Exactly one scope is required. Repeated `--root` values are allowed;
`--repo`, `--root` and `--all` cannot be combined.

| Option | Behavior |
| --- | --- |
| `--repo PATH` | Inspect one Git family through an explicit repository path. Include its registered worktrees. |
| `--root PATH` | Discover repositories below a selected root; repeat for multiple roots. |
| `--all` | Discover from kernel-reported visible mounts, preserving coverage gaps. |
| `--workers N` | Parallel metadata reads, from 1 to 8; default 1. Git calls remain sequential. |
| `--local` | Collect bounded resident-file manifests, index evidence and the worktree's own HEAD reflog. Off by default. |

Discovery does not silently exclude hidden or ignored directories. It also
looks for nested repositories, bare repositories and separated administration.
Unavailable paths, contradictory registrations and traversal errors remain
observations. No repair, pruning or fetch is performed. Relative CLI paths are
stored as absolute paths so replay from another directory preserves the scope.

Discovery progress goes to stderr. The displayed pending count covers known queued paths, including
prefetched metadata; it is not an estimate of all remaining files. Stops distinguish
interruption, deadline, entry, directory and observation-byte limits. The JSON
keeps a conservative remaining-scope cover: retry the requested roots, knowing
that some paths may already have been visited. This is not a resumable checkpoint.

Ctrl-C stops new collection work and attempts a partial report. A bounded Git
read or a blocked filesystem call may finish first. Successful partial publication
still exits `130`. If repository boundaries cannot be verified, file publication
is refused; stdout-only inspection can still be available. No existing output is
overwritten or removed.

## plan

| Option | Behavior |
| --- | --- |
| `--inventory FILE` | Required saved `wt-sweep.inventory/1` artifact. |
| `--target FULL_REF` | Explicit integration ref under `refs/heads/` or `refs/remotes/`; repeat for several. Requires one physical repository family. No default is guessed. |
| `--target-map FILE` | `wt-sweep.targets/1` mapping of physical families to explicit refs. Cannot combine with `--target`. Missing families get no target. |
| `--budget PROFILE` | `standard` by default; `thorough` selects larger finite local, history, deep-object and private-history allowances. Available on `review` and `plan`, not `scan` or forge commands. |
| `--worktree ABSOLUTE_PATH` | Collect history, local files, private history and context for one exact recorded path. Full inventory, registration/control checks and unselected rows remain. This is a fresh targeted collection, not a resumed snapshot. |
| `--filter-repo PATH` | Select a family by its exact common directory or a registered main/linked checkout path in the input inventory. No alias resolution. |
| `--path-prefix PATH` | Select absolute path components: `/a` includes `/a/one`, not `/ab`. Parent traversal is refused. |
| `--branch FULL_REF` | Match exact branch bytes, for example `refs/heads/feature`. |
| `--local-state STATE` | `dirty` for positive local counts, `clean` when current C3 is established, otherwise `unknown`. |
| `--verdict VERDICT` | Select `keep`, `cleanup_candidate` or `verify` after complete judgement. |
| `--group-by GROUP` | Group selected rows by `repo` or parent `directory`, ordered by path bytes. |
| `--forge FILE` | Optional normalized `wt-sweep.forge/1` export. Its raw source and receipt, when supplied, must still be readable and match. |
| `--changelog-path RELATIVE` | Read this tracked path at each recorded HEAD; repeat to replace the default changelog paths. Paths are relative to the repository root. |
| `--no-context` | Skip optional commit samples and changelog reads. Cannot combine with `--changelog-path`. Decisions and their required checks remain the same. |
| `--summary` | Print a structured recap of verdicts, source coverage and remaining checks; incompatible with `--format json`. |

The command first collects bounded commit-history results across worktrees, then
collects current local summaries and checks deep Git relationships
and applies deterministic judgement. It needs access to the repositories and
never contacts a forge. The report uses `wt-sweep.review/2`; saved manifests remain
historical and the current counts are in `items[].local_summary`. No executable
action is emitted. Target refs are deduplicated and sorted by byte order; their input order does
not set priority under collection budgets. Filters intersect and run after analysis. With any filter or
explicit grouping, JSON becomes `wt-sweep.review-view/1`: the complete source
review, all historical family bindings and coverage remain in it. Text reports
show selected/source totals, even when no row matches. Use a narrower scan scope
to reduce discovery work, or `--worktree` to focus evidence collection; a display
filter alone does not reduce either. See
[targets and filters](targets-and-filters.md) for commands and map creation. The three verdicts and their required evidence are described
in [triage policy](contracts.md#triage-policy). See also [counter semantics](contracts.md#current-local-summary).

Optional context follows the decision checks. It lists commits outside the union
of selected target histories and excerpts tracked changelogs at recorded HEAD
OIDs. Default paths are `CHANGELOG.md`, `CHANGELOG`, `CHANGES.md`, `CHANGES`,
`HISTORY.md` and `HISTORY`, at the repository root. Working-copy text is not read
for this context. The report names its revision and object IDs, missing reads and
truncation. Context has its own interval and does not extend the validity of the
earlier checks. See [context semantics](contracts.md#optional-commit-and-changelog-context).

## forge

`forge commandes` prints GitHub/GitLab capture templates and executes nothing.
`forge ingerer` requires a raw local file; `--receipt` supplies the executor's
record of the capture. Without it, positive facts have partial provenance.

Use the [forge guide](forge.md) for supported payloads, receipt fields and a
fully local example. Raw inputs are limited to 16 MiB and receipts to 1 MiB.

## Output modes

| Mode | stdout | Saved files |
| --- | --- | --- |
| Default `scan` / `plan` | Human-readable table / detailed review | None |
| Default `forge ingerer` | JSON | None |
| `--format json` | JSON only | None, unless another output option is used |
| `--out FILE` | Human-readable output, even with `--format json` | New JSON file |
| `--run-dir BASE` | Selected format; human output also gives the run path | New run directory with JSON, `report.txt`, `run.json` |
| `plan --summary --run-dir BASE` | Structured recap, including the saved detail path | Full detail in `report.txt`, JSON and receipt |

`--out` and `--run-dir` are mutually exclusive. `--out` always saves JSON;
`--summary` changes the human output, not the saved review. Forge ingestion keeps
its default JSON stdout with `--run-dir`; add `--format table` to see the run path.

Output parents must already exist. Files and run directories are created
exclusively, outside protected scan/repository boundaries, without following
symlink ancestors. Existing files are refused. Both `scan` and `plan` recheck current worktree
registrations before publication to account for moved worktrees. These checks use
the full collected inventory, before any publication compaction. Older inventories
with relative scope paths, or already omitted families/worktrees, require a fresh
scan before filesystem publication; their missing boundaries cannot be guessed.

Prefer these output options to shell redirection: the shell creates or truncates
a redirected file before the CLI can check its location. A successful receipt
binds the artifacts it lists; it is not a backup or a snapshot. A receipt may already
be visible when a later directory synchronization fails. In that case the command
reports the run path, failing stage and unknown durability. Receipt presence alone
does not establish successful publication.

## Resource limits

`--budget standard` preserves the default ceilings below. Start there, read the
saved gaps and `collection_metrics`, then retry one exact `--worktree` with
`--budget thorough` when an object, byte, traversal or phase ceiling caused
missing evidence. Use a new run directory. A whole-family thorough run is a
separate, explicitly chosen collection after inspecting that focused result.
The larger caps are allowances, not throughput or completion guarantees.

Neither profile excludes ignored or hidden content. Controls, index/path limits,
private roots/anchors/source-entry limits, optional context and publication caps
remain bounded. A missing object, access failure, changed identity or unsupported
index can still leave evidence unknown with `thorough`.

`review` and `plan` write readable progress to stderr. The final `--summary` recap
separates discovery, local collection and decision checks, and groups recurring
history budgets, local budgets and Git timeouts. Other item-specific gaps remain
in the full report. Its JSON stdout remains a single report. The first history turn is allocated across
worktrees before any second turn; this is a guarantee about history scheduling,
not completeness or fairness of the local file collection. Limits may still leave
results unknown. Caches live only for this execution.

If a Git read required for history times out during history or local collection,
the review stops further dependent reads for that repository family, including
final Git revalidation. Counts already observed
remain in the report; skipped reads are explicit unknowns. Other families continue.
A timeout while checking ignore rules, a file-read error or an unsupported index
does not disable independent Git integration checks. The family stop is conservative:
one blocked object can leave other worktrees in that family unexamined for this run.

Local-summary, commit-history and private-history deadlines are separate from
that backend timeout. If the remaining phase allowance shortens a Git call and
expires during it, the exhausted phase stops with an explicit time-budget gap.
It does not quarantine independently collected evidence from the same family.
Missing final checks remain incomplete; cached facts do not become current
without revalidation. Later local summaries can still be collected within the
aggregate review budget. No incomplete summary becomes a clean result.

Local tree and blob reads use batches of at most 128 OIDs. A metadata pass sizes
payloads to stay within the 8 MiB transport limit. Each returned payload still
passes type and OID verification; a missing or malformed response stays unknown.
The graph shares verified target ancestry within one run and physical family.
It does not carry a completed proof into a later run.

The first of 50 present worktrees receives at most 120 / 50 seconds for its local
summary. Unused time remains available to later worktrees. This improves access
to the budget; it does not guarantee that every summary finishes. A targeted
`--worktree` retry spends the local allowance on one path while still checking
the surrounding registrations and preserving the original inventory.

| Scope | Standard ceiling |
| --- | --- |
| Root/global discovery | 1,000,000 entries, 100,000 directories, depth 256, 60 seconds |
| Discovery observation records | Soft 24 MiB serialized-record budget. Collection stops after the bounded operation that crosses it; pending traversal storage has separate entry limits. This is not a resident-memory ceiling. |
| One controlled Git call | 10 seconds, 8 MiB combined captured output |
| Local manifest per worktree | 20,000 combined file/index/administration entries; 64 MiB per file; 256 MiB read; 8 MiB compact manifest; depth 256; 60 seconds |
| Local manifests across one scan | 25,000 combined entries; 256 MiB published hashed content; 24 MiB compact manifests; 120 seconds |
| Current controls per review | 4,096 source entries; 64 KiB per file; 4 MiB source bytes including final checks; 4 MiB output; 60 seconds of control collection work. |
| Current local summary per worktree | 100,000 index entries; 32 MiB raw index; 100,000 paths; 32 MiB cumulative path bytes; 128 MiB read; 32 MiB per file; 4,096 objects; depth 256; 64 KiB summary; 30 seconds |
| Current local summaries across one review | 120 seconds shared across remaining present worktrees (up to 30 seconds each); at most 24 MiB of serialized summaries, including unavailable placeholders; remaining summaries explicitly omitted |
| Commit-history phase | 2,048 object-read attempts; 32 MiB returned object bytes; 65,536 traversal steps; 512 unique expansions per physical family and resolved target OID; 60 seconds collection plus final revalidation |
| One history turn | Yields after 4 object-read attempts, 64 KiB returned bytes or 16 traversal steps; one bounded object may cross the byte threshold; unfinished traversals resume on a later turn |
| History targets and jobs | At most 128 distinct explicit targets; 4,096 jobs assigned across worktrees by target ordinal; omitted jobs remain unknown |
| Final target revalidation | At most 4,096 fresh ref reads; missing revalidation invalidates current claims |
| Private-history sources | 4,096 source entries per family; 8 MiB per file; 32 MiB across the run including final rereads; depth 256 |
| Private-history graph | 4,096 roots and 4,096 anchors per run; 4,096 pending OIDs; 2,048 object reads; 32 MiB returned objects; 65,536 traversal steps; 512 steps per root |
| Private-history and stash phase | 4,096 Git processes; 4 MiB serialized facts; 60 seconds including its final checks |
| Shared stashes | 256 entries per family; 256 changed paths per entry; 240 bytes per message; truncation and incomplete detail remain explicit |
| Optional context across one review | 512 object reads; 4 MiB returned objects; 16,384 traversal steps; 1,024 steps per worktree; 4,096 rows; 1 MiB serialized context; 10 seconds total, at most 1 second per new Git read |
| Optional context detail | 256 KiB per object; 10 sampled commits; 240 bytes per subject; 8 changelog paths, each at most 4,096 bytes and 32 components; 4,096 bytes per excerpt after whole-blob verification |
| Published inventory, review or forge JSON | At most 32 MiB including the final newline, measured by a bounded serializer |
| Published text report | At most 32 MiB; an oversized report is compacted with its inventory or explicitly refused |

When an inventory exceeds the publication limit, optional local manifests and
marker detail are omitted first, with explicit counts. If necessary, the published
family/worktree list becomes a deterministic partial list. Mandatory coverage and
errors must fit or publication is refused. Text and JSON describe the same retained
records. Review and filtered-review output is refused if oversized, preserving its
complete source and digest contract; narrow the scan scope and retry.

The whole-scan content budget counts published hashed content, not all possible
rereads. Limit hits produce explicit gaps or a command refusal. Deadlines bound
controlled work; a syscall blocked in the kernel may not return immediately.
These ceilings are not performance measurements. `--workers` controls metadata
concurrency; `--budget` selects the review allowances below. Neither adds a
resumable checkpoint or a deadline override.

Commit subjects are bounded to 240 bytes. Human output escapes control bytes
and bounds descriptive titles and links. Descriptions never affect classification;
the JSON retains the full HEAD and evidence needed to interpret the result.

## Thorough allowances

All values not listed here retain the standard ceiling. MiB means 1,048,576
bytes. Traversal steps and object reads count different operations.

| Review phase | Standard | Thorough |
| --- | --- | --- |
| Local objects per worktree | 4,096 | 65,536 |
| Local cumulative bytes per worktree | 128 MiB | 512 MiB |
| Local worktree timeout / shared phase timeout | 30 s / 120 s | 120 s / 900 s |
| Commit-history reads / returned bytes | 2,048 / 32 MiB | 65,536 / 128 MiB |
| Commit-history total steps / steps per target | 65,536 / 512 | 262,144 / 65,536 |
| Commit-history phase allowance | 60 s | 300 s |
| Deep-object reads / returned bytes | 2,048 / 32 MiB | 65,536 / 256 MiB |
| Deep total steps / steps per target | 65,536 / 512 | 262,144 / 65,536 |
| Deep phase allowance | 60 s | 300 s |
| Private-history reads / returned bytes | 2,048 / 32 MiB | 65,536 / 128 MiB |
| Private-history total steps / steps per root | 65,536 / 512 | 262,144 / 65,536 |
| Private pending OIDs / Git processes | 4,096 / 4,096 | 65,536 / 65,536 |
| Private-history phase allowance | 60 s | 300 s |

Private source entries, roots and anchors remain capped at 4,096. Controlled Git
calls still have their own timeout and output cap. Local time is shared across
remaining worktrees up to each worktree's individual ceiling. A larger family
can therefore give each worktree less than its maximum allowance.

## Collection metrics and console results

Discovery records optional `coverage.discovery_metrics` on inventories and their
inherited review coverage. `elapsed_ms` uses a monotonic clock. The other fields
are `directory_read_attempts` (including failed reads), `entries_listed` (files
and directories returned by listings), and the requested metadata `workers`.
Counts include work done before a stop. They do not count verified file contents.
A `--repo` scan reads the Git family directly, so its traversal counts are zero.

Progress on stderr shows elapsed time and average entries per second. Text
reports include a `DISCOVERY PERFORMANCE` block with the same counts and a
separate complete/incomplete coverage status. Rates are unavailable below one
millisecond. No completion percentage or finish-time estimate is inferred from
the pending queue: the full filesystem size is unknown. Discovery timing includes
Git family validation, but excludes local analysis and report publication.
For `plan`, this block describes the saved source inventory, not a fresh scan.

See the [Rust benchmark recipe](collection-performance.md#benchmark-rust-reproductible)
for a controlled comparison of worker settings. `--workers 4` is a setting to
measure, not a promised speedup.

Current `wt-sweep.review/2` reports can include optional `collection_metrics`:
`budget_profile`, a numeric `applied_limits` map, and `phases` entries with
`name`, `elapsed_ms` and `counters`. The phases describe controls, commit history,
local files, deep objects, private history, final revalidation and optional
context when enabled. Counter keys
are phase-specific; an absent key does not mean zero work.

Applied limits state what was allowed. Counters state what was observed.
Elapsed time does not tell which cap stopped collection; read the matching gaps
and stop reasons. Object-read attempts, returned bytes, Git process counts and
cache hits are distinct measurements. Do not compare them as interchangeable
units or treat a large count as proof of complete evidence.

These phase timings include optional context when enabled, but exclude
discovery and publication; they are not the whole command duration. `history_metrics` remains the history-specific record.
Existing version-2 reports without `collection_metrics` remain readable by the
current reader; missing metrics mean no recorded telemetry. Older strict readers
may need an update to accept the new optional field.

The console's `Resolved verdicts` counts `keep` plus `cleanup_candidate`, with
`verify` listed separately. A positive retention reason can resolve `keep` even
when other candidate conditions remain unknown. `CHECKS STILL NEEDED` lists
conditions for unresolved rows; `COLLECTION TIME` shows recorded phase timings,
local paths per second and Git process counts where those counters exist. The
slowest phase's percentage uses the sum of recorded collection phases only.
A missing counter stays absent; a partial scan can still have measured throughput.
All source gaps remain in JSON and the detailed report.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Command completed. Evidence may still be partial or unknown. |
| `2` | Invalid arguments, unsupported operation, read/publication error, or another explicit refusal. |
| `130` | SIGINT observed. Inspect the report or error: partial publication may have succeeded, or its destination may have been refused. |

Diagnostics go to stderr. There is no separate exit code for incomplete coverage;
consumers must inspect the JSON and [receipt status](contracts.md#run-receipts).
`apply` always returns `2` because preservation and approval gates are not verified.

## Disk usage

`scan --disk-usage` and `review --disk-usage` measure logical file lengths and allocated blocks for each
registered worktree and shared Git directory. It reads metadata without hashing
contents; hidden and ignored files are included. It does not follow symlinks.
The total deduplicates device/inode pairs across rows; allocated bytes are not
an estimate of reclaimable APFS space. Missing sizes remain unknown.

Limits: 30 seconds and 500,000 entries per root, 600 seconds and 2,000,000 entries
overall, depth 128. `--disk-budget thorough` raises these limits to 120 seconds
and 1,000,000 entries per root, 1,800 seconds and 10,000,000 entries overall.
It requires `--disk-usage`. A cap, error, directory mutation or refused boundary makes
that measurement partial. Discovery coverage remains separate; a complete size
walk cannot account for repositories discovery missed.

`review --disk-usage` measures after discovery, before decision evidence collection.
Its disk budget is independent of `--budget`. It covers all inventoried roots,
including when `--worktree` focuses Git/local checks or display filters select a
subset. The console and report label that scope. The summary shows recorded
totals and completeness; `report.txt` lists sizes per root. `review.json` retains
the same `disk_usage` block as `inventory.json`, or `source.disk_usage` in a view.

`plan` carries captured disk observations and their original interval into its
reports without recounting sizes. Old inventories remain readable. When sizes
were not collected, review and plan say so explicitly; no zero total is invented.
Rendering is deterministic from captured data and requires no LLM. See the
[disk usage guide](disk-usage.md) for examples and JSON.
