# Changelog

Changes are grouped under `Unreleased` until a release is published. The package
version in Cargo is a development version, not a release announcement.

[Project overview](README.md) · [Documentation](docs/README.md) · [Contributing](CONTRIBUTING.md)

## [Unreleased]

### Added

- Gemini workflow infographic embedded in the README, with a local-preservation
  example, descriptive alt text, editable logical source and generation record.
  The third render passed text and edge review at native and README sizes;
  two rejected drafts remain documented. No product behavior changed.

- Read-only macOS inventory and review MVP, covering U1–U4 of the
  [acceptance plan](docs/plan.md).
- Explicit repository, root and mounted-volume discovery. Preserve repository
  families, linked/detached worktrees, missing registrations, reversible path
  bytes, full object IDs, coverage gaps and observation intervals.
- Native macOS mount collection and verified process/thread/child policy against
  dataless-file materialization. Configurable parallel metadata reads from 1 to 8.
- Local Git review with verified object bytes, commit identity, ancestry and tree
  identity. Integration targets must be explicit; other refs remain intermediate.
- Offline GitLab/GitHub export ingestion, capture receipts, raw-source replay,
  positive facts from partial exports and explicit association uncertainty.
- Descriptive reports with bounded commit subjects, branches, full HEADs, MR/PR
  titles and links, observations, risks and unknowns. Five-line summary output.
- `scan --local` manifests for tracked, untracked, ignored and hidden resident
  files, with hashes, metadata, index observations and the worktree's own HEAD
  reflog. Ordinary index versions 2/3 and conflict stages are parsed; split-index
  semantics remain unknown to avoid the observed Git metadata side effect.
- `--run-dir` publications in unique directories with JSON, full report and
  artifact-hash receipts. Concurrent runs do not replace each other's outputs.
- Independent conservation oracles, positive Git fixtures, error injection and
  synthetic forge exports. Explicit historical replays remain opt-in.
- Disposable E1 and cleanup-session witnesses: ignored files removed without
  force, untracked-file refusal, status insensitive to content changes, index
  loss through a HEAD-only patch, and writers outside a cwd-only process check.
- Native writer-exclusion capability probe, with the failed lease/revocation
  observations and the next authority boundary recorded in the
  [apply decision](docs/decisions/apply-readiness.md).
- Canonical AI development instructions and four roles, generated for Claude and
  Codex with drift, symlink and preimage checks. This is development tooling;
  the product needs no AI model.
- macOS CI workflow for locked dependency retrieval, offline Rust checks, Python
  suites and harness synchronization. Remote execution is not yet verified.
- User documentation on 2026-10-05: [getting started](docs/getting-started.md),
  [CLI reference](docs/cli.md), [forge imports](docs/forge.md),
  [JSON contracts](docs/contracts.md), [architecture](docs/architecture.md),
  [contribution guide](CONTRIBUTING.md) and linked documentation/decision indexes.
  Includes replayable Git and forge demos on disposable synthetic inputs.

### Changed

- Reworked the README in English with a direct quick start, capabilities,
  classification guide, data flow, roadmap and validation links. Existing French
  engineering records remain available through the documentation index.
- Grouped this changelog by change type while retaining bootstrap history.
- Replaced stale bootstrap status in canonical repository instructions with a
  link to implementation evidence and regenerated the affected projections.
- Kept local handoffs outside tracked source to avoid distributing private paths.

### Fixed

- Main-worktree identification with `--separate-git-dir`.
- Uncertainty retention when mounts change during discovery.
- Output placement inside a repository not yet discovered by a partial global
  scan, or inside a worktree moved after inventory capture.
- Altered normalized forge facts and contradictory host/project fields being
  eligible for precise association; missing GraphQL nodes remain coverage gaps.
- Reserved flags in index v3 and inconsistent entry budgets across files, index
  entries and administrative observations.
- Redundant relation reads when the source commit is unreadable. The FIFO
  regression passes within its original timeout; malformed subjects on valid
  objects still allow Git judgement.
- Test-process cleanup that depended on a global process-table read. Tests now
  reap the process they own, including on failure.

### Read and preservation boundaries

- Controlled Git arguments and environment, bounded I/O and timeouts, executable
  configuration neutralization, and denial of implicit object downloads.
- Commit/tree/blob bytes checked against SHA-1 or SHA-256 OIDs, with SHA-1
  collision detection. Missing or corrupt objects remain unknown.
- Exclusive output publication outside inspected boundaries, with descriptor
  identity checks. Three private unsafe boundaries are documented and reviewed:
  native mounts, materialization policy and descriptor-selected Git cwd.
- `apply` remains unavailable. E1 writer exclusion is not demonstrated; durable
  backup, independent restore and removal are blocked. The
  [finishing ledger](docs/finish-ledger.md) records local checks and remaining limits.

## Bootstrap history

The initial scaffold pinned Rust and Cargo, exposed help/version and refused
unimplemented product commands. It introduced the generated development harness,
CLI/generator tests and acceptance plan before discovery and review existed.
The implementation and scaffold were first committed together in `a1a768f`.
There was no separate bootstrap release.
