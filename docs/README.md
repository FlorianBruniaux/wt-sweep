# Documentation

[Project overview](../README.md) · [Présentation en français](../README.fr.md) · [Changelog](../CHANGELOG.md) · [Contributing](../CONTRIBUTING.md)

These guides cover the read-only macOS MVP and the T01-T09 triage workflow.
The [current delivery record](progress.md#état-publié-et-suite-au-09102026)
separates merged code, CI and real-world observations. The [report delivery,
10 October](https://github.com/FlorianBruniaux/wt-sweep/pull/26) tracks forge links,
HTML opening, the console recap and their final checks. The [roadmap](roadmap.md)
names the remaining #20 evaluation and the separate #14 vision; the
[validation ledger](finish-ledger.md#triage-t01-t09) retains historical checks.
The project overview is available in English and French. The tutorial, assistant
and disk guides are in French; other user guides are in English. Design and
validation records retain their original language and observation dates.

La [préparation de la première alpha](release.md) décrit le périmètre,
les conditions de publication et le paquet macOS prévu.

## Use the CLI

| Document | Contents |
| --- | --- |
| [Tutoriel en français](tutorial.fr.md) | One-command review, disposable trial, tests, advanced replay and a copyable assistant prompt. |
| [Installation guidée (French)](bootstrap-prompt.fr.md) | Copyable assistant prompt: acquire the CLI and bundled skills, build, try a disposable repository and guide a real review. |
| [Assistant skill (French)](assistant-skill.md) | Claude/Codex invocation, progressive report reading, cleanup suggestions and local verification. |
| [Getting started](getting-started.md) | macOS setup, disposable demo, report interpretation and troubleshooting. |
| [Espace disque](disk-usage.md) | Taille par worktree, Git partagé, total dédupliqué et limites de la mesure. |
| [Rapport HTML par projet](html-report-plan.md) | HTML hors ligne, ouverture sur macOS, lecture par famille, liens GitHub/GitLab, copie des chemins et reçus. |
| [Resume discovery](discovery-resume.md) | Save an explicit-root traversal, continue it and interpret separate observation intervals. |
| [CLI reference](cli.md) | Implemented commands, output rules, exit codes and collection limits. |
| [Targets and filters](targets-and-filters.md) | Select refs per repository and inspect a subset while retaining source evidence. |
| [Forge links and offline imports](forge.md) | Repository, branch and commit links, captured PR/MR states, import receipts and association limits. |
| [Evidence and JSON contracts](contracts.md) | Artifact fields, classifications, path encoding, digests and receipts. |

## Develop and verify

| Document | Contents |
| --- | --- |
| [Contributing](../CONTRIBUTING.md) | Local checks, fixture ownership and change requirements. |
| [README illustrations](assets/README.md) | Current Gemini overview, dated native HTML captures, archived illustrations and their provenance. |
| [Architecture](architecture.md) | Module responsibilities, data flow, read boundaries and test strategy. |
| [Native boundaries](native-boundaries.md) | Three reviewed private unsafe boundaries and their evidence limits. |
| [AI development harness](harness.md) | Canonical instructions, generated host files and separate checks for native activation. |
| [Collection measurements](collection-performance.md) | Discovery throughput, reproducible Rust benchmark, budget profiles, phase metrics and observed coverage. |
| [Collection benchmark, 07/10](research/collection-benchmark-2026-10-07.md) | Before/after release samples at 50 and 100 worktrees, first provisional fact, comparable evidence and rejected measurements. |
| [Recette réelle, 09/10](research/real-family-acceptance-2026-10-09.md) | 71 inscriptions, revues ciblées, tailles complètes et limites ; chemins privés conservés localement. |
| [Lectures Git groupées, 08/10](research/history-batch-benchmark-2026-10-08.md) | Processus, durées et preuves sur corpus fixe ; coût mesuré de la reprise de découverte. |
| [Review benchmark runner](../scripts/benchmark-review.py) | Frozen binaries, synthetic Git families, first observed fact, pipe/PTY output and conservation checks. Results need their binary hashes and evidence coverage. |
| [Implementation results](progress.md) | Current checkpoint links followed by the dated U1-U4 validation record. |
| [Recette réelle multi-projets, 07/10](research/multi-project-acceptance-2026-10-07.md) | 69 registered worktrees, matching text/JSON/HTML, partial disk and local coverage, and delivery limits. |
| [Finishing ledger](finish-ledger.md) | Historical RETEX checkpoint, current triage checks, repository trial and remaining gates. |

## Website

| Document | Contents |
| --- | --- |
| [Site implementation](../site/README.md) | Local preview, EN/FR portal, reviewed sources, tests and public-origin configuration. |
| [Website validation](../site/VERIFICATION.md) | Observed checks, reference component status, independent review and remaining evidence limits. |
| [Website design](superpowers/specs/2026-10-09-wt-sweep-site-design.md) | Validated structure, BoldGuy design and product boundaries. |
| [Website plan](superpowers/plans/2026-10-09-wt-sweep-site.md) | Implementation sequence and observed checks. |

## Design and decisions

| Document | Contents |
| --- | --- |
| [Roadmap](roadmap.md) | Delivered scope, large-repository collection limits, next validation steps and separate removal gates. |
| [Plan de tri des worktrees, v2](superpowers/plans/2026-10-05-worktree-triage-v2.md) | Completed T01-T09 plan, decision conditions, acceptance checks and conditional extensions. |
| [Critique du plan et de l'architecture](superpowers/plans/2026-10-05-worktree-triage-review.md) | Findings against the first proposal and current code, alternatives, failure modes and corrections adopted in v2. |
| [Acceptance plan](plan.md) | U1–U7 and E1 acceptance criteria. |
| [Implementation and ownership](implementation.md) | Development units, responsibilities and the recorded model allocation. |
| [Decision index](decisions/README.md) | Failed writer-exclusion experiments and the decision to keep `apply` unavailable. |
| [Cleanup-session lessons](retex-sessions.md) | Sourced lessons and disposable tests; historical shared inventories are not reused. |
| [Worktree source analysis, 07/10](worktree-source-analysis-2026-10-07.md) | Twelve cloned tools, implementation mechanisms, UI/CLI ideas, specialist notes and bounded reuse decisions. |
| [Worktree ecosystem review, 07/10](worktree-ecosystem-2026-10-07.md) | Direct competitors, lifecycle managers, evidence limits and reuse experiments before expanding scope. |
| [Controlled runtime comparison](research/worktree-runtime-comparison-2026-10-07.md) | 48 synthetic Sweep, Holt and AgentRinse audits; native verdicts, writing effects and decisions by capability. |
| [Related tools and references](references.md) | Dated source readings, runtime evidence and limits of reuse. |

The [roadmap](roadmap.md) records delivered scope and remaining work; the
[project README](../README.md#roadmap-and-validation) summarizes current status.
Read the [validation ledger](finish-ledger.md) for test results and
the [apply decision](decisions/apply-readiness.md) for the unmet requirements
that keep removal unavailable.
