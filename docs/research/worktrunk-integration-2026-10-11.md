# Worktrunk integration evaluation, 11 October 2026

[Documentation](../README.md) · [Roadmap](../roadmap.md) · [Issue #27](https://github.com/FlorianBruniaux/wt-sweep/issues/27)

Decision: use Worktrunk's existing external-command dispatch. No adapter is
needed. Someone already navigating worktrees with `wt` can run the Sweep review
from the same command namespace while keeping the standalone CLI available.

## Qualified binaries

| Component | Observed identity |
| --- | --- |
| Worktrunk | Official `v0.80.0` Apple Silicon archive, source target `b49ca7eea9b03145791a5b94eccaf9c59412ed37` |
| Archive SHA-256 | `8a2bb053c4bc80dea7d9ce6c221ff038d10a3d2dca2dc8f60d1b1a094fa783a9` |
| Executable SHA-256 | `0708ca37fc39f9fa48edc1af500a2ff3664ec0155f63909425b02994f29f0fd1` |
| Sweep | Release build from `671e77fc6e8d54f7f76ee99f94cc8eba9ad38315`, `0.1.0-alpha.2` |

The archive matched both the publisher's checksum file and GitHub's asset digest.
The exact release source was read through GitHub's API, separately from the older
comparison checkout. The [dispatch source at the release commit](https://github.com/max-sixty/worktrunk/blob/b49ca7eea9b03145791a5b94eccaf9c59412ed37/src/commands/custom.rs)
resolves built-ins first, then configured aliases, then `wt-<name>` on PATH.
This source reading is separate from the binary tests below.

## Behavior tested

The [replayable suite](../../experiments/worktrunk/test_dispatch.py) exercises 10
cases on owned fixtures. All 10 passed, none skipped, on macOS 26.5.1 arm64.
The [captured result](../../experiments/worktrunk/result-2026-10-11.json) records
binary hashes and case names. The suite checks literal spaces, quotes, shell metacharacters,
empty and raw non-UTF-8 arguments; cwd inheritance and `-C`; separate stdout and
stderr; nonzero exit propagation; help/error parity; command-name collisions;
and real Sweep scan/review behavior. The review must retain untracked work and
identify the clean linked worktree as a candidate. A before/after manifest checks
fixture preservation. The expected result is not merely an exit code of zero.

Worktrunk's user-config alias named `sweep` takes precedence over the executable.
The first matching executable on PATH wins. Neither is a Sweep safety boundary.
An explicit Sweep path avoids both ambiguities. No global config or shell
integration was installed for these tests.

## Usage

With both binaries already installed and visible on PATH:

```sh
command -v wt
command -v wt-sweep
wt sweep --version
wt sweep review --help
```

From the chosen repository, after checking the intended target exists:

```sh
wt sweep review --repo "$PWD" --target refs/heads/main --disk-usage --summary --open
```

`refs/heads/main` is an example, not an inferred target. Use the repository's
accepted ref. The same review is available directly as `wt-sweep review ...`.
Worktrunk forwards the arguments; it does not strengthen Sweep's verdicts or
establish current usage. Removal remains unavailable in Sweep.

The qualification concerns Worktrunk `v0.80.0` on macOS Apple Silicon. Another
version, shell function or local alias needs its own check. The experiment does
not establish that arbitrary existing Worktrunk project configuration is inert.
