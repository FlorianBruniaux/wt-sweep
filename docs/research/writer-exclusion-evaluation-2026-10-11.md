# Writer exclusion evaluation, 11 October 2026

Decision for [#29](https://github.com/FlorianBruniaux/wt-sweep/issues/29):
**NON_DEMONTREE; apply remains BLOCKED.** The session reproduced existing
counterexamples and added mappings, new opens, aliases and interrupted guard
holders. No independently enforced storage authority is available in this
experiment. This completes the bounded evaluation with a stop result, without
satisfying E1 or enabling U5/U6.

## Scope and replay

Base product revision: `671e77fc6e8d54f7f76ee99f94cc8eba9ad38315`.
Observed host: macOS 26.5.1 arm64, effective UID 501; tests used Python 3.14.8.
All mutations affect owned `TemporaryDirectory` fixtures under `/private/tmp`.
No configuration, hooks, mount state, permissions or foreign processes changed.
Only child processes created by the experiment receive termination signals.

```sh
python3 -B -m unittest discover -s experiments/exclusion -p 'test_*.py' -v
python3 -B experiments/exclusion/experiment.py
python3 -B experiments/exclusion/extended.py
python3 -I -S -B experiments/exclusion/probe_native_capabilities.py
```

Use Python 3.11 or later. The local interpreter was
`/opt/homebrew/bin/python3`. The suite passed **9 tests**, including the existing
positive refusal control and five RETEX tests. The two writer tests first failed
against an empty witness result, then passed after real child writes were added.
A third regression test verifies that optimized Python is refused before any
fixture protocol begins, because these witness checks require active assertions.

Raw evidence: [original witnesses](../../experiments/exclusion/results/2026-10-11-original.json),
[extended witnesses](../../experiments/exclusion/results/2026-10-11-extended.json),
[native capability probe](../../experiments/exclusion/results/2026-10-11-native.json).

## Observed failures of candidate mechanisms

| Candidate or witness | Observed result | Evidence boundary |
| --- | --- | --- |
| Exclusive cooperative lock plus directory rename | A previously open FD wrote successfully from an outside cwd, despite refusal to acquire the held lock. | Existing real witness rerun. |
| Size and mtime comparison | Bytes changed while size and `st_mtime_ns` remained equal. | Does not claim unchanged ctime. |
| Captured detached HEAD | A child committed later; the new commit and blob were readable and the sentinel branch stayed unchanged. | An earlier HEAD capture does not include the later commit. |
| Shared writable mapping acquired before rename | Child changed mapped bytes while the parent held `flock`; parent read the changed bytes from the same inode. | Real `mmap`, flushed and closed before verification. |
| New open after rename | Child opened the renamed path after GO and wrote successfully. | The new pathname remained accessible. |
| Hard-link alias outside the renamed tree | Child opened the existing hard link after GO and changed the same inode. | Alias is still inside the owned fixture. |
| Symlink alias outside the renamed tree | Child followed a link to the new path after GO and wrote successfully. | The symlink targets the renamed fixture only. |
| Native write lease | `F_SETLEASE` returned `EPERM`; readback was `F_UNLCK`. | No usable lease acquired by this process. |
| Revoke regular file | `ENOTSUP`; six bytes were subsequently written through the existing FD. | This call did not revoke that FD. |

The extended harness uses READY/GO/DONE handshakes. Deadlines bound waits;
sleeps do not order competing writes. The identities and byte comparisons are
read from the filesystem rather than inferred from a success flag.

## Guard loss and interruption

A separate owned child acquires a real `flock` on a guard file. Four runs stop
it after acquisition, diagnostic capture, diagnostic receipt publication and
immediately before the hypothetical retirement transition. Before interruption,
a second lock acquisition fails. After terminating and reaping the holder, that
acquisition succeeds and source bytes can change. The source remains present
in every case.

These checkpoints exercise a **cooperative candidate protocol**. The copy and
receipt are deliberately diagnostic, not a durable backup; no removal function
exists in this harness. The tests demonstrate guard loss, not fail-safe
production recovery. There is no guard-provider adapter whose OS authority could
be tested through destructive transitions. Inventing a boolean authority token
would hide the missing prerequisite.

An eligible future provider must acquire independent authority before capture,
cover all existing mappings/FDs and new opens, and retain it across data, Git
administration, objects, publication, retirement and postchecks. Every transition
must refuse after loss, identity drift, timeout or interruption. A supervisor
must prevent retirement if the holder dies; simply releasing a cooperative mutex
is insufficient. That provider must be evaluated before a positive retirement
and independent-restore test can count toward U5/U6.

## Pinned external implementations

The following local checkouts were clean at their recorded HEADs. This section
is a fresh source inspection, not a rerun of either cleanup engine.

| Source | What the inspected code establishes | Relation to E1 |
| --- | --- | --- |
| [Holt ownership.mjs](https://github.com/Raed2180416/holt/blob/b99e098e214f4e84388ffa9d6fa6b675f951732a/src/ownership.mjs#L232), `b99e098e214f4e84388ffa9d6fa6b675f951732a` | `BEGIN IMMEDIATE` serializes participating claim and cleanup operations; closing the process releases its SQLite mutex. A separate session lease persists. | This coordinates Holt participants. It does not revoke arbitrary file descriptors or mappings. |
| [Holt actions.mjs](https://github.com/Raed2180416/holt/blob/b99e098e214f4e84388ffa9d6fa6b675f951732a/src/actions.mjs#L2927) | Reassesses content and identity, then keeps the ownership mutex through quarantine. Its comment explicitly limits participation to sessions that claim ownership. | Useful lifecycle coordination, without evidence of continuous exclusion for unregistered writers. |
| [AgentRinse lock.ts](https://github.com/vincentkoc/agentrinse/blob/6d5e047e00ef0193f6b75b952060a80ea310b1a4/src/state/lock.ts#L337), `6d5e047e00ef0193f6b75b952060a80ea310b1a4` | Exclusively creates `apply.lock`, stores owner identity and checks token/device/inode on release. | Serializes its own apply operations. |
| [AgentRinse worktree-executor.ts](https://github.com/vincentkoc/agentrinse/blob/6d5e047e00ef0193f6b75b952060a80ea310b1a4/src/core/worktree-executor.ts#L635) and [safety contract](https://github.com/vincentkoc/agentrinse/blob/6d5e047e00ef0193f6b75b952060a80ea310b1a4/docs/safety.md#L192) | Revalidates filesystem, Git state, mounts and process ownership; claims a Git worktree lock, quarantines with recovery metadata, then purges separately. Its contract excludes hostile same-user processes. | Repeated observations and Git registration locks do not establish Sweep's stronger E1 requirement. |

This is a contract comparison, not a claim that either project violates its
stated guarantees. Neither inspected implementation supplies the missing
independent authority for this evaluation.

## Stop condition

Keep [apply-readiness](../decisions/apply-readiness.md) blocked. No additional
repository writer, backup engine, removal command or privilege request follows
from these results. Restart E1 only with a concrete storage-authority provider
and evidence that it covers previously opened references and guard loss. Durable
backup, full metadata conservation and independent restoration remain unproven.
