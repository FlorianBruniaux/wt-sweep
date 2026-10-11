# Passive provenance evaluation, 11 October 2026

The existing Aristote registry supplies useful declared creator/session context.
The bounded experimental reader preserves that context, its dates and its
unknowns. Stop at this experiment: no provenance field or reader is added to
the shipped Sweep CLI. Automatic association would add a source-specific schema
without establishing the origin of the current worktree instance.

This records the evaluation of [issue #28](https://github.com/FlorianBruniaux/wt-sweep/issues/28).
It does not qualify session capture, active usage, writer exclusion or removal.
The experiment belongs to [the remaining evaluations plan](../superpowers/plans/2026-10-11-remaining-evaluations.md#task-3-passive-provenance-28).

## Source and reproduced need

The user need is to recover who declared a checkout's creation and which
sessions later attached to it, while keeping creation, registration and
intervention dates separate. The synthetic lookup starts without a card, then
adds a card declaring a Codex creator and a later Claude session. The reader
returns these two roles, their evidence labels and dates instead of inferring
an origin from a branch name. A missing creation date stays null.

The existing source is `methode-aristote/app`, inspected locally on 2026-10-11
at HEAD `84120a7980e30341f6ccd52a0980d621cba8b428`. The four inspected paths were
clean at inspection. Their content hashes are frozen in
[source-reference.json](../../experiments/provenance/source-reference.json).
Only implementation and documentation were read; no actual `.worktree.json`,
registry generation, personal session payload or credential was ingested.

| Existing source file | Verified behavior used by this experiment |
| --- | --- |
| `scripts/worktrees/card-schema.mjs` | Schema-v1 required fields, unknown-field refusal, distinct timestamps, session relations and evidence labels. |
| `scripts/worktrees/registry.mjs` | `validateCard`; creation uses a random card id, declared canonical path/common directory and optional initial branch/HEAD. Existing cards are checked by path and common directory. |
| `scripts/worktrees/registry-index.mjs` | Index contains cards and problems; publication switches `registry/current` to `generations/<uuid>`. The index has its own observation date. |
| `doc/guides/tech/developer-tools/worktree-registry.md` | Cards are provenance and indexes generated views. Explicit declarations do not establish native capture; no row authorizes removal. |

The original `validateCard` function was executed on the synthetic fixtures.
It accepted the two positive cards and refused a card with an unknown field.
That checks source-format compatibility for these inputs, without exercising
the original registry writer or claiming full validator equivalence.

## Bounded reader and evidence separation

[passive.py](../../experiments/provenance/passive.py) reads one explicitly selected
card and optionally one selected main checkout's published generation. It does
not discover roots or traverse session stores. The selected card is validated;
unselected index rows are inspected only for path selection. The reader does
not validate the index's unrelated Git/local/forge observations and imports
none of them as decision evidence.

| Bound | Experimental value |
| --- | --- |
| Card bytes | 65,536 |
| Index bytes | 1,048,576 |
| Index rows | 128 |
| Sessions, PR relations or problem labels per selected card/row | 32 each |
| Freshness display threshold | 86,400 seconds |

The output records the selected source path, read time, SHA-256 of its bytes,
card update time and optional generation/observation date. The card id is a
declaration. The digest identifies captured source bytes; neither authenticates
the creator or binds those bytes to the current Git worktree administration.
Matching `path` and `repository.commonDir` therefore retains
`currentWorktree: UNKNOWN` with reason `NO_INSTANCE_BINDING`.

Freshness describes the age of `updatedAt` against the fixed display threshold.
It does not mean an agent is active or a checkout is unused. An index's fresh
`observedAt` receives a separate `publicationFreshness`; it cannot refresh the
embedded card's age. Future dates stay marked `future`, and absent dates stay
unknown where the source allows them.

Declared path/common-directory mismatches retain the origin and explicit
contradictions. Differing card/index creators retain both origins and
`SOURCE_DISAGREEMENT`. Duplicate rows for the selected path are ambiguous.
Missing files, missing cards and unrecorded paths remain distinct. A partial
generation keeps `complete: false`, even when it contains a usable positive
context record.

File reads refuse symlinks and special files, cap input before parsing and
compare pre/post file observations. Generation reads accept only the source's
relative `generations/<uuid>` pointer, record its generation and recheck the
pointer after reading. Source replacement and changed publication produce
explicit changed observations with no accepted origin. These checks are
mutation witnesses, not an atomic filesystem snapshot or writer exclusion.

Every result retains `authorizesRemoval: false`, empty `decisionEvidence`, and
`usage: UNKNOWN`. The experiment imports no C1-C5 conditions, verdict, PR status,
local-state fingerprint or usage confirmation. No product schema changed.

## Observed tests and replay

The failing positive test first observed `absent` where `present` was required.
The implementation then passed 22 behavior tests on 2026-10-11. Added tests
observed and corrected three separate errors: an index publication date made an
old card appear recent, a missing registry was reported as invalid, and a
generation pointer disappearing after capture was reported as initial absence.

| Witness | Observed result |
| --- | --- |
| Positive card and published index | Creator, sessions, source digest, generation and distinct dates retained. |
| Missing source/card/path and partial generation | Explicit absence or recorded partial coverage; no inferred empty history. |
| Invalid JSON/UTF-8, duplicate keys, version/field/type/date/session errors | Invalid context refused; source bytes unchanged. |
| Oversize cards/indexes, excess rows/relations | Explicit bounded refusals. |
| Symlink/FIFO card or redirected generation | Refused without following the card link or blocking on the FIFO. |
| Source replacement or generation switch during capture | Changed observation; accepted origin omitted. |
| Old card in freshly published index | Card remains stale; publication remains recent. |
| Different creator or declared location | Sources and contradictions retained. |
| Real Git path recreated with identical branch/HEAD and copied old card | Matching metadata and no contradiction; current identity remains unknown. |

The last witness uses an owned disposable Git repository. It creates a linked
checkout, writes a synthetic card, removes that checkout, recreates it at the
same path on the same branch/HEAD and restores the old card bytes. The
context before and after is identical. A different inode observed during the
read would identify a capture-time change, but the source has no earlier
administration identity against which to compare a stable recreated path.
The recreated path therefore cannot become a verified origin association.

Validation environment: macOS 26.5.1 arm64, Python 3.14.8, Apple Git 2.50.1
(Git-155), Node v22.18.0. Both test and replay commands exited 0.
Raw synthetic logs, command receipts and environment metadata are retained
under `/private/tmp/wt-sweep-evaluations-20261011/provenance/` for this run.
That temporary location is not a durable repository artifact; the scripts below
reproduce the tests and regenerate the context output.

```sh
python3 -B -m unittest discover -s experiments/provenance -p 'test_*.py' -v
python3 -B experiments/provenance/replay.py
```

The optional original-source check needs the existing inspected code checkout
and an installed Node runtime. It verifies the pinned file hashes before and
after importing the source validator on synthetic input:

```sh
python3 -B experiments/provenance/replay.py --source-root /absolute/path/to/aristote-code-checkout
```

## Decision and reopening condition

Keep the bounded experimental reader and its witnesses. Do not ship automatic
origin association in Sweep on this evidence. Missing instance identity does
not make the declarations worthless: they can direct an operator toward a
reported creator/session, and the prototype preserves them as declared context.
The additional source-specific import/schema maintenance has no demonstrated
report-consumer acceptance in this evaluation, so it is not added to the CLI.

Reopen a product import when an explicitly selected report consumer needs these
declarations and accepts a context-only presentation tested against absent,
stale, contradictory and reused-path sources. That import must show source/date,
preserve unknown current identity and keep its data outside C1-C5 and usage
confirmation. A feature claiming verified current origin additionally needs an
existing source binding to the worktree instance, plus capture-time validation
and a recreated-path positive/negative test. No new registry, daemon or global
hook is required by either path.

Not verified: native session capture, identity/authenticity of real creator
claims, actual session activity, full registry-validator equivalence, concurrent
filesystem exclusion, dataless/network-file reads, host permission failures,
or a shipped CLI provenance feature. The complete Rust
and repository harness checks are the integrator's separate validation.
