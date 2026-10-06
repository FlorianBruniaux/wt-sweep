# Worktree Sweep: repository instructions

Read `README.md`, `docs/roadmap.md`, `docs/plan.md` and
`docs/implementation.md` before implementation. The completed triage contract is
`docs/superpowers/plans/2026-10-05-worktree-triage-v2.md`; current evidence is in
`docs/finish-ledger.md#triage-t01-t09`. `docs/progress.md` links that checkpoint
and retains the historical U1-U4 results.

## Current workflow

- `review` discovers an explicit scope and collects current evidence in one run.
  `scan` and `plan` remain available for separate inventory and review.
- Preserve `keep`, `cleanup_candidate` and `verify` with their C1-C5 conditions.
  Positive retention reasons take priority. Missing evidence remains unknown;
  completed discovery does not imply complete local or history collection.
- Local summaries use finite Git batches with size checks and per-object OID
  verification. Commit history shares a target walk within one physical family.
  Preserve unknowns at the remaining byte/object/depth/time caps.
- `--worktree` narrows expensive evidence collection to an exact recorded path.
  Keep the original inventory and all rows; distinguish deliberate omissions
  from budget exhaustion. Display filters alone do not reduce collection work.
- `--budget standard|thorough` selects finite collection allowances. Inspect
  `collection_metrics`, local gaps and source coverage before a focused thorough
  retry. Recorded phase counters are diagnostics, never decision evidence.
  Private roots share a verified graph within one physical family. Keep final
  ref and administration checks fresh, including on cache hits.
- Large-repository collection can still reach its budgets. Measure the affected
  phase before optimizing; do not relax evidence requirements to yield candidates.
- See `docs/contracts.md` and `docs/targets-and-filters.md` before changing schemas,
  per-family targets or views. Filters must retain the source evidence and coverage.

## Product boundaries

- The MVP is read-only discovery and review. `apply` stays unavailable until the
  writer-exclusion, durable-backup and independent-restore gates pass.
- Keep collection I/O separate from deterministic judgement. Never treat age,
  a closed merge request, patch-id or a model suggestion as proof of integration.
- Preserve unknown, incomplete and contradictory evidence. A completed walk is
  not a machine-wide snapshot. Never silently skip ignored or hidden directories.
- Do not run fetch, prune, gc, branch deletion or cleanup on real repositories.
  Destructive behavior tests use uniquely owned temporary fixtures only.
- No automatic global scan during development. Inject fixture roots and volumes.
- Control Git execution: explicit arguments, bounded input/output and timeouts,
  neutralized executable configuration and implicit object downloads, or refusal.
- Preserve path bytes and complete object IDs. No lossy identity conversion.

## Working on this repository

Inspect branch, HEAD and dirty paths before edits. Preserve concurrent work.
Assign explicit file ownership before delegation. An available agent is not an
instruction to launch it. Only the integrator changes shared contracts and Cargo
metadata. Reviewers do not approve their own implementation.

Use the standard library first. Add a dependency only for an implemented need.
Write a failing behavior test before nontrivial code. Use real disposable Git
repositories for Git proofs; mocks may inject errors but cannot prove integration.
Check exit status before accepting output or an empty result. Test positive cases
as well as refusals, so an implementation that always refuses cannot pass a unit.

Run supported commands through `rtk` when available; use `rtk proxy` when exact
output matters. This affects development commands, never the product's Git parser.
Keep fixture data synthetic or sanitized. Never read personal credentials, local
allowlists or environment files to populate tracked configuration. No global
configuration, hook installation, permission changes or network side effects are
part of this harness. Host permissions remain authoritative.

## Validation and delivery

Run from the repository root with Python 3.11 or newer (`tomllib` is required):

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked --offline -- -D warnings
cargo test --locked --offline
cargo build --locked --offline
python3 -B -m unittest discover -s tests -p 'test_*.py' -v
python3 -B -m unittest discover -s experiments/exclusion -p 'test_*.py' -v
python3 -B scripts/sync-ai.py --check
```

The repository user skill `wt-sweep` supports read-only CLI analysis and cleanup
suggestions; see `docs/assistant-skill.md`. Its canonical bundle lives under
`harness/skills/wt-sweep`, declared by `harness/skills.json`, and is generated into
`.claude/skills` and `.agents/skills`. Preserve its original CLI verdicts and
incomplete evidence. It never authorizes removal.

Edit canonical instructions, agent definitions and skills under `harness/`, then run
`python3 -B scripts/sync-ai.py --write`. Inspect the generated diff and rerun checks.
Do not edit `AGENTS.md`, `CLAUDE.md` or generated host agents and skills independently.
The generator refuses changed generated preimages; reconcile them explicitly.

Update `CHANGELOG.md` for repository changes. Report changed files, observed
checks, unknowns and any commit identifier. Commits, pushes, releases and real
cleanup require the task's authorization. Never claim native agent discovery or
hook activation from file validation. See `docs/harness.md` for evidence levels.
