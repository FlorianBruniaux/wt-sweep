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
| `wt-sweep.review/1` | `plan` | Inventory digest, policy version, classifications, Git observations, forge facts, risks and unknowns |
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

## Reviews and relationships

Each `items` entry has a classification plus observations, requests, risks and
unknowns. Classifications may coexist with unknowns. Saved local manifests are
carried forward without revalidation; `plan` records that limitation.

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
proof. See [forge association](forge.md#association-and-judgement) for `active`.

## Example review item

This excerpt comes from the [disposable demo](getting-started.md#disposable-demo).
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

The same review's `local_inventories` contains a separate entry for the note.
Selected fields from that entry are:

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

Each `--run-dir BASE` invocation creates a unique child directory:

```text
run-<unique-id>/
  inventory.json   # review.json for plan; forge.json for ingestion
  report.txt
  run.json
```

`run.json` names the command, run ID, publication start/end, status and the
length/hash of the JSON and text report. It does not hash itself. Collection
timestamps live inside the artifacts and can precede publication.

| Command | `partial` when |
| --- | --- |
| `scan` | Traversal is incomplete or coverage has gaps, including propagated local-inventory gaps. |
| `plan` | Coverage is incomplete, has gaps, or any review item has unknowns. |
| `forge ingerer` | Forge coverage has gaps or pagination is incomplete. |

`complete` applies only to these command-specific checks. It does not certify
all preservation domains or current server state. A forge receipt may be complete
while `absence_proven` is false. Ordinary reviews currently retain local-file or
forge uncertainty and therefore commonly produce partial receipts.

Treat a directory without a valid `run.json` and matching artifacts as incomplete.
The receipt is written after artifact synchronization, followed by another
directory verification/synchronization. This protocol is not the U5 durable
backup contract. No receipt grants permission to delete a worktree.
