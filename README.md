# Worktree Sweep

**English** · [Français](README.fr.md)

<table>
  <tr>
    <td width="64">
      <a href="https://www.florian.bruniaux.com/about/?utm_source=github&amp;utm_medium=readme&amp;utm_campaign=wt-sweep"><img src="https://cc.bruniaux.com/author.png" width="56" height="56" alt="Florian Bruniaux" /></a>
    </td>
    <td>
      <strong><a href="https://www.florian.bruniaux.com/about/?utm_source=github&amp;utm_medium=readme&amp;utm_campaign=wt-sweep">Florian BRUNIAUX</a></strong> &middot; AI Founding Engineer @ <a href="https://methode-aristote.fr/">Méthode Aristote</a><br />
      13 years from developer to CTO / VP Eng &middot; <a href="https://www.florian.bruniaux.com/blog/?utm_source=github&amp;utm_medium=readme&amp;utm_campaign=wt-sweep">Blog &#8599;</a> &middot; <a href="https://www.florian.bruniaux.com/projects/?utm_source=github&amp;utm_medium=readme&amp;utm_campaign=wt-sweep">Projects &#8599;</a>
    </td>
  </tr>
</table>

**A macOS CLI to inventory Git worktrees and compare their commits with refs you select.**

[![Read-only MVP checks](https://github.com/FlorianBruniaux/wt-sweep/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/FlorianBruniaux/wt-sweep/actions/workflows/ci.yml)
[![Rust toolchain: 1.95.0](https://img.shields.io/badge/Rust-1.95.0-dc7336?logo=rust)](rust-toolchain.toml)
[![Runtime: macOS](https://img.shields.io/badge/Runtime-macOS-597ee5)](docs/getting-started.md#prerequisites)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-597ee5)](LICENSE.md)

[Quick start](#quick-start) · [Features](#features) · [How it works](#how-it-works) · [Documentation](#documentation) · [Changelog](CHANGELOG.md) · [Contributing](CONTRIBUTING.md)

Worktree Sweep (`wt-sweep`) finds repositories and their worktrees within a scope
you choose. It checks local Git relationships and can include file manifests and
captured GitHub or GitLab data in its reports. The Rust CLI writes text and
versioned JSON; it runs locally without a service, forge account or AI model.

**Status: read-only MVP for macOS.** Discovery and review are implemented;
backup, restore and removal are unavailable. `apply` refuses every invocation.
The project name is provisional and no release has been published.

![Example: the worktree HEAD matches refs/heads/main and is classified integration_observed, while notes.txt remains untracked. The local scan records its hash and metadata without backing up its contents. Backup, restore and removal are unavailable.](docs/assets/git-and-local-work.svg)

Reproduce this case with the [disposable demo](docs/getting-started.md#disposable-demo).
The [visual sources and review notes](docs/assets/README.md) include the editable SVG.

## Why it exists

Parallel development leaves worktrees spread across clones, hidden directories
and temporary locations. A branch can look merged while its worktree still
contains untracked notes, ignored files or a different staged version.

The report lists each worktree's full HEAD, its relationships to the refs you
selected, associated PRs or MRs, and any risks or missing evidence. With
`scan --local`, it also records local files and index state. These records help
you inspect what remains in a worktree; they contain no backup of its files.

## Quick start

### Build from source

On macOS with Rust installed, clone the repository and build the CLI.
The repository is currently private, so cloning requires GitHub access:

```sh
git clone https://github.com/FlorianBruniaux/wt-sweep.git
cd wt-sweep
cargo fetch --locked
cargo build --release --locked --offline
./target/release/wt-sweep --help
```

<details>
<summary>Build prerequisites and offline dependencies</summary>

If you already have a checkout, run the build commands from its root.
The repository pins Rust 1.95.0 in [rust-toolchain.toml](rust-toolchain.toml).
Fetching dependencies needs network access; skip that step when they are already
cached. Git is read through `/usr/bin/git`. There is no package-manager install
or global configuration step. See [setup and troubleshooting](docs/getting-started.md).

</details>

### Inspect one repository family

Set `repo_path` to the clone you want to inspect. The report directory must be
outside the inspected repository, its worktrees and Git administration.

```sh
repo_path="/absolute/path/to/your/clone"
report_dir="$(mktemp -d /private/tmp/wt-sweep-report.XXXXXX)"

./target/release/wt-sweep scan --repo "$repo_path" --local \
  --out "$report_dir/inventory.json"

# Choose an existing full ref. Use refs/heads/master if that is your target.
./target/release/wt-sweep plan --inventory "$report_dir/inventory.json" \
  --target refs/heads/main --summary --run-dir "$report_dir"
```

`scan --local` includes tracked, untracked and ignored resident files within
explicit limits. `plan` prints a five-line summary and saves the full report,
JSON and publication receipt in a new run directory. Existing outputs are never
replaced. For a copy-and-run example, use the [disposable demo](docs/getting-started.md#disposable-demo).

### Read the result

The [disposable demo](docs/getting-started.md#disposable-demo) produces this
summary, including two worktrees whose HEAD matches the chosen target:

```text
Worktrees: 2
Active: 0; integration observed: 2; to triage: 0
Unknowns: 2 worktrees; coverage gaps: 0
```

The full report names each worktree, branch, HEAD and observed relationship.
See the [captured report](docs/getting-started.md#read-a-review) and
[annotated JSON excerpt](docs/contracts.md#example-review-item).

| Classification | What was observed |
| --- | --- |
| `active` | An open MR or PR was associated with the worktree's host, project, branch and HEAD. This takes priority over integration observations. |
| `integration_observed` | A verified commit or tree relationship reaches a ref you explicitly selected with `--target`. |
| `to_triage` | Neither condition was established. Inspect the observations and unknowns. |

**None of these classifications authorizes deletion.** A worktree can have an
integrated HEAD and still contain local changes. Missing forge evidence remains
unknown, including when Git integration was observed.

## Features

<details>
<summary>Capabilities and CLI entry points</summary>

| Capability | Use it for | Entry point |
| --- | --- | --- |
| Repository families | Discover the main checkout, linked and detached worktrees, and missing registrations. | `scan --repo PATH` |
| Scoped discovery | Walk selected roots, including hidden, ignored and nested directories, with coverage gaps retained. | `scan --root PATH` |
| Local file manifests | Inspect file hashes, metadata, index entries and selected worktree administration. | `scan --local` |
| Git relationships | Observe identical commits, ancestry and identical trees against named refs. | `plan --target FULL_REF` |
| Offline forge evidence | Import captured GitHub/GitLab data with provenance, without contacting a forge. | `forge ingerer` |
| Human and machine output | Read the detailed report or five-line summary; retain full path bytes and OIDs in JSON. | `--summary`, `--format json` |
| Separate runs | Keep concurrent reports in distinct directories with artifact hashes and publication timestamps. | `--run-dir BASE` |

Global discovery is also available through `scan --all`. It uses visible mounted
volumes and reports access gaps. It has been tested with injected volumes, not
by scanning the development machine. Start with a repository or selected roots.

</details>

## How it works

`scan` saves an inventory within the scope you choose. `plan` reads it, checks
Git relationships against explicit targets and produces a review:

```mermaid
flowchart LR
    Scan["scan<br/>Explicit scope"] --> Inventory["Inventory JSON<br/>Optional files and index"]
    Inventory --> Plan["plan<br/>Verify Git relationships"]
    Targets["Integration refs"] --> Plan
    Forge["Captured forge export"] -. optional .-> Plan
    Plan --> Report["Report and JSON<br/>Risks and unknowns"]
```

`plan` carries saved local manifests forward without rereading their contents.
An observed Git relationship therefore says nothing about changes to local
files since the scan.

<details>
<summary>How a worktree is classified</summary>

An open MR or PR takes priority only when its host, project, source branch and
full HEAD match the worktree. Otherwise, the review checks for a verified commit
or tree relationship to an explicit integration target.

```mermaid
flowchart TD
    Open{"Precisely associated<br/>open MR or PR?"}
    Open -->|Yes| Active["active"]
    Open -->|Not established| Git{"Verified Git relationship<br/>to an explicit integration target?"}
    Git -->|Yes| Integrated["integration_observed"]
    Git -->|Not established| Triage["to_triage"]
    Active --> Review["Retain risks and unknowns<br/>No deletion authorization"]
    Integrated --> Review
    Triage --> Review
```

The [classification table](#read-the-result) and [evidence contracts](docs/contracts.md#reviews-and-relationships)
describe how to read these results.

</details>

The collector limits how much it reads and how long operations can run. It
checks Git object bytes against their full OIDs before the review code
classifies the observations without further I/O. Age, patch-id, a closed MR
and a model suggestion cannot establish integration.

On macOS, the process disables and verifies dataless-file materialization before
protected reads, including its Git children. If that policy cannot be established,
the operation is refused. Access failures and timeouts remain visible. A completed
walk is still an interval of observations, not an atomic filesystem snapshot.

The [architecture](docs/architecture.md) and [evidence contracts](docs/contracts.md)
describe the read and publication boundaries.

## Documentation

| Start here | What it covers |
| --- | --- |
| [Hands-on tutorial (French)](docs/tutorial.fr.md) | Disposable trial, local tests, first real repository and a copyable assistant prompt |
| [Getting started](docs/getting-started.md) | Setup, disposable demo, report interpretation and troubleshooting |
| [CLI reference](docs/cli.md) | Commands, defaults, output modes, limits and exit codes |
| [Offline forge imports](docs/forge.md) | Exports, executor receipts and association limits |
| [Evidence and JSON](docs/contracts.md) | Schema fields, path bytes, digests and run receipts |
| [Architecture](docs/architecture.md) | Module responsibilities, read boundaries and test strategy |
| [Documentation index](docs/README.md) | All guides, design decisions and validation records |

This overview and the hands-on tutorial are available in French. Other detailed
user guides are in English. The original design, experiment and implementation
records are retained in French.

## Roadmap and validation

Discovery and review are implemented. Writer exclusion has not been
demonstrated, so backup, restore and removal remain unavailable.

<details>
<summary>Implementation stages and recorded validation</summary>

| Stage | Current state |
| --- | --- |
| U1–U4: discovery and review | Implemented and tested locally on macOS; evidence limits remain explicit. |
| E1: exclude concurrent writers | **Not demonstrated.** Existing experiments defeat the proposed locking and rename approaches. |
| U5: durable backup and independent restore | Blocked on E1 and a complete preservation contract. |
| U6–U7: removal | Unavailable, pending fixture validation, independent review and exact human approval. |

The [acceptance plan](docs/plan.md) defines the gates. The
[apply decision](docs/decisions/apply-readiness.md) records why they remain closed.

The implementation checkpoint on 2026-10-05 recorded 92 passing Rust tests and
78 passing Python tests. Skips, native experiment results and the tested binary
digest are in the [validation ledger](docs/finish-ledger.md#vérifications).
The CI badge links to remote workflow results, separate from these local checks.
Real FileProvider placeholders, actual TCC denials and a machine-wide snapshot
have not been verified.

</details>

## Development and project status

See [CONTRIBUTING.md](CONTRIBUTING.md) for the build, test commands, disposable
fixtures and documentation conventions. The [changelog](CHANGELOG.md) tracks
unreleased changes; [related tools](docs/references.md) remain candidates for
behavioral comparison before a removal engine is built.

The source checkout is the current distribution. Cargo publishing is disabled;
no package release has been published.

## License

[Functional Source License 1.1, Apache 2.0 future license](LICENSE.md)
(`FSL-1.1-ALv2`), copyright 2026 Florian Bruniaux. The same license is used by
Agent Router.

The FSL restricts competing commercial uses. Each version receives an additional
Apache 2.0 license two years after it is made available. Read the
[full terms](LICENSE.md) and the [FSL explanation](https://fsl.software/).
Third-party dependencies retain their own licenses.
