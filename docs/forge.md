# Offline forge imports

[Documentation index](README.md) · [CLI reference](cli.md) · [Evidence contracts](contracts.md)

Forge data adds MR/PR context to local Git observations. Worktree Sweep reads
files captured by another client; it does not authenticate, query a server or
execute the templates printed by `forge commandes`.

## Capture and import

```sh
./target/release/wt-sweep forge commandes

./target/release/wt-sweep forge ingerer \
  --raw /absolute/path/to/capture/raw.json \
  --receipt /absolute/path/to/capture/receipt.json \
  --out /absolute/path/to/reports/forge.json

./target/release/wt-sweep plan \
  --inventory /absolute/path/to/reports/inventory.json \
  --forge /absolute/path/to/reports/forge.json \
  --target refs/heads/main --format json
```

The output parent must exist. Preserve the raw file and receipt at their recorded
paths: `plan` rereads them, recomputes normalization and rejects substituted raw
bytes or altered normalized facts. Moving only `forge.json` is insufficient.

The importer accepts GitLab MR and GitHub PR fields in JSON arrays, concatenated
pages, arrays of pages and supported GitHub GraphQL connections. GitHub CLI
field names are also normalized. Supported GraphQL envelopes are
`data.repository.pullRequests` and `data.search`, with `nodes` or `edges`.
It is not a generic GraphQL importer. See the [adapter](../src/forge.rs) for the
exact field mappings and [fixture tests](../tests/forge.rs) for accepted shapes.

Without `--receipt`, the importer retains recognized requests but marks their
capture provenance as incomplete. It does not turn a command transcript into
capture evidence.

## Links in review reports

`review` and `plan` collect optional remote context by default. Their HTML and
text reports show repository, branch and full-commit links for GitHub and GitLab.
`scan` records inventory only; it does not collect this context. Old saved reviews
without remote metadata display “not collected”. Rendering never reads Git.

```sh
wt-sweep review --repo /absolute/path/to/repo \
  --target refs/heads/main --forge /absolute/path/to/capture/forge.json --open
```

Omit `--forge` when no capture is available: recognized public remotes still
provide navigation. Use `--no-context` to skip remote, commit and changelog
context. Existing imported PR/MR associations remain available with that option.

| Report item | Meaning |
| --- | --- |
| Repository / Branch / Commit | URL constructed from local configuration. The page may be unavailable. |
| HEAD matches the local tracking ref | The recorded HEAD equals the locally stored upstream OID. That ref can be stale. |
| HEAD differs from the local tracking ref | The OIDs differ. This does not count unpushed commits or establish server state. |
| Publication unknown | No usable configured upstream observation was collected. |
| PR/MR and captured state | An imported request matched the source remote, branch and full HEAD. Its state belongs to the capture. |
| No associated PR/MR | No matching request in the supplied evidence, not proof that none exists. |

The branch link uses the configured upstream's remote branch when available;
otherwise it uses the recorded local branch name as a proposed destination.
Each remote is labeled separately. For a GitHub fork, source branch links point
to the fork while the PR points to its target repository. GitLab fork association
still requires source-path evidence the current importer does not provide; numeric
project IDs alone do not attach an MR from another project.

HTTPS and common SSH/SCP remote forms are recognized. Credentials are discarded.
Ambiguous multi-URL remotes, invalid paths, unknown SSH aliases and unsupported
hosts produce gaps instead of guessed destinations. Self-hosted GitHub/GitLab
routes require an exactly associated imported request to identify the provider.
No custom host configuration or live API adapter is installed.

Collection uses the bounded optional-context budget and caches metadata per Git
family. Local configuration and refs are separate observations, not an atomic
snapshot. Context cannot change a cleanup verdict. Links are rebuilt and checked
at render time, ref bytes are URL-encoded, and HTML labels are escaped. Clicking
an external link opens a new tab; merely opening the report makes no network request.

URL routes follow the [GitHub branch API examples](https://docs.github.com/en/rest/branches/branches)
and [GitLab branch API examples](https://docs.gitlab.com/api/branches/).

## Executor receipt

The client that actually executes and persists the capture should write
`wt-sweep.forge-receipt/1`. The CLI has no live capture adapter today.

| Field | Required meaning |
| --- | --- |
| `schema` | `wt-sweep.forge-receipt/1` |
| `argv` | Actual argument vector; no secrets should be placed in arguments. |
| `provider`, `host`, `project` | Declared forge scope, consistent with the exported requests. |
| `started_unix_ms`, `finished_unix_ms` | Capture interval in Unix milliseconds. |
| `exit_code` | Observed process exit status; missing or nonzero status leaves a gap. |
| `raw_sha256`, `raw_bytes` | SHA-256 and byte length of the file actually persisted. |
| `truncated` | Whether capture was truncated. |
| `pages` | Ordered records with `number`, `items`, `cursor`, `next_cursor`. Page numbering starts at 1. |
| `pagination_complete` | The executor's completion observation, checked against the parsed pages. |

The importer rejects a hash or size mismatch. It records uncertainty for missing
pages, repeated cursors, contradictory data, truncation and nonzero exit codes.
A page with 100 or more items is marked as possibly saturated: more results may
have been available.

`receipt_verified` means the receipt passed local structural and consistency
checks, which do not authenticate the executor's claims. The server may change
between page reads even when pagination completes, so the current adapter
always reports `absence_proven: false` and
`temporal_consistency: "not_guaranteed"`.

## Replay a synthetic export

This example needs Python 3.11+ and the built CLI. Run it from the source checkout.
The helper launches a local fixture producer, saves its actual stdout and exit
status, then hashes the persisted bytes. It makes no forge or network request.

```sh
forge_dir="$(mktemp -d /private/tmp/wt-sweep-forge.XXXXXX)"
python3 -B experiments/forge-capture/capture_fixture.py \
  tests/data/forge/gitlab-pages.json \
  "$forge_dir/raw.json" "$forge_dir/receipt.json"

./target/release/wt-sweep forge ingerer \
  --raw "$forge_dir/raw.json" --receipt "$forge_dir/receipt.json" \
  --out "$forge_dir/forge.json"
```

Expected stdout:

```text
Forge requests: 2; pagination complete: true; temporal consistency: not_guaranteed; absence proven: false
```

The two requests belong to a reserved synthetic host and use synthetic OIDs.
They are unrelated to the [Git demo](getting-started.md#disposable-demo), so
combining the two demos does not establish an MR association.

## Association and judgement

For a precise MR/PR association, the local remote identity, host/project,
source branch and full HEAD must agree. Contradictory fields prevent that
association. A partial export can establish that a request was observed;
an omitted request may still exist on the server.

An associated open request adds the `open_request` reason and classifies a
worktree as `keep`. It does not prove a live process. A closed or merged request
alone cannot satisfy the integration condition for `cleanup_candidate`.
Local object relationships must reach an explicit integration target. Request
titles and links are descriptive; the source HEAD is not an integration target.

The detailed review shows the host/project, recorded HEAD, update time, raw
source path and hash, receipt verification and capture interval beside each
request. These identify the imported observation, not the current server state.
An open request is a reason to retain work, not evidence of a running agent.

Before sharing captures, inspect them for private repository names, paths,
subjects and request titles. Hashing a capture does not redact its contents.
