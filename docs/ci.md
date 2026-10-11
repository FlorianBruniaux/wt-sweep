# CI and repository Actions policy

[Documentation](README.md) · [Contribution checks](../CONTRIBUTING.md)

The [product workflow](../.github/workflows/ci.yml) runs on `push`,
`pull_request` and `workflow_dispatch`. GitHub also applies repository Actions
policy **7082**, named **Owner-triggered workflows**. It permits those three
events for `FlorianBruniaux` only. Other actors remain blocked, including on
public pull requests. A contributor can submit code; the owner must review it
before launching its validation.

The CI token has `contents: read`, checkout credentials are not persisted, and
the [website workflow](../.github/workflows/site.yml) remains manually triggered
on `main`. The policy is repository configuration, not a file restored by Git.
Changing an actor, enforcement or workflow scope is separate from editing YAML.

## What CI checks

The macOS job runs formatting, Clippy, Rust tests and build, Python fixtures,
writer-exclusion experiments, the comparison harness and instruction generation.
It installs the locked website development dependencies to run the HTML browser
checks against synthetic inventory and review reports. These cover volume gauges, command search,
sections, themes, diagram navigation, narrow screens and JavaScript-disabled reading. No host-wide
worktree scan or real cleanup is part of CI.

## Verify automatic execution

After pushing, inspect a run whose event is `push` and whose SHA matches the
pushed commit. After merging, repeat for `main`. A successful manual run does
not establish that automatic events work.

```sh
gh run list --repo FlorianBruniaux/wt-sweep --workflow ci.yml --branch main --limit 5
gh api repos/FlorianBruniaux/wt-sweep/actions/policies/7082
```

If GitHub reports `startup_failure` and no jobs, open the run's annotations.
The 10 October failure said:

```text
Event 'push' is not allowed to trigger Actions workflows. Workflow file: '.github/workflows/ci.yml'.
```

Policy 7082 then allowed only `workflow_dispatch`. On 11 October its event
allowlist was extended to `push` and `pull_request`, preserving the sole owner
actor, active enforcement and workflow scope. Read-back confirmed the change.
The [delivery record](progress.md#dashboard-et-ci-automatique-11102026) links
local validation and the required automatic-run check. This diagnosis is distinct from older billing
failures; do not infer either cause from a red badge alone.

See GitHub's [Actions policy API](https://docs.github.com/en/rest/actions/policies)
for policy fields and [workflow execution protections](https://docs.github.com/en/actions/concepts/about-actions-policies)
for the separate actor and event rules.
