# Remaining evaluations implementation plan

**Goal:** Complete the evaluations specified by issues #20 and #27–#30, then reconcile epic #14 against observed evidence.

**Architecture:** Isolated experiments exercise the existing read-only CLI and candidate APIs on owned fixtures. Adoption requires the stated evidence; a negative result is recorded without weakening the product contract.

**Spec:** Live GitHub issues #20, #27, #28, #29, #30, captured on 2026-10-11; docs/plan.md and docs/decisions/apply-readiness.md constrain destructive operations.

**Tech stack:** Rust, Python standard library, Darwin APIs and disposable Git repositories. No new product dependency is presumed.

## Constraints

- Base: 671e77fc6e8d54f7f76ee99f94cc8eba9ad38315, alpha.2.
- Work only in the isolated checkout; preserve the dirty primary checkout.
- No global hooks, configuration, permission changes, mounts or scans.
- Preserve C1-C5, raw path bytes, missing evidence and read-only apply refusal.
- Freeze binaries before measurements. Serialize benchmarks and full suites.
- Keep experiment results, local checks, CI and deployed content distinct.

## Review focus

- A fast metadata read must preserve refusal and partial coverage semantics.
- A same-path provenance claim does not establish current worktree identity.
- Existing FDs and mappings can survive path changes and cooperative locks.
- APFS allocated bytes are not unique physical bytes or reclaimable space.
- External command dispatch must preserve arguments, exit status and stream separation.

## Task 1: Native disk evaluation (#20)

Owner: Rust specialist. Own experiments/disk-native/, src/disk/native* only if adoption is justified, and docs/research/native-disk-evaluation-2026-10-11.md. Request shared disk.rs changes from the integrator.

- [x] Implement and test bounded decoding: truncation, lengths, absent attributes, per-entry errors and names.
- [ ] Complete preservation qualification. The owned multi-size fixtures match scalar collection and an independent oracle, including descriptor anchoring, identities, limits, links and tested mutation refusals. Real mounts/firmlinks/dataless and the product adapter remain unqualified; the decision is to stop product adoption.
- [x] Benchmark serially only after correctness and conservation checks; retain raw samples and exact environment.
- [x] Decide adoption or stop with remaining limitations explicit. No production speed claim from a one-directory microbenchmark.

## Task 2: Worktrunk interoperability (#27)

Owner: integrator. Own experiments/worktrunk/ and docs/research/worktrunk-integration-2026-10-11.md.

- [x] Pin and inspect the available Worktrunk revision and binary before any adapter.
- [x] Exercise actual external dispatch with argv containing spaces/metacharacters, explicit cwd, stdout/stderr and nonzero exits on fixtures.
- [x] Exercise the real Sweep binary on a synthetic worktree through dispatch, including invalid arguments and command-name collisions.
- [x] Retain only the smallest useful integration, or explain the unsupported dispatch with evidence.

## Task 3: Passive provenance (#28)

Owner: backend specialist. Own experiments/provenance/ and docs/research/provenance-evaluation-2026-10-11.md. Product shared contracts belong to the integrator.

- [x] Identify an existing source schema from code, without ingesting personal session payloads into fixtures.
- [x] Reproduce origin lookup with synthetic cards; record source, dates, freshness, identity and contradictions.
- [x] Test absent/invalid/oversize records, changed generation and path reuse. Context never changes C1-C5.
- [x] Decide bounded adoption or stop; do not build a registry, daemon or global hook.

## Task 4: Writer exclusion (#29)

Owner: systems specialist. Own experiments/exclusion/ and docs/research/writer-exclusion-evaluation-2026-10-11.md.

- [x] Reuse existing real FD, same-size/mtime and late-commit witnesses; add mappings, aliases and new opens.
- [x] Exercise guard loss and interrupted candidate protocol. No destructive production path is enabled.
- [x] Compare the exact Holt/AgentRinse code already cloned and state what it does and does not establish.
- [x] State exact authority boundary or stop result. A rejection witness is not an exclusion proof.

## Task 5: APFS creation (#30)

Owner: systems specialist. Own experiments/apfs-create/ and docs/research/apfs-creation-evaluation-2026-10-11.md.

- [x] Reproduce a dependency-preserving worktree creation need and compare the existing alternative.
- [x] Identify source/staging/destination devices; test clone success, write isolation, Git index, filters and ignored content on owned fixtures.
- [x] Compare full creation costs and document adoption or stop; never infer unique physical savings from stat blocks.

## Task 6: Integration and delivery (#14)

Owner: integrator. Own changelog, README, roadmap, documentation indexes, CI and GitHub state.

- [x] Review each deliverable against its live issue acceptance criteria; record gaps instead of checking unsupported items.
- [x] Run relevant behavior tests, standard Rust/Python checks and independent final review. The [progress ledger](../../progress.md#évaluations-des-extensions-11102026) distinguishes the initial environment failures from focused replays and successful checks.
- [x] Update documentation and link replayable evidence. Prepare the reviewed change with the user's existing delivery authorization.
- [x] Prepare issue-specific decisions, completed criteria and explicit unproved items.

Publication and closure are tracked live by the delivery PR and issues, rather than inferred from this local checklist: publish the reviewed commit, verify its CI, close each completed evaluation, then close the epic after all child issues are resolved.

Execution ledger and raw logs: /private/tmp/wt-sweep-evaluations-20261011/. Independent tasks share no schema interface. Any product schema adoption needs an explicit integration decision and tests before implementation.
