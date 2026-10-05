# Changelog

No release has been published. `Unreleased` covers the current source checkout;
the Cargo package version is a development version.

[Project overview](README.md) · [Documentation](docs/README.md) · [Contributing](CONTRIBUTING.md)

## [Unreleased]

### Added

- [French hands-on tutorial](docs/tutorial.fr.md) with a disposable two-worktree
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
