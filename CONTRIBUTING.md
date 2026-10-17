# Contributing

[Project overview](README.md) · [Documentation](docs/README.md) · [Architecture](docs/architecture.md)

Worktree Sweep is a read-only macOS MVP. Preserve unknowns and original path
bytes, and keep collection I/O separate from judgement. Read the
[roadmap](docs/roadmap.md), [acceptance plan](docs/plan.md) and
[implementation responsibilities](docs/implementation.md) before changing behavior.
For triage changes, also read the [C1-C5 contract](docs/contracts.md#triage-policy)
and [completed T01-T09 plan](docs/superpowers/plans/2026-10-05-worktree-triage-v2.md).

## Set up and verify

Use macOS, the Rust toolchain pinned in [rust-toolchain.toml](rust-toolchain.toml),
`/usr/bin/git`, and Python 3.11 or newer (`tomllib` is required). Fetch dependencies
explicitly if they are not cached:

```sh
cargo fetch --locked
```

Run the local checks from the repository root:

```sh
(
  set -eu
  python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 11) else "Python 3.11+ is required")'
  cargo fmt --all -- --check
  cargo clippy --all-targets --locked --offline -- -D warnings
  cargo test --locked --offline
  cargo build --locked --offline
  python3 -B -m unittest discover -s tests -p 'test_*.py' -v
  python3 -B -m unittest discover -s experiments/exclusion -p 'test_*.py' -v
  python3 -B -m unittest discover -s experiments/provenance -p 'test_*.py' -v
  python3 -B -m unittest discover -s experiments/apfs-create -p 'test_*.py' -v
  python3 -B experiments/disk-native/run-tests.py --product "$PWD/target/debug/wt-sweep"
  python3 -B scripts/comparison/test_harness.py
  python3 -B scripts/sync-ai.py --check
)
```

The version check stops before the suites if `python3` resolves to an older
system runtime. Select a Python 3.11+ executable for the session first;
`hashlib.file_digest` and `tomllib` are required by the tests.

The Python product tests execute `target/debug/wt-sweep`, so rebuild it after
changing Rust code. The repository's [CI workflow](.github/workflows/ci.yml)
defines the same checks. Record local results, skipped tests and the source
revision tested; check the remote run separately before claiming CI passed.

The extension experiments use owned fixtures. Native disk checks require the
macOS SDK and run with address/undefined-behavior sanitizers. They qualify an
experimental candidate, not an enabled product collector. Timed campaigns stay
manual; replay commands and evidence limits are in the [extension decisions](docs/research/extension-decisions-2026-10-11.md).
The optional Worktrunk tests need an explicit tested binary path; a skipped
external-tool case does not qualify that version.

The HTML dashboard checks also run in CI. To repeat them with an installed
Playwright/Chromium (the locked `site` dependencies provide Playwright):

```sh
(
  set -eu
  export PLAYWRIGHT_MODULE="$PWD/site/node_modules/@playwright/test"
  fixture_dir="$(mktemp -d)"
  WT_SWEEP_DASHBOARD_FIXTURE_DIR="$fixture_dir" cargo test --locked --offline --test html_report dashboard_fixture_export -- --exact
  node tests/html-list.cjs
  node tests/html-shortcuts.cjs
  node tests/html-dashboard.cjs "$fixture_dir/report.html" "$fixture_dir/dashboard"
  node tests/html-command.cjs "$fixture_dir/report.html" "$fixture_dir/commands"
  node tests/html-diagram.cjs "$fixture_dir/report.html" "$fixture_dir/diagram" "$fixture_dir/review.html"
  node tests/html-ux.cjs "$fixture_dir/report.html" "$fixture_dir/review.html" "$fixture_dir/ux"
  node tests/html-claude.cjs "$fixture_dir/report.html" "$fixture_dir/review.html" "$fixture_dir/claude"
  node tests/html-comparison.cjs
  node tests/html-sections.cjs "$fixture_dir/report.html" "$fixture_dir/sections"
  node tests/html-theme.cjs "$fixture_dir/report.html" "$fixture_dir/themes"
  node tests/html-sections.cjs "$fixture_dir/review.html" "$fixture_dir/review-sections"
  node tests/html-theme.cjs "$fixture_dir/review.html" "$fixture_dir/review-themes"
)
```

Install development dependencies with `pnpm --dir site install --frozen-lockfile`
and Chromium with `pnpm --dir site exec playwright install chromium` when needed.
The report fixture contains synthetic paths and volume statistics. See
[CI and Actions policy](docs/ci.md) when a workflow fails before any job starts.

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

Keep [README.md](README.md) and [README.fr.md](README.fr.md) aligned when changing
behavior, commands or project status. The [hands-on tutorial](docs/tutorial.fr.md)
is in French; other detailed user guides are in English. Retain the existing
French engineering records in their original language.
Link a new document from its directory's README and connect it to related
guides. User commands belong in the CLI guide; acceptance evidence belongs in
a dated validation record.

Replay documented examples on disposable fixtures, and check links when moving
files or headings. Record changes under `Unreleased` in `CHANGELOG.md`. Release
dates, license statements, benchmarks and CI badges must match their sources.
Setup instructions should work from a source checkout without a global install.

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

Use `blocked` when a prerequisite or decision prevents progress, and name it in
the issue. Record acceptance evidence in the validation ledger, separately from
issue labels.

## AI development harness

The runtime does not need an AI agent. Repository instructions and four optional
development roles are generated from [harness/](harness/). If changing them,
edit the canonical files and regenerate:

```sh
python3 -B scripts/sync-ai.py --write
python3 -B scripts/sync-ai.py --check
```

Do not edit generated `AGENTS.md`, `CLAUDE.md` or host agents independently.
The generator refuses to overwrite files changed since their last generation.
Review its diff, and verify native loading or hook activation separately using
the [harness evidence guide](docs/harness.md).

Delegation needs task authorization and explicit file ownership. The integrator
owns shared contracts and Cargo metadata; independent reviewers do not approve
their own implementation. The task must authorize any commit, push, release or
real cleanup.

## Report a problem

Include the version, OS, exact command, exit status and the smallest reproducible
fixture. Describe expected behavior, observed behavior and relevant gaps.
Sanitize paths, remote URLs, commit subjects and MR/PR titles before sharing
artifacts. Hex-encoded paths are reversible and are not anonymized.

A review should name the changed files, observed checks and remaining unknowns.
For conservation claims, attach independent before/after evidence. For changes
to a schema or its digest, identify consumers and add regression coverage.
