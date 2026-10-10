# Getting started

[Documentation index](README.md) · [CLI reference](cli.md) · [Evidence contracts](contracts.md)

For a French walkthrough with expected results, local test commands and an
assistant prompt, read the [hands-on tutorial](tutorial.fr.md).

## Prerequisites

Run the CLI on macOS with `/usr/bin/git` available. Before protected reads, the
CLI must disable dataless-file materialization through a native macOS policy
and verify that the policy is in force. On other platforms, it refuses operations
that require this guard.
Behavior with actual FileProvider placeholders and TCC denials remains unverified.

Clone the repository, then build with the Rust toolchain pinned in
[rust-toolchain.toml](../rust-toolchain.toml). The public repository can be cloned over HTTPS. For an existing checkout, skip the first two commands:

```sh
git clone https://github.com/FlorianBruniaux/wt-sweep.git
cd wt-sweep
cargo fetch --locked
cargo build --release --locked --offline
./target/release/wt-sweep --version
./target/release/wt-sweep review --help
```

`cargo fetch` retrieves dependencies. Omit it if the locked dependencies
are already cached. Rustup may also need to download the pinned toolchain.
Python 3.11+ is needed for development tests and the optional forge demo, not
for running the CLI. Running scans and reviews needs no credentials, daemon or model.

## Disposable demo

Run this block from the source checkout. It creates a new repository, two linked
worktrees and a local note under a uniquely owned temporary directory. The shell
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
  printf 'Disposable worktree triage demo\n' > "$demo_dir/repo/README.txt"
  git -C "$demo_dir/repo" add README.txt
  git -C "$demo_dir/repo" -c user.name=Demo -c user.email=demo@example.invalid \
    -c commit.gpgSign=false commit -m "Demo baseline"
  git -C "$demo_dir/repo" worktree add -b demo "$demo_dir/worktree"
  git -C "$demo_dir/repo" worktree add -b clean-demo "$demo_dir/clean-worktree"
  printf 'Local note, absent from the commit\n' > "$demo_dir/worktree/notes.txt"

  ./target/release/wt-sweep review --repo "$demo_dir/repo" \
    --target refs/heads/main --summary --html --run-dir "$demo_dir/reports"
  printf 'Demo and reports: %s\n' "$demo_dir"
)
```

The console recap includes these results on a complete run of this fixture:

```text
RESULTS
  Keep: 2
  Cleanup candidates: 1
  Needs verification: 0
```

`CHECKS TO COMPLETE` separates discovery from local checks: complete, partial
or not collected. It also counts unresolved worktrees missing each decision
condition;
`COLLECTION LIMITS` groups recurring budget and timeout diagnostics when present.
With filters, verdicts describe the selection and coverage still describes the
whole source review. Usage by agents or terminals is always reported as not checked.

`REPORT` gives the saved `report.txt` and `report.html` paths. Open the HTML
path in a browser for family groups, search and collapsible evidence. The
[HTML guide](html-report-plan.md) explains source, selected and visible totals.
The full report and JSON retain the
technical evidence and digests. The main checkout is kept as the repository entry
point. The linked `worktree` is kept because of `notes.txt`. The linked
`clean-worktree` can qualify as a candidate; its usage still needs confirmation.
All three HEADs match the target, which alone does not preserve the local note.

Keep `XXXXXX` literally in the `mktemp` template; it creates a unique suffix.
The fixture and its reports remain at the printed path for inspection. Each
review run with `--html` saves the original `inventory.json`, `review.json`,
`report.txt`, `report.html` and `run.json`. The receipt binds the four data
artifacts; it is not a backup. The demo
does not run cleanup. See [run receipts](contracts.md#run-receipts) to verify the
published files, or the [forge demo](forge.md#replay-a-synthetic-export) to inspect
offline MR data separately.

## Review your repository

From the source checkout, enter the repository and target you have confirmed:

```sh
(
  set -eu
  sweep_bin="$PWD/target/release/wt-sweep"
  printf 'Absolute clone path: '
  IFS= read -r repo_path
  printf 'Confirmed full target ref (refs/heads/... or refs/remotes/...): '
  IFS= read -r target_ref
  /usr/bin/git --no-optional-locks -c core.fsmonitor=false -C "$repo_path" \
    show-ref --verify --quiet "$target_ref"
  "$sweep_bin" review --repo "$repo_path" --target "$target_ref" \
    --summary --open
)
```

The reports are saved in `~/Desktop/Worktree Sweep/run-*/` on macOS.
`--open` asks the default HTML application to open the saved page; `--html`
only saves it. Add `--run-dir "/absolute/path/to/reports"` to use an existing
directory outside the inspected scope. If Desktop is missing, inaccessible or
inside a selected root, choose another destination. Opening failure leaves the
saved reports intact and prints a reopen command.

The target must be a full local ref. A `refs/remotes/...` ref is its locally
recorded value; the tool does not fetch it. A missing or unverified target stays
a gap. There is no default `main`, `master` or current branch.

To examine each verdict separately, use the same explicit repository and target:

```sh
./target/release/wt-sweep review --repo "$repo_path" --target "$target_ref" --verdict keep
./target/release/wt-sweep review --repo "$repo_path" --target "$target_ref" --verdict cleanup_candidate
./target/release/wt-sweep review --repo "$repo_path" --target "$target_ref" --verdict verify
```

Set `repo_path` and `target_ref` in your shell first; the interactive subshell
above keeps its variables private. Each command collects a fresh review, then
filters its display. Source totals and gaps remain visible.

## Inspect selected roots

Choose roots explicitly. Repeat `--root` to cover several locations:

```sh
./target/release/wt-sweep scan \
  --root /absolute/path/to/projects \
  --root /absolute/path/to/worktrees \
  --workers 4 --format json
```

The scan includes hidden, ignored and nested directories, and reports access
failures, reached traversal limits and unresolved repository markers.
`--workers` controls parallel metadata reads only; the filesystem can still
change during the scan.

A multi-repository review needs an explicit target map bound to observed
repository identities. Follow [targets and filters](targets-and-filters.md) to
create it from an inventory. `review --root` accepts the same map, view filters,
grouping and context options as `plan`; it does not generate targets for you.

For saved output, use `--out` or `--run-dir` outside every selected root and
discovered repository family. The parent directory must exist and its ancestors
must be ordinary directories. On macOS, `/tmp` is commonly a symlink;
`/private/tmp` is used in the examples to avoid that ambiguity.

## Read a review

For the demo with an explicit target, inspect these rows:

| Path under the demo directory | Verdict | Reason |
| --- | --- | --- |
| `repo` | `keep` | `main_checkout` |
| `worktree` | `keep` | `local_untracked`, from the local note |
| `clean-worktree` | `cleanup_candidate` | All current C1-C5 conditions established |

Without a target, the clean row becomes `verify`; the two retention reasons
still apply. Missing forge data stays visible but does not prevent a candidate
whose Git and local proofs are complete.

Read `decision.keep_reasons` and `decision.missing_conditions` alongside the
facets: current controls, `local_summary`, commit-graph `history`, deep
`observations` and `private_history`. The report gives a next check for each
missing condition. Deep work is skipped when a retention reason already exists.

Pass full integration refs such as `refs/heads/main` or
`refs/remotes/origin/main`. The latter is known locally; the tool does no fetch.
A precisely associated open request yields `keep`, without proving a live process.
Subjects and request titles are descriptive and cannot establish integration.

On the `Local now` line, `0` is exact only for a complete comparison; `>=N` is a
lower bound and `UNKNOWN` means an incomplete zero. The old `scan --local`
manifests remain historical. Use a fresh scan for a new detailed manifest;
`plan` collects its own compact current summary.

Exit code `0` allows partial evidence. Always retain `coverage.gaps`,
`not_collected`, facet gaps and observation intervals when sharing a result.
No verdict authorizes removal. The [decision contract](contracts.md#triage-policy)
explains the evidence and conservation limits.

## Saved inputs and interrupted reviews

Use `scan --local` then `plan --inventory` when you need file hashes and metadata
or a separate inventory workflow. See the [advanced commands in French](tutorial.fr.md#parcours-avancé-scan-puis-plan).
`review` accepts neither `--local` nor `--inventory`. An archived
`wt-sweep.review/1` is a historical report, not a current inventory; use a fresh
scan. It cannot supply current C5 controls.

Progress on stderr records observed work. `Observed history:` and
`Observed local:` lines are provisional collected facts, with partial or
unknown counts preserved. They precede the final judgement and receipt; an
interruption leaves no final verdict established by those lines alone.
Progress is not a durable checkpoint. After an
interruption or budget stop, read `coverage`, discovery remainder, publication
omissions and facet gaps. The remaining cover is conservative and may overlap
visited paths. An explicit-root `scan --checkpoint-out` can save its traversal;
[resume it](discovery-resume.md) with `scan --resume` and a new checkpoint output.
Otherwise, relaunch one family or a narrower root into a new report base.
Review evidence is collected anew; observation intervals remain separate.

A failed publication or a partial directory without a final receipt does not
establish a saved run. If final synchronization fails after a receipt becomes
visible, durability remains unknown. A full original source over the publication
limit is refused for `review`; narrow the scope instead of discarding evidence.
See [CLI limits and exit codes](cli.md).

## Troubleshooting

| Symptom | Meaning and next step |
| --- | --- |
| Offline build cannot resolve a dependency | Run `cargo fetch --locked` with network access, then retry the offline build. |
| Native materialization guard fails | The required macOS policy was not established. Record the error; there is no bypass flag. |
| Output already exists | Select a new filename or use `--run-dir` for a separate run. |
| Output is inside the inspected scope | Use an existing report directory outside the roots, worktrees and Git administration. |
| Output ancestor is not a plain directory | Use the actual directory path without symlink ancestors, such as `/private/tmp` on macOS. |
| `verify` appears | Follow the missing C1-C5 conditions; check targets, current state and object availability. |
| `partial` receipt or coverage gaps | Inspect the JSON details. Narrow the scope if a collection budget was reached; other missing evidence may remain. |
| Split index or index v4 lacks classifications | Raw index evidence is retained where identifiable, but unsupported semantic interpretation stays unknown. |
| `apply` is unavailable | Expected behavior. Read the [decision](decisions/apply-readiness.md); no removal flag is implemented. |

For a bug report, provide the CLI version, OS version, command, exit code and
sanitized error or artifact excerpt. See [contribution guidance](../CONTRIBUTING.md#report-a-problem).
