# Extension decisions, 11 October 2026

[Documentation](../README.md) · [Roadmap](../roadmap.md) · [Evaluation plan](../superpowers/plans/2026-10-11-remaining-evaluations.md)

The alpha.2 read-only audit remains the product boundary. These experiments
resolve the optional extension questions separately from release qualification.
A completed evaluation can stop an option without shipping it.

## Native disk collector (#20)

Keep the scalar product collector while retaining a promising native prototype.
The corrected full-walk experiment matches the independent oracle and alpha.2
totals on six roots. Median native/scalar duration ratios are 0.837, 0.607 and
0.425 at 32, 512 and 4,096 files per linked worktree. Counters confirm fewer
scalar metadata calls. These complete C walks lack the product residency,
cancellation, storage and reporting integration, and do not qualify real
mount/firmlink/dataless cases. The earlier all-fallback campaign is rejected.
Reopen product adoption with that adapter and boundary evidence; the measured
fixture benefit alone does not qualify a shipped Rust speedup.
[Decision and replay](native-disk-evaluation-2026-10-11.md).

## Worktrunk (#27)

Use existing `wt sweep` dispatch. The official Worktrunk `v0.80.0` binary passed
10 black-box cases, including literal arguments, status propagation, collisions
and real Sweep scan/review behavior. No adapter or configuration installer is
needed. [Decision and replay](worktrunk-integration-2026-10-11.md).

## Passive provenance (#28)

Keep a bounded context-only experiment. The existing card/index format can
show declared creators, sessions and dates, but a real same-path/branch/HEAD
recreation keeps the old card indistinguishable. Current identity and usage
remain unknown. No CLI field, registry or hook is added. Reopen when a report
consumer needs this declared context and accepts its explicit identity limits.
A verified-origin feature additionally needs a source binding to the instance.
[Decision and replay](provenance-evaluation-2026-10-11.md).

## Writer exclusion (#29)

Stop the current mechanism evaluation with E1 unproven. Owned child processes
can still write through mappings, new opens and aliases after rename under a
cooperative lock. Interrupting the cooperative guard releases its lock. This
is a counterexample campaign, not an implemented production recovery protocol.
`apply` remains unavailable. Restart only with a concrete independent authority
covering existing references and guard loss.
[Decision and replay](writer-exclusion-evaluation-2026-10-11.md).

## APFS creation (#30)

Keep the clone recipe experimental. On two owned APFS corpora, Git worktree
creation plus copying or cloning a selected ignored dependency directory
preserved the tested Git/index/filter behavior and isolated destination writes.
Measured times apply only to those corpora. No unique physical-space or
reclaimable-space result is established. Creation stays with lifecycle tools;
Sweep gains no creation command.
[Decision and replay](apfs-creation-evaluation-2026-10-11.md).

## Evidence boundaries

Experiments use synthetic fixtures and identify their binaries, sources and
measurement conditions. They do not establish machine-wide completeness,
current agent usage, durable backup, independent restoration or safe removal.
The published alpha.2 archive is unchanged. Any later product change needs its
own commit, CI and installation evidence.
