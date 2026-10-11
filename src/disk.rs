//! Bounded metadata-only disk accounting, separate from cleanup evidence.
pub mod storage;
use crate::model::{BytePath, Gap, Identity, Inventory, SweepError, now_ms};
use rustix::{
    fd::OwnedFd,
    fs::{AtFlags, Dir, Mode, OFlags, Stat, fstat, open, openat, statat},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    ffi::OsString,
    os::fd::AsFd,
    os::unix::ffi::OsStringExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiskLimits {
    pub max_entries: usize,
    pub max_entries_per_root: usize,
    pub max_depth: usize,
    pub timeout: Duration,
    pub timeout_per_root: Duration,
}
impl Default for DiskLimits {
    fn default() -> Self {
        Self {
            max_entries: 2_000_000,
            max_entries_per_root: 500_000,
            max_depth: 128,
            timeout: Duration::from_secs(600),
            timeout_per_root: Duration::from_secs(30),
        }
    }
}
impl DiskLimits {
    pub fn thorough() -> Self {
        Self {
            max_entries: 10_000_000,
            max_entries_per_root: 1_000_000,
            timeout: Duration::from_secs(1800),
            timeout_per_root: Duration::from_secs(120),
            ..Self::default()
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiskBytes {
    pub logical: u64,
    pub allocated: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskKind {
    Worktree,
    GitCommon,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiskMeasurement {
    #[serde(default = "requested_by_default", skip_serializing_if = "is_requested")]
    pub requested: bool,
    pub path: BytePath,
    pub kind: DiskKind,
    pub identity: Option<Identity>,
    /// None means the root was not measured. Partial values are observations only.
    pub bytes: Option<DiskBytes>,
    pub complete: bool,
    pub entries_checked: usize,
    pub excluded_registered_roots: usize,
    pub elapsed_ms: u64,
    pub gaps: Vec<Gap>,
    pub gaps_omitted: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiskUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<storage::StorageUsage>,
    /// None preserves the historical all-roots scope. Some contains exact requested paths.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_paths: Option<Vec<BytePath>>,
    pub started_unix_ms: u64,
    pub finished_unix_ms: u64,
    pub elapsed_ms: u64,
    pub limits: DiskLimits,
    pub complete: bool,
    pub temporal_consistency: String,
    /// Distinct (device, inode) across all roots; excludes unresolved observations.
    pub unique_bytes: DiskBytes,
    pub measurements: Vec<DiskMeasurement>,
}

fn requested_by_default() -> bool {
    true
}
fn is_requested(value: &bool) -> bool {
    *value
}

impl DiskUsage {
    pub fn scope_summary(&self) -> String {
        let requested = self.measurements.iter().filter(|r| r.requested).count();
        let measured = self
            .measurements
            .iter()
            .filter(|r| r.requested && r.bytes.is_some())
            .count();
        format!(
            "{requested} requested, {measured} measured, {} not requested; totals cover requested roots only",
            self.measurements.len() - requested
        )
    }
}

/// Counts only. A completed row is attached once per requested root.
pub struct DiskProgress<'a> {
    pub roots_requested: usize,
    pub roots_completed: usize,
    pub entries_checked: usize,
    pub elapsed_ms: u64,
    pub completed: Option<&'a DiskMeasurement>,
}

const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::CLOEXEC)
    .union(OFlags::NONBLOCK);
type Key = (u64, u64);
fn key(st: &Stat) -> Key {
    (st.st_dev as u64, st.st_ino)
}
fn identity_key(id: &Identity) -> Key {
    (id.device, id.inode)
}
fn millis(start: Instant) -> u64 {
    start.elapsed().as_millis().min(u64::MAX as u128) as u64
}
fn dataless(st: &Stat) -> bool {
    #[cfg(target_os = "macos")]
    {
        st.st_flags & 0x4000_0000 != 0
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = st;
        false
    }
}
fn unchanged(a: &Stat, b: &Stat) -> bool {
    key(a) == key(b)
        && a.st_mode == b.st_mode
        && a.st_size == b.st_size
        && a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
        && a.st_ctime == b.st_ctime
        && a.st_ctime_nsec == b.st_ctime_nsec
}
fn add(to: &mut DiskBytes, value: &DiskBytes) -> Result<(), &'static str> {
    let logical = to
        .logical
        .checked_add(value.logical)
        .ok_or("byte count overflow")?;
    let allocated = to
        .allocated
        .checked_add(value.allocated)
        .ok_or("byte count overflow")?;
    to.logical = logical;
    to.allocated = allocated;
    Ok(())
}
fn gap(row: &mut DiskMeasurement, path: &Path, reason: impl Into<String>) {
    row.complete = false;
    if row.gaps.len() < 8 {
        row.gaps.push(Gap {
            path: BytePath::from_path(path),
            reason: reason.into(),
        });
    } else {
        row.gaps_omitted += 1;
    }
}
struct Collector<'a> {
    progress: &'a mut dyn FnMut(DiskProgress<'_>),
    last_progress: Instant,
    roots_requested: usize,
    roots_completed: usize,
    limits: &'a DiskLimits,
    started: Instant,
    entries: usize,
    roots: HashSet<Key>,
    root_paths: HashSet<PathBuf>,
    seen: HashSet<Key>,
    unique: DiskBytes,
    device_bytes: BTreeMap<u64, DiskBytes>,
    volume_probes: storage::native::Probes<'a>,
}
struct Walk {
    entry_allowance: usize,
    time_allowance: Duration,
    started: Instant,
    seen: HashSet<Key>,
    device: u64,
    row: DiskMeasurement,
}
impl Collector<'_> {
    fn report_progress(&mut self, completed: Option<&DiskMeasurement>) {
        if completed.is_none() && self.last_progress.elapsed() < Duration::from_secs(5) {
            return;
        }
        (self.progress)(DiskProgress {
            roots_requested: self.roots_requested,
            roots_completed: self.roots_completed,
            entries_checked: self.entries,
            elapsed_ms: millis(self.started),
            completed,
        });
        if completed.is_none() {
            self.last_progress = Instant::now();
        }
    }
    fn check(&self, w: &Walk) -> Result<(), &'static str> {
        if crate::cancel::requested() {
            return Err("interrupted");
        }
        if self.started.elapsed() >= self.limits.timeout {
            return Err("disk total time budget reached");
        }
        if w.started.elapsed() >= w.time_allowance {
            return Err(if w.time_allowance < self.limits.timeout_per_root {
                "disk fair-share time budget reached"
            } else {
                "disk root time budget reached"
            });
        }
        if self.entries >= self.limits.max_entries {
            return Err("disk total entry budget reached");
        }
        if w.row.entries_checked >= w.entry_allowance {
            return Err(if w.entry_allowance < self.limits.max_entries_per_root {
                "disk fair-share entry budget reached"
            } else {
                "disk root entry budget reached"
            });
        }
        Ok(())
    }
    fn account(&mut self, w: &mut Walk, st: &Stat) -> Result<bool, &'static str> {
        if st.st_dev as u64 != w.device {
            return Err("filesystem boundary not traversed");
        }
        let id = key(st);
        if w.seen.contains(&id) {
            return Ok(false);
        }
        let kind = st.st_mode & libc::S_IFMT;
        let bytes = DiskBytes {
            logical: if kind == libc::S_IFREG || kind == libc::S_IFLNK {
                u64::try_from(st.st_size).map_err(|_| "negative file length")?
            } else {
                0
            },
            allocated: u64::try_from(st.st_blocks)
                .ok()
                .and_then(|n| n.checked_mul(512))
                .ok_or("invalid allocated block count")?,
        };
        let mut next_row = w.row.bytes.clone().unwrap_or_default();
        add(&mut next_row, &bytes)?;
        if !self.seen.contains(&id) {
            storage::add_unique(&mut self.unique, &mut self.device_bytes, id.0, &bytes)?;
            self.seen.insert(id);
        }
        w.row.bytes = Some(next_row);
        w.seen.insert(id);
        Ok(true)
    }
    fn directory(&mut self, w: &mut Walk, fd: OwnedFd, path: &Path, depth: usize, before: &Stat) {
        let mut dir = match Dir::read_from(&fd) {
            Ok(d) => d,
            Err(e) => {
                gap(&mut w.row, path, e.to_string());
                return;
            }
        };
        loop {
            self.report_progress(None);
            if let Err(e) = self.check(w) {
                gap(&mut w.row, path, e);
                break;
            }
            let entry = match dir.next() {
                None => break,
                Some(Ok(e)) => e,
                Some(Err(e)) => {
                    gap(&mut w.row, path, e.to_string());
                    break;
                }
            };
            let name = entry.file_name().to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            let name = OsString::from_vec(name.to_vec());
            let child_path = path.join(&name);
            self.entries += 1;
            w.row.entries_checked += 1;
            let st = match statat(&fd, &name, AtFlags::SYMLINK_NOFOLLOW) {
                Ok(st) => st,
                Err(e) => {
                    gap(&mut w.row, &child_path, e.to_string());
                    continue;
                }
            };
            let is_dir = st.st_mode & libc::S_IFMT == libc::S_IFDIR;
            // Other registered roots are accounted in their own rows, even if their read fails.
            if is_dir && (self.roots.contains(&key(&st)) || self.root_paths.contains(&child_path)) {
                w.row.excluded_registered_roots += 1;
                continue;
            }
            if st.st_dev as u64 != w.device {
                gap(&mut w.row, &child_path, "filesystem boundary not traversed");
                continue;
            }
            let fresh = match self.account(w, &st) {
                Ok(v) => v,
                Err(e) => {
                    gap(&mut w.row, &child_path, e);
                    break;
                }
            };
            if !is_dir || !fresh {
                continue;
            }
            if dataless(&st) {
                gap(&mut w.row, &child_path, "dataless directory not traversed");
                continue;
            }
            if depth >= self.limits.max_depth {
                gap(&mut w.row, &child_path, "disk depth budget reached");
                continue;
            }
            match openat(&fd, &name, DIRECTORY, Mode::empty()) {
                Ok(child) => match fstat(&child) {
                    Ok(after) if unchanged(&st, &after) => {
                        self.directory(w, child, &child_path, depth + 1, &st)
                    }
                    Ok(_) => gap(&mut w.row, &child_path, "directory changed before open"),
                    Err(e) => gap(&mut w.row, &child_path, e.to_string()),
                },
                Err(e) => gap(&mut w.row, &child_path, e.to_string()),
            }
        }
        match fstat(&fd) {
            Ok(after) if unchanged(before, &after) => {}
            Ok(_) => gap(&mut w.row, path, "directory changed during measurement"),
            Err(e) => gap(&mut w.row, path, e.to_string()),
        }
    }
    fn root(&mut self, row: DiskMeasurement) -> DiskMeasurement {
        let remaining = self
            .roots_requested
            .saturating_sub(self.roots_completed)
            .max(1);
        // Reserve a share for every remaining requested root. Unused shares go forward;
        // a partial root is not revisited and never becomes complete by redistribution.
        let entry_allowance = (self.limits.max_entries.saturating_sub(self.entries) / remaining)
            .min(self.limits.max_entries_per_root);
        let time_allowance = self
            .limits
            .timeout
            .saturating_sub(self.started.elapsed())
            .div_f64(remaining as f64)
            .min(self.limits.timeout_per_root);
        let mut w = Walk {
            entry_allowance,
            time_allowance,
            started: Instant::now(),
            seen: HashSet::new(),
            device: 0,
            row,
        };
        let result = (|| -> Result<(), String> {
            self.check(&w).map_err(str::to_string)?;
            let path = w.row.path.to_path().map_err(|e| e.to_string())?;
            let expected = w
                .row
                .identity
                .as_ref()
                .ok_or("discovered root identity unavailable")?;
            let fd = open(&path, DIRECTORY, Mode::empty()).map_err(|e| e.to_string())?;
            let st = fstat(&fd).map_err(|e| e.to_string())?;
            if key(&st) != identity_key(expected) {
                return Err("root identity changed since discovery".into());
            }
            if dataless(&st) {
                return Err("dataless root not traversed".into());
            }
            w.device = st.st_dev as u64;
            let allowed = self.check(&w).is_ok();
            let total_start = self.started;
            let total_timeout = self.limits.timeout;
            let root_start = w.started;
            let root_timeout = w.time_allowance;
            self.volume_probes.observe(
                fd.as_fd(),
                &Identity {
                    device: w.device,
                    inode: st.st_ino,
                },
                &w.row.path,
                || {
                    allowed
                        && total_start.elapsed() < total_timeout
                        && root_start.elapsed() < root_timeout
                },
            );
            self.entries += 1;
            w.row.entries_checked += 1;
            self.account(&mut w, &st).map_err(str::to_string)?;
            self.directory(&mut w, fd, &path, 0, &st);
            let after = statat(rustix::fs::CWD, &path, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|e| e.to_string())?;
            if !unchanged(&st, &after) {
                return Err("root changed during measurement".into());
            }
            Ok(())
        })();
        if let Err(e) = result {
            let p = w.row.path.to_path().unwrap_or_default();
            gap(&mut w.row, &p, e);
        }
        w.row.elapsed_ms = millis(w.started);
        w.row
    }
}

/// Measures each registered non-bare worktree and each shared Git directory.
/// Progress contains completed rows; this makes no Git reads and reads no file contents.
pub fn collect(
    inventory: &mut Inventory,
    limits: &DiskLimits,
    progress: &mut dyn FnMut(&DiskMeasurement),
) -> Result<(), SweepError> {
    collect_selected(inventory, limits, &[], &mut |event| {
        if let Some(row) = event.completed {
            progress(row);
        }
    })
}

/// Select exact registered roots, retaining all other rows as not requested.
/// Emits periodic count-only progress and one completed-row event per requested root.
pub fn collect_selected(
    inventory: &mut Inventory,
    limits: &DiskLimits,
    requested_paths: &[BytePath],
    progress: &mut dyn FnMut(DiskProgress<'_>),
) -> Result<(), SweepError> {
    crate::residency::ensure_no_materialization()?;
    let started = Instant::now();
    let started_unix_ms = now_ms();
    let mut rows = BTreeMap::new();
    for family in &inventory.families {
        rows.insert(
            family.common_dir.clone(),
            (DiskKind::GitCommon, Some(family.identity.clone())),
        );
        for wt in &family.worktrees {
            if !wt.bare {
                rows.entry(wt.path.clone())
                    .or_insert_with(|| (DiskKind::Worktree, wt.identity.clone()));
            }
        }
    }
    let selected: BTreeSet<_> = requested_paths.iter().cloned().collect();
    for path in &selected {
        let decoded = path.to_path()?;
        if !decoded.is_absolute() || !rows.contains_key(path) {
            return Err(SweepError::new(
                "disk_selection",
                "measure_disk",
                &decoded,
                "--disk-root must match an absolute registered worktree or shared Git directory",
            ));
        }
    }
    let roots_requested = if selected.is_empty() {
        rows.len()
    } else {
        selected.len()
    };
    let roots = rows
        .values()
        .filter_map(|(_, id)| id.as_ref().map(identity_key))
        .collect();
    let root_paths = rows.keys().filter_map(|p| p.to_path().ok()).collect();
    let mut collector = Collector {
        progress,
        last_progress: Instant::now(),
        roots_requested,
        roots_completed: 0,
        limits,
        started,
        entries: 0,
        roots,
        root_paths,
        seen: HashSet::new(),
        unique: DiskBytes::default(),
        device_bytes: BTreeMap::new(),
        volume_probes: storage::native::Probes::new(&storage::native::KernelVolumeBackend, 4096),
    };
    let mut measurements = Vec::with_capacity(rows.len());
    for (path, (kind, identity)) in rows {
        collector.report_progress(None);
        let requested = selected.is_empty() || selected.contains(&path);
        let row = DiskMeasurement {
            requested,
            path,
            kind,
            identity,
            bytes: None,
            complete: requested,
            entries_checked: 0,
            excluded_registered_roots: 0,
            elapsed_ms: 0,
            gaps: Vec::new(),
            gaps_omitted: 0,
        };
        let row = if requested {
            let row = collector.root(row);
            collector.roots_completed += 1;
            collector.report_progress(Some(&row));
            row
        } else {
            row
        };
        measurements.push(row);
    }
    let finished_unix_ms = now_ms();
    let mut volumes = collector.volume_probes.spaces;
    let skipped = collector.volume_probes.skipped;
    let devices = collector
        .device_bytes
        .into_iter()
        .map(|(device, bytes)| {
            let volume = volumes.remove(&device);
            let unavailable_reason = if volume.is_some() {
                None
            } else {
                Some(
                    skipped
                        .get(&device)
                        .copied()
                        .unwrap_or("capacity_not_observed")
                        .into(),
                )
            };
            storage::DeviceStorage {
                device,
                bytes,
                volume,
                unavailable_reason,
            }
        })
        .collect();
    inventory.disk_usage = Some(DiskUsage {
        storage: Some(storage::StorageUsage {
            schema: "wt-sweep.storage/1".into(),
            started_unix_ms,
            finished_unix_ms,
            temporal_consistency: "not_guaranteed".into(),
            devices,
        }),
        requested_paths: (!selected.is_empty()).then(|| selected.into_iter().collect()),
        started_unix_ms,
        finished_unix_ms,
        elapsed_ms: millis(started),
        limits: limits.clone(),
        complete: measurements.iter().all(|r| r.complete),
        temporal_consistency: "not_guaranteed".into(),
        unique_bytes: collector.unique,
        measurements,
    });
    Ok(())
}

pub fn human_bytes(value: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let mut n = value as f64;
    let mut index = 0;
    while n >= 1024.0 && index + 1 < units.len() {
        n /= 1024.0;
        index += 1;
    }
    format!("{n:.2} {}", units[index])
}
pub fn render(
    usage: &DiskUsage,
    families: &[crate::model::Family],
    out: &mut impl std::fmt::Write,
) {
    render_summary(Some(usage), out);
    render_measurements(usage, families, out);
}

pub fn render_measurements(
    usage: &DiskUsage,
    families: &[crate::model::Family],
    out: &mut impl std::fmt::Write,
) {
    let worktrees: BTreeMap<_, _> = families
        .iter()
        .flat_map(|f| f.worktrees.iter().map(move |w| (&w.path, (f, w))))
        .collect();
    let _ = writeln!(
        out,
        "  Hidden/ignored files included; symlink targets not followed. Other registered roots counted separately."
    );
    let _ = writeln!(
        out,
        "  Row sizes may overlap through hardlinks/aliases; only the total deduplicates across rows. No atomic snapshot."
    );
    let entries: usize = usage.measurements.iter().map(|r| r.entries_checked).sum();
    let _ = writeln!(
        out,
        "  Entries checked: {entries}; only requested roots contribute to the total."
    );
    let mut rows: Vec<_> = usage.measurements.iter().collect();
    rows.sort_by_key(|r| {
        (
            std::cmp::Reverse(r.bytes.as_ref().map(|b| b.allocated)),
            &r.path,
        )
    });
    for row in rows {
        let kind = if row.kind == DiskKind::Worktree {
            "Worktree"
        } else {
            "Shared Git"
        };
        let state = if !row.requested {
            "not requested"
        } else if row.complete {
            "complete"
        } else if row.bytes.is_some() {
            "partial"
        } else {
            "not measured"
        };
        let size = row
            .bytes
            .as_ref()
            .map(|b| {
                format!(
                    "{} allocated / {} logical",
                    human_bytes(b.allocated),
                    human_bytes(b.logical)
                )
            })
            .unwrap_or_else(|| "size unknown".into());
        let _ = writeln!(
            out,
            "\n  {kind}: {size} [{state}]\n    {}",
            row.path.display()
        );
        if row.kind == DiskKind::Worktree
            && let Some((family, wt)) = worktrees.get(&row.path)
        {
            let branch = wt
                .branch
                .as_ref()
                .map(BytePath::display)
                .unwrap_or_else(|| {
                    if wt.detached {
                        "(detached)".into()
                    } else {
                        "(none)".into()
                    }
                });
            let _ = writeln!(
                out,
                "    Branch: {branch}\n    Git: {}",
                family.common_dir.display()
            );
        }
        for g in &row.gaps {
            let _ = writeln!(
                out,
                "    Gap: {}: {}",
                g.path.display(),
                BytePath::from_bytes(g.reason.as_bytes()).display()
            );
        }
        if row.gaps_omitted > 0 {
            let _ = writeln!(out, "    Additional gaps: {}", row.gaps_omitted);
        }
    }
}

/// Stable rendering of captured measurements. Does not access the filesystem or Git.
/// The scope always covers the source inventory, even in a filtered review view.
pub fn render_summary(usage: Option<&DiskUsage>, out: &mut impl std::fmt::Write) {
    let _ = writeln!(
        out,
        "\nDISK USAGE ({}, metadata only)",
        if usage.is_some_and(|u| u.requested_paths.is_some()) {
            "explicit disk selection"
        } else {
            "all recorded roots"
        }
    );
    let Some(usage) = usage else {
        let _ = writeln!(out, "  Not measured; use scan or review --disk-usage.");
        return;
    };
    let _ = writeln!(out, "  Disk collection: {}", usage.scope_summary());
    let completed = usage.measurements.iter().filter(|r| r.complete).count();
    let worktrees = usage
        .measurements
        .iter()
        .filter(|r| r.kind == DiskKind::Worktree)
        .count();
    let _ = writeln!(
        out,
        "  Measured completely: {completed}/{} roots; {:.1}s",
        usage.measurements.len(),
        usage.elapsed_ms as f64 / 1000.0
    );
    let _ = writeln!(
        out,
        "  Scope: {worktrees} worktree directories and {} shared Git directories; unaffected by review filters or --worktree.",
        usage.measurements.len() - worktrees
    );
    let _ = writeln!(
        out,
        "  Observed allocated, deduplicated: {}",
        human_bytes(usage.unique_bytes.allocated)
    );
    let _ = writeln!(
        out,
        "  Observed logical file sizes, deduplicated: {}",
        human_bytes(usage.unique_bytes.logical)
    );
    let _ = writeln!(
        out,
        "  Disk observation interval (Unix ms): {}..{}; recorded sizes, not remeasured during rendering.",
        usage.started_unix_ms, usage.finished_unix_ms
    );
    let _ = writeln!(
        out,
        "  These are filesystem observations, not reclaimable space. APFS shared extents and snapshots are not deduplicated."
    );
    if !usage.complete {
        let _ = writeln!(
            out,
            "  INCOMPLETE: totals cover observed entries only; missing sizes are unknown."
        );
    }
}

#[cfg(test)]
mod progress_tests {
    use super::*;

    #[test]
    fn account_refuses_another_device_then_counts_its_own_root_without_duplicate_inodes() {
        let limits = DiskLimits::default();
        let mut progress = |_: DiskProgress<'_>| {};
        let started = Instant::now();
        let mut collector = Collector {
            progress: &mut progress,
            last_progress: started,
            roots_requested: 2,
            roots_completed: 0,
            limits: &limits,
            started,
            entries: 0,
            roots: HashSet::new(),
            root_paths: HashSet::new(),
            seen: HashSet::new(),
            unique: DiskBytes::default(),
            device_bytes: BTreeMap::new(),
            volume_probes: storage::native::Probes::new(
                &storage::native::KernelVolumeBackend,
                4096,
            ),
        };
        let mut walk = Walk {
            entry_allowance: 10,
            time_allowance: limits.timeout_per_root,
            started,
            seen: HashSet::new(),
            device: 7,
            row: DiskMeasurement {
                requested: true,
                path: BytePath::from_bytes(b"/synthetic"),
                kind: DiskKind::Worktree,
                identity: None,
                bytes: None,
                complete: true,
                entries_checked: 0,
                excluded_registered_roots: 0,
                elapsed_ms: 0,
                gaps: vec![],
                gaps_omitted: 0,
            },
        };
        // Only obtain the platform Stat layout; inode data below is synthetic.
        let fd = open(std::env::temp_dir(), DIRECTORY, Mode::empty())
            .unwrap_or_else(|e| panic!("fixture stat: {e}"));
        let mut st = fstat(&fd).unwrap_or_else(|e| panic!("fixture stat: {e}"));
        st.st_dev = 7;
        st.st_ino = 42;
        st.st_size = 5;
        st.st_blocks = 8;
        st.st_mode = libc::S_IFREG;
        assert_eq!(collector.account(&mut walk, &st), Ok(true));
        assert_eq!(collector.account(&mut walk, &st), Ok(false));
        st.st_dev = 8;
        st.st_ino = 42;
        st.st_size = 7;
        st.st_blocks = 16;
        assert_eq!(
            collector.account(&mut walk, &st),
            Err("filesystem boundary not traversed")
        );
        assert_eq!(collector.unique.allocated, 4096);
        assert!(!collector.device_bytes.contains_key(&8));
        walk.device = 8;
        assert_eq!(collector.account(&mut walk, &st), Ok(true));
        assert_eq!(
            (collector.unique.logical, collector.unique.allocated),
            (12, 12288)
        );
        assert_eq!(collector.device_bytes.len(), 2);
    }

    #[test]
    fn root_completion_does_not_postpone_periodic_progress() {
        let limits = DiskLimits::default();
        let started = Instant::now() - Duration::from_millis(4900);
        let mut events = Vec::new();
        let mut callback = |event: DiskProgress<'_>| events.push(event.completed.is_some());
        let mut collector = Collector {
            progress: &mut callback,
            last_progress: started,
            roots_requested: 2,
            roots_completed: 1,
            limits: &limits,
            started,
            entries: 4,
            roots: HashSet::new(),
            root_paths: HashSet::new(),
            seen: HashSet::new(),
            unique: DiskBytes::default(),
            device_bytes: BTreeMap::new(),
            volume_probes: storage::native::Probes::new(
                &storage::native::KernelVolumeBackend,
                4096,
            ),
        };
        let row = DiskMeasurement {
            requested: true,
            path: BytePath::from_bytes(b"/synthetic"),
            kind: DiskKind::Worktree,
            identity: None,
            bytes: None,
            complete: false,
            entries_checked: 4,
            excluded_registered_roots: 0,
            elapsed_ms: 4900,
            gaps: Vec::new(),
            gaps_omitted: 0,
        };
        collector.report_progress(Some(&row));
        std::thread::sleep(Duration::from_millis(150));
        collector.report_progress(None);
        assert_eq!(
            events,
            [true, false],
            "completion must not defer the periodic counter"
        );
    }
}
