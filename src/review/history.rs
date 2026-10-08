//! Bounded raw-commit observations. Tree and blob verification belongs to deep review.
use super::*;
use std::fs;
use std::os::unix::fs::MetadataExt;

// Physical administration identity, byte-exact path and full object IDs bind caches.
type FamilyKey = (u64, u64, BytePath);
type CommitKey = (FamilyKey, String);
type TargetKey = (FamilyKey, BytePath);
// Resolved aliases share a graph only within the same physical family.
type WalkKey = (FamilyKey, String);
struct TargetWalk {
    frontier: VecDeque<String>,
    pending: BTreeSet<String>,
    // Membership is evidence only after object verification and commit parsing.
    seen: BTreeSet<String>,
    terminal_reason: Option<String>,
}
impl TargetWalk {
    fn result(&self, source: &str) -> Option<(&str, Option<String>)> {
        if self.seen.contains(source) {
            Some(("ancestor", None))
        } else if let Some(reason) = &self.terminal_reason {
            Some(("unknown", Some(reason.clone())))
        } else if self.frontier.is_empty() {
            Some(("not_ancestor", None))
        } else {
            None
        }
    }
}
struct Row {
    family: usize,
    path: BytePath,
    source: Option<String>,
    observations: Vec<HistoryObservation>,
    omitted: usize,
    eligible: bool,
}
struct Job {
    row: usize,
    observation: usize,
    initialized: bool,
    done: bool,
}
impl Job {
    fn new(row: usize, observation: usize) -> Self {
        Self {
            row,
            observation,
            initialized: false,
            done: false,
        }
    }
}
pub(super) struct HistoryRun {
    rows: Vec<Row>,
    limits: ReviewLimits,
    commits: BTreeMap<CommitKey, Result<Commit, SweepError>>,
    targets: BTreeMap<TargetKey, Result<String, SweepError>>,
    walks: BTreeMap<WalkKey, TargetWalk>,
    started: Instant,
    collected_time: Duration,
    queued_ids: usize,
    pub metrics: HistoryMetrics,
    pub(super) stopped_families: BTreeMap<(u64, u64), GitRead>,
}
impl HistoryRun {
    fn family_key(family: &Family) -> FamilyKey {
        (
            family.identity.device,
            family.identity.inode,
            family.common_dir.clone(),
        )
    }
    /// Cache-only access keeps physical family identity and spends no evidence budget.
    pub(super) fn cached_commit(
        &self,
        family: &Family,
        oid: &str,
    ) -> Option<&Result<Commit, SweepError>> {
        self.commits
            .get(&(Self::family_key(family), oid.to_ascii_lowercase()))
    }
    fn stopped(&self, family: &Family) -> Option<SweepError> {
        self.stopped_families
            .get(&(family.identity.device, family.identity.inode))
            .map(|operation| error(family_timeout_reason(operation)))
    }
    fn remember_timeout<T>(
        &mut self,
        family: &Family,
        operation: GitRead,
        result: &mut Result<T, SweepError>,
        child_timeout: Duration,
        phase_timeout: Duration,
    ) {
        if !result.as_ref().is_err_and(|e| e.code == "git_timeout") {
            return;
        }
        if child_timeout < ControlledGit::default().timeout_limit()
            && self.started.elapsed() >= phase_timeout
        {
            // A shortened phase allowance does not establish backend failure.
            // Cached facts still require the usual fresh final validity checks.
            let reason = "history time budget exhausted during Git read";
            self.metrics
                .stop_reason
                .get_or_insert_with(|| reason.into());
            *result = Err(error(reason));
            return;
        }
        if stops_family_history(&operation) {
            self.metrics
                .stop_reason
                .get_or_insert_with(|| family_timeout_reason(&operation));
            self.stopped_families
                .entry((family.identity.device, family.identity.inode))
                .or_insert(operation);
        }
    }
    fn stop(&self) -> Option<String> {
        if self.started.elapsed() >= self.limits.timeout {
            Some("history time budget reached".into())
        } else if self.metrics.traversal_steps >= self.limits.traversal_steps as u64 {
            Some("history traversal step budget reached".into())
        } else if self.metrics.object_read_attempts >= self.limits.object_read_attempts as u64 {
            Some("history object attempt budget reached".into())
        } else if self.metrics.object_bytes >= self.limits.object_bytes as u64 {
            Some("history returned byte budget reached".into())
        } else {
            None
        }
    }
    fn read_commit(&mut self, family: &Family, oid: &str) -> Result<Commit, SweepError> {
        if let Some(reason) = self.stopped(family) {
            return Err(reason);
        }
        let canonical = oid.to_ascii_lowercase();
        let oid = canonical.as_str();
        let key = (Self::family_key(family), oid.into());
        if let Some(result) = self.commits.get(&key) {
            self.metrics.cache_hits += 1;
            return result.clone();
        }
        if let Some(reason) = self.stop() {
            return Err(error(reason));
        }
        if !valid_oid(oid) {
            return Err(error("source commit OID invalid"));
        }
        self.metrics.object_read_attempts += 1;
        let remaining = self.limits.timeout.saturating_sub(self.started.elapsed());
        let git = ControlledGit::with_output_limit(
            remaining,
            self.limits
                .object_bytes
                .saturating_sub(self.metrics.object_bytes as usize),
        );
        let mut result = git.read(
            &family.common_dir.to_path()?,
            GitRead::Commit { oid: oid.into() },
        );
        self.metrics.spawned_processes += git.spawned_processes();
        self.remember_timeout(
            family,
            GitRead::Commit { oid: oid.into() },
            &mut result,
            git.timeout_limit(),
            self.limits.timeout,
        );
        let result = result.and_then(|raw| {
            self.metrics.object_bytes += raw.len() as u64;
            verify_object("commit", oid, &raw)?;
            parse_commit(&raw, oid.len())
        });
        self.commits.insert(key, result.clone());
        result
    }
    fn resolve_target(
        &mut self,
        family: &Family,
        reference: &BytePath,
    ) -> Result<String, SweepError> {
        if let Some(reason) = self.stopped(family) {
            return Err(reason);
        }
        let key = (Self::family_key(family), reference.clone());
        if let Some(result) = self.targets.get(&key) {
            self.metrics.cache_hits += 1;
            return result.clone();
        }
        if let Some(reason) = self.stop() {
            return Err(error(format!("{reason}; uncached target not resolved")));
        }
        self.metrics.target_resolution_attempts += 1;
        let git =
            ControlledGit::with_timeout(self.limits.timeout.saturating_sub(self.started.elapsed()));
        let mut result = resolve(&git, &family.common_dir.to_path()?, reference);
        self.metrics.spawned_processes += git.spawned_processes();
        self.remember_timeout(
            family,
            GitRead::ResolveCommit {
                reference: reference.clone(),
            },
            &mut result,
            git.timeout_limit(),
            self.limits.timeout,
        );
        self.targets.insert(key, result.clone());
        result
    }
    fn complete(&mut self, job: &mut Job, _family: &Family, result: &str, reason: Option<String>) {
        job.done = true;
        if reason.as_ref().is_some_and(|r| r.contains("budget"))
            && self.metrics.stop_reason.is_none()
        {
            self.metrics.stop_reason = reason.clone();
        }
        let row = &mut self.rows[job.row];
        let observation = &mut row.observations[job.observation];
        observation.result = result.into();
        observation.stop_reason = reason.clone();
        observation.finished_unix_ms = now_ms();
    }
    fn stop_walk(&mut self, walk: &mut TargetWalk, reason: String) {
        self.queued_ids -= walk.frontier.len();
        walk.frontier.clear();
        walk.pending.clear();
        walk.terminal_reason = Some(reason);
    }

    fn turn(&mut self, job: &mut Job, inventory: &Inventory) {
        let family = &inventory.families[self.rows[job.row].family];
        if let Some(reason) = self.stopped(family) {
            self.complete(job, family, "unknown", Some(reason.to_string()));
            return;
        }
        if !self.rows[job.row].eligible {
            self.complete(
                job,
                family,
                "unknown",
                Some("family identity or worktree unavailable".into()),
            );
            return;
        }
        let Some(source) = self.rows[job.row].source.clone() else {
            self.complete(job, family, "unknown", Some("source HEAD unknown".into()));
            return;
        };
        let source = source.to_ascii_lowercase();
        let reads = self.metrics.object_read_attempts;
        let bytes = self.metrics.object_bytes;
        let steps = self.metrics.traversal_steps;
        if !job.initialized {
            let observation = &mut self.rows[job.row].observations[job.observation];
            observation.started_unix_ms = now_ms();
            observation.stop_reason =
                Some("history comparison in progress; relation not established".into());
            if let Err(e) = self.read_commit(family, &source) {
                self.complete(job, family, "unknown", Some(e.to_string()));
                return;
            }
            let reference = self.rows[job.row].observations[job.observation]
                .target_ref
                .clone();
            let target = match self.resolve_target(family, &reference) {
                Ok(target) => target,
                Err(e) => {
                    self.complete(job, family, "unknown", Some(e.to_string()));
                    return;
                }
            };
            self.rows[job.row].observations[job.observation].target_oid = Some(target.clone());
            if target == source {
                self.complete(job, family, "identical_commit", None);
                return;
            }
            let key = (Self::family_key(family), target.clone());
            if !self.walks.contains_key(&key) {
                let mut walk = TargetWalk {
                    frontier: VecDeque::new(),
                    pending: BTreeSet::new(),
                    seen: BTreeSet::new(),
                    terminal_reason: None,
                };
                if self.queued_ids >= self.limits.traversal_steps.min(65536) {
                    walk.terminal_reason = Some("history frontier memory budget reached".into());
                } else {
                    walk.frontier.push_back(target.clone());
                    walk.pending.insert(target);
                    self.queued_ids += 1;
                }
                self.walks.insert(key, walk);
            }
            job.initialized = true;
        }
        let Some(target) = self.rows[job.row].observations[job.observation]
            .target_oid
            .clone()
        else {
            self.complete(
                job,
                family,
                "unknown",
                Some("resolved target OID unavailable".into()),
            );
            return;
        };
        let key = (Self::family_key(family), target);
        // Take just this graph while expanding it, keeping all accounting run-wide.
        let Some(mut walk) = self.walks.remove(&key) else {
            self.complete(
                job,
                family,
                "unknown",
                Some("target graph unavailable".into()),
            );
            return;
        };
        loop {
            // Verified positives and a complete graph remain usable after read budgets.
            if let Some((result, reason)) = walk.result(&source) {
                self.complete(job, family, result, reason);
                break;
            }
            // A slice yields; it must not turn resumable work into a cached refusal.
            if self.metrics.object_read_attempts - reads
                >= self.limits.slice_object_read_attempts as u64
                || self.metrics.object_bytes - bytes >= self.limits.slice_object_bytes as u64
                || self.metrics.traversal_steps - steps >= self.limits.slice_traversal_steps as u64
            {
                break;
            }
            // Traversal/time bound even cached expansions. Read/byte budgets are
            // checked by read_commit only when a new object must be collected.
            if self.started.elapsed() >= self.limits.timeout
                || self.metrics.traversal_steps >= self.limits.traversal_steps as u64
            {
                self.stop_walk(
                    &mut walk,
                    "history time or traversal step budget reached".into(),
                );
                continue;
            }
            if walk.seen.len() >= self.limits.per_target_steps {
                self.stop_walk(
                    &mut walk,
                    "per-target history traversal step budget reached".into(),
                );
                continue;
            }
            let Some(oid) = walk.frontier.front().cloned() else {
                continue;
            };
            let commit = match self.read_commit(family, &oid) {
                Ok(commit) => commit,
                Err(e) => {
                    self.stop_walk(&mut walk, e.to_string());
                    continue;
                }
            };
            walk.frontier.pop_front();
            walk.pending.remove(&oid);
            self.queued_ids -= 1;
            walk.seen.insert(oid);
            self.metrics.traversal_steps += 1;
            let mut unique = BTreeSet::new();
            let parents: Vec<_> = commit
                .parents
                .into_iter()
                .filter(|parent| {
                    !walk.seen.contains(parent)
                        && !walk.pending.contains(parent)
                        && unique.insert(parent.clone())
                })
                .collect();
            if parents.len()
                > self
                    .limits
                    .traversal_steps
                    .min(65536)
                    .saturating_sub(self.queued_ids)
            {
                self.stop_walk(&mut walk, "history frontier memory budget reached".into());
                continue;
            }
            self.queued_ids += parents.len();
            for parent in parents {
                walk.pending.insert(parent.clone());
                walk.frontier.push_back(parent);
            }
        }
        self.walks.insert(key, walk);
    }
    pub fn collect(
        inventory: &Inventory,
        targets: &FamilyTargets,
        limits: &ReviewLimits,
        stopped: &BTreeMap<(u64, u64), GitRead>,
        callback: &mut dyn FnMut(ProgressEvent),
    ) -> Result<Self, SweepError> {
        let mut run = Self {
            rows: Vec::new(),
            limits: limits.clone(),
            commits: BTreeMap::new(),
            targets: BTreeMap::new(),
            walks: BTreeMap::new(),
            started: Instant::now(),
            collected_time: Duration::ZERO,
            queued_ids: 0,
            metrics: HistoryMetrics::default(),
            stopped_families: stopped.clone(),
        };
        let mut families = BTreeSet::new();
        let mut paths = BTreeSet::new();
        for (index, family) in inventory.families.iter().enumerate() {
            let common = family.common_dir.to_path()?;
            if !common.is_absolute()
                || !families.insert((family.identity.device, family.identity.inode))
            {
                return Err(error("invalid or duplicate inventory family"));
            }
            let eligible = fs::metadata(&common).is_ok_and(|m| {
                m.dev() == family.identity.device && m.ino() == family.identity.inode
            });
            for worktree in &family.worktrees {
                if !paths.insert(worktree.path.clone()) {
                    return Err(error("duplicate worktree path in inventory"));
                }
                run.rows.push(Row {
                    family: index,
                    path: worktree.path.clone(),
                    source: worktree.head.clone(),
                    observations: Vec::new(),
                    omitted: 0,
                    eligible: eligible
                        && matches!(worktree.path_state, DirState::Present | DirState::Absent),
                });
            }
        }
        run.metrics.items_total = run.rows.len() as u64;
        progress(callback, "history", 0, None);
        let max_targets = inventory
            .families
            .iter()
            .map(|family| targets.for_family(family).len())
            .max()
            .unwrap_or(0);
        let mut jobs = Vec::new();
        // A family-local target ordinal spans all rows before another ordinal claims capacity.
        for ordinal in 0..max_targets {
            for (row_index, row) in run.rows.iter_mut().enumerate() {
                let Some(target) = targets
                    .for_family(&inventory.families[row.family])
                    .get(ordinal)
                else {
                    continue;
                };
                if jobs.len() >= limits.target_jobs {
                    row.omitted += 1;
                    continue;
                }
                let observation = row.observations.len();
                let now = now_ms();
                row.observations.push(HistoryObservation {
                    source_oid: row.source.clone(),
                    target_ref: target.clone(),
                    target_oid: None,
                    result: "unknown".into(),
                    observed_result: None,
                    started_unix_ms: now,
                    finished_unix_ms: now,
                    stop_reason: Some("history job not examined".into()),
                });
                jobs.push(Job::new(row_index, observation));
            }
        }
        // Reserve one bounded turn per row before any row receives a second turn.
        for row in 0..run.rows.len() {
            if let Some(job) = jobs.iter_mut().find(|job| job.row == row) {
                run.turn(job, inventory);
                run.metrics.first_pass_items += 1;
                let row = &run.rows[job.row];
                let observation = &row.observations[job.observation];
                callback(ProgressEvent {
                    phase: "history_observation",
                    processed: run.metrics.first_pass_items,
                    stop_reason: observation.stop_reason.clone(),
                    observation: Some(ProgressObservation::History {
                        path: row.path.clone(),
                        head: observation.source_oid.clone(),
                        target_ref: observation.target_ref.clone(),
                        target_oid: observation.target_oid.clone(),
                        result: observation.result.clone(),
                        complete: observation.result != "unknown"
                            && observation.stop_reason.is_none(),
                    }),
                });
            }
        }
        progress(
            callback,
            "history_first_pass",
            run.metrics.first_pass_items,
            run.stop(),
        );
        let mut queue: VecDeque<_> = jobs.into_iter().filter(|job| !job.done).collect();
        while let Some(mut job) = queue.pop_front() {
            run.turn(&mut job, inventory);
            if !job.done {
                queue.push_back(job);
            }
        }
        run.metrics.stop_reason = run.stop().or(run.metrics.stop_reason);
        run.collected_time = run.started.elapsed();
        progress(
            callback,
            "history_complete",
            run.metrics.first_pass_items,
            run.metrics.stop_reason.clone(),
        );
        Ok(run)
    }
    fn final_target(
        &mut self,
        family: &Family,
        reference: &BytePath,
        remaining_time: Duration,
        cache: &mut BTreeMap<TargetKey, Result<String, SweepError>>,
    ) -> Result<String, SweepError> {
        if let Some(reason) = self.stopped(family) {
            return Err(reason);
        }
        let key = (Self::family_key(family), reference.clone());
        if let Some(result) = cache.get(&key) {
            return result.clone();
        }
        if cache.len() >= 4096 || self.started.elapsed() >= remaining_time {
            self.metrics.stop_reason.get_or_insert_with(|| {
                "final target revalidation count or time budget reached".into()
            });
            return Err(error(
                "final target revalidation count or time budget reached",
            ));
        }
        self.metrics.target_resolution_attempts += 1;
        let git =
            ControlledGit::with_timeout(remaining_time.saturating_sub(self.started.elapsed()));
        let mut result = resolve(&git, &family.common_dir.to_path()?, reference);
        self.metrics.spawned_processes += git.spawned_processes();
        self.remember_timeout(
            family,
            GitRead::ResolveCommit {
                reference: reference.clone(),
            },
            &mut result,
            git.timeout_limit(),
            remaining_time,
        );
        cache.insert(key, result.clone());
        result
    }
    pub(super) fn reconcile_stopped(&mut self, inventory: &Inventory, items: &mut [ReviewItem]) {
        for row in &mut self.rows {
            let family = &inventory.families[row.family];
            let Some(operation) = self
                .stopped_families
                .get(&(family.identity.device, family.identity.inode))
            else {
                continue;
            };
            let reason = family_timeout_reason(operation);
            for observation in &mut row.observations {
                invalidate(observation, reason.clone());
                observation.finished_unix_ms = now_ms();
            }
            if let Some(item) = items.iter_mut().find(|item| item.path == row.path) {
                item.history = row.observations.clone();
                item.observations.clear();
                item.requests.clear();
                item.subject = None;
                item.subject_truncated = false;
                if !item.unknowns.contains(&reason) {
                    item.unknowns.push(reason);
                }
            }
        }
        self.metrics.completed_targets = self
            .rows
            .iter()
            .flat_map(|r| &r.observations)
            .filter(|o| o.result != "unknown")
            .count() as u64;
        self.metrics.incomplete_targets = self
            .rows
            .iter()
            .map(|r| {
                r.observations
                    .iter()
                    .filter(|o| o.result == "unknown")
                    .count() as u64
                    + r.omitted as u64
            })
            .sum();
    }
    pub(super) fn attach(&self, item: &mut ReviewItem) {
        if let Some(row) = self.rows.iter().find(|r| r.path == item.path) {
            item.history = row.observations.clone();
            item.history_omitted_targets = row.omitted;
        }
    }
    pub fn finish(
        &mut self,
        inventory: &Inventory,
        items: &mut [ReviewItem],
        final_families: &BTreeMap<(u64, u64), Result<Family, SweepError>>,
    ) -> Result<(), SweepError> {
        self.started = Instant::now();
        let remaining_time = self.limits.timeout.saturating_sub(self.collected_time);
        let mut final_targets: BTreeMap<TargetKey, Result<String, SweepError>> = BTreeMap::new();

        let mut rows = std::mem::take(&mut self.rows);
        for row in &mut rows {
            let family = &inventory.families[row.family];
            let quarantined = self
                .stopped_families
                .get(&(family.identity.device, family.identity.inode))
                .map(family_timeout_reason);
            let current = final_families
                .get(&(family.identity.device, family.identity.inode))
                .cloned()
                .unwrap_or_else(|| Err(error("final family control unavailable")));
            let change_observed = current
                .as_ref()
                .ok()
                .is_some_and(|observed| family_change_observed(family, observed));
            let family_reason = quarantined.unwrap_or_else(|| {
                if change_observed {
                    "family identity or HEAD change observed during final revalidation".into()
                } else {
                    "family identity/HEAD final revalidation incomplete".into()
                }
            });
            let family_valid = current
                .as_ref()
                .ok()
                .filter(|f| f.identity == family.identity && f.common_dir == family.common_dir)
                .and_then(|f| f.worktrees.iter().find(|w| w.path == row.path))
                .zip(family.worktrees.iter().find(|w| w.path == row.path))
                .is_some_and(|(a, b)| {
                    a.identity == b.identity
                        && optional_oid_eq(a.head.as_deref(), b.head.as_deref())
                        && a.branch == b.branch
                        && a.path_state == b.path_state
                });
            for observation in &mut row.observations {
                // Failed initial resolution is an explicit unknown, never a negative graph.
                if observation.target_oid.is_none() {
                    continue;
                }
                if !family_valid {
                    invalidate(observation, family_reason.clone());
                    observation.finished_unix_ms = now_ms();
                    continue;
                }
                let final_target = self.final_target(
                    family,
                    &observation.target_ref,
                    remaining_time,
                    &mut final_targets,
                );
                let reason = if !family_valid {
                    Some(family_reason.clone())
                } else {
                    match final_target {
                        Ok(oid) if Some(oid.as_str()) == observation.target_oid.as_deref() => None,
                        Ok(_) => Some("target changed during history observation".into()),
                        Err(e) => Some(format!("target revalidation incomplete: {e}")),
                    }
                };
                if let Some(reason) = reason {
                    invalidate(observation, reason);
                }
                observation.finished_unix_ms = now_ms();
            }
            if let Some(item) = items.iter_mut().find(|item| item.path == row.path) {
                item.history = row.observations.clone();
                item.history_omitted_targets = row.omitted;
                if row.omitted > 0 {
                    item.unknowns.push(format!("{} explicit history targets omitted: whole-review history job budget reached",row.omitted));
                }
                for o in &item.history {
                    if let Some(reason) = &o.stop_reason {
                        item.unknowns
                            .push(format!("history {}: {reason}", o.target_ref.display()));
                    }
                }
                if !family_valid {
                    // A timeout-skipped final check does not erase independent
                    // raw HEAD/index/disk facts collected earlier in this run.
                    if (change_observed || self.stopped(family).is_none())
                        && let Some(summary) = &mut item.local_summary
                    {
                        invalidate_local_summary(summary, change_observed)?;
                    }
                    item.unknowns.push(family_reason.clone());
                    item.subject = None;
                    item.subject_truncated = false;
                    item.observations.clear();
                    item.requests.clear();
                }
            }
        }
        self.rows = rows;
        // Deep relations can involve graph-omitted targets or intermediate refs.
        // They require the same final fresh ref check, with a separately reserved cap.
        for item in items.iter_mut() {
            let Some(row) = self.rows.iter().find(|row| row.path == item.path) else {
                continue;
            };
            let family = &inventory.families[row.family];
            let mut retained = Vec::new();
            for observation in std::mem::take(&mut item.observations) {
                let final_target = self.final_target(
                    family,
                    &observation.target_ref,
                    remaining_time,
                    &mut final_targets,
                );
                match final_target {
                    Ok(oid) if oid == observation.target_oid => retained.push(observation),
                    Ok(_) => item.unknowns.push(format!(
                        "deep {}: target changed during final revalidation",
                        observation.target_ref.display()
                    )),
                    Err(e) => item.unknowns.push(format!(
                        "deep {}: final target revalidation incomplete: {e}",
                        observation.target_ref.display()
                    )),
                }
            }
            item.observations = retained;
        }
        self.reconcile_stopped(inventory, items);
        if self.started.elapsed() >= remaining_time {
            self.metrics
                .stop_reason
                .get_or_insert_with(|| "final history revalidation time budget reached".into());
        }
        self.metrics.completed_targets = self
            .rows
            .iter()
            .flat_map(|r| &r.observations)
            .filter(|o| o.result != "unknown")
            .count() as u64;
        self.metrics.incomplete_targets = self
            .rows
            .iter()
            .map(|r| {
                r.observations
                    .iter()
                    .filter(|o| o.result == "unknown")
                    .count() as u64
                    + r.omitted as u64
            })
            .sum();
        self.metrics.elapsed_ms = (self.collected_time + self.started.elapsed())
            .as_millis()
            .min(u64::MAX as u128) as u64;
        Ok(())
    }
}
fn invalidate(observation: &mut HistoryObservation, reason: String) {
    if observation.result != "unknown" {
        observation.observed_result = Some(observation.result.clone());
    }
    observation.result = "unknown".into();
    observation.stop_reason = Some(reason);
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod deadline_tests {
    use super::*;
    use std::{
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = PathBuf::from(format!(
                "/private/tmp/wt-sweep-history-deadline-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            let f = Self(path);
            f.git(&["init", "--template=", "-b", "main"]);
            fs::write(f.0.join("file"), b"synthetic history\n").unwrap();
            f.git(&["add", "file"]);
            f.git(&["commit", "-m", "fixture"]);
            f
        }
        fn git(&self, args: &[&str]) -> String {
            let mut command = Command::new("/usr/bin/git");
            command
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
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
                    "-c",
                    "maintenance.auto=false",
                    "-C",
                ])
                .arg(&self.0)
                .args(args);
            String::from_utf8(bounded(command))
                .unwrap()
                .trim_end_matches('\n')
                .into()
        }
        fn block(&self, path: &Path) {
            fs::rename(path, path.with_extension("before")).unwrap();
            let mut command = Command::new("/usr/bin/mkfifo");
            command.env_clear().arg(path);
            bounded(command);
        }
        fn run(&self, targets: &[BytePath]) -> (Inventory, HistoryRun) {
            let inventory = crate::git::scan_family(&self.0).unwrap();
            let bound = FamilyTargets::bind(&inventory, TargetInput::Simple(targets)).unwrap();
            let run = HistoryRun::collect(
                &inventory,
                &bound,
                &ReviewLimits::default(),
                &BTreeMap::new(),
                &mut |_| {},
            )
            .unwrap();
            (inventory, run)
        }
    }
    fn bounded(mut command: Command) -> Vec<u8> {
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let started = Instant::now();
        while child.try_wait().unwrap().is_none() {
            if started.elapsed() >= Duration::from_secs(15) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("owned history fixture child exceeded deadline");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn near_deadline(run: &mut HistoryRun) {
        run.started = Instant::now() - (run.limits.timeout - Duration::from_millis(200));
    }

    #[test]
    fn initialized_history_slice_reports_in_progress_instead_of_unexamined() {
        let f = Fixture::new();
        let source = f.git(&["rev-parse", "HEAD"]);
        let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
        let target = f.git(&["commit-tree", &tree, "-p", &source, "-m", "descendant"]);
        f.git(&["update-ref", "refs/heads/integration", &target]);
        let inventory = crate::git::scan_family(&f.0).unwrap();
        let targets = [BytePath::from_bytes(b"refs/heads/integration")];
        let bound = FamilyTargets::bind(&inventory, TargetInput::Simple(&targets)).unwrap();
        let limits = ReviewLimits {
            slice_object_read_attempts: 1,
            ..ReviewLimits::default()
        };
        let mut events = Vec::new();
        let run = HistoryRun::collect(
            &inventory,
            &bound,
            &limits,
            &BTreeMap::new(),
            &mut |event| events.push(event),
        )
        .unwrap();
        let first = events
            .iter()
            .find(|event| event.observation.is_some())
            .unwrap();
        assert!(
            matches!(first.observation.as_ref(), Some(ProgressObservation::History {
            head, target_oid, result, complete, ..
        }) if head.as_deref() == Some(source.as_str()) && target_oid.as_deref() == Some(target.as_str())
            && result == "unknown" && !complete)
        );
        assert!(
            first
                .stop_reason
                .as_deref()
                .unwrap()
                .contains("in progress"),
            "an examined slice retained a stale unexamined label: {:?}",
            first.stop_reason
        );
        assert_eq!(run.rows[0].observations[0].result, "ancestor");
        assert!(run.rows[0].observations[0].stop_reason.is_none());
        assert!(run.metrics.object_read_attempts >= 2);
    }

    #[test]
    fn shortened_history_commit_and_target_deadlines_do_not_quarantine_family() {
        for commit_read in [true, false] {
            let f = Fixture::new();
            let main = BytePath::from_bytes(b"refs/heads/main");
            let (inventory, mut run) = f.run(&[main]);
            let family = &inventory.families[0];
            let head = family.worktrees[0].head.as_ref().unwrap();
            let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
            let extra = f.git(&["commit-tree", &tree, "-m", "uncached"]);
            let blocked_ref = BytePath::from_bytes(b"refs/heads/blocked");
            f.git(&["update-ref", "refs/heads/blocked", &extra]);
            let blocked = if commit_read {
                f.0.join(".git/objects").join(&extra[..2]).join(&extra[2..])
            } else {
                f.0.join(".git/refs/heads/blocked")
            };
            f.block(&blocked);
            near_deadline(&mut run);
            crate::git::take_child_read_journal();
            let failure = if commit_read {
                run.read_commit(family, &extra).err().unwrap()
            } else {
                run.resolve_target(family, &blocked_ref).unwrap_err()
            };
            let reads = crate::git::take_child_read_journal();
            assert_eq!(reads.len(), 1, "must exercise actual Git child timeout");
            assert!(
                run.stopped_families.is_empty(),
                "short phase deadline quarantined family"
            );
            assert!(
                failure.to_string().contains("history time budget"),
                "{failure}"
            );
            assert!(run.cached_commit(family, head).unwrap().is_ok());
            assert!(run.read_commit(family, &extra).is_err());
            assert!(
                crate::git::take_child_read_journal().is_empty(),
                "expired history spawned dependent child"
            );
        }
    }

    #[test]
    fn shortened_final_history_deadline_preserves_fresh_proof_and_demotes_blocked_cached_fact() {
        let f = Fixture::new();
        f.git(&["branch", "zz_blocked"]);
        let targets = [
            BytePath::from_bytes(b"refs/heads/main"),
            BytePath::from_bytes(b"refs/heads/zz_blocked"),
        ];
        let (inventory, mut run) = f.run(&targets);
        assert!(
            run.rows[0]
                .observations
                .iter()
                .all(|o| o.result == "identical_commit")
        );
        // Fresh family validation precedes the final target deadline. Only the
        // later explicit target ref is made unreadable, in the owned fixture.
        let family = &inventory.families[0];
        let live = crate::git::scan_family(&f.0).unwrap().families.remove(0);
        let final_families =
            BTreeMap::from([((family.identity.device, family.identity.inode), Ok(live))]);
        f.block(&f.0.join(".git/refs/heads/zz_blocked"));
        run.collected_time = run.limits.timeout - Duration::from_millis(200);
        crate::git::take_child_read_journal();
        run.finish(&inventory, &mut [], &final_families).unwrap();
        let reads = crate::git::take_child_read_journal();
        assert_eq!(
            reads.len(),
            2,
            "fresh main validation and one blocked child required"
        );
        assert!(
            run.stopped_families.is_empty(),
            "final phase deadline quarantined family"
        );
        let stable = &run.rows[0].observations[0];
        assert_eq!(
            stable.result, "identical_commit",
            "fresh independent proof was erased"
        );
        assert!(stable.stop_reason.is_none());
        let blocked = &run.rows[0].observations[1];
        assert_eq!(
            blocked.result, "unknown",
            "unvalidated cached fact stayed current"
        );
        assert_eq!(blocked.observed_result.as_deref(), Some("identical_commit"));
        assert!(blocked.stop_reason.as_deref().unwrap().contains("budget"));
        assert_eq!(run.metrics.completed_targets, 1);
        assert_eq!(run.metrics.incomplete_targets, 1);
    }

    #[test]
    fn actual_history_backend_timeout_still_quarantines_family_and_refuses_dependents() {
        let f = Fixture::new();
        let (inventory, mut run) = f.run(&[BytePath::from_bytes(b"refs/heads/main")]);
        let family = &inventory.families[0];
        f.git(&["branch", "blocked"]);
        f.block(&f.0.join(".git/refs/heads/blocked"));
        crate::git::take_child_read_journal();
        let failure = run
            .resolve_target(family, &BytePath::from_bytes(b"refs/heads/blocked"))
            .unwrap_err();
        assert_eq!(failure.code, "git_timeout");
        assert_eq!(crate::git::take_child_read_journal().len(), 1);
        assert_eq!(run.stopped_families.len(), 1);
        let head = family.worktrees[0].head.as_ref().unwrap();
        assert!(
            run.read_commit(family, head).is_err(),
            "quarantine allowed cached evidence promotion"
        );
        assert!(crate::git::take_child_read_journal().is_empty());
    }
}
