//! Local Git observations and a pure review policy. Never authorizes an action.
use crate::git::{ControlledGit, GitRead, scan_family};
use crate::model::*;
use crate::output::canonical_digest;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

fn error(message: impl Into<String>) -> SweepError {
    SweepError::new("invalid_inventory", "review", Path::new(""), message)
}

fn valid_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value.bytes().all(|b| b.is_ascii_hexdigit())
        && value.bytes().any(|b| b != b'0')
}

fn subject(bytes: &[u8]) -> Result<(BytePath, bool), SweepError> {
    let start = bytes
        .windows(2)
        .position(|pair| pair == b"\n\n")
        .ok_or_else(|| error("commit message separator absent"))?
        + 2;
    let first = bytes[start..]
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    if first.contains(&0) {
        return Err(error(
            "commit subject contains NUL; description unavailable",
        ));
    }
    Ok((
        BytePath::from_bytes(&first[..first.len().min(240)]),
        first.len() > 240,
    ))
}

fn full_ref(reference: &BytePath) -> Result<(), SweepError> {
    let bytes = reference.bytes()?;
    if !(bytes.starts_with(b"refs/heads/") || bytes.starts_with(b"refs/remotes/"))
        || bytes
            .iter()
            .any(|b| *b <= b' ' || *b == 127 || b"~^:?*[\\".contains(b))
        || bytes
            .windows(2)
            .any(|p| p == b".." || p == b"@{" || p == b"//")
        || bytes.ends_with(b"/")
        || bytes.ends_with(b".")
        || bytes.ends_with(b".lock")
    {
        return Err(error(
            "integration targets must be full branch refs, without revision expressions",
        ));
    }
    Ok(())
}

fn remote_identity(url: &str) -> Option<(String, String)> {
    if url.bytes().any(|b| b <= b' ' || b == 127) {
        return None;
    }
    let (authority, path) = if let Some((scheme, rest)) = url.split_once("://") {
        if !["https", "http", "ssh", "git"].contains(&scheme) {
            return None;
        }
        rest.split_once('/')?
    } else {
        if !url.contains('@') {
            return None;
        }
        url.split_once(':')?
    };
    let host = authority.rsplit('@').next()?;
    if host.is_empty() || host.contains('[') || host.contains(']') {
        return None;
    }
    let path = path
        .trim_end_matches('/')
        .strip_suffix(".git")
        .unwrap_or(path.trim_end_matches('/'));
    if path.is_empty() {
        return None;
    }
    Some((host.to_ascii_lowercase(), path.into()))
}

fn remotes(git: &ControlledGit, common: &Path) -> Result<BTreeSet<(String, String)>, SweepError> {
    let raw = git.read(common, GitRead::RemoteUrls)?;
    let mut identities = BTreeSet::new();
    for field in raw.split(|b| *b == 0).filter(|b| !b.is_empty()) {
        let Some(split) = field.iter().position(|b| *b == b'\n') else {
            return Err(error("malformed remote identity output"));
        };
        if let Ok(url) = std::str::from_utf8(&field[split + 1..]) {
            if let Some(identity) = remote_identity(url) {
                identities.insert(identity);
            }
        }
    }
    Ok(identities)
}

fn matching_requests(
    export: &ForgeExport,
    remotes: &BTreeSet<(String, String)>,
    worktree: &Worktree,
) -> Vec<ForgeRequest> {
    export
        .requests
        .iter()
        .filter(|request| {
            let Some(host) = request.host.as_deref() else {
                return false;
            };
            let Some(source) = request.head_oid.as_deref() else {
                return false;
            };
            if worktree.head.as_deref() != Some(source) {
                return false;
            }
            if let Some(branch) = &worktree.branch {
                let Some(source_branch) = &request.source_branch else {
                    return false;
                };
                if branch.bytes().ok().as_deref()
                    != Some(format!("refs/heads/{source_branch}").as_bytes())
                {
                    return false;
                }
            }
            let mut project = request.source_project.clone();
            if request.provider == "gitlab" {
                // A GitLab numeric ID cannot identify a fork's path. Only equal known
                // source/target IDs let the target URL establish the source namespace.
                if request.source_project.is_none()
                    || request.source_project != request.target_project
                {
                    return false;
                }
                project = request
                    .url
                    .as_deref()
                    .and_then(|u| u.split_once("/-/merge_requests/"))
                    .and_then(|(base, _)| remote_identity(base))
                    .map(|(_, project)| project);
            }
            project.is_some_and(|p| remotes.contains(&(host.to_ascii_lowercase(), p)))
        })
        .cloned()
        .collect()
}

fn verify_forge(export: &ForgeExport) -> Result<(), SweepError> {
    if export.schema != FORGE_SCHEMA || export.sources.len() != 1 {
        return Err(error("unsupported forge schema or source count"));
    }
    let source = &export.sources[0];
    let raw = source.raw_path.to_path()?;
    let receipt = source
        .receipt_path
        .as_ref()
        .map(BytePath::to_path)
        .transpose()?;
    let replay = crate::forge::ingest(&raw, receipt.as_deref())?;
    if canonical_digest(&replay)? != canonical_digest(export)? {
        return Err(error(
            "forge artifact no longer matches its persisted raw evidence",
        ));
    }
    Ok(())
}

struct ObjectReader {
    git: ControlledGit,
    cache: BTreeMap<(BytePath, String, String), Rc<Vec<u8>>>,
    reads_left: usize,
    bytes_left: usize,
    started: Instant,
}
impl ObjectReader {
    fn new() -> Self {
        Self {
            git: ControlledGit::default(),
            cache: BTreeMap::new(),
            reads_left: 2048,
            bytes_left: 32 * 1024 * 1024,
            started: Instant::now(),
        }
    }
    fn expired(&self) -> bool {
        self.started.elapsed() >= Duration::from_secs(60)
    }
    fn read(&mut self, common: &Path, kind: &str, oid: &str) -> Result<Rc<Vec<u8>>, SweepError> {
        if self.expired() {
            return Err(error("whole-review time budget reached"));
        }
        let key = (BytePath::from_path(common), kind.into(), oid.into());
        if let Some(bytes) = self.cache.get(&key) {
            return Ok(bytes.clone());
        }
        if self.reads_left == 0 {
            return Err(error("whole-review object budget reached"));
        }
        if self.bytes_left == 0 {
            return Err(error("whole-review byte budget reached"));
        }
        self.reads_left -= 1;
        let operation = match kind {
            "commit" => GitRead::Commit { oid: oid.into() },
            "tree" => GitRead::Tree { oid: oid.into() },
            "blob" => GitRead::Blob { oid: oid.into() },
            _ => return Err(error("unsupported object kind")),
        };
        let bytes = self.git.read(common, operation)?;
        let Some(remaining) = self.bytes_left.checked_sub(bytes.len()) else {
            self.bytes_left = 0;
            return Err(error("whole-review byte budget reached"));
        };
        self.bytes_left = remaining;
        if self.expired() {
            return Err(error("whole-review time budget reached"));
        }
        verify_object(kind, oid, &bytes)?;
        let bytes = Rc::new(bytes);
        self.cache.insert(key, bytes.clone());
        Ok(bytes)
    }
}

struct Commit {
    tree: String,
    parents: Vec<String>,
}

fn commit(reader: &mut ObjectReader, common: &Path, oid: &str) -> Result<Commit, SweepError> {
    if !valid_oid(oid) {
        return Err(error("invalid or absent commit OID"));
    }
    let bytes = reader.read(common, "commit", oid)?;
    let mut tree = None;
    let mut parents = Vec::new();
    for line in bytes
        .split(|b| *b == b'\n')
        .take_while(|line| !line.is_empty())
    {
        if let Some(value) = line.strip_prefix(b"tree ") {
            let value = std::str::from_utf8(value).map_err(|_| error("invalid tree OID"))?;
            if tree.is_some() || !valid_oid(value) {
                return Err(error("invalid commit tree header"));
            }
            tree = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix(b"parent ") {
            let value = std::str::from_utf8(value).map_err(|_| error("invalid parent OID"))?;
            if !valid_oid(value) {
                return Err(error("invalid commit parent header"));
            }
            parents.push(value.into());
        }
    }
    Ok(Commit {
        tree: tree.ok_or_else(|| error("missing commit tree header"))?,
        parents,
    })
}

fn resolve(git: &ControlledGit, common: &Path, reference: &BytePath) -> Result<String, SweepError> {
    let raw = git.read(
        common,
        GitRead::ResolveCommit {
            reference: reference.clone(),
        },
    )?;
    let oid = raw
        .strip_suffix(b"\n")
        .and_then(|v| std::str::from_utf8(v).ok())
        .filter(|s| valid_oid(s))
        .ok_or_else(|| error("invalid resolved target OID"))?;
    Ok(oid.into())
}

fn local_refs(git: &ControlledGit, common: &Path) -> Result<Vec<BytePath>, SweepError> {
    let bytes = git.read(common, GitRead::LocalRefs)?;
    let mut refs = Vec::new();
    for record in bytes.split(|b| *b == b'\n').filter(|b| !b.is_empty()) {
        let fields: Vec<_> = record.split(|b| *b == 0).collect();
        if fields.len() != 3 || !fields[2].is_empty() {
            return Err(error("malformed local ref output"));
        }
        refs.push(BytePath::from_bytes(fields[0]));
    }
    Ok(refs)
}

fn relation(
    reader: &mut ObjectReader,
    common: &Path,
    source: &str,
    target: &str,
) -> Result<Option<&'static str>, SweepError> {
    let source_commit = commit(reader, common, source)?;
    let target_commit = commit(reader, common, target)?;
    inspect_tree(reader, common, &source_commit.tree)?;
    inspect_tree(reader, common, &target_commit.tree)?;
    if source == target {
        return Ok(Some("identical_commit"));
    }
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::from([target.to_string()]);
    while let Some(oid) = queue.pop_front() {
        if !visited.insert(oid.clone()) {
            continue;
        }
        if visited.len() > 512 {
            return Err(error("commit traversal budget reached; ancestry unknown"));
        }
        if oid == source {
            return Ok(Some("ancestor"));
        }
        for parent in commit(reader, common, &oid)?.parents {
            queue.push_back(parent);
        }
    }
    if source_commit.tree == target_commit.tree {
        return Ok(Some("identical_tree"));
    }
    Ok(None)
}

fn inspect_tree(reader: &mut ObjectReader, common: &Path, oid: &str) -> Result<(), SweepError> {
    let width = oid.len() / 2;
    let mut queue = VecDeque::from([oid.to_string()]);
    let mut seen = BTreeSet::new();
    while let Some(oid) = queue.pop_front() {
        if !seen.insert(oid.clone()) {
            continue;
        }
        if seen.len() > 512 {
            return Err(error("tree traversal budget reached"));
        }
        let raw = reader.read(common, "tree", &oid)?;
        let mut rest = raw.as_slice();
        while !rest.is_empty() {
            let end = rest
                .iter()
                .position(|b| *b == 0)
                .ok_or_else(|| error("malformed tree entry"))?;
            let header = &rest[..end];
            let mode_end = header
                .iter()
                .position(|b| *b == b' ')
                .ok_or_else(|| error("malformed tree mode"))?;
            let mode = &header[..mode_end];
            if ![
                b"40000".as_slice(),
                b"100644",
                b"100755",
                b"120000",
                b"160000",
            ]
            .contains(&mode)
            {
                return Err(error("unsupported tree mode"));
            }
            if header.len() <= mode_end + 1 || rest.len() < end + 1 + width {
                return Err(error("truncated tree entry"));
            }
            let child = &rest[end + 1..end + 1 + width];
            // Tree OID identity includes each gitlink's exact mode and object ID.
            // No diff or submodule-ignore setting can hide it here.
            if mode == b"40000" {
                queue.push_back(BytePath::from_bytes(child).hex);
            } else if mode != b"160000" {
                reader.read(common, "blob", &BytePath::from_bytes(child).hex)?;
            }
            rest = &rest[end + 1 + width..];
        }
    }
    Ok(())
}

pub fn enrich(
    inventory: &Inventory,
    targets: &[BytePath],
    forge: Option<&ForgeExport>,
) -> Result<Observations, SweepError> {
    if inventory.schema != INVENTORY_SCHEMA {
        return Err(error("unsupported inventory schema"));
    }
    for target in targets {
        full_ref(target)?;
    }
    if let Some(export) = forge {
        verify_forge(export)?;
    }
    let started_unix_ms = now_ms();
    let git = ControlledGit::default();
    let mut reader = ObjectReader::new();
    let mut items = Vec::new();
    let mut identities = BTreeSet::new();
    for family in &inventory.families {
        let common = family.common_dir.to_path()?;
        if !common.is_absolute() {
            return Err(error("common directory must be absolute"));
        }
        if !identities.insert((family.identity.device, family.identity.inode)) {
            return Err(error("duplicate inventory family"));
        }
        if reader.expired() {
            for worktree in &family.worktrees {
                items.push(ReviewItem {
                    path: worktree.path.clone(),
                    branch: worktree.branch.clone(),
                    head: worktree.head.clone(),
                    subject: None,
                    subject_truncated: false,
                    classification: "to_triage".into(),
                    observations: Vec::new(),
                    requests: Vec::new(),
                    risks: Vec::new(),
                    unknowns: vec![
                        "whole-review time budget reached; family not revalidated".into(),
                    ],
                });
            }
            continue;
        }
        // Porcelain puts the principal worktree first. A separate administration
        // need not contain a backlink, so re-enter through the recorded tree.
        // The common directory and identity are still checked below.
        let entrypoint = family
            .worktrees
            .first()
            .map(|tree| tree.path.to_path())
            .transpose()?
            .unwrap_or_else(|| common.clone());
        let current = scan_family(&entrypoint);
        let before = current.as_ref().ok().and_then(|i| i.families.first());
        let refs = local_refs(&git, &common);
        let remote_identities = forge.map(|_| remotes(&git, &common));
        let mut candidate_refs: BTreeSet<BytePath> = targets.iter().cloned().collect();
        if let Ok(refs) = &refs {
            candidate_refs.extend(refs.iter().cloned());
        }
        let mut paths = BTreeSet::new();
        for worktree in &family.worktrees {
            if !paths.insert(worktree.path.clone()) {
                return Err(error("duplicate worktree in inventory"));
            }
            let mut item = ReviewItem {
                path: worktree.path.clone(),
                branch: worktree.branch.clone(),
                head: worktree.head.clone(),
                subject: None,
                subject_truncated: false,
                classification: "to_triage".into(),
                observations: Vec::new(),
                requests: Vec::new(),
                risks: vec![
                    "integration observations do not establish preservation of local files".into(),
                ],
                unknowns: Vec::new(),
            };
            let same = before
                .filter(|f| f.identity == family.identity && f.common_dir == family.common_dir)
                .and_then(|f| f.worktrees.iter().find(|w| w.path == worktree.path))
                .is_some_and(|w| {
                    w.identity == worktree.identity
                        && w.head == worktree.head
                        && w.branch == worktree.branch
                        && w.path_state == worktree.path_state
                });
            if !same {
                item.unknowns.push(
                    "inventory identity or HEAD changed, or current family cannot be read".into(),
                );
                if let Err(e) = &current {
                    item.unknowns.push(e.to_string());
                }
                items.push(item);
                continue;
            }
            if inventory
                .local_inventories
                .iter()
                .any(|local| local.worktree == worktree.path)
            {
                item.unknowns.push("local file manifest describes the inventory interval; current file bytes are not revalidated by this review".into());
            } else {
                item.unknowns.push("local file contents and index not inventoried; scan --local for bounded observations".into());
            }
            if !matches!(worktree.path_state, DirState::Present | DirState::Absent) {
                item.unknowns.push("worktree path unavailable".into());
                items.push(item);
                continue;
            }
            if matches!(worktree.path_state, DirState::Absent) {
                item.risks.push(
                    "registered worktree directory absent; administrative state remains".into(),
                );
            }
            let Some(source) = worktree.head.as_deref() else {
                item.unknowns.push("source HEAD unknown".into());
                items.push(item);
                continue;
            };
            let source_bytes = reader.read(&common, "commit", source);
            let source_available = source_bytes.is_ok();
            match source_bytes.and_then(|raw| subject(&raw)) {
                Ok((subject, truncated)) => {
                    item.subject = Some(subject);
                    item.subject_truncated = truncated;
                }
                Err(e) => item.unknowns.push(format!("description: {e}")),
            }
            if targets.is_empty() {
                item.unknowns
                    .push("no explicitly designated integration target".into());
            }
            if let Err(e) = &refs {
                item.unknowns.push(e.to_string());
            }
            for (index, reference) in candidate_refs.iter().enumerate() {
                // Every relation requires the source commit. A failed source
                // read already makes it unknown; repeated target reads cannot
                // repair that observation and can multiply a blocked-read delay.
                // A malformed subject alone does not prevent Git judgement.
                if !source_available {
                    item.unknowns
                        .push("integration unknown: source commit unavailable".into());
                    break;
                }
                if index >= 128 || reader.expired() {
                    item.unknowns.push("target count or whole-review time budget reached; remaining targets unknown".into());
                    break;
                }
                if worktree.branch.as_ref() == Some(reference) && !targets.contains(reference) {
                    continue;
                }
                let observed_start = now_ms();
                let result = (|| {
                    let target = resolve(&git, &common, reference)?;
                    let relation = relation(&mut reader, &common, source, &target)?;
                    let again = resolve(&git, &common, reference)?;
                    if target != again {
                        return Err(error("target changed during observation"));
                    }
                    Ok((target, relation))
                })();
                match result {
                    Ok((target, Some(relation))) => {
                        item.observations.push(IntegrationObservation {
                            source_oid: source.into(),
                            target_ref: reference.clone(),
                            target_oid: target,
                            relation: relation.into(),
                            target_role: if targets.contains(reference) {
                                "integration"
                            } else {
                                "intermediate"
                            }
                            .into(),
                            started_unix_ms: observed_start,
                            finished_unix_ms: now_ms(),
                        })
                    }
                    Ok((_, None)) => {}
                    Err(e) => item.unknowns.push(format!("{}: {e}", reference.display())),
                }
            }
            if let Some(export) = forge {
                if let Some(Ok(remotes)) = &remote_identities {
                    item.requests = matching_requests(export, remotes, worktree);
                }
                if item.requests.is_empty() {
                    item.unknowns.push(
                        "no scoped forge association established; request absence unknown".into(),
                    );
                }
                item.unknowns.push("export observations do not establish current server state or absence of requests".into());
            } else {
                item.unknowns.push("forge state unknown (no export)".into());
            }
            items.push(item);
        }
        // A second read detects observed changes, but cannot establish a filesystem snapshot.
        let after = scan_family(&entrypoint);
        let stable = match (&current, &after) {
            (Ok(a), Ok(b)) => {
                serde_json::to_value(&a.families).ok() == serde_json::to_value(&b.families).ok()
            }
            _ => false,
        };
        if !stable {
            let family_paths: BTreeSet<_> = family.worktrees.iter().map(|w| &w.path).collect();
            for item in items.iter_mut().filter(|i| family_paths.contains(&i.path)) {
                item.observations.clear();
                item.requests.clear();
                item.subject = None;
                item.subject_truncated = false;
                item.unknowns
                    .push("family changed during enrichment".into());
            }
        }
    }
    items.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Observations {
        inventory: inventory.clone(),
        items,
        forge: forge.cloned(),
        started_unix_ms,
        finished_unix_ms: now_ms(),
    })
}

pub fn judge(facts: &Observations) -> Result<Review, SweepError> {
    let mut items = facts.items.clone();
    for item in &mut items {
        item.classification = if item
            .requests
            .iter()
            .any(|r| r.state == "open" || r.state == "opened")
        {
            "active"
        } else if item
            .observations
            .iter()
            .any(|o| o.target_role == "integration")
        {
            "integration_observed"
        } else {
            "to_triage"
        }
        .into();
    }
    let mut review = Review {
        schema: REVIEW_SCHEMA.into(),
        policy_version: "local-evidence/1".into(),
        inventory_sha256: canonical_digest(&facts.inventory)?,
        digest_sha256: String::new(),
        started_unix_ms: facts.started_unix_ms,
        finished_unix_ms: facts.finished_unix_ms,
        coverage: facts.inventory.coverage.clone(),
        items,
        forge: facts.forge.clone(),
        not_collected: facts.inventory.not_collected.clone(),
        local_inventories: facts.inventory.local_inventories.clone(),
    };
    review.digest_sha256 = canonical_digest(&review)?;
    Ok(review)
}

fn verify_object(kind: &str, oid: &str, bytes: &[u8]) -> Result<(), SweepError> {
    use sha2::Digest;
    let header = format!("{kind} {}\0", bytes.len());
    let computed = match oid.len() {
        40 => {
            let mut hash = sha1_checked::Sha1::new();
            hash.update(header.as_bytes());
            hash.update(bytes);
            let result = hash.try_finalize();
            if result.has_collision() {
                return Err(error("SHA-1 collision detected in Git object"));
            }
            format!("{:x}", result.hash())
        }
        64 => {
            let mut hash = sha2::Sha256::new();
            hash.update(header.as_bytes());
            hash.update(bytes);
            format!("{:x}", hash.finalize())
        }
        _ => return Err(error("unsupported object ID length")),
    };
    if !computed.eq_ignore_ascii_case(oid) {
        return Err(error("Git object bytes do not match their OID"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_git_object_bytes_must_match_their_full_oid() {
        let empty_blob = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";
        assert!(verify_object("blob", empty_blob, b"").is_ok());
        assert!(verify_object("blob", empty_blob, b"forged bytes").is_err());
    }

    #[test]
    fn exhausted_review_budgets_refuse_before_trying_a_repository_read() {
        let path = Path::new("/fixture-does-not-exist");
        let oid = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";
        for (resource, message) in [
            ("objects", "whole-review object budget reached"),
            ("bytes", "whole-review byte budget reached"),
            ("time", "whole-review time budget reached"),
        ] {
            let mut reader = ObjectReader::new();
            match resource {
                "objects" => reader.reads_left = 0,
                "bytes" => reader.bytes_left = 0,
                _ => reader.started = Instant::now() - Duration::from_secs(61),
            }
            let result = reader.read(path, "blob", oid);
            assert!(
                matches!(result, Err(e) if e.message == message),
                "{resource}"
            );
        }
    }
}
