# Native disk collector experiment

The experiment compares complete descriptor-anchored scalar and `getattrlistbulk`
walks on owned Git fixtures. It does not replace `src/disk.rs`.
The [evaluation report](../../docs/research/native-disk-evaluation-2026-10-11.md)
records the adoption decision, samples and unsupported cases.

## Fast correctness checks

From the repository root, on macOS with Python 3.11+ and its native SDK:

```sh
python3 -B experiments/disk-native/run-tests.py --product "$PWD/target/debug/wt-sweep"
```

The runner uses a unique temporary build directory, compiles against SDK
`sys/attr.h`, and enables AddressSanitizer and UndefinedBehaviorSanitizer.
It runs the decoder assertions, complete-walk oracle, partial/refusal checks,
external mutation witnesses and process deadline/cleanup tests. `--product`
also tests the actual CLI's missing registered root record. Without that option,
the product comparison is explicitly skipped.

Unix socket creation and real non-UTF8 names are optional platform cases with
explicit skips when the host refuses them. Ordinary complete traversal, FIFO,
symlink, sparse-file and resource-fork checks remain required. The decoder
must preserve arbitrary name bytes even when the local volume cannot create them.

## Serialized measurement

Reserve a quiet measurement slot before running:

```sh
python3 -B experiments/disk-native/benchmark.py \
  --product "$PWD/target/release/wt-sweep" \
  --out /private/tmp/wt-sweep-native-replay-new
```

The output directory must be new. The runner freezes one optimized C binary,
records source and binary hashes, and creates one main checkout, its shared Git
directory and four linked worktrees. Default sizes are 32, 512 and 4096 files per
linked worktree. Each corpus includes ignored/hidden/deep files, symlinks,
cross-root hard links, sparse files and resource forks.

Before timing, the actual Sweep CLI, both experimental walks and a separate
Python `lstat`/inode oracle must agree on every requested root and unique totals.
Native metadata calls must eliminate scalar calls; a candidate that falls back
for every file is refused before measurement. A separate content and metadata
manifest must remain equal before and after collection. Five alternating trials
follow one warmup per mode; creation, oracle and JSON encoding are outside the
collector's monotonic interval. Caches remain warm and are not cleared.

The C experiment omits product discovery, storage probes, signal cancellation
and report callbacks. Product scalar correctness and its observed elapsed time
are saved separately. The scalar/native C ratio measures those complete
experimental walks; it is not a measured Rust product acceleration.

## Decoder and fallback boundary

`decoder.c` reads fields with checked offsets and `memcpy`, using SDK types and
constants. It rejects truncated/alignment-invalid records, invalid lengths and
names, unknown layouts, negative sizes and invalid attribute references.
Missing usable metadata falls back to controlled `fstatat`; an entry error or
undecodable name leaves an explicit gap. Successful Darwin entries omit
`ATTR_CMN_ERROR`; that omission does not trigger a metadata fallback.

Native regular files and symlinks use returned identity and sizes. Every
directory and special-file entry uses descriptor-relative scalar metadata.
This keeps the target lookup needed for mounts and firmlinks. Directory descent
retains `openat(O_NOFOLLOW)`, pre-open and post-walk identity/change checks,
device boundaries, registered-root exclusions, dataless refusal, inode
deduplication and finite entry/depth/time limits.

The API behavior is specified by Apple's
[getattrlistbulk manual](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/man/man2/getattrlistbulk.2).
Real mounts, firmlinks, FileProvider placeholders, TCC denials and other Darwin
or filesystem versions remain outside the observed fixture coverage.

## Retained rejected run

[rejected-campaign-2026-10-11.json](rejected-campaign-2026-10-11.json)
preserves the first completed campaign, whose decoder incorrectly required the
optional success-error field. Its native mode made all scalar metadata calls,
so those timings describe bulk enumeration plus scalar fallback. They cannot
decide the performance of the corrected candidate. Its hashes and samples remain
distinct from the corrected campaign; the original binary is retained in the
local evidence directory linked by the evaluation report.
