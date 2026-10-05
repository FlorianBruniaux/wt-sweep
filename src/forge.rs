//! Offline forge evidence. Pagination completion never certifies server absence.
use crate::model::{
    BytePath, FORGE_SCHEMA, ForgeCoverage, ForgeExport, ForgeRequest, ForgeSource, SweepError,
};
use crate::output::sha256;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::Path,
};

const RAW_LIMIT: u64 = 16 * 1024 * 1024;
const RECEIPT_LIMIT: u64 = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: String,
    argv: Vec<String>,
    provider: String,
    host: String,
    project: String,
    started_unix_ms: u64,
    finished_unix_ms: u64,
    exit_code: Option<i32>,
    raw_sha256: String,
    raw_bytes: u64,
    truncated: bool,
    pages: Vec<PageReceipt>,
    pagination_complete: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageReceipt {
    number: u64,
    items: usize,
    cursor: Option<String>,
    next_cursor: Option<String>,
}

struct Page {
    rows: Vec<Value>,
    has_next: Option<bool>,
    cursor: Option<String>,
    incomplete_nodes: bool,
}

fn failure(path: &Path, message: impl Into<String>) -> SweepError {
    SweepError::new("invalid_forge_export", "forge_ingest", path, message)
}

fn bounded_read(path: &Path, limit: u64) -> Result<Vec<u8>, SweepError> {
    use rustix::fs::{Mode, OFlags, open};
    use std::os::unix::fs::MetadataExt;
    let fd = open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|e| failure(path, e.to_string()))?;
    let mut file = File::from(fd);
    let before = file.metadata().map_err(|e| failure(path, e.to_string()))?;
    if !before.is_file() || before.len() > limit {
        return Err(failure(
            path,
            "expected bounded regular file, no symlink/FIFO",
        ));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| failure(path, e.to_string()))?;
    let after = file.metadata().map_err(|e| failure(path, e.to_string()))?;
    let stamp = |m: &std::fs::Metadata| {
        (
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    if bytes.len() as u64 > limit
        || stamp(&before) != stamp(&after)
        || after.len() != bytes.len() as u64
    {
        return Err(failure(
            path,
            "file changed during read or exceeded size limit",
        ));
    }
    Ok(bytes)
}

fn pages(
    value: Value,
    result: &mut Vec<Page>,
    gaps: &mut Vec<String>,
    path: &Path,
) -> Result<(), SweepError> {
    if let Some(rows) = value.as_array() {
        // gh api --slurp emits arrays of pages; glab emits concatenated arrays.
        if !rows.is_empty() && rows.iter().all(Value::is_array) {
            for row in rows {
                pages(row.clone(), result, gaps, path)?;
            }
        } else {
            result.push(Page {
                rows: rows.clone(),
                has_next: None,
                cursor: None,
                incomplete_nodes: false,
            });
        }
        return Ok(());
    }
    if value
        .get("errors")
        .is_some_and(|errors| errors.as_array().is_none_or(|items| !items.is_empty()))
    {
        gaps.push("forge returned errors; positive data only".into());
    }
    if let Some(enclosed) = value.get("pages").and_then(Value::as_array) {
        for page in enclosed {
            pages(page.clone(), result, gaps, path)?;
        }
        return Ok(());
    }
    if let Some(connection) = value
        .pointer("/data/repository/pullRequests")
        .or_else(|| value.pointer("/data/search"))
    {
        let rows = if let Some(nodes) = connection.get("nodes").and_then(Value::as_array) {
            nodes.clone()
        } else if let Some(edges) = connection.get("edges").and_then(Value::as_array) {
            edges
                .iter()
                // Retain a placeholder for every raw edge. Dropping missing
                // nodes would let a reduced receipt count hide incomplete data.
                .map(|edge| edge.get("node").cloned().unwrap_or(Value::Null))
                .collect()
        } else {
            return Err(failure(path, "GraphQL connection has no nodes or edges"));
        };
        let incomplete_nodes = rows.iter().any(|node| {
            node.get("number")
                .and_then(Value::as_u64)
                .is_none_or(|id| id == 0)
        });
        if incomplete_nodes {
            gaps.push("GraphQL page has missing or malformed nodes".into());
        }
        result.push(Page {
            rows,
            has_next: connection
                .pointer("/pageInfo/hasNextPage")
                .and_then(Value::as_bool),
            cursor: text(connection.pointer("/pageInfo/endCursor")),
            incomplete_nodes,
        });
        return Ok(());
    }
    if let Some(rows) = value.get("items").and_then(Value::as_array) {
        result.push(Page {
            rows: rows.clone(),
            has_next: value.get("has_next_page").and_then(Value::as_bool),
            cursor: text(value.get("next_cursor")),
            incomplete_nodes: false,
        });
        return Ok(());
    }
    if value.get("iid").is_some() || value.get("number").is_some() {
        result.push(Page {
            rows: vec![value],
            has_next: None,
            cursor: None,
            incomplete_nodes: false,
        });
        return Ok(());
    }
    Err(failure(path, "unsupported forge JSON envelope"))
}

fn text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
fn identifier(value: Option<&Value>) -> Option<String> {
    value.and_then(|value| {
        value
            .as_u64()
            .map(|n| n.to_string())
            .or_else(|| text(Some(value)))
    })
}

fn url_identity(url: &str, provider: &str) -> Option<(String, String, u64)> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    if host.is_empty() || host.contains('@') || host.chars().any(char::is_whitespace) {
        return None;
    }
    let path = path.split(['?', '#']).next()?;
    let (project, number) = if provider == "gitlab" {
        path.split_once("/-/merge_requests/")?
    } else {
        path.split_once("/pull/").or_else(|| {
            path.strip_prefix("repos/")
                .or_else(|| path.strip_prefix("api/v3/repos/"))
                .and_then(|path| path.split_once("/pulls/"))
        })?
    };
    if project.is_empty() {
        return None;
    }
    let host = if provider == "github" && host.eq_ignore_ascii_case("api.github.com") {
        "github.com".into()
    } else {
        host.to_ascii_lowercase()
    };
    Some((
        host,
        project.to_owned(),
        number.trim_end_matches('/').parse().ok()?,
    ))
}

fn oid(value: Option<&Value>, gaps: &mut Vec<String>) -> Option<String> {
    let value = text(value)?;
    if matches!(value.len(), 40 | 64)
        && value.bytes().all(|b| b.is_ascii_hexdigit())
        && value.bytes().any(|b| b != b'0')
    {
        Some(value.to_ascii_lowercase())
    } else {
        gaps.push("invalid or shortened object ID retained as unknown".into());
        None
    }
}

fn agreed(
    field: &str,
    alternatives: impl IntoIterator<Item = Option<String>>,
    gaps: &mut Vec<String>,
) -> Option<String> {
    let known: BTreeSet<_> = alternatives.into_iter().flatten().collect();
    if known.len() > 1 {
        gaps.push(format!(
            "contradictory alternatives for {field}; value unknown"
        ));
        None
    } else {
        known.into_iter().next()
    }
}

fn agreed_oid(
    field: &str,
    alternatives: &[Option<&Value>],
    gaps: &mut Vec<String>,
) -> Option<String> {
    let before = gaps.len();
    let values: Vec<_> = alternatives.iter().map(|value| oid(*value, gaps)).collect();
    let invalid = gaps.len() != before;
    let value = agreed(field, values, gaps);
    if invalid { None } else { value }
}

fn request(row: &Value, gaps: &mut Vec<String>) -> Option<ForgeRequest> {
    let first_gap = gaps.len();
    let (provider, iid) = if let Some(iid) = row.get("iid").and_then(Value::as_u64) {
        ("gitlab", iid)
    } else if let Some(iid) = row.get("number").and_then(Value::as_u64) {
        ("github", iid)
    } else {
        gaps.push("unrecognized request row or missing IID".into());
        return None;
    };
    if iid == 0 {
        gaps.push("zero request IID".into());
        return None;
    }
    let urls = [
        text(row.get("web_url")),
        text(row.get("html_url")),
        text(row.get("url")),
    ];
    // Public request pages and REST endpoints can spell the same identity
    // differently. Compare parsed host/project/IID, retaining the human URL.
    let request_urls: Vec<_> = urls
        .iter()
        .flatten()
        .filter_map(|url| url_identity(url, provider).map(|identity| (url, identity)))
        .collect();
    let url_identities: BTreeSet<_> = request_urls.iter().map(|(_, identity)| identity).collect();
    let url = if url_identities.len() > 1 || url_identities.iter().any(|identity| identity.2 != iid)
    {
        gaps.push("contradictory alternatives for request URL identity; value unknown".into());
        None
    } else if let Some((url, _)) = request_urls.first() {
        Some((*url).clone())
    } else {
        agreed("request URL", urls, gaps)
    };
    let identity = url.as_deref().and_then(|url| url_identity(url, provider));
    let source_project;
    let target_project;
    let source_branch;
    let target_branch;
    let head_oid;
    let merge_oid;
    if provider == "gitlab" {
        source_project = identifier(row.get("source_project_id"));
        target_project = agreed(
            "target project",
            [
                identifier(row.get("target_project_id")),
                identifier(row.get("project_id")),
            ],
            gaps,
        );
        source_branch = text(row.get("source_branch"));
        target_branch = text(row.get("target_branch"));
        head_oid = agreed_oid(
            "source OID",
            &[row.get("sha"), row.pointer("/diff_refs/head_sha")],
            gaps,
        );
        merge_oid = oid(row.get("merge_commit_sha"), gaps);
    } else {
        source_project = agreed(
            "source project",
            [
                text(row.pointer("/head/repo/full_name")),
                text(row.pointer("/headRepository/nameWithOwner")),
            ],
            gaps,
        );
        target_project = agreed(
            "target project",
            [
                text(row.pointer("/base/repo/full_name")),
                text(row.pointer("/baseRepository/nameWithOwner")),
                identity.as_ref().map(|(_, project, _)| project.clone()),
            ],
            gaps,
        );
        source_branch = agreed(
            "source branch",
            [text(row.pointer("/head/ref")), text(row.get("headRefName"))],
            gaps,
        );
        target_branch = agreed(
            "target branch",
            [text(row.pointer("/base/ref")), text(row.get("baseRefName"))],
            gaps,
        );
        head_oid = agreed_oid(
            "source OID",
            &[row.pointer("/head/sha"), row.get("headRefOid")],
            gaps,
        );
        merge_oid = agreed_oid(
            "merge OID",
            &[row.get("merge_commit_sha"), row.pointer("/mergeCommit/oid")],
            gaps,
        );
    }
    let state = match text(row.get("state"))
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("open" | "opened") => "opened",
        Some("merged") => "merged",
        Some("closed")
            if row.get("merged_at").is_some_and(|v| !v.is_null())
                || row.get("merged").and_then(Value::as_bool) == Some(true) =>
        {
            "merged"
        }
        Some("closed") => "closed",
        _ => {
            gaps.push("request state unknown".into());
            "unknown"
        }
    }
    .to_owned();
    let updated_at = agreed(
        "updated timestamp",
        [text(row.get("updated_at")), text(row.get("updatedAt"))],
        gaps,
    );
    // Keep the observed row, but never give a contradictory row a precise
    // repository association, including when the conflict is in another field.
    let contradictory = gaps[first_gap..]
        .iter()
        .any(|gap| gap.starts_with("contradictory alternatives"));
    Some(ForgeRequest {
        provider: provider.into(),
        host: if contradictory {
            None
        } else {
            identity.map(|(host, _, _)| host)
        },
        project: target_project.clone(),
        iid,
        source_project,
        target_project,
        source_branch,
        target_branch,
        state,
        title: text(row.get("title")),
        head_oid,
        merge_oid,
        updated_at,
        url,
        source_index: 0,
    })
}

fn verify_receipt(
    receipt: &Receipt,
    parsed: &[Page],
    requests: &[ForgeRequest],
    gaps: &mut Vec<String>,
) {
    if receipt.exit_code != Some(0) {
        gaps.push("executor exit status missing or nonzero".into());
    }
    if receipt.argv.is_empty() || receipt.argv.iter().any(|arg| arg.contains('\0')) {
        gaps.push("executor argv missing or invalid".into());
    }
    if receipt.started_unix_ms > receipt.finished_unix_ms {
        gaps.push("executor interval reversed".into());
    }
    if receipt.truncated {
        gaps.push("executor output was truncated".into());
    }
    if !receipt.pagination_complete {
        gaps.push("executor pagination not complete".into());
    }
    if !matches!(receipt.provider.as_str(), "github" | "gitlab")
        || receipt.host.is_empty()
        || receipt.project.is_empty()
    {
        gaps.push("executor scope incomplete".into());
    }
    for request in requests {
        if request.provider != receipt.provider
            || request.host.as_ref().is_some_and(|h| h != &receipt.host)
            || request
                .project
                .as_ref()
                .is_some_and(|p| p != &receipt.project)
        {
            gaps.push("executor scope contradicts request identity".into());
        }
    }
    if receipt.pages.len() != parsed.len() || receipt.pages.is_empty() {
        gaps.push("missing or extra receipt pages".into());
    }
    let mut cursors = BTreeSet::new();
    for (index, page) in receipt.pages.iter().enumerate() {
        if parsed.get(index).is_some_and(|page| page.incomplete_nodes) {
            gaps.push("GraphQL page has missing or malformed nodes".into());
        }
        if page.number != index as u64 + 1
            || parsed.get(index).is_none_or(|p| p.rows.len() != page.items)
        {
            gaps.push("page number or count mismatch".into());
        }
        if let Some(cursor) = &page.cursor {
            if cursor.is_empty() || !cursors.insert(cursor) {
                gaps.push("repeated or empty page cursor".into());
            }
        }
        if let Some(next) = &page.next_cursor {
            if receipt.pages.get(index + 1).and_then(|p| p.cursor.as_ref()) != Some(next) {
                gaps.push("missing page after cursor".into());
            }
        } else if receipt
            .pages
            .get(index + 1)
            .is_some_and(|p| p.cursor.is_some())
        {
            gaps.push("cursor chain is incomplete".into());
        }
        if parsed.get(index).is_some_and(|p| p.has_next == Some(true)) && index + 1 == parsed.len()
        {
            gaps.push("raw page still has a next page".into());
        }
        if let Some(raw_cursor) = parsed.get(index).and_then(|p| p.cursor.as_ref()) {
            if index + 1 < parsed.len() && page.next_cursor.as_ref() != Some(raw_cursor) {
                gaps.push("raw cursor contradicts receipt".into());
            }
        }
    }
}

pub fn ingest(raw: &Path, receipt_path: Option<&Path>) -> Result<ForgeExport, SweepError> {
    crate::residency::ensure_no_materialization()?;
    let bytes = bounded_read(raw, RAW_LIMIT)?;
    let digest = sha256(&bytes);
    let mut parsed = Vec::new();
    let mut gaps = Vec::new();
    for value in serde_json::Deserializer::from_slice(&bytes).into_iter::<Value>() {
        pages(
            value.map_err(|e| failure(raw, e.to_string()))?,
            &mut parsed,
            &mut gaps,
            raw,
        )?;
    }
    if parsed.is_empty() {
        return Err(failure(raw, "empty export has no JSON page"));
    }
    let mut requests = Vec::new();
    let mut identities = BTreeMap::new();
    for page in &parsed {
        if page.rows.len() >= 100 {
            gaps.push(
                "page saturation at 100 or aggregate limit 2000; completeness unproven".into(),
            );
        }
        for row in &page.rows {
            if let Some(request) = request(row, &mut gaps) {
                let key = (
                    request.provider.clone(),
                    request.host.clone(),
                    request.project.clone(),
                    request.iid,
                );
                if let Some(previous) = identities.insert(key, row.clone()) {
                    gaps.push(
                        if previous == *row {
                            "duplicate request identity"
                        } else {
                            "contradictory versions of request identity"
                        }
                        .into(),
                    );
                }
                requests.push(request);
            }
        }
    }
    let mut source = ForgeSource {
        raw_path: BytePath::from_path(
            &std::fs::canonicalize(raw).map_err(|e| failure(raw, e.to_string()))?,
        ),
        sha256: digest.clone(),
        receipt_path: receipt_path
            .map(|path| {
                std::fs::canonicalize(path)
                    .map(|path| BytePath::from_path(&path))
                    .map_err(|e| failure(path, e.to_string()))
            })
            .transpose()?,
        receipt_sha256: None,
        receipt_verified: false,
        started_unix_ms: None,
        finished_unix_ms: None,
        execution: None,
        gaps: Vec::new(),
    };
    let mut pagination_complete = false;
    if let Some(path) = receipt_path {
        let receipt_bytes = bounded_read(path, RECEIPT_LIMIT)?;
        let receipt: Receipt =
            serde_json::from_slice(&receipt_bytes).map_err(|e| failure(path, e.to_string()))?;
        if receipt.schema != "wt-sweep.forge-receipt/1" {
            return Err(failure(path, "unsupported receipt schema"));
        }
        if receipt.raw_sha256 != digest || receipt.raw_bytes != bytes.len() as u64 {
            return Err(failure(
                path,
                "receipt hash or size does not match raw bytes",
            ));
        }
        source.receipt_sha256 = Some(sha256(&receipt_bytes));
        source.started_unix_ms = Some(receipt.started_unix_ms);
        source.finished_unix_ms = Some(receipt.finished_unix_ms);
        source.execution =
            Some(serde_json::from_slice(&receipt_bytes).map_err(|e| failure(path, e.to_string()))?);
        let mut receipt_gaps = Vec::new();
        verify_receipt(&receipt, &parsed, &requests, &mut receipt_gaps);
        source.receipt_verified = receipt_gaps.is_empty();
        pagination_complete = source.receipt_verified
            && receipt.pagination_complete
            && parsed.iter().all(|page| page.rows.len() < 100);
        gaps.extend(receipt_gaps);
    } else {
        gaps.push("executor receipt missing; positive facts have partial provenance".into());
    }
    for request in &requests {
        if request.host.is_none() || request.project.is_none() {
            gaps.push("request host/project identity incomplete".into());
        }
    }
    gaps.sort();
    gaps.dedup();
    source.gaps = gaps.clone();
    Ok(ForgeExport {
        schema: FORGE_SCHEMA.into(),
        requests,
        sources: vec![source],
        coverage: ForgeCoverage {
            pagination_complete,
            temporal_consistency: "not_guaranteed".into(),
            absence_proven: false,
            gaps,
        },
    })
}
