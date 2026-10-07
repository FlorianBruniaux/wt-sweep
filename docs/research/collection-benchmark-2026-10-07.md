# Collection benchmark, 2026-10-07

The release comparison confirms earlier observable progress with identical final proofs in four clean pairs. The updated collector emits a complete provisional history status on stderr in 3.405–5.960 seconds; baseline emits no qualifying stderr fact. These statuses may say `unknown` and `partial`. They are neither final verdicts nor complete identified worktree rows.

Total runtime improvement is **not established**. This is one sample per case, in a fixed baseline-first order, with OS caches neither cleared nor controlled. A separate 100-worktree standard comparison was refused because its collected proofs differed. The unchanged existing thorough profile was then used for one bounded 100-worktree campaign, with no further retries.

[Compact evidence JSON](../../tests/data/performance/review-benchmark-2026-10-07.json) contains exact timings, profiles, counters, coverage, hashes, refusals and native proof paths. All sources are synthetic disposable real Git fixtures. No real repository was scanned or changed by this benchmark.

## Eight clean release samples

Every time below is in seconds. **Total means duration of the `plan` command only**. Preparation by `scan`, fixture creation, hash checks and source manifests are outside the timer. This is not a measurement of review plus discovery, disk usage and HTML generation. stdout remains a pipe in both modes; mode describes stderr.

| Worktrees | Profile | Mode | Binary | First useful stdout | First complete provisional stderr status | Total plan |
| ---: | --- | --- | --- | ---: | ---: | ---: |
| 50 | standard | pipe | baseline | 101.982 | None | 102.122 |
| 50 | standard | pipe | after | 98.828 | 3.405 | 98.831 |
| 50 | standard | PTY | baseline | 100.410 | None | 100.431 |
| 50 | standard | PTY | after | 100.145 | 3.476 | 100.160 |
| 100 | thorough | pipe | baseline | 158.789 | None | 158.829 |
| 100 | thorough | pipe | after | 147.407 | 5.630 | 147.413 |
| 100 | thorough | PTY | baseline | 178.395 | None | 178.409 |
| 100 | thorough | PTY | after | 172.316 | 5.960 | 172.335 |

Each before/after pair has zero differences in normalized item proofs, shared stashes and relevant discovery coverage, with identical applied limits, graph results, source manifests and classifications. Normalization removes observation clocks, their derived digest and judgement fields; it keeps identities, source hashes, completeness, facts and refusal causes. Equal classifications alone do not certify comparability. Saved native reports were also rechecked with the final strict comparison function, without rerunning collection.

| Campaign, both modes and binaries | Complete local summaries | Complete private enumeration/anchors | Final classification | Measured Git processes |
| --- | --- | --- | --- | ---: |
| 50 standard | 50/50 | 50/50 for each | 1 keep, 49 cleanup_candidate | 1824 each |
| 100 thorough | 100/100 | 100/100 for each | 1 keep, 99 cleanup_candidate | 3212 each |

The 50-worktree graph results are 48 ancestors, one identical commit and one explicit unknown at the unchanged 512-step cap. Thorough results are 98 ancestors, one identical commit and one not-ancestor, with all 600 target commits visited. The main checkout is retained in both campaigns. Do not compare 50 standard against 100 thorough as a performance pair. Git counts cover exposed collection phases; unmeasured discovery/control calls are excluded.

## Refused 100-worktree standard passage

Two `plan` attempts produced different proofs and one rejected performance pair. Baseline had 100 complete local summaries and 77 complete private enumerations; after had 97 and 17, respectively. Anchors were complete for 100 rows in each. Baseline classified 1 keep, 76 cleanup_candidate and 23 verify; after classified 1 keep, 14 cleanup_candidate and 85 verify. No 100 standard gain or stability fix is claimed.

After rows `simple-010`, `simple-011` and `simple-047` carry the explicit cause `summary time budget exhausted during Git read; dependent facet reads refused`, followed where applicable by stopped facet/administration revalidation gaps. They add missing C2/C3 evidence. Local collection used 55.507 seconds against a 120-second phase cap: source inspection identifies the per-row fair share, `min(worktree cap, remaining phase time / pending worktrees)`, rather than whole-phase exhaustion. Exact assigned row allowances were not persisted. Private final-validation gaps separately make C4/C5 incomplete while preserving observed covered roots. The deadline sensitivity remains open.

The original script saved the after raw JSON/stderr before paired comparison but lost its external timing record when the strict oracle raised. Its total plan and first-fact timings are **UNKNOWN**, and are not reconstructed from phase metrics. Baseline total plan was 154.559 seconds. The subsequent script persists every independently validated sample before comparison and records a refusal without weakening the oracle or product budgets.

Native rejected proof: `/private/tmp/wt-sweep-task11-evidence-straj5lz/rejected-standard-summary.json`, plus `release-comparison-{baseline,after}-100-pipe-forward-0.json` and their `.stderr` files in the same directory.

## Bounded ignored-file probe

One separate before/after pipe pair used 50 standard worktrees, eight ignored files of 65536 bytes each: 400 files, 25 MiB of synthetic payload. Both observed all 400 ignored paths, retained all 50 worktrees, collected 50 complete local summaries and private enumerations/anchors, and conserved source bytes/metadata. Their normalized proofs, stashes, relevant coverage and applied limits matched exactly.

| Local measurement | Baseline | After |
| --- | ---: | ---: |
| Local phase seconds | 41.666 | 36.373 |
| Local Git processes | 500 | 500 |
| Local object reads | 150 | 150 |
| Local paths checked | 450 | 450 |
| All exposed collection Git processes | 1672 | 1672 |
| First useful stdout seconds | 141.267 | 110.784 |
| First provisional stderr status seconds | None | 3.602 |
| Total plan seconds | 141.280 | 110.788 |

These single durations are descriptive. No isolated marginal cost or payload throughput is established. Do not compare total plan duration against the clean corpus: positive ignored evidence retains every row and skips deep candidate verification. Native campaign: `/private/tmp/wt-sweep-task11-evidence-straj5lz/ignored-comparison.json`; exact per-binary reports/stderr and SHA-256 hashes are listed in the compact JSON.

## Serialization and protocol

The implementation reuses a bounded serialized size only while retained private facts stay unchanged, then reserializes after any omission/truncation. The real positive fixture regression observed four calls before and two after with identical proof JSON; its tiny-cap refusal also passes. After release counters record 2601 private output serializations plus 51 during revalidation at 50 worktrees, and 10201 plus 101 at 100. Baseline does not expose this counter, so its large-corpus count is UNKNOWN. The reduced operation count is proven by the controlled fixture; a causal runtime acceleration is not demonstrated by these samples.

The corpus has 600 commits and distinct full HEADs. Rows must exactly match fixture paths, HEADs and target ref/OID; an empty report cannot be successful. Pipe capture must have no ANSI, PTY capture must have live ANSI redraws, and stdout must remain plain JSON. PTY width is 200 columns; the live formatter caps its line at 68 characters. Useful detection waits for a complete status/count token or full fixture HEAD, not a heading or split prefix. Capture timings use `time.monotonic_ns`. Each owned process group has a 720-second cap and 32 MiB capture cap, and cleanup includes descendants after an exited leader. Independent recursive conservation excludes atime and includes administration/index metadata and an outside sentinel. Temporal consistency remains `not_guaranteed`.

Environment: macOS 26.5.1 arm64, Apple M5 Max, 18 logical CPUs, 128 GiB RAM, Python 3.14.8, Apple Git 2.50.1. Runs were sequential in a reserved quiet window. Twelve release plan attempts comprise eight clean samples, two refused standard attempts and two ignored-probe samples. No additional run followed the ignored pair.

| Artifact | SHA-256 |
| --- | --- |
| Baseline release | `0514002dcfb002d99f95482a43446a6a9754f25f4652083906578ca38bf5d445` |
| After release | `a622edf51eb32ef8fe58991703f9b404bbc67d1526bddaec9f4950206d6f26bf` |
| Original 50 standard / rejected 100 standard protocol | `a86499dc22692bf0f175c29fddf7876314ec401fec3469feed16eb67eedeff02` |
| Final 100 thorough / ignored protocol | `5fa45916004459f483b4b3726da921401e43a58e1c4e6567992bc88410c1e53c` |

Baseline is an inert archive of HEAD `14f00ea454795fd6df1f86dec72ec6573d3e546e` plus all 29 verified initial dirty preimages. After source hashes include compile-time CSS/JS. Executables were hashed before and after every valid sample. Source capsules, the protocol diff, native campaigns `release-comparison.json`, `release-thorough-comparison.json`, and raw reports are retained under `/private/tmp/wt-sweep-task11-evidence-straj5lz`; their audit hashes appear in the compact JSON. These native temporary proofs must be retained separately if required after that directory is removed. The checked-in compact artifact preserves summaries and hashes, not full raw evidence.

Six focused protocol tests passed, including real two-worktree pipe/PTY, exact fixture rows, split chunks, profile/cap checks, strict comparability, persistence after refusal and exited-leader descendant cleanup. Independent source review approved the frozen final protocol before the bounded campaign. The integrator separately reported fmt/Clippy success, Rust 361 passed with 3 ignored, and Python 291 cases: 285 passed and 6 skipped before the final protocol additions. Remote CI and real-repository behavior are outside this evidence.

Skills used: flow-lean, tdd, superpowers:systematic-debugging, superpowers:test-driven-development, superpowers:receiving-code-review.
