# Collection benchmark, 2026-10-07

The updated release displays a **known complete `identical_commit` relation** on stderr in 1.267 to 3.984 seconds across four comparable thorough pairs. Receipt times are recomputable from timestamped chunks and the exact visible byte ranges. Baseline displays no qualifying stderr fact. These are provisional observations, not final cleanup verdicts.

**Total runtime acceleration is not established.** After is slower in two of the four comparable samples. Each case has one sample, fixed baseline-first order and uncontrolled OS caches. Two new 50-standard performance pairs were refused because local proofs differed. The 50-thorough complement was selected after those refusals, with independent review and explicit authorization; its results do not establish comparability or completeness for the default standard profile.

[Compact evidence JSON](../../tests/data/performance/review-benchmark-2026-10-07.json) preserves all 24 release plan attempts: 12 historical, eight initial strict attempts and four separately approved complement attempts. Three performance pairs were refused: historical 100 standard, and the two new 50 standard pairs. Historical first-strict-fact times remain **UNKNOWN**. Only disposable synthetic real Git families were measured.

## Eight comparable strict samples

All times are seconds, rounded computationally to three decimals. **Total is duration of the `plan` command only**. Fixture creation, preparatory `scan`, binary/source checks and manifests are outside that timer. This does not measure the complete review, discovery, disk reporting and HTML workflow. stdout remains a pipe; mode describes stderr.

| Worktrees | Profile | Mode | Binary | First strict stdout fact | First strict stderr fact | Total plan |
| ---: | --- | --- | --- | ---: | ---: | ---: |
| 50 | thorough | pipe | baseline | 50.808 | None | 50.819 |
| 50 | thorough | pipe | after | 60.987 | 1.977 | 60.993 |
| 50 | thorough | PTY | baseline | 70.517 | None | 70.520 |
| 50 | thorough | PTY | after | 65.508 | 1.267 | 65.512 |
| 100 | thorough | pipe | baseline | 82.124 | None | 82.135 |
| 100 | thorough | pipe | after | 123.180 | 3.984 | 123.198 |
| 100 | thorough | PTY | baseline | 123.634 | None | 123.640 |
| 100 | thorough | PTY | after | 95.848 | 3.449 | 95.853 |

Every pair has identical normalized item proofs, shared stashes, relevant discovery coverage, applied limits, source manifests, graph results and classifications. Normalization removes observation clocks, their derived digest and judgement fields; it retains identities, source hashes, completeness, positive facts and refusal causes. Equal classifications alone cannot certify comparability.

| Both modes/binaries | Complete local summaries | Complete private enumeration/anchors | Final classification | Measured Git processes |
| --- | --- | --- | --- | ---: |
| 50 thorough | 50/50 | 50/50 each | 1 keep, 49 cleanup_candidate | 1912 each |
| 100 thorough | 100/100 | 100/100 each | 1 keep, 99 cleanup_candidate | 3212 each |

Each thorough run visits the 600-commit target graph completely: one not-ancestor main checkout, one identical commit, and 48/98 ancestors. Git counts cover exposed collection phases; unmeasured discovery/control calls are excluded. The same exact owned family was conserved across the four samples in each campaign. Separate campaigns instantiate the same fixture recipe; no cross-profile performance comparison is made.

## New 50-standard refusals

The initial strict campaign ran 50 standard and 100 thorough, before the separately authorized 50-thorough complement. Both 50-standard baseline samples had 50 complete local summaries; after had 49. Private enumeration/anchors stayed complete for 50 in each. Baseline classified 1 keep and 49 cleanup candidates; after classified 1 keep, 48 candidates and 1 verify.

| Mode | Baseline total | After total | After strict stderr fact | Local refusal row |
| --- | ---: | ---: | ---: | --- |
| pipe | 65.704 | 85.258 | 3.926 | `simple-013` |
| PTY | 128.296 | 92.606 | 3.252 | `simple-002` |

Both rows report `HEAD tree unavailable: summary time budget exhausted during Git read; dependent facet reads refused`, followed by stopped facet/administration revalidation. Missing conditions are C2, C3 and C5. Source inspection identifies the per-row fair share, `min(worktree cap, remaining phase time / pending worktrees)`, rather than whole-phase exhaustion; precise row allowances were not persisted. Full paths, HEADs, gaps and captures are retained. These first facts are observable, but their timings cannot certify performance on equal final proofs. Neither the product's standard caps nor its defaults changed, and these cases were not retried until success.

## Historical evidence remains preserved

The original twelve attempts used the earlier after release. Four clean pairs, 50 standard and 100 thorough in pipe/PTY, had equal normalized proofs. Their 3.405 to 5.960-second stderr matches were **provisional statuses**, sometimes unknown/partial, and stdout could match a bare inventory HEAD. The original streams lack per-chunk timestamps, so the first strict visible fact cannot be retrospectively dated. Original timings, profiles, hashes, coverage and raw proof references remain in the JSON's historical groups.

The historical 100-standard pair remains refused: local completeness changed 100 to 97, private enumeration 77 to 17, with complete anchors for 100 each. After rows `simple-010`, `simple-011` and `simple-047` have explicit local Git deadlines; separate private final-validation gaps make C4/C5 incomplete while preserving observed covered roots. Baseline classified 1 keep, 76 candidates and 23 verify; after classified 1 keep, 14 candidates and 85 verify. Baseline total plan was 154.559 seconds. After external timing is UNKNOWN because its record was lost on the original paired assertion; phase metrics are not used to invent it. Subsequent protocols persist each validated sample before comparison and preserve refusal.

The historical ignored probe remains separate: 50 standard worktrees, eight ignored files of 65536 bytes each, 400 observed paths and 25 MiB payload. Both collect complete local/private proofs, retain all 50 rows, and conserve sources with equal normalized evidence. Local phases were 41.666/36.373 seconds, with 500 local Git processes, 150 object reads and 450 paths in each. Total plans were 141.280/110.788 seconds. These durations cannot be compared with clean-corpus totals: retained rows skip deep candidate verification. No marginal payload-throughput claim is established. Historical first-strict-fact time remains UNKNOWN here too.

## Strict capture and operation evidence

stderr qualifies only a whole displayed known relation (`identical_commit`, `ancestor`, `not_ancestor`) with `provisional, complete`, or an actual numeric local count with its fixture identity visible. Unknown/partial statuses, all-unknown counts, unidentified prefixes and inventory HEADs alone never qualify. stdout requires a complete history proof object with recognized source/target/ref and no gap, or an identified actual numeric local observation. Different redraw frames cannot be joined into a fact. Initial provisional status timing is recorded separately.

Every receipt records stream, contiguous byte offset, length and elapsed nanoseconds from `time.monotonic_ns`. Hash-bound stdout/stderr files and `.capture.json` allow exact offline recomputation of first output, first status, first strict fact and its byte range. All twelve new samples replay exactly. Capture is bounded to 32 MiB output, 16384 chunks, 2 MiB metadata and 720 seconds per plan. Truncated/changed captures, discontinuous offsets and invalid times refuse. An owned process group is terminated and its leader reaped even if descendants outlive the leader.

PTY width is 200 columns and the formatter caps its live line at 68 characters. Known complete relationships fit within the visible status; clipped local identity does not qualify. `--color auto`, controlled TERM and absent NO_COLOR are used. Live ANSI redraws are verified in PTY, absent in pipes, and stdout remains plain JSON. Exact nonempty fixture paths, full HEADs and target ref/OID are checked independently against the DAG. Recursive source bytes/metadata and an outside sentinel are checked before/after, with atime excluded. Temporal consistency remains `not_guaranteed`.

The unchanged-facts serialization regression proves four calls before versus two after with identical positive proof JSON, and verifies tiny-cap refusal. Actual after counters are 2601 private serializations plus 51 during revalidation at 50 worktrees; 10201 plus 101 at 100. Baseline large-corpus counter is UNKNOWN because that release does not expose it. This operation reduction does not establish a causal runtime gain.

## Provenance and checks

| Artifact | SHA-256 |
| --- | --- |
| Baseline release | `0514002dcfb002d99f95482a43446a6a9754f25f4652083906578ca38bf5d445` |
| Strict after release | `e8a87860773e566b12f8696812591981101380d225f85de0944b4a87bd35f4e3` |
| Historical after release | `a622edf51eb32ef8fe58991703f9b404bbc67d1526bddaec9f4950206d6f26bf` |
| Strict measurement protocol | `5d077405f1b0de772f0e1ba16af6b6ca867f1ed88238eb73668768ca74f3a76c` |

Baseline is an inert archive of HEAD `14f00ea454795fd6df1f86dec72ec6573d3e546e` plus 29 verified initial dirty preimages. Strict after's 35-source capsule includes compile-time CSS/JS. Executable hashes are unchanged before/after every valid sample. Native campaigns `strict-50standard-comparison.json`, `strict-100thorough-comparison.json`, `strict-50thorough-comparison.json`, raw JSON/stderr/captures, historical campaigns and source capsules are retained under `/private/tmp/wt-sweep-task11-evidence-straj5lz`. Their SHA-256 references are in the compact artifact. Native temporary evidence needs separate retention if that directory is removed; tracked summaries do not contain full raw reports.

Environment: macOS 26.5.1 arm64, Apple M5 Max, 18 logical CPUs, 128 GiB RAM, Python 3.14.8 and Apple Git 2.50.1. Runs were sequential in reserved quiet windows; OS caches were neither cleared nor controlled. Profile selection after refusal is explicit. No further collection followed the four-plan complement.

RED/GREEN: the old matcher accepted unknown/partial statuses and isolated HEADs; the corrected predicate and timestamped replay pass. A real Git forced-slice regression exposed the stale `history job not examined` label after reads; it now reports comparison in progress, with final ancestor proof unchanged. Six focused pure/owned-helper checks passed; independent reviewer passed both matcher/replay tests. Before collection, the integrator reported all seven protocol tests including real two-worktree pipe/PTY passed, terminal 8/8, progress/gate tests passed, formatting and all-target Clippy passed. Remote CI and user-repository behavior are separate evidence.

Skills used: flow-lean, tdd, superpowers:systematic-debugging, superpowers:test-driven-development, superpowers:receiving-code-review.
