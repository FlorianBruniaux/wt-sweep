# Contributing

[Project overview](README.md) · [Documentation](docs/README.md) · [Architecture](docs/architecture.md)

Worktree Sweep is a read-only macOS MVP. Contributions should preserve explicit
unknowns, reversible path identities and the separation between collection and
judgement. Read the [acceptance plan](docs/plan.md) and
[implementation responsibilities](docs/implementation.md) before changing behavior.

## Set up and verify

Use macOS, the Rust toolchain pinned in [rust-toolchain.toml](rust-toolchain.toml),
`/usr/bin/git`, and Python 3.11 or newer (`tomllib` is required). Fetch dependencies
explicitly if they are not cached:

```sh
cargo fetch --locked
```

Run the local checks from the repository root:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked --offline -- -D warnings
cargo test --locked --offline
cargo build --locked --offline
python3 -B -m unittest discover -s tests -p 'test_*.py' -v
python3 -B -m unittest discover -s experiments/exclusion -p 'test_*.py' -v
python3 -B scripts/sync-ai.py --check
```

The Python product tests execute `target/debug/wt-sweep`, so rebuild it after
changing Rust code. The repository's [CI workflow](.github/workflows/ci.yml)
defines the same checks. A workflow file does not establish a successful remote
run. Record actual results, skipped tests and the source revision tested.

Where `rtk` is installed, follow repository instructions and use its supported
wrappers. Use `rtk proxy` when exact output matters, especially for Git evidence.
The product's Git parser must never consume transformed wrapper output.

## Change behavior

1. Inspect the branch, HEAD and dirty paths. Preserve unrelated changes.
2. Write a failing behavior test for nontrivial changes. Include positive cases
   so an implementation that always refuses cannot satisfy the contract.
3. Use real, uniquely owned temporary Git repositories for Git proofs. Mocks can
   inject errors, but cannot prove commit relationships or conservation.
4. Keep collection I/O outside deterministic judgement. Standard-library code
   comes first; dependencies need an implemented use.
5. Update the affected guide and [changelog](CHANGELOG.md). Run the relevant
   regression tests and required repository checks before delivery.

Never test cleanup against a real project. Inject roots and volumes into global
discovery tests; do not scan the host machine during development. Do not fetch,
prune, garbage-collect or delete branches in real repositories as part of a test.
Synthetic fixtures must not contain credentials, personal configuration or
private forge exports.

`apply` remains unavailable until writer exclusion, durable backup and
independent restoration are demonstrated. See the
[decision record](docs/decisions/apply-readiness.md). File inventories, closed
requests, patch IDs and clean status output cannot replace those conditions.

## Documentation

Keep public user guides in English and retain the existing French engineering
records in their original language. Link a new document from its directory's
README and connect it to related guides. User commands belong in the CLI guide;
acceptance evidence belongs in a dated validation record.

Replay documented examples on disposable fixtures. Check paths and anchors when
moving files. Update `CHANGELOG.md` under `Unreleased`; do not invent a release
date, license, benchmark or CI badge. Keep source/setup instructions usable
without a global install.

## License and labels

The project's [license](LICENSE.md) is FSL-1.1-ALv2. Preserve third-party notices
when adding or adapting external code.

GitHub issues use the standard type labels, such as `bug`, `enhancement` and
`documentation`. Add an area label when the scope is known:

| Label | Scope |
| --- | --- |
| `area:discovery` | Repository families, traversal and coverage |
| `area:review` | Git relationships, classifications and reports |
| `area:forge` | Offline GitHub/GitLab exports and association |
| `area:preservation` | Local manifests, writer exclusion and future restore/removal gates |
| `area:documentation` | README, guides, examples and diagrams |
| `area:governance` | License, packaging, CI and project metadata |

Use `blocked` when a named prerequisite or decision prevents progress. A label
does not establish that an acceptance gate passed.

## AI development harness

The runtime does not need an AI agent. Repository instructions and four optional
development roles are generated from [harness/](harness/). If changing them,
edit the canonical files and regenerate:

```sh
python3 -B scripts/sync-ai.py --write
python3 -B scripts/sync-ai.py --check
```

Do not edit generated `AGENTS.md`, `CLAUDE.md` or host agents independently.
The generator refuses drifted preimages. Review the generated diff. Validation
does not prove native loading or hook activation; see [harness evidence](docs/harness.md).

Delegation needs task authorization and explicit file ownership. The integrator
owns shared contracts and Cargo metadata; independent reviewers do not approve
their own implementation. Commits, pushes, releases and real cleanup follow the
task's authorization, not an agent's availability.

## Report a problem

Include the version, OS, exact command, exit status and the smallest reproducible
fixture. Describe expected behavior, observed behavior and relevant gaps.
Sanitize paths, remote URLs, commit subjects and MR/PR titles before sharing
artifacts. Hex-encoded paths are reversible and are not anonymized.

A review should name the changed files, observed checks and remaining unknowns.
For conservation claims, attach independent before/after evidence. For changes
to a schema or its digest, identify consumers and add regression coverage.
