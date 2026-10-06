//! Descriptor-anchored, bounded history sources. No Git interpretation lives here.
use super::*;
use crate::model::HistorySource;
use std::os::unix::ffi::OsStrExt;

type SourceObservation = (Option<Stat>, Vec<u8>, Vec<OsString>);
/// Only direct identity/stat/hash comparisons construct Changed. I/O prose has no authority.
enum SourceFailure {
    Unavailable(String),
    Changed(String),
}
impl From<String> for SourceFailure {
    fn from(reason: String) -> Self {
        Self::Unavailable(reason)
    }
}
impl From<&str> for SourceFailure {
    fn from(reason: &str) -> Self {
        Self::Unavailable(reason.into())
    }
}
impl SourceFailure {
    fn validation(self) -> Validation {
        match self {
            Self::Unavailable(reason) => Validation {
                changed: false,
                reason,
            },
            Self::Changed(reason) => Validation {
                changed: true,
                reason,
            },
        }
    }
}

#[derive(Clone)]
struct Saved {
    source: HistorySource,
    stat: Option<Stat>,
    directory: bool,
    initial_error: Option<Validation>,
}
pub(crate) struct Sources<'a> {
    pub(crate) records: Vec<HistorySource>,
    saved: Vec<Saved>,
    hooks: &'a dyn LocalHooks,
    pub(crate) bytes: usize,
    max_bytes: usize,
    max_file: usize,
    max_entries: usize,
    max_output: usize,
    output_bytes: usize,
    started: Instant,
    timeout: Duration,
}
#[derive(Clone, Debug)]
pub(crate) struct Validation {
    pub(crate) changed: bool,
    pub(crate) reason: String,
}
impl<'a> Sources<'a> {
    pub(crate) fn new(
        hooks: &'a dyn LocalHooks,
        entries: usize,
        bytes: usize,
        file: usize,
        output: usize,
        timeout: Duration,
    ) -> Self {
        Self {
            records: Vec::new(),
            saved: Vec::new(),
            hooks,
            bytes: 0,
            max_bytes: bytes,
            max_file: file,
            max_entries: entries,
            max_output: output,
            output_bytes: 0,
            started: Instant::now(),
            timeout,
        }
    }
    pub(crate) fn reserve_final_bytes(&mut self, remaining: usize) {
        self.max_bytes = self.bytes.saturating_add(remaining);
    }
    pub(crate) fn output_charge(&self) -> usize {
        self.output_bytes
    }
    fn check(&self) -> Result<(), String> {
        if self.hooks.cancelled() || self.started.elapsed() >= self.timeout {
            Err("history source collection cancelled or time budget reached".into())
        } else {
            Ok(())
        }
    }
    fn reserve(&mut self, path: &Path, kind: &str, directory: bool) -> Result<usize, String> {
        self.check()?;
        let charge = path
            .as_os_str()
            .as_bytes()
            .len()
            .saturating_add(kind.len() + 256);
        if self.records.len() >= self.max_entries
            || self.output_bytes.saturating_add(charge) > self.max_output
        {
            return Err("history source entry or output budget reached".into());
        }
        self.output_bytes += charge;
        let source = HistorySource {
            path: BytePath::from_path(path),
            kind: kind.into(),
            exists: None,
            sha256: None,
            bytes: None,
            complete: false,
        };
        let i = self.records.len();
        self.records.push(source.clone());
        self.saved.push(Saved {
            source,
            stat: None,
            directory,
            initial_error: None,
        });
        Ok(i)
    }
    pub(crate) fn file(
        &mut self,
        path: &Path,
        kind: &str,
    ) -> Result<(usize, Option<Vec<u8>>), String> {
        let i = self.reserve(path, kind, false)?;
        match self.observe(path, false) {
            Ok((stat, raw, _)) => {
                self.accept(i, stat, &raw);
                Ok((i, stat.map(|_| raw)))
            }
            Err(e) => {
                let validation = e.validation();
                self.saved[i].initial_error = Some(Validation {
                    changed: validation.changed,
                    reason: validation.reason.clone(),
                });
                Err(format!(
                    "{}: {}",
                    BytePath::from_path(path).display(),
                    validation.reason
                ))
            }
        }
    }
    pub(crate) fn directory(
        &mut self,
        path: &Path,
        kind: &str,
    ) -> Result<(usize, Vec<OsString>), String> {
        let i = self.reserve(path, kind, true)?;
        match self.observe(path, true) {
            Ok((stat, raw, names)) => {
                self.accept(i, stat, &raw);
                Ok((i, names))
            }
            Err(e) => {
                let validation = e.validation();
                self.saved[i].initial_error = Some(Validation {
                    changed: validation.changed,
                    reason: validation.reason.clone(),
                });
                Err(format!(
                    "{}: {}",
                    BytePath::from_path(path).display(),
                    validation.reason
                ))
            }
        }
    }
    fn accept(&mut self, i: usize, stat: Option<Stat>, raw: &[u8]) {
        let source = &mut self.records[i];
        source.exists = Some(stat.is_some());
        source.complete = true;
        if stat.is_some() {
            source.bytes = Some(raw.len() as u64);
            source.sha256 = Some(format!("{:x}", Sha256::digest(raw)));
        }
        self.saved[i].source = source.clone();
        self.saved[i].stat = stat;
    }
    fn observe(
        &mut self,
        path: &Path,
        directory: bool,
    ) -> Result<SourceObservation, SourceFailure> {
        self.check()?;
        self.hooks
            .before_entry(&BytePath::from_path(path))
            .map_err(|e| e.to_string())?;
        let parent = path.parent().ok_or("source parent absent")?;
        let Some(fd) = optional_directory(parent, self.hooks)? else {
            return Ok((None, Vec::new(), Vec::new()));
        };
        let name = path.file_name().ok_or("source filename absent")?;
        let before = match statat(&fd, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(s) => s,
            Err(e) if e == rustix::io::Errno::NOENT => return Ok((None, Vec::new(), Vec::new())),
            Err(e) => return Err(e.to_string().into()),
        };
        if self.hooks.is_dataless(&BytePath::from_path(path), &before) {
            return Err("nonresident history source refused".into());
        }
        let expected = if directory { 0o040000 } else { 0o100000 };
        if before.st_mode & 0o170000 != expected {
            return Err("history source is not a regular file or expected directory".into());
        }
        let opened = openat(
            &fd,
            name,
            if directory {
                DIRECTORY
            } else {
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC
            },
            Mode::empty(),
        )
        .map_err(|e| e.to_string())?;
        self.hooks
            .after_file_open(&BytePath::from_path(path))
            .map_err(|e| e.to_string())?;
        let initial = fstat(&opened).map_err(|e| e.to_string())?;
        if !same_stat(&before, &initial) {
            return Err(SourceFailure::Changed(
                "history source changed while opening".into(),
            ));
        }
        let mut raw = Vec::new();
        let mut names = Vec::new();
        if directory {
            // A directory digest binds its own identity as well as member identities.
            raw.extend_from_slice(&(initial.st_dev as u64).to_be_bytes());
            raw.extend_from_slice(&initial.st_ino.to_be_bytes());
            raw.extend_from_slice(&initial.st_mode.to_be_bytes());
            if raw.len() > self.max_file || raw.len() > self.max_bytes.saturating_sub(self.bytes) {
                return Err("history directory digest byte budget reached".into());
            }
            let mut dir = Dir::read_from(&opened).map_err(|e| e.to_string())?;
            for entry in &mut dir {
                self.check()?;
                let entry = entry.map_err(|e| e.to_string())?;
                let b = entry.file_name().to_bytes();
                if b == b"." || b == b".." {
                    continue;
                }
                if names.len() >= self.max_entries {
                    return Err("history directory membership budget reached".into());
                }
                names.push(OsString::from_vec(b.to_vec()));
            }
            names.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            for name in &names {
                let stat =
                    statat(&opened, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|e| e.to_string())?;
                let b = name.as_bytes();
                raw.extend_from_slice(&(b.len() as u64).to_be_bytes());
                raw.extend_from_slice(b);
                raw.extend_from_slice(&(stat.st_dev as u64).to_be_bytes());
                raw.extend_from_slice(&stat.st_ino.to_be_bytes());
                raw.extend_from_slice(&stat.st_mode.to_be_bytes());
                if raw.len() > self.max_file
                    || raw.len() > self.max_bytes.saturating_sub(self.bytes)
                {
                    return Err("history directory digest byte budget reached".into());
                }
            }
            self.bytes = self.bytes.saturating_add(raw.len());
        } else {
            let size = usize::try_from(before.st_size).map_err(|_| "invalid source size")?;
            if size > self.max_file || size > self.max_bytes.saturating_sub(self.bytes) {
                return Err("history source file or total byte budget reached".into());
            }
            let remaining = self.max_file.min(self.max_bytes.saturating_sub(self.bytes));
            File::from(opened.try_clone().map_err(|e| e.to_string())?)
                .take(remaining.saturating_add(1) as u64)
                .read_to_end(&mut raw)
                .map_err(|e| e.to_string())?;
            self.bytes = self.bytes.saturating_add(raw.len());
            if raw.len() > remaining {
                return Err("history source byte budget reached".into());
            }
        }
        let after = fstat(&opened).map_err(|e| e.to_string())?;
        let named = statat(&fd, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|e| e.to_string())?;
        if !same_stat(&before, &after) || !same_stat(&after, &named) {
            return Err(SourceFailure::Changed(
                "history source changed during read".into(),
            ));
        }
        Ok((Some(after), raw, names))
    }
    pub(crate) fn validate(&mut self) -> Vec<Option<Validation>> {
        self.saved
            .clone()
            .into_iter()
            .map(|saved| {
                if !saved.source.complete {
                    return saved.initial_error.or(Some(Validation {
                        changed: false,
                        reason: "initial source read incomplete".into(),
                    }));
                }
                let path = match saved.source.path.to_path() {
                    Ok(p) => p,
                    Err(e) => {
                        return Some(Validation {
                            changed: false,
                            reason: e.to_string(),
                        });
                    }
                };
                match self.observe(&path, saved.directory) {
                    Ok((stat, raw, _)) => {
                        let matches = match (saved.stat, stat) {
                            (None, None) => true,
                            (Some(a), Some(b)) => same_stat(&a, &b),
                            _ => false,
                        };
                        let hash = stat.map(|_| format!("{:x}", Sha256::digest(&raw)));
                        if matches && hash == saved.source.sha256 {
                            None
                        } else {
                            Some(Validation {
                                changed: true,
                                reason: format!(
                                    "history source changed: {}",
                                    saved.source.path.display()
                                ),
                            })
                        }
                    }
                    Err(e) => {
                        let mut validation = e.validation();
                        validation.reason = format!(
                            "history source final validation incomplete: {}",
                            validation.reason
                        );
                        Some(validation)
                    }
                }
            })
            .collect()
    }
}
/// Resolve an existing family directory without following links or enumerating it.
pub(crate) fn directory_identity(
    path: &Path,
    hooks: &dyn LocalHooks,
) -> Result<Identity, SweepError> {
    crate::residency::ensure_no_materialization()?;
    let error = |message| SweepError::new("family_identity", "directory_identity", path, message);
    if hooks.cancelled() {
        return Err(error("family identity collection cancelled".to_owned()));
    }
    hooks
        .before_entry(&BytePath::from_path(path))
        .map_err(|e| error(e.to_string()))?;
    let fd = optional_directory(path, hooks)
        .map_err(|e| error(e.validation().reason))?
        .ok_or_else(|| error("family directory absent".to_owned()))?;
    if hooks.cancelled() {
        return Err(error("family identity collection cancelled".to_owned()));
    }
    let stat = fstat(&fd).map_err(|e| error(e.to_string()))?;
    Ok(identity(&stat))
}

fn optional_directory(
    path: &Path,
    hooks: &dyn LocalHooks,
) -> Result<Option<OwnedFd>, SourceFailure> {
    if !path.is_absolute() {
        return Err("source parent must be absolute".into());
    }
    let mut fd = rustix::fs::open("/", DIRECTORY, Mode::empty()).map_err(|e| e.to_string())?;
    let mut partial = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                partial.push(name);
                let stat = match statat(&fd, name, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(s) => s,
                    Err(e) if e == rustix::io::Errno::NOENT => return Ok(None),
                    Err(e) => return Err(e.to_string().into()),
                };
                if hooks.is_dataless(&BytePath::from_path(&partial), &stat) {
                    return Err("nonresident source directory refused".into());
                }
                let opened =
                    openat(&fd, name, DIRECTORY, Mode::empty()).map_err(|e| e.to_string())?;
                let opened_stat = fstat(&opened).map_err(|e| e.to_string())?;
                let named =
                    statat(&fd, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|e| e.to_string())?;
                // Ancestors establish path continuity, not a membership snapshot.
                // Unrelated siblings may change their size, nlink and timestamps.
                if !same_directory_identity(&stat, &opened_stat)
                    || !same_directory_identity(&opened_stat, &named)
                {
                    return Err(SourceFailure::Changed("source parent changed".into()));
                }
                fd = opened;
            }
            _ => return Err("unsafe source parent component".into()),
        }
    }
    Ok(Some(fd))
}

fn same_directory_identity(a: &Stat, b: &Stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_mode & 0o170000 == 0o040000
        && b.st_mode & 0o170000 == 0o040000
}
