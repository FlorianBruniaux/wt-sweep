Collection observes files and Git objects. Deterministic judgement reads those facts separately. Missing, incomplete and contradictory evidence remains visible.

## Decision conditions

`keep` takes priority when retention evidence exists. `cleanup_candidate` requires all current C1–C5 conditions. Otherwise the row remains `verify`.

Read `keep_reasons`, `missing_conditions`, current controls, local summary, history, deep observations and private history together. Main checkouts, local work and precisely associated open requests have explicit retention reasons. Age, patch-id, a closed request or a model suggestion never proves integration.

[The canonical triage contract](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/contracts.md#triage-policy) defines C1–C5. The portal does not introduce an alternative rule.

## Coverage is not a snapshot

A completed discovery walk is not a machine-wide or atomic snapshot. Hidden and ignored directories are in scope; access failures, traversal limits and uncollected evidence remain reported.

`0` is exact only for a complete local comparison. `>=N` is a lower bound. An incomplete zero stays `UNKNOWN`.

## Receipts and preservation

`run.json` binds bytes, sizes and SHA-256 of saved artifacts. It does not prove a restorable backup, independent restore or the last successful disk synchronization.

Writer exclusion, durable backup and independent restoration gates have not passed. `apply` remains unavailable. No verdict authorizes removal.

## Platform limits

The CLI establishes a native macOS dataless materialization guard. Actual FileProvider placeholders and TCC refusals remain unverified. Kernel-blocked operations may exceed userspace deadlines. [Disk sizes](/docs/disk/) never promise recoverable APFS space.

## Forge context

Repository, branch and commit URLs are built from captured local metadata. They are not checked against the server. Only exactly associated imported PR/MR observations are shown as requests; remote links do not alter triage. [Report links and provenance](/docs/report/#github-and-gitlab-links).

## Storage format compatibility

Source builds after `v0.1.0-alpha.1` can record the optional `disk_usage.storage` block. New readers accept older inventories without changing their canonical digest. Older strict readers can reject enriched files. Unsupported or inconsistent volume data leaves percentages unavailable while retaining recorded sizes and rows. Capacity never changes C1–C5.
