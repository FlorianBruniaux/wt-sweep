//! Optional descriptions of recorded Git objects, separate from triage evidence.
use super::*;

type ObjectKey = (u64, u64, BytePath, String, String);
const OMISSION: &str = "context rows or detail omitted by output or row budget";

/// Relative byte paths only. No revision expressions or live-file interpretation.
pub fn validate_path(path: &BytePath, limits: &ContextLimits) -> Result<(), SweepError> {
    let raw = path.bytes()?;
    let components = raw.split(|b| *b == b'/').collect::<Vec<_>>();
    if raw.is_empty()
        || raw.len() > limits.path_bytes
        || raw.contains(&0)
        || components.len() > limits.path_components
        || components
            .iter()
            .any(|c| c.is_empty() || *c == b"." || *c == b"..")
    {
        return Err(error(
            "changelog path must be a bounded byte-relative path without empty, dot or parent components",
        ));
    }
    Ok(())
}
impl ContextOptions {
    pub fn validate(&self) -> Result<(), SweepError> {
        let l = &self.limits;
        for (value, cap) in [
            (l.object_read_attempts, 512),
            (l.object_bytes, 4 * 1024 * 1024),
            (l.traversal_steps, 16384),
            (l.per_worktree_steps, 1024),
            (l.object_output_bytes, 256 * 1024),
            (l.commit_samples, 10),
            (l.changelog_paths, 8),
            (l.path_bytes, 4096),
            (l.path_components, 32),
            (l.excerpt_bytes, 4096),
            (l.output_bytes, 1024 * 1024),
            (l.rows, 4096),
        ] {
            if value == 0 || value > cap {
                return Err(error("invalid context work-unit limit"));
            }
        }
        if l.timeout.is_zero() || l.timeout > Duration::from_secs(10) {
            return Err(error("invalid context timeout limit"));
        }
        if self.changelog_paths.len() > l.changelog_paths {
            return Err(error("context changelog path limit reached"));
        }
        for path in &self.changelog_paths {
            validate_path(path, l)?;
        }
        Ok(())
    }
}
struct Reader<'a> {
    limits: &'a ContextLimits,
    history: &'a history::HistoryRun,
    raw: &'a ObjectReader,
    hooks: &'a dyn control::ControlHooks,
    cache: BTreeMap<ObjectKey, Rc<Vec<u8>>>,
    stopped: BTreeMap<(u64, u64), GitRead>,
    started: Instant,
    reads: usize,
    bytes: usize,
    steps: usize,
    row_steps: usize,
}
impl Reader<'_> {
    fn allowed(&self, family: &Family) -> Result<(), SweepError> {
        if let Some(operation) = self
            .stopped
            .get(&(family.identity.device, family.identity.inode))
        {
            return Err(error(family_timeout_reason(operation)));
        }
        if self.hooks.cancelled() {
            return Err(error("context collection cancelled"));
        }
        if self.started.elapsed() >= self.limits.timeout {
            return Err(error("context time budget reached"));
        }
        Ok(())
    }
    fn step(&mut self, family: &Family) -> Result<(), SweepError> {
        self.allowed(family)?;
        if self.steps >= self.limits.traversal_steps
            || self.row_steps >= self.limits.per_worktree_steps
        {
            return Err(error("context traversal step budget reached"));
        }
        self.steps += 1;
        self.row_steps += 1;
        Ok(())
    }
    fn read(&mut self, family: &Family, kind: &str, oid: &str) -> Result<Rc<Vec<u8>>, SweepError> {
        self.allowed(family)?;
        let canonical_oid = oid.to_ascii_lowercase();
        let oid = canonical_oid.as_str();
        if !valid_oid(oid) {
            return Err(error("invalid context object OID"));
        }
        let key = (
            family.identity.device,
            family.identity.inode,
            family.common_dir.clone(),
            kind.into(),
            oid.into(),
        );
        let existing = self.cache.get(&key).cloned().or_else(|| {
            self.raw
                .cache
                .get(&(
                    family.identity.device,
                    family.identity.inode,
                    family.common_dir.clone(),
                    kind.into(),
                    oid.into(),
                ))
                .cloned()
        });
        if let Some(bytes) = existing {
            if bytes.len() > self.limits.object_output_bytes {
                return Err(error("context object output byte budget reached"));
            }
            return Ok(bytes);
        }
        let remaining = self.limits.timeout.saturating_sub(self.started.elapsed());
        if remaining.is_zero() {
            return Err(error("context time budget reached"));
        }
        if self.reads >= self.limits.object_read_attempts {
            return Err(error("context object attempt budget reached"));
        }
        if self.bytes >= self.limits.object_bytes {
            return Err(error("context object byte budget reached"));
        }
        let operation = match kind {
            "commit" => GitRead::Commit { oid: oid.into() },
            "tree" => GitRead::Tree { oid: oid.into() },
            "blob" => GitRead::Blob { oid: oid.into() },
            _ => return Err(error("unsupported context object kind")),
        };
        let git = ControlledGit::with_output_limit(
            remaining.min(Duration::from_secs(1)),
            self.limits
                .object_output_bytes
                .min(self.limits.object_bytes - self.bytes),
        );
        self.reads += 1;
        let result = git.read(&family.common_dir.to_path()?, operation.clone());
        if result.as_ref().is_err_and(|e| e.code == "git_timeout") {
            self.stopped
                .insert((family.identity.device, family.identity.inode), operation);
        }
        let bytes = result?;
        self.bytes += bytes.len();
        verify_object(kind, oid, &bytes)?;
        let bytes = Rc::new(bytes);
        self.cache.insert(key, bytes.clone());
        Ok(bytes)
    }
    fn metadata_read(
        &mut self,
        family: &Family,
        operation: GitRead,
    ) -> Result<Vec<u8>, SweepError> {
        self.allowed(family)?;
        if self.reads >= self.limits.object_read_attempts || self.bytes >= self.limits.object_bytes
        {
            return Err(error("context metadata read budget reached"));
        }
        let remaining = self.limits.timeout.saturating_sub(self.started.elapsed());
        let git = ControlledGit::with_output_limit(
            remaining.min(Duration::from_secs(1)),
            self.limits
                .object_output_bytes
                .min(self.limits.object_bytes - self.bytes),
        );
        self.reads += 1;
        let result = git.read(&family.common_dir.to_path()?, operation.clone());
        if result.as_ref().is_err_and(|e| e.code == "git_timeout") {
            self.stopped
                .insert((family.identity.device, family.identity.inode), operation);
        }
        // Never retain Git stderr here: malformed config errors can quote credential URLs.
        let bytes =
            result.map_err(|e| error(format!("remote metadata unavailable ({})", e.code)))?;
        self.bytes += bytes.len();
        Ok(bytes)
    }
    fn remote_snapshot(
        &mut self,
        family: &Family,
        items: &[ReviewItem],
    ) -> crate::forge_links::Snapshot {
        let mut snapshot = crate::forge_links::Snapshot::default();
        snapshot.started_unix_ms = now_ms();
        let hints = items
            .iter()
            .filter(|i| family.worktrees.iter().any(|w| w.path == i.path))
            .flat_map(|i| &i.requests)
            .filter(|r| crate::forge_links::request_url(r).is_some())
            .filter_map(|r| {
                r.host
                    .as_ref()
                    .map(|h| (h.to_ascii_lowercase(), r.provider.clone()))
            })
            .collect();
        match self.metadata_read(family, GitRead::RemoteUrls) {
            Ok(raw) => {
                snapshot.parse_remotes(&raw, &hints);
                if snapshot.has_remotes() {
                    match self.metadata_read(family, GitRead::TrackingRefs) {
                        Ok(raw) => {
                            if let Err(reason) = snapshot.parse_refs(&raw) {
                                snapshot.gaps.push(reason.into());
                            }
                        }
                        Err(e) => snapshot.gaps.push(e.message),
                    }
                }
            }
            Err(e) => snapshot.gaps.push(e.message),
        }
        snapshot.finished_unix_ms = now_ms();
        snapshot
    }
    fn commit(&mut self, family: &Family, oid: &str) -> Result<Commit, SweepError> {
        // A parsed history cache entry cannot prove the raw structural framing.
        // Existing verified raw bytes are reused; otherwise this spends only context budgets.
        let bytes = self.read(family, "commit", oid)?;
        let parsed = strict_commit(&bytes, oid.len(), self.limits.per_worktree_steps)?;
        if let Some(Ok(cached)) = self.history.cached_commit(family, oid)
            && (!cached.tree.eq_ignore_ascii_case(&parsed.tree)
                || cached.parents.len() != parsed.parents.len()
                || cached
                    .parents
                    .iter()
                    .zip(&parsed.parents)
                    .any(|(a, b)| !a.eq_ignore_ascii_case(b)))
        {
            return Err(error(
                "cached commit structure contradicts verified raw context",
            ));
        }
        Ok(parsed)
    }
    fn closure(
        &mut self,
        family: &Family,
        roots: &[String],
    ) -> Result<BTreeSet<String>, SweepError> {
        let mut seen = BTreeSet::new();
        let mut queued = roots
            .iter()
            .map(|oid| oid.to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        if queued.len() > self.limits.per_worktree_steps {
            return Err(error("context graph queue budget reached"));
        }
        let mut queue = VecDeque::from_iter(queued.iter().cloned());
        while let Some(oid) = queue.pop_front() {
            self.step(family)?;
            seen.insert(oid.clone());
            let commit = self.commit(family, &oid)?;
            for parent in commit.parents {
                if queued.insert(parent.clone()) {
                    if queued.len() > self.limits.per_worktree_steps {
                        return Err(error("context graph queue budget reached"));
                    }
                    queue.push_back(parent);
                }
            }
        }
        Ok(seen)
    }
    fn uncovered(
        &mut self,
        family: &Family,
        item: &ReviewItem,
        targets: &[BytePath],
    ) -> UncoveredCommits {
        let mut output = UncoveredCommits {
            targets: targets
                .iter()
                .map(|r| ContextTarget {
                    reference: r.clone(),
                    oid: item
                        .history
                        .iter()
                        .find(|h| h.target_ref == *r)
                        .and_then(|h| h.target_oid.clone()),
                })
                .collect(),
            observed: 0,
            complete: false,
            samples: Vec::new(),
            samples_omitted: 0,
            gaps: Vec::new(),
        };
        let result = (|| -> Result<(), SweepError> {
            self.allowed(family)?;
            let recorded_head = item
                .head
                .as_deref()
                .filter(|h| valid_oid(h))
                .ok_or_else(|| error("recorded HEAD unavailable"))?;
            let canonical_head = recorded_head.to_ascii_lowercase();
            let head = canonical_head.as_str();
            if targets.is_empty() {
                return Err(error(
                    "no explicitly selected targets; graph noncoverage unknown",
                ));
            }
            if item.history.iter().any(|h| {
                targets.contains(&h.target_ref)
                    && h.source_oid
                        .as_deref()
                        .is_some_and(|source| source.eq_ignore_ascii_case(head))
                    && h.target_oid.as_deref().is_some_and(valid_oid)
                    && h.stop_reason.is_none()
                    && h.observed_result.is_none()
                    && matches!(h.result.as_str(), "ancestor" | "identical_commit")
            }) {
                output.complete = true;
                return Ok(());
            }
            if item.history_omitted_targets > 0
                || targets.iter().any(|r| {
                    item.history
                        .iter()
                        .find(|h| h.target_ref == *r)
                        .is_none_or(|h| {
                            h.source_oid
                                .as_deref()
                                .is_none_or(|source| !source.eq_ignore_ascii_case(head))
                                || h.target_oid.as_deref().is_none_or(|o| !valid_oid(o))
                                || h.result == "unknown"
                                || h.observed_result.is_some()
                                || h.stop_reason.is_some()
                        })
                })
            {
                return Err(error(
                    "selected target missing, incomplete or invalidated; graph noncoverage unknown",
                ));
            }
            let roots = output
                .targets
                .iter()
                .filter_map(|t| t.oid.clone())
                .collect::<Vec<_>>();
            let covered = self.closure(family, &roots)?;
            let mut queued = BTreeSet::from([head.to_string()]);
            let mut queue = VecDeque::from([head.to_string()]);
            while let Some(oid) = queue.pop_front() {
                if covered.contains(&oid) {
                    continue;
                }
                self.step(family)?;
                let commit = self.commit(family, &oid)?;
                output.observed += 1;
                if output.samples.len() < self.limits.commit_samples {
                    let mut sample = CommitContext {
                        oid: oid.clone(),
                        subject: None,
                        subject_truncated: false,
                    };
                    match self
                        .read(family, "commit", &oid)
                        .and_then(|raw| subject(&raw))
                    {
                        Ok((subject, truncated)) => {
                            sample.subject = Some(subject);
                            sample.subject_truncated = truncated;
                        }
                        Err(_) => {
                            if !output
                                .gaps
                                .iter()
                                .any(|g| g == "some verified commit subjects unavailable")
                            {
                                output
                                    .gaps
                                    .push("some verified commit subjects unavailable".into());
                            }
                        }
                    }
                    output.samples.push(sample);
                } else {
                    output.samples_omitted += 1;
                }
                for parent in commit.parents {
                    if !covered.contains(&parent) && queued.insert(parent.clone()) {
                        if queued.len() > self.limits.per_worktree_steps {
                            return Err(error("context graph queue budget reached"));
                        }
                        queue.push_back(parent);
                    }
                }
            }
            output.complete = true;
            Ok(())
        })();
        if let Err(e) = result {
            output.gaps.push(e.message);
        }
        output
    }
    fn changelog(
        &mut self,
        family: &Family,
        head: Option<&str>,
        path: &BytePath,
    ) -> ChangelogContext {
        let mut output = ChangelogContext {
            revision_oid: head.unwrap_or_default().into(),
            path: path.clone(),
            tree_oid: None,
            blob_oid: None,
            status: "unknown".into(),
            blob_bytes: None,
            excerpt: None,
            excerpt_truncated: false,
            gaps: Vec::new(),
        };
        let result = (|| -> Result<(), SweepError> {
            let head = head
                .filter(|h| valid_oid(h))
                .ok_or_else(|| error("recorded HEAD unavailable"))?;
            let mut tree = self.commit(family, head)?.tree;
            output.tree_oid = Some(tree.clone());
            let raw_path = path.bytes()?;
            let parts = raw_path.split(|b| *b == b'/').collect::<Vec<_>>();
            for (index, part) in parts.iter().enumerate() {
                self.step(family)?;
                let raw = self.read(family, "tree", &tree)?;
                let found = tree_entry(&raw, head.len() / 2, part)?;
                let Some((mode, child)) = found else {
                    output.status = "absent".into();
                    return Ok(());
                };
                if index + 1 < parts.len() {
                    if mode != b"40000" {
                        return Err(error(
                            "changelog intermediate component is not a tree; symlink or gitlink not followed",
                        ));
                    }
                    tree = child;
                } else {
                    if mode != b"100644" && mode != b"100755" {
                        return Err(error(
                            "changelog final mode refused; symlink or gitlink not followed",
                        ));
                    }
                    output.blob_oid = Some(child.clone());
                    let blob = self.read(family, "blob", &child)?;
                    output.blob_bytes = Some(blob.len());
                    if blob.contains(&0) {
                        return Err(error(
                            "changelog blob contains NUL; binary excerpt unavailable",
                        ));
                    }
                    output.excerpt = Some(BytePath::from_bytes(
                        &blob[..blob.len().min(self.limits.excerpt_bytes)],
                    ));
                    output.excerpt_truncated = blob.len() > self.limits.excerpt_bytes;
                    output.status = "present".into();
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            output.gaps.push(e.message);
        }
        output
    }
}
// Structural framing only: author identity/date semantics and signature validity
// remain outside this optional description parser.
fn strict_commit(raw: &[u8], width: usize, parent_limit: usize) -> Result<Commit, SweepError> {
    parse_commit_bounded(raw, width, parent_limit)
}

// Parse the entire verified tree before trusting presence or absence of a component.
fn tree_entry(
    raw: &[u8],
    width: usize,
    name: &[u8],
) -> Result<Option<(Vec<u8>, String)>, SweepError> {
    let mut rest = raw;
    let mut names = BTreeSet::new();
    let mut found = None;
    while !rest.is_empty() {
        let end = rest
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| error("malformed context tree entry"))?;
        let header = &rest[..end];
        let split = header
            .iter()
            .position(|b| *b == b' ')
            .ok_or_else(|| error("malformed context tree mode"))?;
        let mode = &header[..split];
        let entry_name = &header[split + 1..];
        if ![
            b"40000".as_slice(),
            b"100644",
            b"100755",
            b"120000",
            b"160000",
        ]
        .contains(&mode)
            || entry_name.is_empty()
            || entry_name.contains(&b'/')
            || entry_name == b"."
            || entry_name == b".."
            || !names.insert(entry_name)
            || rest.len() < end + 1 + width
        {
            return Err(error("invalid or truncated context tree entry"));
        }
        let oid = BytePath::from_bytes(&rest[end + 1..end + 1 + width]).hex;
        if !valid_oid(&oid) {
            return Err(error("invalid context tree child OID"));
        }
        if entry_name == name {
            found = Some((mode.to_vec(), oid));
        }
        rest = &rest[end + 1 + width..];
    }
    Ok(found)
}

pub(super) fn collect(
    inventory: &Inventory,
    items: &[ReviewItem],
    targets: &FamilyTargets,
    history: &history::HistoryRun,
    raw: &ObjectReader,
    options: &ContextOptions,
    hooks: &dyn control::ControlHooks,
) -> Result<ReviewContext, SweepError> {
    let mut output = ReviewContext {
        started_unix_ms: now_ms(),
        finished_unix_ms: u64::MAX,
        items: Vec::new(),
        omitted_items: items.len(),
        gaps: vec![OMISSION.into()],
    };
    let mut charged = serde_json::to_vec(&output)
        .map_err(|e| error(e.to_string()))?
        .len();
    if charged > options.limits.output_bytes {
        return Err(error(
            "context output cap cannot hold essential omission metadata",
        ));
    }
    let mut reader = Reader {
        limits: &options.limits,
        history,
        raw,
        hooks,
        cache: BTreeMap::new(),
        stopped: history.stopped_families.clone(),
        started: Instant::now(),
        reads: 0,
        bytes: 0,
        steps: 0,
        row_steps: 0,
    };
    let mut remote_snapshots = BTreeMap::new();
    let mut detail_omitted = false;
    for item in items {
        if output.items.len() >= options.limits.rows {
            break;
        }
        let Some(family) = inventory
            .families
            .iter()
            .find(|f| f.worktrees.iter().any(|w| w.path == item.path))
        else {
            continue;
        };
        reader.row_steps = 0;
        let mut row = WorktreeContext {
            remote: None,
            path: item.path.clone(),
            common_dir: family.common_dir.clone(),
            family_identity: family.identity.clone(),
            head_oid: item.head.clone(),
            uncovered: UncoveredCommits {
                targets: targets
                    .for_family(family)
                    .iter()
                    .map(|reference| ContextTarget {
                        reference: reference.clone(),
                        oid: item
                            .history
                            .iter()
                            .find(|h| h.target_ref == *reference)
                            .and_then(|h| h.target_oid.clone()),
                    })
                    .collect(),
                observed: 0,
                complete: false,
                samples: Vec::new(),
                samples_omitted: 0,
                gaps: Vec::new(),
            },
            changelogs: Vec::new(),
            gaps: Vec::new(),
        };
        let separator = usize::from(!output.items.is_empty());
        let size = |row: &WorktreeContext| -> Result<usize, SweepError> {
            Ok(serde_json::to_vec(row)
                .map_err(|e| error(e.to_string()))?
                .len()
                + separator)
        };
        // Reserve essential row provenance before any optional object read.
        if charged + size(&row)? > options.limits.output_bytes {
            break;
        }
        if !remote_snapshots.contains_key(&family.common_dir) {
            remote_snapshots.insert(
                family.common_dir.clone(),
                reader.remote_snapshot(family, items),
            );
        }
        row.remote = remote_snapshots
            .get(&family.common_dir)
            .map(|s| s.for_worktree(item.branch.as_ref(), item.head.as_deref()));
        if charged + size(&row)? > options.limits.output_bytes {
            row.remote = None;
            row.gaps
                .push("remote metadata omitted by output budget".into());
            detail_omitted = true;
            if charged + size(&row)? > options.limits.output_bytes {
                break;
            }
        }
        row.uncovered = reader.uncovered(family, item, targets.for_family(family));
        if charged + size(&row)? > options.limits.output_bytes {
            row.uncovered.samples_omitted += row.uncovered.samples.len();
            row.uncovered.samples.clear();
            row.gaps
                .push("commit samples omitted by output budget".into());
            detail_omitted = true;
            if charged + size(&row)? > options.limits.output_bytes {
                break;
            }
        }
        for (index, path) in options.changelog_paths.iter().enumerate() {
            row.changelogs
                .push(reader.changelog(family, item.head.as_deref(), path));
            if charged + size(&row)? > options.limits.output_bytes {
                for changelog in &mut row.changelogs {
                    if changelog.excerpt.take().is_some() {
                        changelog.excerpt_truncated = true;
                    }
                }
                if index + 1 < options.changelog_paths.len() {
                    row.gaps
                        .push("remaining changelog paths omitted by output budget".into());
                }
                detail_omitted = true;
                break;
            }
        }
        let mut bytes = serde_json::to_vec(&row)
            .map_err(|e| error(e.to_string()))?
            .len()
            + usize::from(!output.items.is_empty());
        if charged + bytes > options.limits.output_bytes {
            let count = row.uncovered.samples.len();
            row.uncovered.samples.clear();
            row.uncovered.samples_omitted += count;
            for changelog in &mut row.changelogs {
                if changelog.excerpt.take().is_some() {
                    changelog.excerpt_truncated = true;
                }
            }
            row.gaps
                .push("context sample or excerpt detail omitted by output budget".into());
            bytes = serde_json::to_vec(&row)
                .map_err(|e| error(e.to_string()))?
                .len()
                + usize::from(!output.items.is_empty());
            detail_omitted = true;
        }
        if charged + bytes > options.limits.output_bytes {
            break;
        }
        charged += bytes;
        output.items.push(row);
        output.omitted_items -= 1;
    }
    output.finished_unix_ms = now_ms();
    if output.omitted_items == 0 && !detail_omitted {
        output.gaps.clear();
    }
    if serde_json::to_vec(&output)
        .map_err(|e| error(e.to_string()))?
        .len()
        > options.limits.output_bytes
    {
        return Err(error("context serialization exceeds output cap"));
    }
    Ok(output)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::git::take_child_read_journal;
    use crate::local::{LocalHooks, NativeHooks};
    use std::{
        fs,
        path::PathBuf,
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = PathBuf::from(format!(
                "/private/tmp/wt-sweep-context-child-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            let f = Self(path);
            f.git(&["init", "-b", "main"]);
            fs::write(f.0.join("CHANGELOG.md"), b"synthetic child evidence\n").unwrap();
            f.git(&["add", "."]);
            f.git(&["commit", "-m", "synthetic"]);
            f
        }
        fn git(&self, args: &[&str]) -> String {
            let out = Command::new("/usr/bin/git")
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_AUTHOR_NAME", "Fixture")
                .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
                .env("GIT_COMMITTER_NAME", "Fixture")
                .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
                .arg("-C")
                .arg(&self.0)
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout).unwrap().trim().into()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn reader<'a>(
        history: &'a history::HistoryRun,
        raw: &'a ObjectReader,
        limits: &'a ContextLimits,
        hooks: &'a dyn control::ControlHooks,
    ) -> Reader<'a> {
        Reader {
            limits,
            history,
            raw,
            hooks,
            cache: BTreeMap::new(),
            stopped: history.stopped_families.clone(),
            started: Instant::now(),
            reads: 0,
            bytes: 0,
            steps: 0,
            row_steps: 0,
        }
    }
    #[test]
    fn typed_context_timeout_spawns_no_later_family_child_but_healthy_family_continues() {
        let blocked = Fixture::new();
        let healthy = Fixture::new();
        let mut inventory = scan_family_with_git(&blocked.0, &ControlledGit::default()).unwrap();
        inventory.families.extend(
            scan_family_with_git(&healthy.0, &ControlledGit::default())
                .unwrap()
                .families,
        );
        let targets = FamilyTargets::bind(&inventory, TargetInput::Simple(&[])).unwrap();
        let history = history::HistoryRun::collect(
            &inventory,
            &targets,
            &ReviewLimits::default(),
            &BTreeMap::new(),
            &mut |_| {},
        )
        .unwrap();
        let raw = ObjectReader::new();
        let limits = ContextLimits::default();
        let mut reader = reader(&history, &raw, &limits, &NativeHooks);
        let tree = blocked.git(&["rev-parse", "HEAD^{tree}"]);
        let object = blocked
            .0
            .join(".git/objects")
            .join(&tree[..2])
            .join(&tree[2..]);
        let original = fs::read(&object).unwrap();
        fs::remove_file(&object).unwrap();
        assert!(
            Command::new("/usr/bin/mkfifo")
                .env_clear()
                .arg(&object)
                .status()
                .unwrap()
                .success()
        );
        let blocked_head = blocked.git(&["rev-parse", "HEAD"]);
        let healthy_head = healthy.git(&["rev-parse", "HEAD"]);
        take_child_read_journal();
        let failed = reader.changelog(
            &inventory.families[0],
            Some(&blocked_head),
            &BytePath::from_bytes(b"CHANGELOG.md"),
        );
        let first = take_child_read_journal();
        assert_eq!(failed.status, "unknown");
        assert!(reader.stopped.contains_key(&(
            inventory.families[0].identity.device,
            inventory.families[0].identity.inode
        )));
        assert!(!failed.gaps.is_empty());
        assert!(
            first
                .iter()
                .any(|(_, op)| matches!(op, GitRead::Tree { .. }))
        );
        fs::remove_file(&object).unwrap();
        fs::write(&object, original).unwrap();
        assert_eq!(
            reader
                .changelog(
                    &inventory.families[0],
                    Some(&blocked_head),
                    &BytePath::from_bytes(b"CHANGELOG.md")
                )
                .status,
            "unknown"
        );
        assert!(
            take_child_read_journal().is_empty(),
            "quarantined family retried a child"
        );
        let healthy = reader.changelog(
            &inventory.families[1],
            Some(&healthy_head),
            &BytePath::from_bytes(b"CHANGELOG.md"),
        );
        assert_eq!(healthy.status, "present", "{:?}", healthy.gaps);
        let journal = take_child_read_journal();
        assert!(!journal.is_empty());
        assert!(
            journal
                .iter()
                .all(|(path, _)| *path == inventory.families[1].common_dir)
        );
    }
    struct Cancelled;
    impl LocalHooks for Cancelled {
        fn cancelled(&self) -> bool {
            true
        }
    }
    impl control::ControlHooks for Cancelled {}
    #[test]
    fn already_stopped_and_cancelled_context_spawn_zero_children() {
        let f = Fixture::new();
        let inventory = scan_family_with_git(&f.0, &ControlledGit::default()).unwrap();
        let targets = FamilyTargets::bind(&inventory, TargetInput::Simple(&[])).unwrap();
        let history = history::HistoryRun::collect(
            &inventory,
            &targets,
            &ReviewLimits::default(),
            &BTreeMap::new(),
            &mut |_| {},
        )
        .unwrap();
        let raw = ObjectReader::new();
        let limits = ContextLimits::default();
        let head = f.git(&["rev-parse", "HEAD"]);
        let family = &inventory.families[0];
        for cancelled in [false, true] {
            let hooks: &dyn control::ControlHooks =
                if cancelled { &Cancelled } else { &NativeHooks };
            let mut reader = reader(&history, &raw, &limits, hooks);
            if !cancelled {
                reader.stopped.insert(
                    (family.identity.device, family.identity.inode),
                    GitRead::Tree { oid: head.clone() },
                );
            }
            take_child_read_journal();
            assert_eq!(
                reader
                    .changelog(family, Some(&head), &BytePath::from_bytes(b"CHANGELOG.md"))
                    .status,
                "unknown"
            );
            let remote = reader.remote_snapshot(family, &[]);
            assert!(!remote.has_remotes());
            assert!(!remote.gaps.is_empty());
            assert!(take_child_read_journal().is_empty());
        }
    }
    #[test]
    fn graph_zero_shortcut_is_cache_only_and_does_not_spend_evidence_budget() {
        let f = Fixture::new();
        let inventory = scan_family_with_git(&f.0, &ControlledGit::default()).unwrap();
        let refs = [
            BytePath::from_bytes(b"refs/heads/absent"),
            BytePath::from_bytes(b"refs/heads/main"),
        ];
        let targets = FamilyTargets::bind(&inventory, TargetInput::Simple(&refs)).unwrap();
        let history = history::HistoryRun::collect(
            &inventory,
            &targets,
            &ReviewLimits::default(),
            &BTreeMap::new(),
            &mut |_| {},
        )
        .unwrap();
        let mut options = ReviewOptions::default();
        options.context.enabled = false;
        let facts = enrich_with_options(
            &inventory,
            TargetInput::Simple(&refs),
            None,
            &options,
            &NativeHooks,
            &mut |_| {},
        )
        .unwrap();
        let before = serde_json::to_value(&history.metrics).unwrap();
        let raw = ObjectReader::new();
        let limits = ContextLimits::default();
        let mut reader = reader(&history, &raw, &limits, &NativeHooks);
        take_child_read_journal();
        let output = reader.uncovered(&inventory.families[0], &facts.items[0], &refs);
        assert!(output.complete);
        assert_eq!(output.observed, 0);
        assert_eq!(reader.reads, 0);
        assert_eq!(reader.steps, 0);
        assert!(take_child_read_journal().is_empty());
        assert_eq!(serde_json::to_value(&history.metrics).unwrap(), before);
    }
    #[test]
    fn output_cap_below_a_row_stops_before_any_optional_child() {
        let f = Fixture::new();
        let inventory = scan_family_with_git(&f.0, &ControlledGit::default()).unwrap();
        let refs = [BytePath::from_bytes(b"refs/heads/main")];
        let targets = FamilyTargets::bind(&inventory, TargetInput::Simple(&refs)).unwrap();
        let history = history::HistoryRun::collect(
            &inventory,
            &targets,
            &ReviewLimits::default(),
            &BTreeMap::new(),
            &mut |_| {},
        )
        .unwrap();
        let mut options = ReviewOptions::default();
        options.context.enabled = false;
        let facts = enrich_with_options(
            &inventory,
            TargetInput::Simple(&refs),
            None,
            &options,
            &NativeHooks,
            &mut |_| {},
        )
        .unwrap();
        let raw = ObjectReader::new();
        options.context.limits.output_bytes = 350;
        take_child_read_journal();
        let output = collect(
            &inventory,
            &facts.items,
            &targets,
            &history,
            &raw,
            &options.context,
            &NativeHooks,
        )
        .unwrap();
        assert_eq!(output.omitted_items, 1);
        assert!(output.items.is_empty());
        assert!(
            take_child_read_journal().is_empty(),
            "output budget caused needless optional child reads"
        );
    }
    #[test]
    fn refused_history_cache_without_raw_proof_spends_optional_read_and_rejects_malformed_header() {
        let f = Fixture::new();
        let base = f.git(&["rev-parse", "HEAD"]);
        let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
        f.git(&["update-ref", "refs/heads/retained", &base]);
        fs::write(f.0.join(".git/context-raw"), format!("tree {tree}\nparent\t{base}\nauthor Fixture <fixture@example.invalid> 1 +0000\ncommitter Fixture <fixture@example.invalid> 1 +0000\n\nMalformed fallback\n")).unwrap();
        let oid = f.git(&[
            "hash-object",
            "--literally",
            "-t",
            "commit",
            "-w",
            ".git/context-raw",
        ]);
        fs::write(f.0.join(".git/refs/heads/main"), format!("{oid}\n")).unwrap();
        let inventory = scan_family_with_git(&f.0, &ControlledGit::default()).unwrap();
        let refs = [BytePath::from_bytes(b"refs/heads/retained")];
        let targets = FamilyTargets::bind(&inventory, TargetInput::Simple(&refs)).unwrap();
        let history = history::HistoryRun::collect(
            &inventory,
            &targets,
            &ReviewLimits::default(),
            &BTreeMap::new(),
            &mut |_| {},
        )
        .unwrap();
        let family = &inventory.families[0];
        assert!(
            history
                .cached_commit(family, &oid)
                .is_some_and(Result::is_err)
        );
        let raw = ObjectReader::new();
        let limits = ContextLimits::default();
        let mut reader = reader(&history, &raw, &limits, &NativeHooks);
        take_child_read_journal();
        assert!(reader.commit(family, &oid).is_err());
        let journal = take_child_read_journal();
        assert_eq!(journal.len(), 1);
        assert!(matches!(&journal[0].1, GitRead::Commit { oid: read } if read == &oid));
        assert_eq!(reader.reads, 1);
        assert!(reader.commit(family, &oid).is_err());
        assert!(
            take_child_read_journal().is_empty(),
            "verified raw cache should reject without another child"
        );
    }
}
