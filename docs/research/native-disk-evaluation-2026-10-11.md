# Native disk evaluation, 11 October 2026

**Keep the product scalar collector.** The corrected native experiment passes
resident-fixture conservation and improves complete C walks by about 16%, 39%
and 58% across its three corpus sizes. It does not qualify a new Rust product
backend: the product residency, cancellation, storage and reporting controls are
outside this C probe, and real mount/firmlink/dataless cases remain `UNKNOWN`.
The first completed campaign exercised scalar metadata fallback for every file;
its timings are excluded from the corrected candidate's performance verdict.

Issue [#20](https://github.com/FlorianBruniaux/wt-sweep/issues/20) permits a stop
without native integration. This report evaluates that boundary against
source commit `671e77fc6e8d54f7f76ee99f94cc8eba9ad38315`, alpha.2. The experiment
uses the macOS SDK ABI rather than a product dependency.

## Replay and observed checks

The [experiment README](../../experiments/disk-native/README.md) gives both
commands. The fast check uses `run-tests.py --product PATH`; the timed runner
requires a new output directory and a quiet slot. Fixture roots are unique
temporary directories. No real repository, mounted-volume inventory, credentials,
global configuration, hook, permission setting or mount is changed.

The observed fast command ran against the frozen alpha.2 release binary:

```sh
/opt/homebrew/bin/python3 -B experiments/disk-native/run-tests.py \
  --product /private/tmp/wt-sweep-alpha2-3yv968ct/release-source/target/release/wt-sweep
```

It exited successfully: 11 Python cases passed, two platform cases were explicitly
skipped, and both native decoder/entry-handler assertion groups passed under
AddressSanitizer and UndefinedBehaviorSanitizer. This is the experiment's local
validation. Repository-wide checks, CI and a deployed binary are separate evidence.

| Source and case | Observed boundary |
| --- | --- |
| `test_decoder.c` | Valid nonempty identity/sizes and arbitrary name bytes; every record truncation; zero, misaligned and oversized lengths; negative/out-of-range attribute offsets; unterminated, embedded-NUL, slash and empty names; missing metadata; per-entry errors. |
| `test_decoder.c`, compact success record | Success without `ATTR_CMN_ERROR` is valid; optional-field omission shifts the following fields. This test failed before the decoder fix. |
| `test_entry.c`, error injection | The actual entry handler records a gap and does not count synthetic sizes on an entry error. It also resolves missing metadata through real descriptor-relative `fstatat`. |
| `test_entry.c`, directory hint injection | Wrong bulk device/inode and a synthetic firmlink flag do not override the real directory target metadata. This tests routing, not a live firmlink. |
| `test_walk.py`, complete oracle | Both collectors return nonempty complete counts matching a separate Python `lstat`/device/inode oracle. Native mode must use the bulk API and reduce scalar metadata calls. |
| `test_walk.py`, scope and special files | Nested registered roots, global and per-root hard links, ignored/hidden paths, FIFO, symlinks, sparse allocation and resource-fork allocation agree with the oracle. |
| `test_walk.py`, budgets and root refusal | Entry, depth and monotonic time caps leave incomplete coverage. A symlink root is refused with null bytes. |
| `test_walk.py`, external mutations | A second process replaces the root, replaces a directory with a symlink, or adds a late entry. The collector preserves its descriptor anchor and refuses a complete claim. |
| `HarnessDeadlineTests` | Silent, prematurely exiting and partial-marker children cannot wait indefinitely; termination and reaping occur in `finally`. |
| `test_product.py` | The exact Sweep CLI and both experimental collectors retain a missing registered synthetic worktree as incomplete, with null bytes and a nonempty gap. |

## Complete-walk protocol

Each corpus has six measured roots: main checkout, shared Git directory and four
linked worktrees. Corpus sizes are 32, 512 and 4096 extra files per linked worktree,
distributed across ignored, hidden and nested directories. Links across requested
roots exercise unique and per-root accounting. The runner also adds directory
and broken symlinks, a FIFO, sparse files, resource forks and Unicode/newline
names. APFS rejected non-UTF8 fixture creation with `EILSEQ`; the runner preserves
that `UNKNOWN` separately from decoder byte tests.

The product scalar CLI, C scalar collector, C native candidate and independent
oracle must match each requested root and the global unique total before timing.
Candidate API counters must show fewer scalar metadata calls. Content/identity
manifests must remain equal before and after collection. One warmup per mode
precedes five alternating measured trials. The monotonic interval covers the
complete C collection, excluding fixture creation, oracle reads and JSON encoding.
Caches are not cleared.

These counters count C API invocations, not total kernel syscalls. Product
`disk_usage.elapsed_ms` and full CLI wall time are saved as separate observations;
they are not the timing denominator for the C scalar/native ratio. The candidate
does not include product storage probes, report callbacks, signal cancellation
or the process/thread materialization guard in `src/residency.rs`. No native Rust
product binary was built, so this experiment cannot establish a product speedup.

## Rejected campaign and correction

Campaign 1 stopped during fixture creation, before collecting a timing sample,
because the owned APFS volume refused a non-UTF8 name. Campaign 2 completed its
oracle and conservation checks but made identical scalar metadata call counts
in both modes: 377, 2297 and 16633 across its three corpora.

An owned one-file SDK probe reported requested common bits `a204000b`, returned
bits `8204000b`, and missing bit `20000000`, `ATTR_CMN_ERROR`. Successful entries
omit that bit and its field. The decoder previously required it, so every success
became a missing-metadata scalar fallback. The corrected decoder permits its
absence, preserves explicit errors, and the runner now refuses timing when all
metadata falls back.

[rejected-campaign-2026-10-11.json](../../experiments/disk-native/rejected-campaign-2026-10-11.json)
retains the rejected samples and original source/binary hashes. Its raw result
has SHA-256 `9fc9c17daaeb1a96fd176d2762b971209b62b65674ce87594467707daf18f42c`.
Its binary has SHA-256 `fce05ce340bb5a0481f8d681489b972d5cee7c0c12ca156b1c6e5cf78191f3f3`.
The raw originals remain under
`/private/tmp/wt-sweep-evaluations-20261011/disk-native/campaign-2/`.
Those samples cannot guide adoption of the corrected candidate.

The pause-marker deadline and process-cleanup repairs also occurred after that
rejected campaign. Their own failing regression and final checks remain separate
from its frozen timing hashes. The subsequent corrected campaign uses the repaired
harness and a new frozen binary.

## Corrected campaign

The corrected campaign exited successfully after the integrator's full Rust suite
finished and other agent benchmarks had stopped. It froze the source and optimized
C binary before creating the corpora. The original JSON is retained under
`/private/tmp/wt-sweep-evaluations-20261011/disk-native/campaign-3/result.json`;
[result-2026-10-11.json](../../experiments/disk-native/result-2026-10-11.json)
retains every sample, API counter and frozen source hash with the decision added.

| Files per linked worktree | Checked entries, six roots | Scalar median, ms | Native median, ms | Native/scalar | Scalar metadata calls | Native metadata calls | Bulk calls |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 32 | 277 | 3.305 | 2.767 | 0.837 | 377 | 155 | 93 |
| 512 | 2197 | 17.812 | 10.818 | 0.607 | 2297 | 155 | 93 |
| 4096 | 16533 | 84.126 | 35.722 | 0.425 | 16633 | 155 | 105 |

Both modes open 50 directories in each corpus. Native mode removes regular-file
and symlink scalar reads while keeping directory/special-file reads and identity
rechecks. All six per-root totals and unique totals match the exact alpha.2
product and independent oracle. Before timing, both modes return identical object
identity/size sets; the separate fixture content/metadata manifests remain unchanged before
and after timing.

The actual scalar product records disk-phase elapsed times of 4, 18 and 157 ms;
its complete CLI wall times are 222.651, 441.710334 and 442.782208 ms. Those are
separate single observations, including product work absent from the C comparison.
They are not used to claim a native product speedup.

The environment is macOS 26.5.1, Darwin 25.5.0, arm64, SDK 26.5, Apple clang
21.0.0 (`clang-2100.1.1.101`) and Python 3.14.8. Caches were not cleared. Five
alternating samples on one host are a reproduced fixture benefit, not a universal
performance guarantee.

| Frozen object | SHA-256 |
| --- | --- |
| Alpha.2 scalar product | `30f4fb01361adf49216197410f5d3508fc317d724293267d0671bea8a16dd16b` |
| Corrected optimized probe | `4a5ee0b2b009cc27d72da209d5189ecb41a4b8cd76fc07e4f6f6c4238d69bf99` |
| Corrected decoder source | `bc6695b9a0b9ae95233d823c481458ea42ab56ba6434a257aadfc68f34ca42a7` |
| Complete-walk source | `f8a987ed69851de94d3a959baa710853e74677ae136bee2e48fa3928503c2d69` |
| Campaign runner | `0c70d0eaa4e8002017aec425d4538aaa0988338de7570bc30931bbf1eeb42b89` |

The raw directory also retains `walk`, `oracle.json`, `product.json`, product
stderr and scalar/native complete object records for each `corpus-32`,
`corpus-512` and `corpus-4096`. The rejected campaign's originals and one-file
attribute diagnostic remain beside it. Tracked files contain synthetic summary
data; complete temporary-path manifests stay in that local evidence directory.

## API and qualification limits

The [Apple getattrlistbulk manual](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man2/getattrlistbulk.2)
specifies descriptor-based iteration, link metadata, optional returned attributes
and per-entry errors. Mountpoints and firmlinks expose underlying-object metadata,
so the candidate re-reads every directory target through `fstatat` before descent.
The native SDK supplies record types, attribute bits and `SF_DATALESS`; the
decoder bounds every field and reference before access.

| Case | State and evidence limit |
| --- | --- |
| Resident APFS fixture traversal and accounting | Verified on the stated synthetic cases; observations span an interval. |
| Runtime non-UTF8 paths | `UNKNOWN` on this volume: owned fixture creation returned errno 92. Decoder preservation is separately verified. |
| Unix socket entry | `UNKNOWN` in this sandbox: owned `bind` returned `EPERM`; the check explicitly skips it. FIFO metadata is verified. |
| Real mount and firmlink crossings | `UNKNOWN`; creating mounts or firmlinks is outside the authorized experiment. Injected hints establish fallback routing only. |
| Real FileProvider/dataless directories and materialization | `UNKNOWN`; resident fixtures do not test placeholders or the product process/thread guard around the native API. |
| Actual TCC denial and live per-entry kernel errors | `UNKNOWN`; synthetic decoder/entry failures preserve gaps but do not establish those provider behaviors. |
| Other Darwin SDK/OS versions and filesystems | `UNKNOWN`; this campaign qualifies only its recorded macOS/arm64/SDK environment. |
| Recoverable or unique physical APFS storage | Not measured. Allocated bytes remain inode-based accounting. |

An eventual product backend must preserve those boundaries, retain the original
scalar fallback or refusal, and run inside the current residency/cancellation/
storage/reporting controls. A complete resident fixture does not qualify those
unobserved runtime cases or authorize cleanup.

This closes the experiment with a positive measured traversal result and a stop
at the product integration boundary. Reopening native integration requires a
Rust adapter that preserves the current controls, product-native behavior tests
on the same oracle corpus, and explicit qualification or documented scalar refusal
of the unobserved runtime cases. Product source, Cargo metadata and schemas remain
unchanged. This evaluation made no real scan, commit, push or cleanup.
