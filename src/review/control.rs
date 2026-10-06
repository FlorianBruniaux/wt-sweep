//! Finite, descriptor-anchored current controls. No operation-content traversal.
use super::*;
use crate::local::{LocalHooks, NativeHooks};
use rustix::fs::{AtFlags, Mode, OFlags, Stat, fstat, openat, statat};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::path::{Component, PathBuf};

pub trait ControlHooks: LocalHooks {
    fn before_final_controls(&self) -> io::Result<()> {
        Ok(())
    }
}
impl ControlHooks for NativeHooks {}
#[derive(Clone, Debug)]
pub struct ControlLimits {
    pub source_entries: usize,
    pub file_bytes: usize,
    pub source_bytes: usize,
    pub output_bytes: usize,
    pub timeout: Duration,
}
impl Default for ControlLimits {
    fn default() -> Self {
        Self {
            source_entries: 4096,
            file_bytes: 64 * 1024,
            source_bytes: 4 * 1024 * 1024,
            output_bytes: 4 * 1024 * 1024,
            timeout: Duration::from_secs(60),
        }
    }
}
impl ControlLimits {
    fn validate(&self) -> Result<(), SweepError> {
        let d = Self::default();
        if [
            (self.source_entries, d.source_entries),
            (self.file_bytes, d.file_bytes),
            (self.source_bytes, d.source_bytes),
            (self.output_bytes, d.output_bytes),
        ]
        .iter()
        .any(|(v, max)| *v == 0 || v > max)
            || self.timeout.is_zero()
            || self.timeout > d.timeout
        {
            return Err(error("invalid control work-unit limits"));
        }
        Ok(())
    }
}
type FamilyId = (u64, u64);
const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);
#[derive(Clone)]
struct Probe {
    path: PathBuf,
    directory: bool,
    kind: String,
    stat: Option<Stat>,
    hash: Option<String>,
    complete: bool,
}
#[derive(Clone)]
struct Failure {
    changed: bool,
    reason: String,
}
impl From<io::Error> for Failure {
    fn from(e: io::Error) -> Self {
        Self {
            changed: false,
            reason: e.to_string(),
        }
    }
}
impl From<rustix::io::Errno> for Failure {
    fn from(e: rustix::io::Errno) -> Self {
        Self {
            changed: false,
            reason: e.to_string(),
        }
    }
}
fn unavailable(reason: impl Into<String>) -> Failure {
    Failure {
        changed: false,
        reason: reason.into(),
    }
}
fn changed(reason: impl Into<String>) -> Failure {
    Failure {
        changed: true,
        reason: reason.into(),
    }
}
fn same_stat(a: &Stat, b: &Stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_mode == b.st_mode
        && a.st_size == b.st_size
        && a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
        && a.st_ctime == b.st_ctime
        && a.st_ctime_nsec == b.st_ctime_nsec
        && a.st_uid == b.st_uid
        && a.st_gid == b.st_gid
        && a.st_nlink == b.st_nlink
}
fn identity(s: &Stat) -> Identity {
    Identity {
        device: s.st_dev as u64,
        inode: s.st_ino,
    }
}
fn parse_path(raw: &[u8]) -> Result<PathBuf, SweepError> {
    let b = raw
        .strip_suffix(b"\n")
        .filter(|b| !b.is_empty() && !b.contains(&0))
        .ok_or_else(|| error("invalid control administration path"))?;
    let p = BytePath::from_bytes(b).to_path()?;
    if !p.is_absolute() {
        return Err(error("control administration is not absolute"));
    }
    Ok(p)
}
pub(super) struct ControlRun<'a> {
    pub states: BTreeMap<BytePath, WorktreeState>,
    pub initial: BTreeMap<FamilyId, Result<Family, SweepError>>,
    pub final_families: BTreeMap<FamilyId, Result<Family, SweepError>>,
    pub stopped_families: BTreeMap<FamilyId, GitRead>,
    probes: BTreeMap<BytePath, Vec<Probe>>,
    entrypoints: BTreeMap<FamilyId, PathBuf>,
    hooks: &'a dyn ControlHooks,
    limits: ControlLimits,
    started: Instant,
    active: Duration,
    bytes: usize,
    entries: usize,
    output: usize,
}
impl<'a> ControlRun<'a> {
    fn remaining(&self) -> Duration {
        self.limits
            .timeout
            .saturating_sub(self.active + self.started.elapsed())
    }
    fn check(&self) -> Result<(), Failure> {
        if self.remaining().is_zero() || self.hooks.cancelled() {
            Err(unavailable("control time budget or cancellation"))
        } else {
            Ok(())
        }
    }
    fn parent(&self, path: &Path) -> Result<Option<OwnedFd>, Failure> {
        if !path.is_absolute() {
            return Err(unavailable("control path not absolute"));
        }
        let mut fd = rustix::fs::open("/", DIRECTORY, Mode::empty())?;
        let mut current = PathBuf::from("/");
        for c in path.components() {
            self.check()?;
            match c {
                Component::RootDir | Component::CurDir => {}
                Component::Normal(name) => {
                    current.push(name);
                    let s = match statat(&fd, name, AtFlags::SYMLINK_NOFOLLOW) {
                        Ok(s) => s,
                        Err(e) if e == rustix::io::Errno::NOENT => return Ok(None),
                        Err(e) => return Err(e.into()),
                    };
                    if self.hooks.is_dataless(&BytePath::from_path(&current), &s) {
                        return Err(unavailable("nonresident control directory refused"));
                    }
                    let next = openat(&fd, name, DIRECTORY, Mode::empty())?;
                    if !same_stat(&s, &fstat(&next)?) {
                        return Err(changed("control parent changed"));
                    }
                    fd = next;
                }
                _ => return Err(unavailable("unsafe control path component")),
            }
        }
        Ok(Some(fd))
    }
    fn observe(
        &mut self,
        p: &Path,
        directory: bool,
        present: &mut bool,
    ) -> Result<(Option<Stat>, Option<String>), Failure> {
        self.check()?;
        self.hooks.before_entry(&BytePath::from_path(p))?;
        let parent = p
            .parent()
            .ok_or_else(|| unavailable("control parent missing"))?;
        let Some(fd) = self.parent(parent)? else {
            return Ok((None, None));
        };
        let name = p
            .file_name()
            .ok_or_else(|| unavailable("control name missing"))?;
        let s = match statat(&fd, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(s) => s,
            Err(e) if e == rustix::io::Errno::NOENT => return Ok((None, None)),
            Err(e) => return Err(e.into()),
        };
        if self.hooks.is_dataless(&BytePath::from_path(p), &s) {
            return Err(unavailable("nonresident control marker refused"));
        }
        if s.st_mode & 0o170000 != if directory { 0o040000 } else { 0o100000 } {
            return Err(unavailable("control marker unexpected type or symlink"));
        }
        *present = true;
        let opened = openat(
            &fd,
            name,
            if directory {
                DIRECTORY
            } else {
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC
            },
            Mode::empty(),
        )?;
        self.hooks.after_file_open(&BytePath::from_path(p))?;
        if !same_stat(&s, &fstat(&opened)?) {
            return Err(changed("control marker changed while opening"));
        }
        let hash = if directory {
            None
        } else {
            let size = usize::try_from(s.st_size)
                .map_err(|_| unavailable("invalid control marker size"))?;
            let cap = self
                .limits
                .file_bytes
                .min(self.limits.source_bytes.saturating_sub(self.bytes));
            if size > cap {
                return Err(unavailable("control source byte budget reached"));
            }
            let mut raw = Vec::new();
            File::from(opened.try_clone()?)
                .take(cap.saturating_add(1) as u64)
                .read_to_end(&mut raw)?;
            self.bytes = self.bytes.saturating_add(raw.len());
            if raw.len() > cap {
                return Err(unavailable("control source byte budget reached"));
            }
            Some(format!("{:x}", Sha256::digest(&raw)))
        };
        if !same_stat(&s, &fstat(&opened)?)
            || !same_stat(&s, &statat(&fd, name, AtFlags::SYMLINK_NOFOLLOW)?)
        {
            return Err(changed("control marker changed during read"));
        }
        Ok((Some(s), hash))
    }
    fn probe(
        &mut self,
        path: &Path,
        directory: bool,
        kind: &str,
        state: &mut WorktreeState,
    ) -> bool {
        // Reserve the actual encoded source and its possible positive path before reading.
        let template = HistorySource {
            path: BytePath::from_path(path),
            kind: kind.into(),
            exists: Some(false),
            sha256: Some("f".repeat(64)),
            bytes: Some(u64::MAX),
            complete: false,
        };
        let charge = serde_json::to_vec(&template)
            .map(|b| b.len())
            .unwrap_or(usize::MAX)
            .saturating_add(
                serde_json::to_vec(&template.path)
                    .map(|b| b.len())
                    .unwrap_or(usize::MAX),
            )
            .saturating_add(2);
        let pending = serde_json::to_vec(state)
            .map(|b| b.len())
            .unwrap_or(usize::MAX);
        if self.entries >= self.limits.source_entries
            || pending.saturating_add(charge).saturating_add(1)
                > self.limits.output_bytes.saturating_sub(self.output)
        {
            if !state
                .gaps
                .iter()
                .any(|s| s == "control source entry/output budget reached")
            {
                state
                    .gaps
                    .push("control source entry/output budget reached".into());
            }
            return false;
        }
        self.entries += 1;
        let mut present = false;
        let result = self.observe(path, directory, &mut present);
        let mut source = HistorySource {
            path: BytePath::from_path(path),
            kind: kind.into(),
            exists: None,
            sha256: None,
            bytes: None,
            complete: false,
        };
        let mut saved = Probe {
            path: path.into(),
            directory,
            kind: kind.into(),
            stat: None,
            hash: None,
            complete: false,
        };
        match result {
            Ok((s, h)) => {
                source.exists = Some(s.is_some());
                source.sha256 = h.clone();
                source.bytes = s.filter(|_| !directory).map(|s| s.st_size as u64);
                source.complete = true;
                saved.stat = s;
                saved.hash = h;
                saved.complete = true;
            }
            Err(e) => {
                state
                    .gaps
                    .push(format!("{}: {}", source.path.display(), e.reason));
                if e.changed {
                    state.checks = "change_observed".into();
                }
            }
        }
        state.sources.push(source);
        self.probes
            .entry(
                state
                    .observed
                    .as_ref()
                    .map(|w| w.path.clone())
                    .unwrap_or_else(|| BytePath::from_path(path)),
            )
            .or_default()
            .push(saved);
        present
    }
    fn scan(&mut self, family: &Family) -> Result<Family, SweepError> {
        self.check().map_err(|e| error(e.reason))?;
        let key = (family.identity.device, family.identity.inode);
        let path = if let Some(path) = self.entrypoints.get(&key) {
            path.clone()
        } else {
            let mut fallback = None;
            let mut primary = None;
            // Choose by the current administration relationship, never inventory order.
            for w in family
                .worktrees
                .iter()
                .filter(|w| matches!(w.path_state, DirState::Present))
            {
                self.check().map_err(|e| error(e.reason))?;
                let path = w.path.to_path()?;
                fallback.get_or_insert_with(|| path.clone());
                let git = ControlledGit::with_timeout(self.remaining());
                let result = git
                    .read(&path, GitRead::GitDir)
                    .and_then(|raw| parse_path(&raw));
                if let Some(op) = git.first_timeout() {
                    self.stopped_families.entry(key).or_insert(op);
                    return Err(result
                        .err()
                        .unwrap_or_else(|| error("control entry selection timeout")));
                }
                if result.is_ok_and(|dir| BytePath::from_path(&dir) == family.common_dir) {
                    primary = Some(path);
                    break;
                }
            }
            let path = primary.or(fallback).unwrap_or(family.common_dir.to_path()?);
            self.entrypoints.insert(key, path.clone());
            path
        };
        let git = ControlledGit::with_timeout(self.remaining());
        let result = scan_family_with_git(&path, &git).and_then(|i| {
            i.families
                .into_iter()
                .next()
                .ok_or_else(|| error("control family absent"))
        });
        if let Some(op) = git.first_timeout() {
            self.stopped_families
                .entry((family.identity.device, family.identity.inode))
                .or_insert(op);
        }
        result
    }
    pub fn collect(
        inventory: &Inventory,
        limits: &ControlLimits,
        hooks: &'a dyn ControlHooks,
    ) -> Result<Self, SweepError> {
        limits.validate()?;
        crate::residency::ensure_no_materialization()?;
        let mut run = Self {
            states: BTreeMap::new(),
            initial: BTreeMap::new(),
            final_families: BTreeMap::new(),
            stopped_families: BTreeMap::new(),
            probes: BTreeMap::new(),
            entrypoints: BTreeMap::new(),
            hooks,
            limits: limits.clone(),
            started: Instant::now(),
            active: Duration::ZERO,
            bytes: 0,
            entries: 0,
            output: 2,
        };
        for family in &inventory.families {
            let key = (family.identity.device, family.identity.inode);
            if run.initial.contains_key(&key) {
                return Err(error("duplicate control family"));
            }
            let family_started = now_ms();
            let current = run.scan(family);
            for w in &family.worktrees {
                if run.states.contains_key(&w.path) {
                    return Err(error("duplicate control worktree"));
                }
                let observed = current
                    .as_ref()
                    .ok()
                    .filter(|f| f.common_dir == family.common_dir && f.identity == family.identity)
                    .and_then(|f| f.worktrees.iter().find(|t| t.path == w.path))
                    .cloned();
                let mut state = WorktreeState {
                    observed: observed.clone(),
                    common_dir: family.common_dir.clone(),
                    family_identity: current.as_ref().ok().map(|f| f.identity.clone()),
                    git_dir: None,
                    git_dir_identity: None,
                    role: "unknown".into(),
                    protections: vec![],
                    protection_complete: current.is_ok(),
                    operations: vec![],
                    operations_complete: true,
                    checks: "incomplete".into(),
                    started_unix_ms: family_started,
                    finished_unix_ms: now_ms(),
                    sources: vec![],
                    gaps: vec![],
                };
                if !observed.as_ref().is_some_and(|a| worktree_matches(a, w)) {
                    state
                        .gaps
                        .push("inventory identity or HEAD differs; fresh scan required".into());
                }
                if observed.as_ref().is_some_and(|t| t.bare) {
                    state.role = "bare".into();
                }
                if let Some(t) = &observed
                    && matches!(t.path_state, DirState::Present)
                    && !run.stopped_families.contains_key(&key)
                    && !run.remaining().is_zero()
                {
                    let git = ControlledGit::with_timeout(run.remaining());
                    let admin = git
                        .read(&w.path.to_path()?, GitRead::GitDir)
                        .and_then(|raw| parse_path(&raw));
                    if let Some(op) = git.first_timeout() {
                        run.stopped_families.entry(key).or_insert(op);
                    }
                    match admin {
                        Ok(dir) => {
                            state.git_dir = Some(BytePath::from_path(&dir));
                            run.probe(&dir, true, "administration", &mut state);
                            state.git_dir_identity = state
                                .sources
                                .last()
                                .filter(|s| s.kind == "administration" && s.complete)
                                .and_then(|_| {
                                    run.probes
                                        .get(&w.path)
                                        .and_then(|p| p.last())
                                        .and_then(|p| p.stat.as_ref())
                                })
                                .map(identity);
                            if state.git_dir_identity.is_some() && !t.bare {
                                state.role = if BytePath::from_path(&dir) == family.common_dir {
                                    "main"
                                } else {
                                    "linked"
                                }
                                .into();
                            }
                            for name in [
                                "index.lock",
                                "HEAD.lock",
                                "config.worktree.lock",
                                "shallow.lock",
                            ] {
                                let path = dir.join(name);
                                if run.probe(&path, false, "protection", &mut state) {
                                    state.protections.push(BytePath::from_path(&path));
                                }
                            }
                            for name in [
                                "MERGE_HEAD",
                                "CHERRY_PICK_HEAD",
                                "REVERT_HEAD",
                                "rebase-apply",
                                "rebase-merge",
                                "sequencer",
                                "BISECT_LOG",
                                "BISECT_START",
                            ] {
                                let path = dir.join(name);
                                if run.probe(
                                    &path,
                                    matches!(name, "rebase-apply" | "rebase-merge" | "sequencer"),
                                    "operation",
                                    &mut state,
                                ) {
                                    state.operations.push(BytePath::from_path(&path));
                                }
                            }
                        }
                        Err(e) => state.gaps.push(e.to_string()),
                    }
                }
                state.protection_complete &= state.git_dir_identity.is_some()
                    && state
                        .sources
                        .iter()
                        .filter(|s| s.kind == "protection")
                        .count()
                        == 4
                    && state
                        .sources
                        .iter()
                        .filter(|s| s.kind == "protection")
                        .all(|s| s.complete);
                state.operations_complete = state
                    .sources
                    .iter()
                    .filter(|s| s.kind == "operation")
                    .count()
                    == 8
                    && state
                        .sources
                        .iter()
                        .filter(|s| s.kind == "operation")
                        .all(|s| s.complete);
                if state.gaps.is_empty()
                    && state.protection_complete
                    && state.operations_complete
                    && observed.as_ref().is_some_and(|a| worktree_matches(a, w))
                {
                    state.checks = "complete".into();
                }
                state.finished_unix_ms = now_ms();
                let comma = usize::from(!run.states.is_empty());
                let size = fit_state(
                    &mut state,
                    run.limits
                        .output_bytes
                        .saturating_sub(run.output)
                        .saturating_add(1 - comma),
                )?;
                run.output = run.output.saturating_add(size + comma);
                run.states.insert(w.path.clone(), state);
            }
            run.initial.insert(key, current);
        }
        run.active = run.started.elapsed();
        Ok(run)
    }
    pub fn finish(
        &mut self,
        inventory: &Inventory,
        stopped: &BTreeMap<FamilyId, GitRead>,
    ) -> Result<(), SweepError> {
        self.started = Instant::now();
        self.stopped_families.extend(stopped.clone());
        let hook = self.hooks.before_final_controls();
        for f in &inventory.families {
            let key = (f.identity.device, f.identity.inode);
            let current = if self.stopped_families.contains_key(&key) {
                Err(error(
                    "control family final check skipped after Git timeout",
                ))
            } else {
                self.scan(f)
            };
            for w in &f.worktrees {
                let Some(mut state) = self.states.remove(&w.path) else {
                    continue;
                };
                self.output = self.output.saturating_sub(
                    serde_json::to_vec(&state)
                        .map_err(|e| error(e.to_string()))?
                        .len()
                        + 1,
                );
                let final_tree = current
                    .as_ref()
                    .ok()
                    .filter(|a| a.identity == f.identity && a.common_dir == f.common_dir)
                    .and_then(|a| a.worktrees.iter().find(|t| t.path == w.path));
                let drift = final_tree
                    .zip(state.observed.as_ref())
                    .is_some_and(|(a, b)| !worktree_matches(a, b));
                if drift {
                    state.checks = "change_observed".into();
                    state
                        .gaps
                        .push("worktree/HEAD/registration protection change observed".into());
                } else if !final_tree.is_some_and(|a| worktree_matches(a, w)) || hook.is_err() {
                    state.checks = "incomplete".into();
                    state
                        .gaps
                        .push("control final family or hook validation incomplete".into());
                }
                // Current positive protections remain useful even if another facet fails.
                if let Some(t) = final_tree
                    && t.locked.is_some()
                {
                    if let Some(observed) = &mut state.observed {
                        observed.locked = t.locked.clone();
                    }
                }
                if self.stopped_families.contains_key(&key) {
                    state.checks = "incomplete".into();
                    state
                        .gaps
                        .push("final controls skipped after family timeout".into());
                } else if let Err(e) = self.check() {
                    if state.checks != "change_observed" {
                        state.checks = "incomplete".into();
                    }
                    state.gaps.push(e.reason);
                } else {
                    let git = ControlledGit::with_timeout(self.remaining());
                    let final_dir = git
                        .read(&w.path.to_path()?, GitRead::GitDir)
                        .and_then(|raw| parse_path(&raw));
                    if let Some(op) = git.first_timeout() {
                        self.stopped_families.entry(key).or_insert(op);
                    }
                    match final_dir {
                        Ok(p) if Some(BytePath::from_path(&p)) == state.git_dir => {}
                        Ok(_) => {
                            state.checks = "change_observed".into();
                            state
                                .gaps
                                .push("control administration path changed".into());
                        }
                        Err(e) => {
                            state.checks = "incomplete".into();
                            state.gaps.push(e.to_string());
                        }
                    }
                    if !self.stopped_families.contains_key(&key) {
                        for p in self.probes.get(&w.path).cloned().unwrap_or_default() {
                            let mut present = false;
                            let result = self.observe(&p.path, p.directory, &mut present);
                            let marker = BytePath::from_path(&p.path);
                            if present {
                                let destination = match p.kind.as_str() {
                                    "protection" => Some(&mut state.protections),
                                    "operation" => Some(&mut state.operations),
                                    _ => None,
                                };
                                if let Some(v) = destination
                                    && !v.contains(&marker)
                                {
                                    v.push(marker.clone());
                                }
                            }
                            let failure = match result {
                                Ok((s, h))
                                    if p.complete
                                        && match (p.stat, s) {
                                            (None, None) => true,
                                            (Some(a), Some(b)) => same_stat(&a, &b),
                                            _ => false,
                                        }
                                        && p.hash == h =>
                                {
                                    None
                                }
                                Ok(_) if p.complete => Some(changed(
                                    "control source identity/content/presence changed",
                                )),
                                Ok(_) => Some(unavailable("initial control source incomplete")),
                                Err(e) => Some(e),
                            };
                            if let Some(e) = failure {
                                if state.checks != "change_observed" {
                                    state.checks = if e.changed {
                                        "change_observed"
                                    } else {
                                        "incomplete"
                                    }
                                    .into();
                                }
                                state
                                    .gaps
                                    .push(format!("{}: {}", marker.display(), e.reason));
                            }
                        }
                    }
                }
                state.finished_unix_ms = now_ms();
                let comma = usize::from(!self.states.is_empty());
                let size = fit_state(
                    &mut state,
                    self.limits
                        .output_bytes
                        .saturating_sub(self.output)
                        .saturating_add(1 - comma),
                )?;
                self.output = self.output.saturating_add(size + comma);
                self.states.insert(w.path.clone(), state);
            }
            self.final_families.insert(key, current);
        }
        Ok(())
    }
}

fn fit_state(state: &mut WorktreeState, remaining: usize) -> Result<usize, SweepError> {
    let size = serde_json::to_vec(state)
        .map_err(|e| error(e.to_string()))?
        .len();
    if size.saturating_add(1) <= remaining {
        return Ok(size);
    }
    state.sources.clear();
    state.protection_complete = false;
    state.operations_complete = false;
    if state.checks != "change_observed" {
        state.checks = "incomplete".into();
    }
    state.gaps = vec!["control output byte budget reached; source details omitted".into()];
    let size = serde_json::to_vec(state)
        .map_err(|e| error(e.to_string()))?
        .len();
    if size.saturating_add(1) > remaining {
        return Err(error(
            "control output byte budget cannot retain current identities/protections/operations; retry a narrower scope",
        ));
    }
    Ok(size)
}
pub(super) fn bound_item_states(items: &mut [ReviewItem], limit: usize) -> Result<(), SweepError> {
    let mut used = 2usize;
    for (index, state) in items
        .iter_mut()
        .filter_map(|i| i.worktree_state.as_mut())
        .enumerate()
    {
        let comma = usize::from(index > 0);
        used = used.saturating_add(
            fit_state(state, limit.saturating_sub(used).saturating_add(1 - comma))? + comma,
        );
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod final_stop_regression {
    use super::*;
    use std::{
        cell::Cell,
        fs,
        process::{Command, Stdio},
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = PathBuf::from(format!(
                "/private/tmp/wt-sweep-control-stop-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            let f = Self(p);
            f.git(&["init", "--template=", "-b", "main"]);
            fs::write(f.0.join("file"), b"synthetic stop fixture\n").unwrap();
            f.git(&["add", "file"]);
            f.git(&["commit", "-m", "synthetic"]);
            f
        }
        fn git(&self, args: &[&str]) {
            let mut child = Command::new("/usr/bin/git")
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_ALLOW_PROTOCOL", "")
                .env("GIT_NO_LAZY_FETCH", "1")
                .env("GIT_AUTHOR_NAME", "Fixture")
                .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
                .env("GIT_COMMITTER_NAME", "Fixture")
                .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
                .args([
                    "-c",
                    "core.hooksPath=/dev/null",
                    "-c",
                    "core.fsmonitor=false",
                    "-c",
                    "gc.auto=0",
                    "-C",
                ])
                .arg(&self.0)
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let start = Instant::now();
            while child.try_wait().unwrap().is_none() {
                if start.elapsed() > Duration::from_secs(15) {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("owned fixture child deadline");
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let out = child.wait_with_output().unwrap();
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    struct FinalCancel(Cell<bool>);
    impl LocalHooks for FinalCancel {
        fn cancelled(&self) -> bool {
            self.0.get()
        }
    }
    impl ControlHooks for FinalCancel {
        fn before_final_controls(&self) -> io::Result<()> {
            self.0.set(true);
            Ok(())
        }
    }
    #[test]
    fn stopped_final_controls_spawn_no_git_children_and_keep_observed_locks() {
        let f = Fixture::new();
        let inventory = crate::git::scan_family(&f.0).unwrap();
        let index = fs::read(f.0.join(".git/index")).unwrap();
        let lock = f.0.join(".git/index.lock");
        fs::write(&lock, b"synthetic lock\n").unwrap();
        let limits = ControlLimits::default();
        // A healthy final phase really launches GitDir and retains the positive.
        let mut healthy = ControlRun::collect(&inventory, &limits, &NativeHooks).unwrap();
        crate::git::take_child_read_journal();
        healthy.finish(&inventory, &BTreeMap::new()).unwrap();
        let witness = crate::git::take_child_read_journal();
        assert!(
            witness.iter().any(|(_, op)| matches!(op, GitRead::GitDir)),
            "{witness:?}"
        );
        assert_eq!(
            healthy.states[&BytePath::from_path(&f.0)].checks,
            "complete"
        );
        for cancelled in [false, true] {
            let hooks = FinalCancel(Cell::new(false));
            let hook: &dyn ControlHooks = if cancelled { &hooks } else { &NativeHooks };
            let mut run = ControlRun::collect(&inventory, &limits, hook).unwrap();
            if !cancelled {
                run.active = limits.timeout;
            }
            crate::git::take_child_read_journal();
            run.finish(&inventory, &BTreeMap::new()).unwrap();
            let final_children = crate::git::take_child_read_journal();
            assert!(
                final_children.is_empty(),
                "new children after final control stop (cancelled={cancelled}): {final_children:?}"
            );
            let state = &run.states[&BytePath::from_path(&f.0)];
            assert!(state.protections.contains(&BytePath::from_path(&lock)));
            assert_eq!(state.checks, "incomplete");
            assert_eq!(fs::read(f.0.join(".git/index")).unwrap(), index);
        }
    }
}
