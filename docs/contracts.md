# Evidence and JSON contracts

[Documentation index](README.md) · [CLI reference](cli.md) · [Architecture](architecture.md)

The CLI's JSON artifacts store observations, unknowns and source records.
They contain no cleanup actions. Field definitions are in
[model.rs](../src/model.rs), with publication receipts in
[run.rs](../src/run.rs); no separate JSON Schema validator is provided.

## Formats

| Schema | Producer | Main contents |
| --- | --- | --- |
| `wt-sweep.inventory/1` | `scan` | Scope, interval, families, registrations, path states, discovery coverage and optional local manifests |
| `wt-sweep.discovery-checkpoint/1` | `scan --checkpoint-out` | Explicit root identities, traversal state, candidate markers and separate intervals; no saved HEAD or Git-family evidence |
| `wt-sweep.review/2` | `plan` | Inventory digest, live local summaries, policy version, classifications, Git observations, forge facts, risks and unknowns |
| `wt-sweep.forge/1` | `forge ingerer` | Normalized requests, raw-source identity, receipt metadata and coverage |
| `wt-sweep.forge-receipt/1` | External capture executor | Actual invocation, scope, interval, exit status, pages and raw-file hash |
| `wt-sweep.run/1` | Any artifact command with `--run-dir` | Run identity, publication interval, status and hashes of published artifacts |

Check `schema` before consuming a format. Do not infer a release from a schema
number. Current Rust model deserialization rejects unknown fields; additions
need explicit compatibility decisions and regression coverage.

## Paths, refs and object IDs

`BytePath` stores canonical lowercase hex without NUL. It is used for paths,
byte-preserving refs and commit subjects. For example:

```json
{"hex":"2f7265706f"}
```

These bytes spell `/repo`. Decode to bytes first; do not replace invalid UTF-8
with replacement characters. Human output escapes control and non-ASCII bytes.
Hex encoding is reversible and provides no anonymization.

Git OIDs remain full-length strings: 40 hex characters for SHA-1 or 64 for
SHA-256. Filesystem identities include device and inode. Neither a path string
nor an inode alone establishes a stable machine-wide identity over time.

## Inventory and local manifests

Read `coverage.traversal_complete`, `coverage.gaps` and `not_collected` together.
An empty list of discovered repositories after a failed read is not proof of
absence. `coverage.temporal_consistency` remains `not_guaranteed`; start and end
timestamps describe an observation interval.

`local_inventories` is populated only by `scan --local`. Each manifest has:

- `entries`: local paths, types, categories, identities, modes, ownership, sizes,
  timestamps, link counts, flags, optional SHA-256 and symlink targets.
- `index_entries`: parsed path, mode, full OID and stage where supported.
- `administration_entries`: observations of the raw index, identifiable shared
  index and the worktree's own HEAD reflog.
- Its own interval, completeness flag, gaps, omitted domains and content digest.

**A local manifest cannot restore a worktree.** It stores hashes and metadata
without copying file contents, including the index and reflog. The collector
records symlinks without following them, and reports gaps for special files
instead of reading them as regular content.

The index parser supports ordinary versions 2 and 3, including conflict stages.
Split indexes, version 4 and unsupported mandatory extensions retain unknown
semantic interpretation. Identifiable raw files are still observed. Git's
tested split-index read changed metadata, so the collector avoids that read.

ACLs, extended attributes, shared reflogs and complete object preservation are
outside this manifest. See [limits](cli.md#resource-limits),
[native boundaries](native-boundaries.md) and the [finishing ledger](finish-ledger.md).

## Discovery remainder and publication omissions

Inventory coverage can contain optional fields. Readers that reject unknown
fields need this extended inventory contract. Absent fields preserve the earlier
canonical representation.

`coverage.discovery_metrics` holds diagnostic `elapsed_ms`,
`directory_read_attempts`, `entries_listed` and `workers`. Elapsed time is
monotonic and covers discovery only, including Git family validation. Directory
reads count attempts, including failures; listed entries count files and
directories returned before stopping, including hidden and ignored entries.
Direct `--repo` discovery has zero traversal counts. No telemetry field is a
proof of completeness or a triage condition. Review coverage carries the source
inventory's metrics unchanged, including when `plan` reads an older inventory.
Older inventories without this field remain readable; strict older binaries may
reject the addition. Rates are derived for display, not stored as floating-point
facts. A zero recorded duration does not yield a rate.

`coverage.remaining` records a global discovery stop: `interrupted`, `deadline`,
`entry_budget`, `directory_budget` or `observation_budget`. `known_pending_paths`
counts known queued entries, not all filesystem work left. Its
`cover: requested_scope` refers to the original scope and roots, including unknown
parts of a requested mounted-volume scan. This conservative cover can contain
already visited paths. Refused access and depth-limited subtrees keep their own
gaps. Neither this field nor progress output is a durable checkpoint.

`coverage.publication` records observed data omitted from a published inventory:
`local_inventories`, `markers`, `families` and `worktrees`. Omitted worktree counts
include those in omitted families. Its
`cover: requested_scope_and_registered_worktrees` includes registrations outside
the discovery roots. These are omitted observed records, not counts of undiscovered
repositories. Optional omissions do not rewrite the traversal observation; the
publication receipt is partial and the report shows the missing data.

Publication first budgets mandatory scope and coverage, then optional detail and
compact rows. The exact pretty JSON bytes and final newline must fit the hard
limit. A soft observation budget also stops collection before another operation,
while retaining the bounded operation that crossed the ceiling. Queue storage,
collected models and canonical-digest working memory have separate bounds; the
publication limit is not a bound on all process memory.

A saved inventory with omitted families or worktrees cannot authorize filesystem
publication of a later review: historical outside-root boundaries may be missing.
Rescan before publishing. Marker-only or local-manifest omissions do not remove
family boundaries. A fresh scan checks its original full observations before
compacting output. A filtered view keeps the complete source review and inherited
coverage, even when no row is selected.

## Discovery checkpoints

`scan --checkpoint-out` publishes a separate file bounded to 24 MiB. Its SHA-256
covers serialized traversal state and detects accidental alteration, not authorship.
Unknown fields, versions, invalid path relationships and digest mismatches are
refused. Files are created outside inspected roots without replacement.
`--resume` revalidates root, completed-directory and alias identities; it replays
an interrupted directory and collects known candidate families again. HEADs,
refs, worktree registrations and verdicts are not serialized in this checkpoint.

The optional `coverage.discovery_intervals` array contains `started_unix_ms` and
`finished_unix_ms` for every pass. It is absent when empty, preserving older
canonical inventory digests. Strict older readers may reject it. More than one
interval sets `temporal_consistency` to `multiple_observation_intervals_no_snapshot`.
Completed subtrees remain historical observations: additions inside them require
a fresh scan. [Resume discovery](discovery-resume.md) defines allowances and refusals.

## Reviews and relationships

Each `items` entry has a classification plus observations, requests, risks and
unknowns. Classifications may coexist with unknowns. Saved local manifests are
referenced as historical records in `local_inventory_refs`, without copying
their file, index or administration arrays into the review. `plan` separately collects current local
state in `items[].local_summary`; it does not refresh the saved manifests.

| Relation | What it establishes |
| --- | --- |
| `identical_commit` | Source and target resolve to the same verified commit. |
| `ancestor` | The source commit is reachable through the target's raw commit-parent graph. |
| `identical_tree` | Verified committed trees match, even if the commits differ. |

Each observation names `source_oid`, `target_ref`, `target_oid`, `target_role`
and its interval. Only an explicitly designated ref has role `integration`.
Other refs have role `intermediate`. Tree equality does not mean the histories
are identical, and no relationship preserves local uncommitted files.

Object reads verify the bytes against their OIDs, including SHA-1 collision
detection. Missing or corrupt objects remain unknown. No patch-id, merge
simulation, age, request title or closed-request state serves as integration
proof. A precisely associated open request is a conservative reason to keep the
worktree; it does not establish a live process. See [forge association](forge.md#association-and-judgement).

## Triage policy

Current reviews use `policy_version: triage/1` and three classifications:

| Classification | Rule |
| --- | --- |
| `keep` | At least one positive reason to retain the worktree |
| `cleanup_candidate` | No keep reason, and all five conditions below established |
| `verify` | No keep reason, but one or more required conditions remain unestablished |

`items[].decision.keep_reasons` contains finite reason codes. Main checkout,
bare storage, Git registration or administration locks, operation markers,
positive local counts, a scoped open request, complete HEAD non-integration and
an uncovered private-history root are retention reasons. They accumulate even
when another domain is incomplete. A zero partial count is never evidence of
absence.

`items[].decision.missing_conditions` names the missing conditions in C1-C5 order:

| Condition | Required evidence |
| --- | --- |
| C1 | Present linked worktree, current matching family/path/HEAD/administration identities, complete protection and operation checks |
| C2 | Current deep `identical_commit` or `ancestor` observation for an explicitly accepted target; graph-only or tree equality cannot qualify |
| C3 | Complete HEAD/index/disk interpretation and five complete zero local counts, with no blocking local gap or observed change |
| C4 | Complete private-root enumeration, including HEAD, with each root currently covered by a retained named ref |
| C5 | Required evidence collected in this review, consistently bound to the current identities and HEAD, with successful final checks |

A failed unrelated target does not erase another verified integration target.
Conversely, HEAD non-integration requires complete negative graph results for
all accepted targets, with none omitted. Missing forge, subjects, changelogs,
shared stash context and usage information do not substitute for, or automatically
block, C1-C5. Historical `observed_result` values cannot satisfy a current condition.

`items[].worktree_state` carries fresh registration and administration evidence:
observed worktree, common directory and physical identities, main/linked/bare
role, administrative `protections`, `operations`, source records and interval.
`checks` is `complete`, `incomplete` or `change_observed`. A successful before/after
check is an observation, not writer exclusion. A changed saved inventory requires
a new scan; it is not silently rebound and certified.

The finite administrative lock set is `index.lock`, `HEAD.lock`,
`config.worktree.lock` and `shallow.lock`. Operation markers are `MERGE_HEAD`,
`CHERRY_PICK_HEAD`, `REVERT_HEAD`, `rebase-apply`, `rebase-merge`, `sequencer`,
`BISECT_LOG` and `BISECT_START`. `ORIG_HEAD`, `FETCH_HEAD` and a reflog alone do not
identify an operation in progress. Unreadable markers are unknown, not absent.

Deep tree/blob work is reserved for possible candidates after current controls
and local facts. The commit-graph pass still describes other worktrees. Final
private-history checks can reject a provisional candidate. Resource exhaustion
retains collected facts and blocks any promotion whose prerequisites are missing.

Every candidate states that usage remains unknown and must be checked before
removal. No report authorizes deletion. ACLs, extended attributes and unreachable
objects outside the examined roots remain outside this triage decision; it does
not establish a complete backup or restore path.

## Commit history and deep verification

Review version 2 also carries `items[].history`, separately from
`items[].observations`. Each history record identifies the source and explicit
target, their full observed OIDs, its observation interval and one result:

| History result | Meaning |
| --- | --- |
| `identical_commit` | The verified source commit has the resolved target OID |
| `ancestor` | A path of verified commit objects reaches the source from the target |
| `not_ancestor` | The complete examined target ancestry does not contain the verified source |
| `unknown` | A required read, traversal, budget or final validation is incomplete |

History records do not verify trees or blobs. They cannot populate the deep
integration evidence or promote classification on their own. A missing blob can
therefore leave an `ancestor` history result alongside no deep observation.
A negative result for one target says nothing about another target.

Targets are resolved again after local collection and deep verification. If a
reference or family changes, or cannot be revalidated, the current result becomes
`unknown`. `observed_result` retains a previously completed graph result and the
reason explains why it is no longer current. Job-cap omissions are counted in
`history_omitted_targets`; omission never means complete non-integration.

`history_metrics` describes the history phase only: first-pass worktrees, complete
and incomplete target results, object-read attempts, returned object payload
bytes, traversal steps, cache hits, target-resolution attempts, actual Git child
processes and elapsed collection/revalidation time. Traversal steps count unique verified
expansions per physical family and target OID, shared across worktrees and alias
refs. A partial traversal can establish an encountered ancestor, but only a
complete error-free traversal can establish non-ancestry. Each ref is revalidated
separately. Local and deep work are not
included. Failed I/O bytes and total operating-system work are not measured by
`object_bytes`. See [resource limits](cli.md#resource-limits).

## Collection profiles and diagnostics

Version-2 reviews may contain top-level `collection_metrics`. Older artifacts
without this field retain their canonical representation; absence means metrics
were not recorded. These diagnostics never enter C1-C5 or classification.

`budget_profile` records the requested `standard` or `thorough` profile.
`applied_limits` records effective core ceilings in explicit units, including
local per-worktree and phase milliseconds. Library callers may use smaller
limits; the recorded values, rather than the profile name, describe that run.
Additional fixed input/output, file, path and root caps remain documented in
[CLI resource limits](cli.md#resource-limits).

Each `phases[]` entry contains `name`, `elapsed_ms` and `counters`. Phases cover
controls, history, local files, deep verification, private history, final
revalidation and optional context. Discovery and publication are excluded, so
summing phases does not give total command time. Context is recorded after the
decision interval and does not extend its consistency claims. Empty counter maps
mean that only elapsed time was measured.

Counters retain their phase-specific meaning:

- History and private `object_reads` count attempted reads; local counts decoded
  object payloads offered to verification. Deep counts charged read attempts.
- `object_bytes` counts returned or decoded object payload bytes charged by the
  phase, not all I/O or memory. Protocol framing and filesystem metadata are not
  included.
- `git_processes` counts actual child launches, including failed reads. Final
  checks record separate history and private Git-process deltas.
- Local `cache_hits` includes repeated cached refusals as well as successful
  object reuse. `paths_checked` counts main disk metadata attempts, excluding
  `.git`. A zero value can mean collection stopped before walking disk paths.

Deep objects and complete verified trees are reused only inside the current
review, keyed by physical family identity, path, object type and full OID.
Private ancestry is shared within the same family. Cached immutable objects do
not replace final checks of refs, administration, indexes or local files. No
persistent cache or continuation from a previous report is implemented.

## Optional commit, changelog and remote context

Version-2 reviews may contain top-level `context`, with its own interval, rows,
`omitted_items` and gaps. Each row binds a worktree path, common-directory
identity and recorded HEAD. An absent field means context was not collected;
it preserves the canonical digest format of older version-2 reviews.

Context is collected after decision checks finish. It never changes classifications,
keep reasons, missing conditions or the decision evidence interval. A later
context read describes recorded object IDs, without re-establishing current
HEAD, target or filesystem continuity. Read failures and timeouts leave context
unknown; they do not alter the earlier judgement.

`uncovered` describes commits reachable from the recorded HEAD but absent from
the **union** of the selected target histories. Each target retains its full ref
and recorded OID. A current verified ancestor or identical-commit result proves
zero for that union. Otherwise, all selected target histories must be examined
before counting uncovered commits. Missing targets or omitted target results
leave the comparison incomplete. With no selected target, the result is unknown.

`observed` is a lower bound until `complete` is true. Samples retain full OIDs,
optional byte-preserving subjects and a subject-truncation flag. Counting can
continue after the sample limit, with `samples_omitted` recording known unsampled
commits. These are graph results: squash merges, cherry-picks and later reverts
can require manual interpretation. Subject text cannot establish integration.

Each `changelogs` entry names `revision_oid`, its verified **root** `tree_oid`,
the exact relative path and `blob_oid` when found. Status is `present`, `absent`
or `unknown`. An empty verified file is present with zero bytes. Absence requires
successful tree inspection; missing or corrupt objects, binary NUL content,
unsupported modes and size limits remain unknown. Intermediate trees and final
regular blobs are read by exact path bytes. Symlinks and gitlinks are not followed.

An excerpt is published only after verifying the whole bounded blob. `blob_bytes`
records its size; `excerpt_truncated` records an excerpt limit. Excerpts use
BytePath encoding and escaped human output. Output-budget omissions remain
explicit, including when a whole row cannot fit. A statement such as “all work
merged” in a changelog is descriptive text and has no role in C1-C5.

### Remote metadata

`context.items[].remote` is optional and omitted when absent, preserving older
review digests. It records its own interval, remotes and gaps. Each remote keeps
its byte-preserving name, provider, host, project, proposed remote branch,
configured tracking ref and observed full tracking OID. `head_matches_tracking`
is `null` without a comparison; `true` and `false` describe only equality with
the recorded HEAD. Neither proves current server state or an actual push.

The controlled collector reads remote URLs and a fixed `for-each-ref` format
once per selected family, within the context attempt, byte, output and time caps.
Credentials and unsupported destinations are not retained. The renderer derives
and validates web URLs from these fields; it does not execute Git or contact a
forge. `--no-context` omits remote metadata. `requests` keep their existing
association rules and captured provenance independently of this field.

## Targeted evidence collection

With `--worktree`, the source review still binds the full original inventory and
keeps every recorded row. History, local summaries, private history and optional
context are collected for the selected exact path. Registration and control
checks still cover the original scope. Other rows name their deliberate omission
in `unknowns`; missing required conditions prevent candidate classification.
A display filter can select the chosen row without discarding those omissions.
This is a new observation interval, not a checkpoint resume.

## Current local summary

Git timeouts may stop subsequent dependent reads for the same family. This is an
explicit collection stop, not evidence of a missing object or a corrupt repository.
Previously observed local counts remain available. Exhausting a shorter local
summary allowance stops that summary without quarantining the repository family;
a Git call that exhausts its full backend allowance retains the family-stop rule.
See [timeout behavior](cli.md#resource-limits).

Review version 2 adds `items[].local_summary`. Inventory version 1 remains the
input format. Older review readers that reject unknown fields must explicitly
support version 2. An item without `local_summary` is not a successful current observation.
Version-1 reviews remain historical artifacts; they are not inventory inputs and
cannot establish current C5 verification. Current classification follows `triage/1`
as described above.

`local_inventory_refs` retains each saved manifest's worktree, interval, content
digest, coverage flags and counts. Its full records stay in the input inventory,
which is bound by the review's `inventory_sha256`. When the aggregate summary-byte
budget is exhausted, `local_summary` is null and the item names that omission
in `unknowns`; no completed zero is invented.

Each collected summary includes its interval, observed HEAD, worktree identity, Git
administration path, source-file hashes, gaps and a digest. It compares HEAD to
index for `staged`, and index to disk for `unstaged`. `conflicts` counts paths,
not index stages. `untracked` and `ignored` count additional local files; a
structural directory containing only tracked files is not an untracked file.
Raw bytes, Git executable mode, entry type and literal symlink target matter.
Clean/smudge filters are never executed, so these raw-content checks need not
match Git status after configured content conversion.

Every count contains `observed`, `complete`, bounded `samples` and
`samples_omitted`. Interpret them together:

| Values | Meaning | Text rendering |
| --- | --- | --- |
| `observed: 0, complete: true` | No change found in a fully examined comparison | `0` |
| `observed: 0, complete: false` | No positive observation; comparison incomplete | `UNKNOWN` |
| `observed: 3, complete: false` | At least three paths observed | `>=3` |
| `observed: 3, complete: true` | Exactly three paths observed | `3` |

`index_complete` describes index interpretation, including checksum, flags and
stages. It does not promise that every referenced blob exists. `head_complete`
covers the HEAD commit, trees and required blobs. `disk_complete` covers local
traversal and content observations. Counts have their own completeness: a valid
index with a missing staged blob can retain an exact staged change while its
unstaged comparison remains incomplete. Positive work is retained after a later
read failure. Inspect every required domain before drawing a conclusion.

Ordinary index versions 2 and 3 are supported for SHA-1 and SHA-256
repositories. An intent-to-add entry (`git add -N`) records a future addition,
not staged content. A new intent-to-add path counts as unstaged, including
an empty or subsequently absent file, without being counted as untracked.
Its placeholder blob is not required to establish staged content. Split index,
version 4 and unsupported mandatory extensions remain explicit gaps. Skip-worktree and
assume-valid do not authorize skipping resident-byte comparisons. Special files,
submodules, dataless content, inaccessible objects and observed drift prevent
complete coverage. Sampling or diagnostic truncation sets `details_truncated`;
counts remain bounded observations. The summary is neither a snapshot nor a
backup. Judgement combines it with the other required conditions; a clean summary
alone does not establish a cleanup candidate.

## Private worktree history and shared stashes

`items[].private_history` records the current worktree's history roots. Each root
has a complete OID, source paths and a result:

| Result | Meaning |
| --- | --- |
| `covered` | A verified commit-parent path connects the root to the named retained anchor at the observed OID. |
| `not_preserved_by_retained_refs` | Complete retained-anchor enumeration and traversal found no covering anchor. |
| `unknown` | A source, object, traversal or final check is missing or incomplete. |

Private roots include HEAD, private refs and both nonzero OIDs of own reflog
entries, including logs left after a ref disappears. Sources carry their paths,
hashes and completeness. File hashes cover their bytes; directory hashes cover
the directory identity and type, then its sorted membership, entry identities
and types. An absent source is recorded
separately from a failed read. `enumeration_complete` concerns the roots and their
sources; `anchors_complete` concerns eligible named-ref enumeration and observed
anchor failures. It does not assert that every named ref's object was eagerly
read. Direct identity roots are verified first; other roots use one bounded
ancestry union for the physical family. Anchors already covering a root receive
at most 32 priority expansions, charged to the same budgets, before the ordinary
frontier resumes. No pending route is discarded by that scheduling change.

A failed anchor does not erase coverage already verified through another current
anchor. It prevents a complete negative when that anchor could cover the root.
Only exhausted, error-free traversal with complete eligible-ref enumeration can
establish `not_preserved_by_retained_refs`; a budget stop remains `unknown`.

A final source or anchor change invalidates the current result. `observed_result`
retains the earlier outcome; when it was `covered`, `anchor` describes the earlier
observation. Consumers must read these fields together. A source read failure is
not reported as an observed change.

The collector supports the Git `files` ref backend and commit-parent traversal.
It leaves unsupported backends and non-commit roots unknown. Named shared refs
can serve as anchors; private namespaces, other worktree HEADs and reflogs alone
cannot. Ambiguous ownership of a private ref in shared packed refs leaves the
affected linked-worktree enumeration incomplete. Annotated tags are not peeled. This graph check does not replace the
separate tree/blob integration proof or establish a durable backup. A named ref
can still be deleted after the observation interval.

`shared_stashes` has one entry per physical common-directory identity. Each entry
contains the observed stash list and its interval. A stash records its OID, base,
timestamp, bounded message and bounded file paths. `enumeration_complete`
describes list membership; each stash has separate `metadata_complete` and
`files_complete` flags. An incomplete file list does not hide a known stash.
Two clones with the same remote remain separate families. Branch wording in a
stash message does not identify the worktree that created it.

Paths and messages use `BytePath`; human reports escape control bytes. These
facts feed [C4 in the triage policy](#triage-policy). A shared stash does not
by itself keep every linked worktree, nor prove that any worktree is backed up.

## Example review item

This historical version-1 excerpt comes from the [disposable demo](getting-started.md#disposable-demo).
New version-2 reports add a separate current local summary.
It selects fields from the linked worktree's `items` entry and omits collection
timestamps and other fields. Because it is incomplete, it cannot be passed back
to the CLI as an artifact.

```json
{
  "head": "65a79c0b7669faf921fa0b3d88fe7f24735e470c",
  "classification": "integration_observed",
  "observations": [
    {
      "source_oid": "65a79c0b7669faf921fa0b3d88fe7f24735e470c",
      "target_ref": {"hex": "726566732f68656164732f6d61696e"},
      "target_oid": "65a79c0b7669faf921fa0b3d88fe7f24735e470c",
      "relation": "identical_commit",
      "target_role": "integration"
    }
  ],
  "requests": [],
  "risks": [
    "integration observations do not establish preservation of local files"
  ],
  "unknowns": [
    "local file manifest describes the inventory interval; current file bytes are not revalidated by this review",
    "forge state unknown (no export)"
  ]
}
```

| Field | How to read this result |
| --- | --- |
| `classification` | The explicit integration target matched the source commit. |
| `target_ref.hex` | Decodes to `refs/heads/main`; `target_role` records that the caller selected it. |
| `source_oid`, `target_oid` | Equal, full object IDs support `identical_commit`. |
| `requests: []` | No requests were supplied by a forge export. It does not prove absence on GitHub or GitLab. |
| `unknowns`, `risks` | Keep these alongside the classification when displaying or consuming it. |

The same historical version-1 review included `local_inventories`. In version 2,
find the full entry in the input inventory and its reference in
`local_inventory_refs`. Selected fields for the note are:

```json
{
  "path": {"hex": "6e6f7465732e747874"},
  "kind": "file",
  "category": "untracked",
  "size": 35,
  "content_sha256": "4af3c73d8b287395967842e5f0711ef165b35a9930d83e596e5a39ef714864e0"
}
```

The path decodes to `notes.txt`. Match the manifest's `worktree` to the review
item's `path` before combining them. The hash describes the note's bytes during
the scan; you still need the file itself, which may have changed since then.

## Digests

| Field | Bytes bound by the digest |
| --- | --- |
| Review `inventory_sha256` | Canonical JSON representation of the input inventory |
| Review `digest_sha256` | Canonical review JSON with its top-level `digest_sha256` field omitted |
| Current summary `digest_sha256` | Canonical summary JSON with only its own `digest_sha256` omitted, including its interval |
| Local `content_digest_sha256` | Compact JSON of the five sorted arrays: entries, index entries, administration entries, gaps, not-collected domains |
| Forge source `sha256` and `receipt_sha256` | Exact raw-export and receipt file bytes |
| Run artifact `sha256` | Exact published file bytes |

The general canonical serializer sorts object keys and preserves array order.
The local-manifest serializer uses model field order and excludes collection
timestamps; see [content_digest](../src/local.rs). These are different recipes.
Hashing the pretty-printed review file will not reproduce its canonical digest.

To reproduce a digest, follow the corresponding serializer. A matching digest
only establishes which bytes it covers. It does not verify freshness, capture
authenticity, writer exclusion, durability or approval.

## Run receipts

Each `--run-dir BASE` invocation creates a unique child directory. The JSON
artifacts depend on the command:

```text
run-<unique-id>/
  inventory.json   # scan or review
  review.json      # plan or review
  forge.json       # forge ingestion
  report.txt
  report.html      # optional with --html for scan/review/plan
  run.json
```

`run.json` names the command, run ID, publication start/end, status and the
length/hash of the JSON, text report and optional HTML report. It does not
hash itself. Collection timestamps live inside the artifacts and can precede
publication.

| Command | `partial` when |
| --- | --- |
| `scan` | Traversal is incomplete, coverage has gaps, a discovery remainder exists, or records were omitted for publication. |
| `plan` | Coverage is incomplete, has gaps/remainder/publication omissions, or any review item has unknowns. |
| `forge ingerer` | Forge coverage has gaps or pagination is incomplete. |

`complete` applies only to these command-specific checks. It does not certify
all preservation domains or current server state. A forge receipt may be complete
while `absence_proven` is false. Ordinary reviews currently retain local-file or
forge uncertainty and therefore commonly produce partial receipts.

Treat a directory without a valid `run.json` and matching artifacts as incomplete.
The receipt is written after artifact synchronization, followed by another
directory verification/synchronization and synchronization of the parent that
contains the new run directory. Its `publication` object states:

- `meaning: artifacts_synced_before_receipt`;
- `completion: requires_successful_publisher_return`;
- `durability: unknown_from_receipt_alone`.

If a later synchronization fails, a readable receipt can remain. The error names
the owned run path, stage and whether the receipt may be visible. Existing files
are retained for inspection; the command does not claim successful durability. This protocol is not the U5 durable
backup contract. No receipt grants permission to delete a worktree.


## Offline HTML projection

`scan`, `review` and `plan` accept `--html`. With no `--run-dir BASE`, the
macOS destination is `~/Desktop/Worktree Sweep/`; Desktop must already exist.
An explicit BASE must exist. Scope, symlink, exclusive-creation and receipt
rules apply to both destinations. `--open` implies `--html` and invokes the
native macOS opener only after the publisher returns successfully. This
separate presentation step changes no artifact, digest or verdict. An opening
failure is a warning, not a failed publication.
The renderer reads an `Inventory` or a `ReviewView` already in memory, without
Git calls, filesystem collection or new judgement. A scan is labelled
`not reviewed`; a review retains its source coverage, selected rows, verdicts
and full identifiers. Browser filters change visible rows only, while disk
totals still describe the source observations.

`report.html` embeds its styles and script and requests no network resources.
Paths, branch names and other collected strings are escaped as data, including
control characters. Its byte budget is 32 MiB. Rendering over that limit
refuses publication before a run directory is created; no evidence is silently
removed to make the page fit. The publisher writes and synchronizes the HTML
before binding its byte length and SHA-256 in `run.json`. HTML write or sync
failures use the same incomplete-publication rules as the other artifacts.
A later directory-sync failure can still leave a visible receipt.

The page shows the saved observation interval, not a fresh observation when
opened. It establishes neither backup durability nor removal permission.
See the [HTML report guide](html-report-plan.md) for generation and reading.

## Per-family target maps

`wt-sweep.targets/1` contains `schema` and `families`. Each family entry has
`common_dir` (BytePath), `identity` (device/inode), and `targets` (full branch refs
as BytePath). The binding must match both the saved inventory and the initially
observed directory. Later controls still detect observed changes. Replacement
never silently transfers a mapping to a new repository.

Target refs deduplicate and sort by their original bytes in both simple lists
and maps. Input order does not set priority. All phases use that same order;
when the job budget stops a run, omitted refs remain unknown.

Identical mappings deduplicate. Contradictory paths, identities or target sets
are refused, as are ambiguous source worktree paths. An omitted family receives
no targets. A simple nonempty `--target` list is refused for multiple physical
families, including legacy library entry points. If that single family becomes
unreadable after scanning, a simple target still allows an unknown stdout report
with the saved identities. Explicit maps refuse a missing or mismatched initial
identity. Neither mode silently binds to another family, and an unverified output
boundary still refuses publication. The standalone private-history API also
requires branch refs for explicit targets; private namespaces are refused as
inputs and remain excluded from retained anchors. Graph, deep-content and private
history phases use the same bound lookup. No branch default or fetch is implicit.

## Filtered review views

`wt-sweep.review-view/1` has `schema`, `source`, `row_bindings`, `selected_paths`,
`group_by`, `filters` and `digest_sha256`. `source` is the complete unchanged
`wt-sweep.review/2`. Its original digest and inventory digest are verified before
selection. The wrapper digest binds all its fields except its own digest field;
the nested source digest stays included.

Every `row_bindings` entry contains `path`, `common_dir` and `identity`, copied
from the matching inventory. These are historical grouping facts, not a current
C1 proof. Duplicate or missing row bindings refuse publication of a view.
`selected_paths` is ordered by group then path bytes. Repository groups use common
directory bytes; directory groups use the worktree path's parent. No filesystem
lookup or path alias resolution participates in selection.

`filters` always contains `repository`, `path_prefix`, `branch`, `local_state`
and `verdict`, with null for unused fields. Repository, prefix and branch values
are BytePath; local state and verdict are enum strings. The repository filter
matches either an exact common directory or an exact registered checkout path
and selects its family. Unknown paths select zero rows; ambiguous bindings refuse.
Filters intersect. Local state uses positive typed counts and the existing C3
result, not diagnostic prose or a second judgement policy.

Zero selection retains every source row, shared stash, historical binding and
coverage gap. Text shows selected and source totals. Output-location checks use
the entire inventory, including hidden rows. Unfiltered, ungrouped JSON remains
`wt-sweep.review/2`.

## Combined review publication

`review` reuses discovery and the same judgement as `plan`. Its run contains the
full original `inventory.json`, `review.json`, `report.txt` and `run.json`. The
receipt binds all three data artifacts by byte length and SHA-256, plus
`report.html` when `--html` is requested; the review
also records the inventory's canonical digest. A filtered view retains its full
source review. Source inventory compaction is unavailable in this workflow:
exceeding the publication limit causes refusal with a narrower-scope instruction.

The additional inventory artifact is written and synced before the receipt.
An inventory write or sync failure has the same incomplete-publication semantics
as a review or report failure. Existing scan/plan/forge runs keep their artifact
sets. stdout and `--out` emit the review alone.

## Optional disk accounting

Inventory `/1` and review `/2` accept optional `disk_usage`, absent unless
requested with `scan --disk-usage` or `review --disk-usage`. Judgement copies the
source inventory block unchanged into the review, where the canonical digest
covers it. A filtered view retains it in `source.disk_usage`, with all source
measurements and totals regardless of selected paths. This diagnostic block
never establishes cleanup eligibility or changes C1-C5.
`measurements` contains path bytes, root identity, `kind` (`worktree` or
`git_common`), optional `bytes` (`logical`, `allocated`), `complete`, entries
checked, excluded registered roots, duration and capped gap examples with an
omitted count. Null bytes mean no measurement; partial bytes mean observed data.

An optional `requested_paths` list records exact paths selected with `--disk-root`.
Its absence means all registered roots were requested. Every row remains in
`measurements`; rows outside the selection have `requested: false`, null bytes,
zero entries, `complete: false` and no collection gaps. Missing `requested`
means true, preserving the serialized form and digests of older reports.
`complete` continues to cover all recorded roots, so selecting a strict subset
leaves it false even when every requested root completes. Reports print the
requested, measured and unrequested counts next to the captured total.

`unique_bytes` deduplicates device/inode identities across every measured root.
`complete` concerns the disk walks, independently of discovery coverage. The
block also records limits, timestamps, monotonic elapsed time and
`temporal_consistency: not_guaranteed`. Per-row totals may overlap; APFS shared
extents and reclaimable storage are not measured. See [disk accounting semantics](disk-usage.md).

Publication retains this block intact, including when unrelated inventory rows
are compacted, or refuses publication if it exceeds the mandatory budget.
Disk incompleteness makes scan/review receipts partial. `plan` retains captured
sizes and timestamps without a new disk walk. Text reports and console summaries
render this block deterministically; absent measurements are labeled as such.
Detailed reports preserve every disk root, even when review rows are filtered.
The size observation interval is separate from the later decision interval.

### Captured volume space

`disk_usage.storage` is an optional diagnostic extension with
`schema: wt-sweep.storage/1`. Absent storage is not serialized, preserving
historical canonical digests. The new reader accepts older artifacts; an older
strict reader may refuse artifacts containing this extension. Inventory and
review envelope versions stay unchanged. It never changes C1-C5.

`devices` holds device/inode-deduplicated logical and allocated bytes per device.
Its sums equal `unique_bytes`; root sizes must not be summed to construct it.
Each device has optional `volume` and `unavailable_reason`. A volume records
the verified descriptor anchor, path bytes, native mount names, capacity,
free and user-available bytes, method, interval and bounded gaps. Unknown
quantities are null. Capacity is filesystem-reported, not a physical disk
capacity. Volume capacities and available space are never added together.
Source storage is copied intact into reviews and filtered views. Rendering
uses no live disk query; invalid or unsupported storage leaves the historical
sizes and list available, with percentages unavailable.
