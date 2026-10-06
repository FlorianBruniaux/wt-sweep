//! Bounded raw-commit observations. Tree and blob verification belongs to deep review.
use super::*;
use std::fs;
use std::os::unix::fs::MetadataExt;

// Physical administration identity, byte-exact path and full object IDs bind caches.
type FamilyKey = (u64, u64, BytePath);
type CommitKey = (FamilyKey, String);
type TargetKey = (FamilyKey, BytePath);
type RelationKey = (FamilyKey, String, String);
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
    queue: VecDeque<String>,
    visited: BTreeSet<String>,
    steps: usize,
    done: bool,
}
impl Job {
    fn new(row: usize, observation: usize) -> Self {
        Self {
            row,
            observation,
            initialized: false,
            queue: VecDeque::new(),
            visited: BTreeSet::new(),
            steps: 0,
            done: false,
        }
    }
}
pub(super) struct HistoryRun {
    rows: Vec<Row>,
    limits: ReviewLimits,
    commits: BTreeMap<CommitKey, Result<Commit, SweepError>>,
    targets: BTreeMap<TargetKey, Result<String, SweepError>>,
    relations: BTreeMap<RelationKey, (String, Option<String>)>,
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
        result: &Result<T, SweepError>,
    ) {
        if result.as_ref().is_err_and(|e| e.code == "git_timeout")
            && stops_family_history(&operation)
        {
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
        let result = git.read(
            &family.common_dir.to_path()?,
            GitRead::Commit { oid: oid.into() },
        );
        self.metrics.spawned_processes += git.spawned_processes();
        self.remember_timeout(family, GitRead::Commit { oid: oid.into() }, &result);
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
        let result = resolve(&git, &family.common_dir.to_path()?, reference);
        self.metrics.spawned_processes += git.spawned_processes();
        self.remember_timeout(
            family,
            GitRead::ResolveCommit {
                reference: reference.clone(),
            },
            &result,
        );
        self.targets.insert(key, result.clone());
        result
    }
    fn complete(&mut self, job: &mut Job, family: &Family, result: &str, reason: Option<String>) {
        job.done = true;
        self.queued_ids = self.queued_ids.saturating_sub(job.queue.len());
        job.queue.clear();
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
        if let (Some(source), Some(target)) = (&row.source, &observation.target_oid) {
            self.relations.insert(
                (
                    Self::family_key(family),
                    source.to_ascii_lowercase(),
                    target.to_ascii_lowercase(),
                ),
                (result.into(), reason),
            );
        }
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
            self.rows[job.row].observations[job.observation].started_unix_ms = now_ms();
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
            if let Some((result, reason)) = self
                .relations
                .get(&(Self::family_key(family), source.clone(), target.clone()))
                .cloned()
            {
                self.metrics.cache_hits += 1;
                self.complete(job, family, &result, reason);
                return;
            }
            if target == source {
                self.complete(job, family, "identical_commit", None);
                return;
            }
            if self.queued_ids >= self.limits.traversal_steps.min(65536) {
                self.complete(
                    job,
                    family,
                    "unknown",
                    Some("history frontier memory budget reached".into()),
                );
                return;
            }
            job.queue.push_back(target);
            self.queued_ids += 1;
            job.initialized = true;
        }
        while let Some(oid) = job.queue.front().cloned() {
            // Already observed identity spends no read or traversal budget.
            if job.visited.contains(&oid) {
                job.queue.pop_front();
                self.queued_ids -= 1;
                continue;
            }
            // A slice yields; it must not turn resumable work into a cached refusal.
            if self.metrics.object_read_attempts - reads
                >= self.limits.slice_object_read_attempts as u64
                || self.metrics.object_bytes - bytes >= self.limits.slice_object_bytes as u64
                || self.metrics.traversal_steps - steps >= self.limits.slice_traversal_steps as u64
            {
                return;
            }
            if self.started.elapsed() >= self.limits.timeout
                || self.metrics.traversal_steps >= self.limits.traversal_steps as u64
            {
                self.complete(
                    job,
                    family,
                    "unknown",
                    Some("history time or traversal step budget reached".into()),
                );
                return;
            }
            if job.steps >= self.limits.per_target_steps {
                self.complete(
                    job,
                    family,
                    "unknown",
                    Some("per-target history traversal step budget reached".into()),
                );
                return;
            }
            let commit = match self.read_commit(family, &oid) {
                Ok(commit) => commit,
                Err(e) => {
                    self.complete(job, family, "unknown", Some(e.to_string()));
                    return;
                }
            };
            job.queue.pop_front();
            self.queued_ids -= 1;
            job.visited.insert(oid.clone());
            job.steps += 1;
            self.metrics.traversal_steps += 1;
            if oid == source {
                self.complete(job, family, "ancestor", None);
                return;
            }
            if commit.parents.len()
                > self
                    .limits
                    .traversal_steps
                    .min(65536)
                    .saturating_sub(self.queued_ids)
            {
                self.complete(
                    job,
                    family,
                    "unknown",
                    Some("history frontier memory budget reached".into()),
                );
                return;
            }
            self.queued_ids += commit.parents.len();
            job.queue.extend(commit.parents);
        }
        self.complete(job, family, "not_ancestor", None);
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
            relations: BTreeMap::new(),
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
        let result = resolve(&git, &family.common_dir.to_path()?, reference);
        self.metrics.spawned_processes += git.spawned_processes();
        self.remember_timeout(
            family,
            GitRead::ResolveCommit {
                reference: reference.clone(),
            },
            &result,
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
