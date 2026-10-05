# Worktree Sweep: repository instructions

Read `README.md`, `docs/plan.md` and `docs/implementation.md` before implementation.
Consult `docs/progress.md` for implemented behavior, observed checks and open gates.

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

Run from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked --offline -- -D warnings
cargo test --locked --offline
python3 -B -m unittest discover -s tests -p 'test_*.py' -v
python3 -B scripts/sync-ai.py --check
```

Edit canonical instructions and agent definitions under `harness/`, then run
`python3 -B scripts/sync-ai.py --write`. Inspect the generated diff and rerun checks.
Do not edit `AGENTS.md`, `CLAUDE.md` or generated host agents independently.
The generator refuses changed generated preimages; reconcile them explicitly.

Update `CHANGELOG.md` for repository changes. Report changed files, observed
checks, unknowns and any commit identifier. Commits, pushes, releases and real
cleanup require the task's authorization. Never claim native agent discovery or
hook activation from file validation. See `docs/harness.md` for evidence levels.
