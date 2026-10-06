# Model, effort and delegation policy

[Workflow](../SKILL.md) · [Report interpretation](reports.md)

These are task-based starting points, not benchmark results or claims that one
model is always best. Check the active host's model catalog, tool schema, role
restrictions, available slots and user choices before applying them. Model names
and supported effort levels can change; an unavailable name is not a fallback.

## Choose the smallest sufficient profile

| Profile | Use | Codex example, if exposed by the host | Claude Code example, if available |
| --- | --- | --- | --- |
| Extract | Copy facts, counts and field references from a bounded saved report; no judgement | `gpt-6-luna`, `low` | `haiku`; omit effort if unsupported |
| Review | Explain local facts, C1-C5 gaps and the CLI's original verdicts; prepare the recap | `gpt-6.1-sol`, `medium` | `sonnet`, `medium` if supported |
| Investigate | Reconcile contradictory observations or difficult private-history/index explanations | `gpt-6.1-sol`, `high` | `sonnet`, `high` if supported |
| Independent check | Review consequential recommendations when ambiguity remains after investigation | `gpt-6-astra`, `high` | `opus`, `high` if supported |

Start with Review. Select Extract only for mechanical reading, with the
coordinator checking identity, counts and completeness. Do not launch an agent
merely to print a JSON count that a local reader can compute. Investigate is
triggered by a specific contradiction to explain, not by the number of rows.
Use Independent check only when another reader could change a recommendation.
A different model is optional; a separate analysis of the original evidence is
what makes the check independent.

A timeout, exhausted budget, missing object or uncollected local files does not
justify raising model effort. State the missing observation and the useful next
collection step, if one exists. Higher reasoning effort cannot fill that gap.
Do not select `xhigh`, `max` or `ultra` routinely. No cost, accuracy or throughput
improvement has been measured for these profiles on wt-sweep.

## Apply only controls the host actually exposes

**Codex:** use the active platform's exact model IDs and supported reasoning
levels. When a child tool exposes `model` and `reasoning_effort`, pass the selected
values there. Some fork modes inherit the parent's settings and reject overrides:
choose a supported task-only fork and provide a self-contained assignment when
changing a child's model. Honor fixed-effort roles. If those fields or the model
are unavailable, inherit the current settings and say which recommendation could
not be applied. Do not write a model ID into project or global configuration to
work around a missing runtime control.

**Claude Code:** use only the selected provider's available model aliases or IDs.
Pass a per-invocation model when the child tool supports it. Effort can depend on
the client version, model and existing subagent definition; inspect the exposed
control before sending it. If no invocation-level effort field exists, retain
the inherited or already configured effort and label a different requested level
as unapplied. Do not invent an Agent tool parameter or edit a role to force it.
The [Claude model settings](https://code.claude.com/docs/en/model-config) and
[subagent reference](https://code.claude.com/docs/en/sub-agents) describe these
host-specific controls; they do not establish the local client's capabilities.

The parent continues on its current model unless a supported, authorized runtime
control actually changes it. If the host cannot do that, state the recommended
session setting once and continue useful work with the current model. Do not
claim a switch from a prompt phrase such as “think harder”. Do not change global
settings, install hooks, launch another client or switch providers for this task.
Honor user budget limits and platform consent requirements for model access.

Report selected model/effort separately from execution evidence. A successful
spawn request confirms acceptance of those arguments; report the effective model
or effort only if the host exposes it. Otherwise retain “effective setting not
verified”. Inherited settings may also be unknown.

## Delegate only independent reading or verification

The coordinator runs the scoped collection, checks the receipt and source, then
owns final totals and recommendations. Use children only when at least two
independent report batches would benefit from parallel reading, or when one
bounded independent check is warranted. A single straightforward report stays
sequential. `--workers` controls CLI metadata reads, not LLM subagents.

Keep at most two children active, reduced by the host limit and occupied slots.
Do not spawn a child per worktree or permit recursive delegation. Readers may
handle distinct physical Git families or disjoint row batches from one frozen
run. Keep shared stashes and family-level coverage with the coordinator. Never
split collection of one family among children or run duplicate scans to feed them.

Each assignment must include:

- Read-only responsibility, exact run path, expected receipt/source digests,
  assigned path identities and confirmed target refs. Do not send the whole
  conversation when those facts suffice.
- Relevant skill boundaries and permission limits. Other agents may be working;
  do not edit their files, reports or target repositories. No collection, Git
  mutation, network export or further delegation.
- Required return: reviewed path identities and full OIDs, original CLI verdicts,
  cited fields and artifact paths, missing checks, proposed next actions and
  unread rows. Return a compact result to the coordinator.

The coordinator matches returned identities and digests to the supplied evidence,
deduplicates rows, checks counts and preserves global gaps. A child's failure or
partial return leaves its rows unread until the coordinator finishes them or
reports them pending. Never accept a changed verdict or remove an unknown because
a child sounds confident. An independent checker reads the original relevant
rows and conditions, not only the first reader's explanation. Disagreement stays
visible until evidence resolves it; voting is not proof.

In the final recap, record the profile, requested/observed settings, reason for
any escalation, delegated scopes, returned/pending rows and remaining limits.
Keep this compact. Model routing and delegation never authorize deletion.
