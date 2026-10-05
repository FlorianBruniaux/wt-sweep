# Getting started

[Documentation index](README.md) · [CLI reference](cli.md) · [Evidence contracts](contracts.md)

## Prerequisites

The supported runtime is macOS. Protected reads require native control of
dataless-file materialization; other platforms refuse operations that require
that guard. `/usr/bin/git` must be available. Actual FileProvider placeholders
and TCC denial behavior remain outside the verified coverage.

Build from an existing source checkout with the Rust toolchain pinned in
[rust-toolchain.toml](../rust-toolchain.toml):

```sh
cargo fetch --locked
cargo build --release --locked --offline
./target/release/wt-sweep --version
./target/release/wt-sweep --help
```

The first command retrieves dependencies. Omit it if the locked dependencies
are already cached. Rustup may also need to download the pinned toolchain.
Python 3.11+ is needed for development tests and the optional forge demo, not
for running the CLI. No credentials, daemon or model are needed.

## Disposable demo

Run this block from the source checkout. It creates a new repository, one linked
worktree and a local note under a uniquely owned temporary directory. The shell
settings apply only inside the subshell. Git configuration and templates are
isolated for the fixture.

```sh
(
  set -eu
  export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_COUNT=0
  unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE
  unset GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES

  demo_dir="$(mktemp -d /private/tmp/wt-sweep-demo.XXXXXX)"
  mkdir "$demo_dir/reports"
  git init --template= --initial-branch=main "$demo_dir/repo"
  git -C "$demo_dir/repo" -c user.name=Demo -c user.email=demo@example.invalid \
    -c commit.gpgSign=false commit --allow-empty -m "Demo baseline"
  git -C "$demo_dir/repo" worktree add -b demo "$demo_dir/worktree"
  printf 'Local note, absent from the commit\n' > "$demo_dir/worktree/notes.txt"

  ./target/release/wt-sweep scan --repo "$demo_dir/repo" --local \
    --out "$demo_dir/reports/inventory.json"
  ./target/release/wt-sweep plan --inventory "$demo_dir/reports/inventory.json" \
    --target refs/heads/main --summary --run-dir "$demo_dir/reports"
  printf 'Demo and reports: %s\n' "$demo_dir"
)
```

The review starts with these three lines on a complete run of this fixture:

```text
Worktrees: 2
Active: 0; integration observed: 2; to triage: 0
Unknowns: 2 worktrees; coverage gaps: 0
```

The remaining two summary lines give the saved detail path and review digest.
Their values vary per run. The main checkout counts as a worktree. Both HEADs
match `refs/heads/main`, but `notes.txt` remains untracked local work. That is why
an integration observation cannot authorize removal. Both worktrees have
unknowns: forge evidence is missing and saved local manifests were not revalidated.

The fixture and its reports remain at the printed path for inspection. The demo
does not run cleanup. See [run receipts](contracts.md#run-receipts) to verify the
published files, or the [forge demo](forge.md#replay-a-synthetic-export) to inspect
offline MR data separately.

## Inspect selected roots

Choose roots explicitly. Repeat `--root` to cover several locations:

```sh
./target/release/wt-sweep scan \
  --root /absolute/path/to/projects \
  --root /absolute/path/to/worktrees \
  --workers 4 --format json
```

Hidden, ignored and nested directories are included. Access failures, traversal
budgets and unresolved markers stay in the output. `--workers` controls parallel
metadata reads only. It does not make the scan an atomic snapshot.

For saved output, use `--out` or `--run-dir` outside every selected root and
discovered repository family. The parent directory must exist and its ancestors
must be ordinary directories. On macOS, `/tmp` is commonly a symlink;
`/private/tmp` is used in the examples to avoid that ambiguity.

## Read a review

Start with `classification`, then inspect `observations`, `requests`, `risks`
and `unknowns` for each item. In JSON, also read `coverage.gaps`, `not_collected`
and any local-manifest gaps. A successful process exit can carry partial evidence.

Pass full integration refs such as `refs/heads/main` or
`refs/remotes/origin/main`. The latter is the local remote-tracking ref; no fetch
occurs. Omitting `--target` still produces a report, but local relationships only
have an intermediate role and cannot produce `integration_observed`.

Subjects and MR/PR titles help you recognize work. They do not affect judgement.
An open, precisely associated MR/PR takes priority and yields `active`.
Missing associations never prove there are no requests.

`plan` checks Git state during enrichment. Local manifests retain the scan's
timestamp and are marked as not revalidated. To collect current local contents,
run a new `scan --local` into a new output. Keep each run's context and interval.

## Troubleshooting

| Symptom | Meaning and next step |
| --- | --- |
| Offline build cannot resolve a dependency | Run `cargo fetch --locked` with network access, then retry the offline build. |
| Native materialization guard fails | The required macOS policy was not established. Record the error; there is no bypass flag. |
| Output already exists | Select a new filename or use `--run-dir` for a separate run. |
| Output is inside the inspected scope | Use an existing report directory outside the roots, worktrees and Git administration. |
| Output ancestor is not a plain directory | Use the actual directory path without symlink ancestors, such as `/private/tmp` on macOS. |
| Only `to_triage` appears | Check explicit targets, source and target object availability, and item unknowns. This can be a valid result. |
| `partial` receipt or coverage gaps | Inspect the JSON details. Narrow the scope if a collection budget was reached; other missing evidence may remain. |
| Split index or index v4 lacks classifications | Raw index evidence is retained where identifiable, but unsupported semantic interpretation stays unknown. |
| `apply` is unavailable | Expected behavior. Read the [decision](decisions/apply-readiness.md); no removal flag is implemented. |

For a bug report, provide the CLI version, OS version, command, exit code and
sanitized error or artifact excerpt. See [contribution guidance](../CONTRIBUTING.md#report-a-problem).
