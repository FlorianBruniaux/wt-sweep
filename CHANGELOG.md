# Changelog

No release has been published. `Unreleased` covers the current source checkout;
the Cargo package version is a development version.

[Project overview](README.md) · [Documentation](docs/README.md) · [Contributing](CONTRIBUTING.md)

## [Unreleased]

### Added

- Opt-in `scan --disk-usage` and `review --disk-usage`: allocated and logical sizes per worktree and
  shared Git directory, inode-deduplicated totals, bounded metadata-only walks
  and explicit partial measurements. Includes ignored files, hardlink and
  nested-root accounting, tests and a [disk usage guide](docs/disk-usage.md).
  `review` publishes sizes in its console recap, `report.txt` and `review.json`;
  `plan` carries captured sizes and their original interval from the inventory.
  Filters retain source totals. Missing measurements are explicit, and disk
  accounting never changes cleanup verdicts.

- Discovery performance in progress, text reports and optional
  `coverage.discovery_metrics`: monotonic elapsed time, directory-read attempts,
  listed entries and worker count. Console rates stay separate from coverage.
- Review recaps show measured local paths per second, per-phase Git process
  counts when available, and the slowest recorded collection phase.
- An opt-in Rust discovery benchmark compares one and four metadata workers on
  the same disposable Git corpus, checking exact results and source conservation.

- `review` and `plan --budget standard|thorough` select finite collection
  allowances. Standard preserves the previous defaults; thorough permits larger
  local inventories, commit graphs and private-history checks. Applied limits
  and phase counters are recorded in optional `collection_metrics`.
- Large end-to-end Git fixtures cover a complete candidate with 4,200 distinct
  blobs and 600 commits, private roots, missing/corrupt objects and a family of
  50 worktrees. Their independent oracle checks source conservation.
- Console recaps separate resolved verdicts from incomplete candidate evidence,
  show collection time by phase and count missing checks on unresolved rows.
  The assistant skill documents focused retries and their evidence limits.

- `review` and `plan --worktree ABSOLUTE_PATH` focus history, local, private
  history and context collection on one recorded worktree. Other rows and the
  original inventory remain in the report with explicit missing evidence.


- Task-based model and reasoning profiles for the `wt-sweep` skill, with
  host-capability checks, inherited-setting fallbacks and at most two read-only
  subagents for independent report reading or verification.

- Repository `wt-sweep` skill for Claude Code and Codex: scoped CLI review,
  saved-report interpretation in pages, and evidence-based conservation and
  verification suggestions before cleanup. Includes French usage documentation
  and English/French routing scenarios.
- Shared skill generation from `harness/skills`, with resource/link checks and
  preservation of changed generated files and unrelated project skills.

- Structured `--summary` console recap with verdicts, source coverage, local
  checks completed or missing, remaining decision checks and grouped collection
  limits. Saved runs print the full report path in a separate block.
- Readable discovery and review progress on stderr. JSON output and decision
  rules are unchanged; full reports retain technical evidence and digests.

- Add `review` to discover and judge worktrees in one command, preserving the input
  inventory beside the review and report in a single run with artifact hashes.
- Add command-specific help and an explicit diagnostic for historical review/1
  archives passed where an inventory is required.

- Discovery progress, distinct stop reasons and a conservative remaining-scope
  cover. Ctrl-C retains partial observations when possible and exits `130`;
  destination verification can still refuse publication.
- Exact JSON/text publication limits. Oversized inventories declare omitted
  detail or rows; oversized reviews refuse without changing their source data.
- Publication fault tests using real files, short writes, full-disk and interrupted
  writes, and failures at each synchronization stage. Run receipts distinguish
  artifact hashes from successful publisher completion.

- Optional commit and changelog context at recorded Git OIDs, with separate
  observation intervals, bounded excerpts and explicit incomplete results.
  Commit counts compare HEAD against the union of selected target histories.
  `--changelog-path` selects tracked paths; `--no-context` skips these reads.
  Neither option changes the triage decision rules.
- Forge provenance in detailed reports: host/project, recorded HEAD, update
  time, capture interval, raw-source hash and receipt verification.

- Per-repository integration targets via an identity-bound `--target-map`. A plain
  `--target` now requires one physical repository family; missing mappings never
  select a default branch. Target lists now use canonical byte order in every
  phase rather than input order; omitted targets remain unknown.
- Report filters for repository, path, branch, local state and verdict, with
  grouping by repository or parent directory. Filtered JSON keeps the complete
  source review, original coverage and digest, plus a separately digested view.

- Three triage verdicts: `keep`, `cleanup_candidate` and `verify`. Positive
  retention reasons take priority; candidates require current controls, verified
  Git integration, complete clean local state and covered private history.
  Reports name missing conditions and the next checks. Usage remains unknown.
- Current worktree role, registration protection, administrative locks and Git
  operation markers, checked before and after collection. Deep object checks
  are reserved for possible candidates; collected graph facts remain visible.

- Current private-history roots and shared stash context in version-2 reviews.
  Root coverage uses verified commit ancestry and explicit named anchors, with
  separate results for uncovered and incomplete history. Earlier observations
  remain labeled after failed final checks.
- One stash list per physical repository family, with separate membership,
  metadata and file-detail completeness. Human reports keep shared stash context
  separate from each worktree's own history.
- A bounded commit-history pass across worktrees before local file collection and
  deep Git checks. Per-target graph results distinguish complete non-ancestry
  from incomplete reads and remain separate from tree/blob verification.
- History progress on stderr and counters in `history_metrics`, with in-run
  caches, work-unit budgets and explicit omitted-target counts. References are
  revalidated after local and deep work; earlier graph results remain visible
  when the current result becomes unknown.
- Family-level timeout containment across history, local collection, deep checks
  and final Git revalidation, so distinct HEADs do not repeatedly retry a blocked
  repository while other families wait.
- Current local summaries in `plan`, under `items[].local_summary` in
  `wt-sweep.review/2`. Separate counts describe staged and unstaged changes,
  conflicts, untracked and ignored files, with observation intervals, source
  hashes, partial counts and explicit gaps. Input inventories remain version 1.
- Separate limits for compact local collection, including ordinary indexes with
  more than 20,000 entries. Detailed `scan --local` manifest limits are unchanged.
- [Worktree triage action plan, v2](docs/superpowers/plans/2026-10-05-worktree-triage-v2.md)
  and [architecture/plan critique](docs/superpowers/plans/2026-10-05-worktree-triage-review.md),
  covering decision conditions, large-repository limits, local work and stashes,
  machine discovery and filtering. The revised plan defines delivery dependencies
  and acceptance criteria. Delivery status is recorded in the plan.
- [French hands-on tutorial](docs/tutorial.fr.md) with a disposable three-worktree
  trial, expected results with and without an integration target, local test
  commands and a copyable prompt for an assistant reviewing one repository.
- [French README](README.fr.md) with translated Mermaid diagrams and expandable
  sections, plus links to switch between the English and French overviews.
- Dark SVG infographic showing a matching commit alongside an untracked local
  file, based on the disposable demo. Includes editable text and accessible
  descriptions; no image-generation service was used for this visual.
- README Mermaid diagrams for the scan/review flow and classification rules.
- Read-only macOS inventory and review MVP, covering U1–U4 of the
  [acceptance plan](docs/plan.md).
- Discovery from a repository path, selected roots or mounted volumes. Results
  include repository families, linked and detached worktrees, missing
  registrations, coverage gaps and observation intervals. Paths retain their
  original bytes, and object IDs remain full-length.
- Native macOS mount collection and a verified policy that disables dataless-file
  materialization for the process, reading threads and Git children. Metadata
  reads can use 1 to 8 workers.
- Local Git review that verifies object bytes and checks commit identity,
  ancestry and tree identity. Integration targets must be explicit; other refs
  remain intermediate.
- Offline GitLab/GitHub export imports with capture receipts and raw-source
  replay. Partial exports retain observed requests and gaps in their provenance
  or association with a worktree.
- Detailed reports with bounded commit subjects, branches, full HEADs, MR/PR
  titles and links, observations, risks and unknowns. `--summary` prints five lines.
- `scan --local` manifests for tracked, untracked, ignored and hidden resident
  files, with hashes, metadata, index observations and the worktree's own HEAD
  reflog. Ordinary index versions 2/3 and conflict stages are parsed; split-index
  semantics remain unknown to avoid the observed Git metadata side effect.
- `--run-dir` to save JSON, the full report and an artifact-hash receipt in a
  unique directory. Concurrent runs do not replace each other's outputs.
- User documentation on 2026-10-05: [getting started](docs/getting-started.md),
  [CLI reference](docs/cli.md), [forge imports](docs/forge.md),
  [JSON contracts](docs/contracts.md), [architecture](docs/architecture.md),
  [contribution guide](CONTRIBUTING.md) and linked documentation/decision indexes.
  Includes replayable Git and forge demos, a captured report and annotated JSON.
- Workflow infographic with descriptive alt text and an
  [editable source and generation record](docs/assets/README.md).
- Disposable Git fixtures, independent conservation checks and a macOS CI
  workflow. Observed results and exclusions are in the
  [validation ledger](docs/finish-ledger.md#vérifications).
- Writer-exclusion experiments and the
  [decision to keep removal unavailable](docs/decisions/apply-readiness.md).
- Optional [AI development harness](docs/harness.md) with generated Claude/Codex
  instructions and four roles. The CLI needs no AI model.
- [FSL-1.1-ALv2 license](LICENSE.md), matching Agent Router, declared in Cargo.
- README badges for the CI workflow, pinned Rust toolchain, macOS runtime
  and license; six GitHub area labels documented in the contribution guide.
- Author portrait, role and profile/blog/project links in the README, matching
  the AgentSec presentation block.

### Changed

- Deep candidate checks batch distinct blobs and reuse verified trees within the
  current physical family and review, under the same object-integrity checks.
- Private-history checks share one bounded ancestry union. Already useful
  anchors receive up to 32 priority expansions before the ordinary frontier
  resumes; complete negatives still require every required route to finish.

- Separate README sections with horizontal rules in both languages, split navigation
  into two rows, and display the read-only status in a GitHub note.

- Local summaries group tree and blob reads into bounded Git batches, size
  payloads before reading them and verify each object against its full OID.
- Commit history shares a verified target traversal across worktrees and alias
  refs in the same physical family. The per-target step cap counts unique
  expansions; each ref still receives its own final check.
- Local collection reserves a share of the remaining time for each remaining
  worktree. A focused retry gets the normal per-worktree allowance.
- The console distinguishes local time, object and byte caps and explains a
  focused retry without implying that per-worktree caps are raised.

- Align the English/French README, acceptance plan and progress entry points with
  the merged T01-T09 triage. Add a roadmap separating large-repository collection
  work from the gated backup and removal phases, with dated delivery evidence.
- Regenerate repository Claude/Codex instructions and all four agent roles from
  canonical sources with the current triage contracts and validation commands.
  Model inheritance and permissions are unchanged; native loading remains unverified.

- Fresh scan paths are stored as absolute paths. Filesystem publication refuses
  ambiguous legacy relative scopes and inventories with omitted family boundaries.
- Scan and review share final output-boundary revalidation. Parent directories
  are synchronized after creating a run; visible receipts after a later failure
  remain available with explicit unknown durability.

- Simple single-family target lists retain unknown report rows when that family
  disappears after scanning. Explicit target maps still refuse an unreadable or
  mismatched initial identity; unsafe publication remains refused.

- Replaced the README workflow image with the Git/local-file example. Mermaid
  diagrams explain the workflow; the earlier Gemini image and generation history
  remain in the assets documentation.
- Collapsed README build notes, capability details, classification diagram and
  validation records into expandable sections. Quick-start commands and current
  product limits remain visible.
- Rewrote the README in English with cloning instructions,
  report examples and links to the existing French engineering records.
- Replaced the README slogan and repetitive guide prose with concrete
  descriptions. Commands, captured outputs and product limits are unchanged.
- Grouped the changelog by change type and retained bootstrap history.
- Named the private visual reference without linking to an inaccessible page.
- Replaced stale bootstrap status in canonical repository instructions with a
  link to implementation evidence and regenerated the affected projections.
- Kept local handoffs outside tracked source to avoid distributing private paths.

### Fixed

- Git output collection waits for readable/writable descriptors instead of
  sleeping after each producer burst. Input/output caps, deadlines, failed
  exit checks and process-group cleanup remain enforced.
- Count deep Git processes across all families and auxiliary reads; refuse
  invalid public local-phase deadlines before collection starts.
- Restore exhausted object/byte refusals before reading repository identity.
- Preserve the original local byte-limit reason instead of reporting a later
  check of an already stopped collector as a timeout.
- Retain the requested full object IDs when a deep blob batch fails, without
  attributing a batch timeout to an unverified individual object.
- Wait for actual SIGINT-handler delivery in the isolated signal test after
  sending one signal; the production handler is unchanged.

- Separate commit-history and private-history phase deadlines from full Git-call
  timeouts. Budget exhaustion leaves the affected checks incomplete without
  quarantining unrelated evidence in the same repository family.
- Failed final private checks now invalidate enumeration completeness, keeping
  both private coverage and final consistency unknown.

- Give the expanded macOS CI suite 45 minutes: the 15-minute and intermediate
  30-minute job limits interrupted runs. Test commands and product time budgets
  are unchanged.

- Distinguish an exhausted local-summary allowance from a full Git-call timeout.
  A slow worktree no longer suppresses later worktrees merely because its own
  shorter time budget expired during a child read.

- Normalize full hexadecimal Git object identities across graph, deep and private
  history checks. Valid uppercase parent IDs no longer produce a complete
  non-ancestry result; repeated case aliases do not consume extra graph steps.
- Refuse malformed commit graph headers as incomplete evidence instead of
  treating an unread parent as a complete negative relationship.

- Stop repeated dependent reads within a repository family after a Git timeout
  during current local collection. Preserve observed counts and continue other
  families; ignore-rule and physical-file errors keep independent Git checks.

- Explicit integration targets are reviewed before intermediate refs. Repositories
  with many branches can now produce an observation for a requested target before
  reaching the candidate limit. Object verification and the 128-candidate limit
  are unchanged; shared review budgets can still leave worktrees unexamined.
- Python fixture Git commands now close standard input explicitly. Running the
  suite from a terminal previously made `pack-objects --all` wait for input and
  time out during the FIFO-pack test setup. A regression keeps the caller's
  input open while checking that fixture packing completes; timeouts are unchanged.
- Transport-denial test setup now uses the standard ten-second Git budget.
  The first GitHub run exceeded its one-second fixture-initialization limit.
  The assertion requires a protocol refusal rather than accepting any error;
  production deadlines are unchanged.
- Main-worktree identification when Git administration uses `--separate-git-dir`.
- Discovery now retains uncertainty when mounts change during a scan.
- Output checks now reject locations inside a repository not yet discovered by
  a partial global scan, or inside a worktree moved after inventory capture.
- Forge association no longer accepts altered normalized facts or contradictory
  host/project fields. Missing GraphQL nodes remain coverage gaps.
- Corrected index v3 reserved-flag handling and inconsistent entry budgets
  across files, index entries and administrative observations.
- Redundant relation reads when the source commit is unreadable. The FIFO
  regression passes within its original timeout; malformed subjects on valid
  objects still allow Git judgement.
- Test-process cleanup that depended on a global process-table read. Tests now
  reap the process they own, including on failure.

### Read and preservation boundaries

- Git runs with controlled arguments and environment, I/O limits and timeouts.
  The CLI neutralizes executable configuration and denies implicit object downloads.
- Object reads check commit, tree and blob bytes against SHA-1 or SHA-256 OIDs,
  with SHA-1 collision detection. Missing or corrupt objects remain unknown.
- Outputs require new files outside inspected boundaries and descriptor identity
  checks. Three private unsafe boundaries are documented and reviewed: native
  mounts, materialization policy and descriptor-selected Git cwd.
- `apply` remains unavailable. E1 writer exclusion is not demonstrated; durable
  backup, independent restore and removal are blocked. The
  [finishing ledger](docs/finish-ledger.md) records local checks and remaining limits.

## Bootstrap history

The initial scaffold pinned Rust and Cargo, exposed help/version and refused
unimplemented product commands. It introduced the generated development harness,
CLI/generator tests and acceptance plan before discovery and review existed.
The implementation and scaffold were first committed together in `a1a768f`.
There was no separate bootstrap release.
