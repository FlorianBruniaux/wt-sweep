# APFS creation evaluation, 11 October 2026

Decision for [#30](https://github.com/FlorianBruniaux/wt-sweep/issues/30):
**retain an experimental recipe; do not adopt a Sweep creation command.**
Actual `clonefile` calls reduced dependency-transfer time on two synthetic
corpora. A standard Git checkout followed by a selected dependency copy already
satisfies the tested need; Worktrunk also passed that dependency-selection case. The clone variant offers a local optimization, but
this evidence does not justify broadening the read-only product or replacing
its Git state handling.

## Need reproduced before measurement

The fixture source contains committed tracked files, a staged source-only edit,
a required clean/smudge filter, ignored `node_modules` and a synthetic ignored
`.env`. A stock `git worktree add --detach ... HEAD` creates clean tracked state
and applies the filter, but does not reproduce `node_modules`. The concrete need
is a clean new worktree with selected existing dependencies, while retaining the
source edit and excluding unrelated ignored files.

The ordinary alternative is stock worktree creation plus a userspace byte copy
of the explicitly selected `node_modules` subtree. The candidate uses identical
Git creation, a directory `clonefile` into owned staging, then same-device
`rename` into the new worktree. It does not copy the source index. Both variants
preserve the same tested content and Git behavior.

The existing [claude-cow-worktree implementation](https://github.com/palmin/claude-cow-worktree/blob/68c5957bad4bb24c2d46378dd3323b80edcca60a/apfs-worktree-add.py),
pinned at clean HEAD `68c5957bad4bb24c2d46378dd3323b80edcca60a`, was inspected
and executed on one owned 8 MiB fixture. It returned zero, but dependencies were
absent afterward. The script clones the whole source through staging, copies its
index, resets to the requested target and runs `git clean -ffdx`. That final
step implements its clean-checkout goal and removes the dependencies needed
here. This experiment does not treat that behavior as an upstream defect.

## Existing Worktrunk alternative

Fresh source inspection at clean checkout
`e4a25d6e37cc7bfa6d133ba625359c58fd3cda08` found
[copy-ignored selection](https://github.com/max-sixty/worktrunk/blob/e4a25d6e37cc7bfa6d133ba625359c58fd3cda08/src/commands/step/copy_ignored.rs#L118)
through `.worktreeinclude` and
[per-file reflink/copy](https://github.com/max-sixty/worktrunk/blob/e4a25d6e37cc7bfa6d133ba625359c58fd3cda08/src/copy.rs#L188).
The source copy function tries a reflink, can fall back to ordinary copying, and
skips an existing destination without force.

A separately identified official **Worktrunk v0.80.0** binary, release source
`b49ca7eea9b03145791a5b94eccaf9c59412ed37`, then ran on an owned tiny fixture.
Its SHA-256 is recorded in the
[Worktrunk runtime result](../../experiments/apfs-create/results/2026-10-11-worktrunk.json).
This binary is not attributed to the different source checkout above.

The actual command was `wt step copy-ignored --from main --to destination
--require-include --format json`, with explicit fixture cwd and empty owned user
config. `.worktreeinclude` selected only `node_modules/`. The dependency bytes
arrived, `.env` stayed absent, target Git state stayed clean and the source
staged edit survived. A destination write left source bytes intact; repeating
the command preserved the changed destination. Worktrunk reported one reflinked
file and zero written files. That tool report is not a physical-space measurement.
No Worktrunk hook was installed. This successful existing alternative supports
keeping creation outside Sweep rather than duplicating a lifecycle feature.

## Replay and safety boundary

Use Python 3.11 or later on macOS with APFS:

```sh
python3 -B -m unittest discover -s experiments/apfs-create -p 'test_*.py' -v
python3 -B experiments/apfs-create/creation.py --samples 3 --files 32 --bytes-per-file 262144
python3 -B experiments/apfs-create/creation.py --samples 3 --files 128 --bytes-per-file 262144
python3 -B experiments/apfs-create/worktrunk_reference.py /absolute/path/to/pinned/wt
```

To repeat the optional upstream comparison, add
`--upstream /absolute/path/to/pinned/apfs-worktree-add.py`. That flag executes
the supplied script on the owned fixture, so inspect and pin it first. The raw
8 MiB result records its SHA-256.

The harness creates its source, staging and destinations inside one uniquely
owned `/private/tmp/wt-sweep-apfs-create-*` directory. It accepts no repository
or destination argument. Its Git environment disables global configuration,
hooks, signing and network protocols. The required filter is a synthetic local
script; no package manager runs. Cleanup removes only the owned temporary tree.
No global hooks or permissions change.

The two core behavior tests first failed against an empty experiment and a no-op
clone, then passed. They cover actual clone success through complete creation,
byte equality, source edit preservation, destination index cleanliness, filter
output, ignored-file selection, one-direction write isolation and refusal to
overwrite an existing destination. An additional optional Worktrunk behavior test first failed against a no-op
reference and passed with the pinned binary. Set `WT_APFS_WORKTRUNK` to that
binary path to run it in the unittest command; otherwise it skips. The core
creation tests skip on non-Darwin systems. On
Darwin, a non-APFS fixture is a refusal, not a silently copied substitute.

## Observed results

Host: macOS 26.5.1 arm64; Python 3.14.8 at `/opt/homebrew/bin/python3`.
Native `statfs` confirmed `apfs`, device **16777231**, filesystem ID
`[16777231, 26]`. Source, staging and destination devices were each recorded
as **16777231** in every sample. `diskutil` could not access DiskManagement in
this session; the final harness uses `statfs` directly and checks its return code.

Corpora contained 32 or 128 regular files of 262,144 non-sparse bytes each.
Source bytes are deterministic, nonuniform data. Three repetitions alternate
copy/clone then clone/copy then copy/clone. The source is warm from creation and
hashing; there is no cache purge or discarded warmup. Heavy repository checks
and the native collector benchmark were serialized outside these measurements;
small concurrent development activity was not excluded.

Timings cover Git worktree creation, selected dependency staging and publication.
Fixture setup, hashing, correctness checks and cleanup are outside the measured
interval. Neither method claims a durable backup. Raw observations:
[8 MiB](../../experiments/apfs-create/results/2026-10-11-8mib.json) and
[32 MiB](../../experiments/apfs-create/results/2026-10-11-32mib.json).

| Corpus | Method | Median Git creation | Median dependency stage | Median full creation | Individual full creation samples |
| --- | --- | ---: | ---: | ---: | --- |
| 8 MiB | Byte copy | 191.636 ms | 168.697 ms | 361.275 ms | 361.275, 247.885, 928.078 ms |
| 8 MiB | APFS clone | 261.411 ms | 4.782 ms | 281.235 ms | 281.235, 548.197, 178.334 ms |
| 32 MiB | Byte copy | 171.770 ms | 291.270 ms | 511.427 ms | 511.427, 745.597, 452.833 ms |
| 32 MiB | APFS clone | 210.954 ms | 23.283 ms | 235.418 ms | 475.475, 235.418, 215.935 ms |

All 12 creation samples produced identical dependency bytes before the write
test, distinct source/destination file inodes, a clean destination index,
committed target tracked content, correct smudge output and no unselected
`.env`. Changing a destination dependency left the source hashes unchanged.
Source staged entries, status and tracked bytes stayed unchanged. The upstream
run took 859.281 ms; this single observation is not a comparable performance
estimate for its different whole-source algorithm.

## Interpretation and limits

The clone stage was faster in these observations. Full creation shows substantial
variance and includes Git/filter process startup; three samples cannot support a
stable general speedup. No real dependency installer, cold cache, massive
small-file tree or different physical device was benchmarked. Cross-device
creation remains untested and unsupported by this recipe; rename has no copy
fallback. Capacity exhaustion, interrupted publication, concurrent source writes,
symlinks, hardlinks, ACL/xattr preservation, sparse checkouts, submodules and
version compatibility remain outside this fixture's qualification.

Allocated size and inode identity do not establish unique physical bytes,
reclaimable space or the cost of later copy-on-write divergence. Both physical
space fields stay `UNKNOWN`; no storage-savings claim is made. A clone also
provides no writer exclusion and does not satisfy E1 or backup durability.

Retain stock Git as the tested authority for HEAD/index/filter creation and the
explicit dependency selection as the boundary for copied ignored content. The
ordinary copy remains the portable alternative. Keep the APFS script under
`experiments/`, with no lifecycle hooks, automatic fallback or product command.
A future adoption proposal must begin with a real workload and its full creation
cost, then qualify failure cleanup and metadata semantics for that scope.
