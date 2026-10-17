//! Bounded local observations. This is an inventory, never a backup or snapshot.
pub mod comparison;
pub(crate) mod history;
use crate::git::{ControlledGit, GitRead};
use crate::model::{
    BytePath, Gap, Identity, IndexEntry, LocalEntry, LocalInventory, SweepError, Worktree, now_ms,
};
use rustix::fs::{AtFlags, Dir, Mode, OFlags, Stat, fstat, openat, statat};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStringExt;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct LocalLimits {
    pub max_entries: usize,
    pub max_total_bytes: u64,
    pub max_file_bytes: u64,
    pub max_depth: usize,
    pub max_manifest_bytes: usize,
    pub timeout: Duration,
}
impl Default for LocalLimits {
    fn default() -> Self {
        Self {
            max_entries: 20_000,
            max_total_bytes: 256 * 1024 * 1024,
            max_file_bytes: 64 * 1024 * 1024,
            max_depth: 256,
            max_manifest_bytes: 8 * 1024 * 1024,
            timeout: Duration::from_secs(60),
        }
    }
}

/// Error/mutation injection only. Real filesystem observations always use the
/// same descriptor-anchored implementation as production.
pub trait LocalHooks {
    fn before_entry(&self, _path: &BytePath) -> io::Result<()> {
        Ok(())
    }
    fn after_file_open(&self, _path: &BytePath) -> io::Result<()> {
        Ok(())
    }
    fn cancelled(&self) -> bool {
        false
    }
    fn is_dataless(&self, _path: &BytePath, stat: &Stat) -> bool {
        flags(stat) & 0x4000_0000 != 0
    }
}
pub struct NativeHooks;
impl LocalHooks for NativeHooks {
    fn cancelled(&self) -> bool {
        crate::cancel::requested()
    }
}

pub fn collect(worktree: &Worktree, limits: &LocalLimits) -> Result<LocalInventory, SweepError> {
    collect_with(worktree, limits, &NativeHooks)
}

fn flags(stat: &Stat) -> u32 {
    #[cfg(target_os = "macos")]
    {
        stat.st_flags
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = stat;
        0
    }
}
fn identity(stat: &Stat) -> Identity {
    Identity {
        device: stat.st_dev as u64,
        inode: stat.st_ino,
    }
}
fn same_stat(a: &Stat, b: &Stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_mode == b.st_mode
        && a.st_uid == b.st_uid
        && a.st_gid == b.st_gid
        && a.st_nlink == b.st_nlink
        && a.st_size == b.st_size
        && a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
        && a.st_ctime == b.st_ctime
        && a.st_ctime_nsec == b.st_ctime_nsec
        && flags(a) == flags(b)
}
const DIRECTORY: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);
fn open_absolute_directory(path: &Path) -> io::Result<OwnedFd> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "directory must be absolute",
        ));
    }
    let mut fd = rustix::fs::open("/", DIRECTORY, Mode::empty())?;
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => {
                fd = openat(&fd, name, DIRECTORY, Mode::empty())?;
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "directory contains an unsafe component",
                ));
            }
        }
    }
    Ok(fd)
}
fn path_from_output(raw: &[u8]) -> io::Result<PathBuf> {
    let bytes = raw
        .strip_suffix(b"\n")
        .ok_or_else(|| io::Error::other("Git path lacks final newline"))?;
    let path = PathBuf::from(OsString::from_vec(bytes.to_vec()));
    if bytes.is_empty() || bytes.contains(&0) || !path.is_absolute() {
        return Err(io::Error::other("invalid absolute Git path"));
    }
    Ok(path)
}
fn valid_relative(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && !bytes.contains(&0)
        && !bytes.starts_with(b"/")
        && bytes
            .split(|b| *b == b'/')
            .all(|part| !part.is_empty() && part != b"." && part != b"..")
}
fn parse_paths(raw: &[u8]) -> io::Result<BTreeSet<BytePath>> {
    if !raw.is_empty() && !raw.ends_with(&[0]) {
        return Err(io::Error::other("unterminated Git path list"));
    }
    let mut paths = BTreeSet::new();
    for path in raw.split(|b| *b == 0).filter(|p| !p.is_empty()) {
        let path = path.strip_prefix(b"./").unwrap_or(path);
        let path = path.strip_suffix(b"/").unwrap_or(path);
        if !valid_relative(path) || !paths.insert(BytePath::from_bytes(path)) {
            return Err(io::Error::other("malformed or duplicate Git path"));
        }
    }
    Ok(paths)
}
fn parse_index_text(raw: &[u8], max_entries: usize) -> io::Result<Vec<IndexEntry>> {
    if !raw.is_empty() && !raw.ends_with(&[0]) {
        return Err(io::Error::other("unterminated Git index list"));
    }
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for record in raw.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        if entries.len() >= max_entries {
            return Err(io::Error::other("Git index entry limit reached"));
        }
        let split = record
            .iter()
            .position(|b| *b == b'\t')
            .ok_or_else(|| io::Error::other("missing index path separator"))?;
        let fields: Vec<_> = record[..split].split(|b| *b == b' ').collect();
        if fields.len() != 3 || !valid_relative(&record[split + 1..]) {
            return Err(io::Error::other("malformed index entry"));
        }
        fn text(bytes: &[u8]) -> io::Result<&str> {
            std::str::from_utf8(bytes).map_err(|_| io::Error::other("nonASCII index metadata"))
        }
        let mode = u32::from_str_radix(text(fields[0])?, 8)
            .map_err(|_| io::Error::other("invalid index mode"))?;
        if !matches!(mode, 0o100644 | 0o100755 | 0o120000 | 0o160000)
            || !matches!(fields[1].len(), 40 | 64)
            || !fields[1].iter().all(u8::is_ascii_hexdigit)
            || fields[1].iter().all(|b| *b == b'0')
            || fields[2].len() != 1
            || !(b'0'..=b'3').contains(&fields[2][0])
        {
            return Err(io::Error::other("invalid index mode, OID or stage"));
        }
        let path = BytePath::from_bytes(&record[split + 1..]);
        let stage = fields[2][0] - b'0';
        if !seen.insert((path.clone(), stage)) {
            return Err(io::Error::other("duplicate index path/stage"));
        }
        entries.push(IndexEntry {
            path,
            mode,
            oid: text(fields[1])?.to_ascii_lowercase(),
            stage,
        });
    }
    entries.sort_by(|a, b| (&a.path, a.stage).cmp(&(&b.path, b.stage)));
    Ok(entries)
}

struct RawIndex {
    entries: Vec<IndexEntry>,
    shared: Option<String>,
    intent_to_add: BTreeSet<BytePath>,
}
fn parse_raw_index(raw: &[u8], hash_size: usize, max_entries: usize) -> io::Result<RawIndex> {
    let invalid = || io::Error::other("malformed or unsupported raw index");
    if !matches!(hash_size, 20 | 32) || raw.len() < 12 + hash_size || &raw[..4] != b"DIRC" {
        return Err(invalid());
    }
    let number = |bytes: &[u8]| -> io::Result<u32> {
        let array: [u8; 4] = bytes.try_into().map_err(|_| invalid())?;
        Ok(u32::from_be_bytes(array))
    };
    let version = number(&raw[4..8])?;
    if !matches!(version, 2 | 3) {
        return Err(io::Error::other(
            "raw index version is not supported; index classifications remain unknown",
        ));
    }
    let count = number(&raw[8..12])? as usize;
    if count > max_entries {
        return Err(io::Error::other("raw index entry limit reached"));
    }
    let end = raw.len() - hash_size;
    let computed = if hash_size == 20 {
        let mut hash = sha1_checked::Sha1::new();
        hash.update(&raw[..end]);
        let result = hash.try_finalize();
        if result.has_collision() {
            return Err(io::Error::other("SHA1 collision in raw index"));
        }
        result.hash().to_vec()
    } else {
        Sha256::digest(&raw[..end]).to_vec()
    };
    if computed != raw[end..] {
        return Err(io::Error::other("raw index checksum mismatch"));
    }
    let mut offset = 12usize;
    let mut entries = Vec::new();
    let mut intent_to_add = BTreeSet::new();
    for _ in 0..count {
        let start = offset;
        let fixed = 40 + hash_size + 2;
        let header = raw
            .get(offset..offset.checked_add(fixed).ok_or_else(invalid)?)
            .filter(|_| offset + fixed <= end)
            .ok_or_else(invalid)?;
        let mode = number(&header[24..28])?;
        let oid = header[40..40 + hash_size]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let flags = u16::from_be_bytes([header[40 + hash_size], header[41 + hash_size]]);
        offset += fixed;
        let mut intent = false;
        if flags & 0x4000 != 0 {
            if version != 3 || offset + 2 > end {
                return Err(invalid());
            }
            let extended_flags = u16::from_be_bytes([raw[offset], raw[offset + 1]]);
            if extended_flags & !0x6000 != 0 {
                return Err(io::Error::other("reserved extended flags in raw index"));
            }
            intent = extended_flags & 0x2000 != 0;
            offset += 2;
        }
        let remainder = raw.get(offset..end).ok_or_else(invalid)?;
        let length = remainder.iter().position(|b| *b == 0).ok_or_else(invalid)?;
        let path = &remainder[..length];
        let encoded_length = (flags & 0x0fff) as usize;
        if encoded_length != 0x0fff && encoded_length != length {
            return Err(invalid());
        }
        let padded = (offset - start + length + 1)
            .checked_add(7)
            .ok_or_else(invalid)?
            / 8
            * 8;
        let next = start
            .checked_add(padded)
            .filter(|n| *n <= end)
            .ok_or_else(invalid)?;
        if raw[offset + length..next].iter().any(|b| *b != 0) {
            return Err(invalid());
        }
        if intent {
            if flags & 0x3000 != 0 {
                return Err(io::Error::other(
                    "intent-to-add conflict stage is unsupported",
                ));
            }
            intent_to_add.insert(BytePath::from_bytes(path));
        }
        entries.push(IndexEntry {
            path: BytePath::from_bytes(path),
            mode,
            oid,
            stage: ((flags & 0x3000) >> 12) as u8,
        });
        offset = next;
    }
    let mut shared = None;
    let mut extensions = BTreeSet::new();
    while offset < end {
        let header = raw
            .get(offset..offset + 8)
            .filter(|_| offset + 8 <= end)
            .ok_or_else(invalid)?;
        let signature = &header[..4];
        let size = number(&header[4..8])? as usize;
        let next = offset
            .checked_add(8)
            .and_then(|v| v.checked_add(size))
            .filter(|v| *v <= end)
            .ok_or_else(invalid)?;
        if !extensions.insert(signature.to_vec()) {
            return Err(invalid());
        }
        if signature == b"link" {
            if size < hash_size {
                return Err(invalid());
            }
            let hash = &raw[offset + 8..offset + 8 + hash_size];
            if hash.iter().all(|b| *b == 0) {
                return Err(invalid());
            }
            shared = Some(hash.iter().map(|b| format!("{b:02x}")).collect());
        } else if !signature[0].is_ascii_uppercase() {
            return Err(io::Error::other(
                "required raw index extension is unsupported",
            ));
        }
        offset = next;
    }
    if shared.is_none() {
        // Reuse the strict public Git index record grammar after structurally
        // locating every raw entry and verifying the checksum.
        let mut text = Vec::new();
        for entry in &entries {
            text.extend_from_slice(
                format!("{:06o} {} {}\t", entry.mode, entry.oid, entry.stage).as_bytes(),
            );
            text.extend_from_slice(&entry.path.bytes().map_err(|_| invalid())?);
            text.push(0);
        }
        entries = parse_index_text(&text, max_entries)?;
    } else {
        entries.clear();
    }
    Ok(RawIndex {
        entries,
        shared,
        intent_to_add,
    })
}

struct Context<'a> {
    inventory: LocalInventory,
    limits: &'a LocalLimits,
    hooks: &'a dyn LocalHooks,
    started: Instant,
    total_bytes: u64,
    manifest_bytes: usize,
    stopped: bool,
    categories: BTreeMap<BytePath, String>,
    raw_index: Option<Vec<u8>>,
}
impl Context<'_> {
    fn entry_count(&self) -> usize {
        self.inventory.entries.len()
            + self.inventory.index_entries.len()
            + self.inventory.administration_entries.len()
    }
    fn gap(&mut self, path: &Path, reason: impl Into<String>) {
        self.inventory.traversal_complete = false;
        let reason = reason.into();
        // Keep diagnostics themselves bounded when a budget stops the walk.
        if self.inventory.gaps.len() < 256 {
            self.inventory.gaps.push(Gap {
                path: BytePath::from_path(path),
                reason,
            });
        }
    }
    fn check(&mut self, path: &Path) -> bool {
        if self.stopped {
            return false;
        }
        let reason = if self.hooks.cancelled() {
            Some("local inventory cancelled")
        } else if self.started.elapsed() >= self.limits.timeout {
            Some("local inventory deadline reached")
        } else if self.entry_count() >= self.limits.max_entries {
            Some("local inventory entry limit reached")
        } else if self.manifest_bytes >= self.limits.max_manifest_bytes {
            Some("local manifest byte limit reached")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.gap(path, reason);
            self.stopped = true;
            false
        } else {
            true
        }
    }
    fn push(&mut self, entry: LocalEntry, admin: bool) {
        if self.entry_count() >= self.limits.max_entries {
            self.gap(
                &entry.path.to_path().unwrap_or_else(|_| PathBuf::from(".")),
                "local inventory entry limit reached",
            );
            self.stopped = true;
            return;
        }
        let bytes = serde_json::to_vec(&entry)
            .map(|bytes| bytes.len())
            .unwrap_or(usize::MAX);
        if self.manifest_bytes.saturating_add(bytes) > self.limits.max_manifest_bytes {
            self.gap(
                &entry.path.to_path().unwrap_or_else(|_| PathBuf::from(".")),
                "local manifest byte limit reached",
            );
            self.stopped = true;
            return;
        }
        self.manifest_bytes += bytes;
        if admin {
            self.inventory.administration_entries.push(entry);
        } else {
            self.inventory.entries.push(entry);
        }
    }
    fn category(&self, path: &Path) -> String {
        let bytes = BytePath::from_path(path);
        if let Some(category) = self.categories.get(&bytes) {
            return category.clone();
        }
        "unknown".into()
    }
    fn read_entry(
        &mut self,
        parent: &OwnedFd,
        name: &std::ffi::OsStr,
        path: &Path,
        category: String,
    ) -> Option<(LocalEntry, Option<OwnedFd>)> {
        let before = match statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(value) => value,
            Err(error) => {
                self.gap(path, format!("metadata read failed: {error}"));
                return None;
            }
        };
        let bp = BytePath::from_path(path);
        if let Err(error) = self.hooks.before_entry(&bp) {
            self.gap(path, format!("entry refused: {error}"));
            return None;
        }
        let kind = match before.st_mode & libc::S_IFMT {
            libc::S_IFREG => "file",
            libc::S_IFDIR => "directory",
            libc::S_IFLNK => "symlink",
            libc::S_IFIFO => "fifo",
            libc::S_IFSOCK => "socket",
            libc::S_IFCHR => "character_device",
            libc::S_IFBLK => "block_device",
            _ => "unknown",
        };
        let mut entry = LocalEntry {
            path: bp.clone(),
            kind: kind.into(),
            category,
            identity: identity(&before),
            mode: before.st_mode as u32,
            uid: before.st_uid,
            gid: before.st_gid,
            size: before.st_size.max(0) as u64,
            links: before.st_nlink as u64,
            modified_sec: before.st_mtime as i64,
            modified_nsec: before.st_mtime_nsec as i64,
            changed_sec: before.st_ctime as i64,
            changed_nsec: before.st_ctime_nsec as i64,
            flags: flags(&before),
            content_sha256: None,
            link_target: None,
        };
        if self.hooks.is_dataless(&bp, &before) {
            self.gap(path, "dataless entry; content read refused");
            return Some((entry, None));
        }
        let mut child = None;
        match kind {
            "directory" => match openat(parent, name, DIRECTORY, Mode::empty()) {
                Ok(fd) => match fstat(&fd) {
                    Ok(stat) if same_stat(&before, &stat) => child = Some(fd),
                    Ok(_) => self.gap(path, "directory changed between observation and open"),
                    Err(error) => self.gap(path, format!("directory fstat failed: {error}")),
                },
                Err(error) => self.gap(path, format!("directory open refused: {error}")),
            },
            "symlink" => {
                let mut buffer = [0u8; 4096];
                match rustix::fs::readlinkat_raw(parent, name, &mut buffer[..]) {
                    Ok(size) if size < buffer.len() => {
                        entry.link_target = Some(BytePath::from_bytes(&buffer[..size]))
                    }
                    Ok(_) => self.gap(path, "symlink target byte limit reached"),
                    Err(error) => self.gap(path, format!("symlink read failed: {error}")),
                }
            }
            "file" => {
                if entry.size > self.limits.max_file_bytes
                    || entry.size > self.limits.max_total_bytes.saturating_sub(self.total_bytes)
                {
                    self.gap(path, "file or total content byte limit reached");
                } else {
                    match openat(
                        parent,
                        name,
                        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                        Mode::empty(),
                    ) {
                        Ok(fd) => match fstat(&fd) {
                            Ok(stat) if same_stat(&before, &stat) => {
                                if let Err(error) = self.hooks.after_file_open(&bp) {
                                    self.gap(path, format!("opened file refused: {error}"));
                                } else {
                                    let mut file = File::from(fd);
                                    let mut hash = Sha256::new();
                                    let mut raw = if entry.category == "index" {
                                        Some(Vec::new())
                                    } else {
                                        None
                                    };
                                    let mut buffer = [0u8; 16384];
                                    let mut read = 0u64;
                                    let mut valid = true;
                                    loop {
                                        if self.hooks.cancelled()
                                            || self.started.elapsed() >= self.limits.timeout
                                        {
                                            self.gap(
                                                path,
                                                "file read cancelled or deadline reached",
                                            );
                                            self.stopped = true;
                                            valid = false;
                                            break;
                                        }
                                        let available =
                                            self.limits.max_file_bytes.saturating_sub(read).min(
                                                self.limits
                                                    .max_total_bytes
                                                    .saturating_sub(self.total_bytes),
                                            );
                                        let bound =
                                            buffer.len().min(available.saturating_add(1) as usize);
                                        match file.read(&mut buffer[..bound]) {
                                            Ok(0) => break,
                                            Ok(size) => {
                                                read += size as u64;
                                                self.total_bytes += size as u64;
                                                if size as u64 > available {
                                                    self.gap(
                                                        path,
                                                        "file grew past content byte limit",
                                                    );
                                                    valid = false;
                                                    break;
                                                }
                                                hash.update(&buffer[..size]);
                                                if let Some(raw) = raw.as_mut() {
                                                    raw.extend_from_slice(&buffer[..size]);
                                                }
                                            }
                                            Err(error) => {
                                                self.gap(
                                                    path,
                                                    format!("content read failed: {error}"),
                                                );
                                                valid = false;
                                                break;
                                            }
                                        }
                                    }
                                    match file.metadata() {
                                        Ok(after)
                                            if after.dev() == entry.identity.device
                                                && after.ino() == entry.identity.inode
                                                && after.len() == entry.size
                                                && after.mtime() == entry.modified_sec
                                                && after.mtime_nsec() == entry.modified_nsec
                                                && after.ctime() == entry.changed_sec
                                                && after.ctime_nsec() == entry.changed_nsec => {}
                                        Ok(_) => {
                                            self.gap(
                                                path,
                                                "file metadata changed during content read",
                                            );
                                            valid = false;
                                        }
                                        Err(error) => {
                                            self.gap(
                                                path,
                                                format!("final file metadata failed: {error}"),
                                            );
                                            valid = false;
                                        }
                                    }
                                    if valid {
                                        entry.content_sha256 =
                                            Some(format!("{:x}", hash.finalize()));
                                        if entry.category == "index" {
                                            self.raw_index = raw;
                                        }
                                    }
                                }
                            }
                            Ok(_) => self.gap(
                                path,
                                "file identity or metadata changed before content read",
                            ),
                            Err(error) => self.gap(path, format!("file fstat failed: {error}")),
                        },
                        Err(error) => self.gap(path, format!("file open refused: {error}")),
                    }
                }
            }
            _ => self.gap(path, "unsupported special file; content not read"),
        }
        match statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(after) if same_stat(&before, &after) => {}
            Ok(_) => self.gap(path, "directory entry metadata changed during observation"),
            Err(error) => self.gap(path, format!("final entry metadata failed: {error}")),
        }
        Some((entry, child))
    }
}
use std::os::unix::fs::MetadataExt;

fn walk(root: &OwnedFd, context: &mut Context<'_>) {
    let initial = match openat(root, ".", DIRECTORY, Mode::empty()) {
        Ok(fd) => fd,
        Err(error) => {
            context.gap(Path::new("."), format!("root reopen failed: {error}"));
            return;
        }
    };
    let mut stack = vec![(initial, PathBuf::new(), 0usize)];
    while let Some((fd, relative, depth)) = stack.pop() {
        if !context.check(&relative) {
            break;
        }
        let before = match fstat(&fd) {
            Ok(stat) => stat,
            Err(error) => {
                context.gap(&relative, format!("directory metadata failed: {error}"));
                continue;
            }
        };
        let mut dir = match Dir::read_from(&fd) {
            Ok(dir) => dir,
            Err(error) => {
                context.gap(&relative, format!("directory read refused: {error}"));
                continue;
            }
        };
        let mut names = Vec::new();
        for entry in &mut dir {
            if !context.check(&relative) {
                break;
            }
            match entry {
                Ok(entry) => {
                    let name = entry.file_name().to_bytes();
                    if name == b"." || name == b".." {
                        continue;
                    }
                    if names.len() + context.entry_count() >= context.limits.max_entries {
                        context.gap(&relative, "directory entry limit reached");
                        context.stopped = true;
                        break;
                    }
                    names.push(name.to_vec());
                }
                Err(error) => {
                    context.gap(&relative, format!("directory entry read failed: {error}"))
                }
            }
        }
        names.sort();
        for name in names {
            if !context.check(&relative) {
                break;
            }
            let name = OsString::from_vec(name);
            let path = relative.join(&name);
            let category = if name == ".git" {
                "administration_marker".into()
            } else {
                context.category(&path)
            };
            if let Some((entry, child)) = context.read_entry(&fd, &name, &path, category) {
                context.push(entry, false);
                if let Some(child) = child {
                    if name == ".git" {
                        continue;
                    }
                    if depth >= context.limits.max_depth {
                        context.gap(&path, "local directory depth limit reached");
                    } else {
                        stack.push((child, path, depth + 1));
                    }
                }
            }
        }
        match fstat(&fd) {
            Ok(after) if same_stat(&before, &after) => {}
            Ok(_) => context.gap(&relative, "directory changed while enumerating entries"),
            Err(error) => context.gap(
                &relative,
                format!("final directory metadata failed: {error}"),
            ),
        }
    }
}

fn bounded_git_read(
    context: &Context<'_>,
    root: &OwnedFd,
    path: &Path,
    read: GitRead,
) -> Result<Vec<u8>, SweepError> {
    let remaining = context
        .limits
        .timeout
        .saturating_sub(context.started.elapsed());
    if remaining.is_zero() || context.hooks.cancelled() {
        return Err(SweepError::new(
            "local_deadline",
            "local_git",
            path,
            "local deadline or cancellation reached",
        ));
    }
    ControlledGit::with_timeout(remaining).read_from_directory(root, path, read)
}

fn capture_admin_file(
    context: &mut Context<'_>,
    parent: &OwnedFd,
    name: &std::ffi::OsStr,
    path: &Path,
    category: &str,
    required: bool,
) {
    match statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Err(error) if error == rustix::io::Errno::NOENT && !required => return,
        Err(error) => {
            context.gap(path, format!("administration entry unavailable: {error}"));
            return;
        }
        Ok(_) => {}
    }
    if context.check(path)
        && let Some((entry, _)) = context.read_entry(parent, name, path, category.into())
    {
        if entry.kind != "file" {
            context.gap(path, "administration entry is not a regular file");
        }
        context.push(entry, true);
    }
}

/// Digest input is compact serde JSON of the five sorted arrays, in this order:
/// entries,index_entries,administration_entries,gaps,not_collected. Struct field
/// order is the versioned model declaration order. Collection timestamps and
/// the digest field itself are excluded. This binds hashes and UNKNOWN facts.
pub fn content_digest(inventory: &LocalInventory) -> Result<String, SweepError> {
    let bytes = serde_json::to_vec(&(
        &inventory.entries,
        &inventory.index_entries,
        &inventory.administration_entries,
        &inventory.gaps,
        &inventory.not_collected,
    ))
    .map_err(|e| SweepError {
        code: "local_serialize".into(),
        operation: "local_digest".into(),
        path: Some(inventory.worktree.clone()),
        message: e.to_string(),
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub fn collect_with(
    worktree: &Worktree,
    limits: &LocalLimits,
    hooks: &dyn LocalHooks,
) -> Result<LocalInventory, SweepError> {
    crate::residency::ensure_no_materialization()?;
    let path = worktree.path.to_path()?;
    let error =
        |code: &str, message: String| SweepError::new(code, "local_inventory", &path, message);
    if worktree.bare {
        return Err(error(
            "local_unavailable",
            "bare repository has no local worktree".into(),
        ));
    }
    if limits.max_entries == 0
        || limits.max_entries > 20_000
        || limits.max_manifest_bytes < 4096
        || limits.max_manifest_bytes > 8 * 1024 * 1024
        || limits.max_depth > 256
        || limits.max_file_bytes > 64 * 1024 * 1024
        || limits.max_total_bytes > 256 * 1024 * 1024
        || limits.timeout > Duration::from_secs(60)
    {
        return Err(error(
            "local_limits",
            "local limits exceed supported bounds".into(),
        ));
    }
    let root = open_absolute_directory(&path).map_err(|e| error("local_root", e.to_string()))?;
    let root_before = fstat(&root).map_err(|e| error("local_root", e.to_string()))?;
    if worktree.identity.as_ref() != Some(&identity(&root_before)) {
        return Err(error(
            "local_identity",
            "worktree identity differs from the supplied observation".into(),
        ));
    }
    let mut context=Context {inventory:LocalInventory {worktree:worktree.path.clone(),started_unix_ms:now_ms(),finished_unix_ms:0,entries:Vec::new(),index_entries:Vec::new(),administration_entries:Vec::new(),traversal_complete:true,temporal_consistency:"not_guaranteed".into(),gaps:Vec::new(),not_collected:vec!["ACLs and extended attributes".into(),"Git administration directories (.git) are not traversed as local content; only the selected index, shared index and own HEAD reflog are collected".into(),"Git configuration, object database and shared ref reflogs; backup, restore and writer exclusion".into()],content_digest_sha256:String::new()},limits,hooks,started:Instant::now(),total_bytes:0,manifest_bytes:0,stopped:false,categories:BTreeMap::new(),raw_index:None};
    let head_before = bounded_git_read(
        &context,
        &root,
        &path,
        GitRead::ResolveCommit {
            reference: BytePath::from_bytes(b"HEAD"),
        },
    );
    if let Ok(ref raw) = head_before {
        if worktree.head.as_deref().map(str::as_bytes) != raw.strip_suffix(b"\n") {
            context.gap(&path, "HEAD differs from the supplied worktree observation");
        }
    } else {
        context.gap(&path, "HEAD object was unavailable before local collection");
    }
    let admin_path = bounded_git_read(&context, &root, &path, GitRead::GitDir)
        .map_err(|e| io::Error::other(e.to_string()))
        .and_then(|raw| path_from_output(&raw));
    let hash_size = match bounded_git_read(&context, &root, &path, GitRead::ObjectFormat) {
        Ok(raw) if raw == b"sha1\n" => Some(20),
        Ok(raw) if raw == b"sha256\n" => Some(32),
        _ => {
            context.gap(&path, "repository object format is unknown");
            None
        }
    };
    let mut admin = None;
    let mut index_known = false;
    match admin_path {
        Ok(admin_path) => match open_absolute_directory(&admin_path) {
            Ok(fd) => {
                capture_admin_file(
                    &mut context,
                    &fd,
                    std::ffi::OsStr::new("index"),
                    &admin_path.join("index"),
                    "index",
                    true,
                );
                if let (Some(raw), Some(hash_size)) = (context.raw_index.as_ref(), hash_size) {
                    match parse_raw_index(raw,hash_size,limits.max_entries) {
                        Ok(parsed)=>{
                            if let Some(hash)=parsed.shared {
                                let name=OsString::from(format!("sharedindex.{hash}"));
                                capture_admin_file(&mut context,&fd,&name,&admin_path.join(&name),"shared_index",true);
                                context.gap(&admin_path.join("index"),"split index preserved as raw bytes; index entries and file classifications remain unknown to avoid Git refreshing source metadata");
                            }else {
                                let bytes=serde_json::to_vec(&parsed.entries).map(|b|b.len()).unwrap_or(usize::MAX);
                                if parsed.entries.len().saturating_add(context.entry_count())>limits.max_entries {context.gap(&admin_path.join("index"),"local inventory entry limit reached; index classifications remain unknown");}
                                else if bytes>limits.max_manifest_bytes.saturating_sub(65536) {context.gap(&admin_path.join("index"),"index manifest byte limit reached; entry classifications remain unknown");}
                                else {
                                    context.manifest_bytes=context.manifest_bytes.saturating_add(bytes);
                                    for entry in &parsed.entries {context.categories.insert(entry.path.clone(),"tracked".into());}
                                    context.inventory.index_entries=parsed.entries;index_known=true;
                                }
                            }
                        },
                        Err(error)=>context.gap(&admin_path.join("index"),format!("raw index unsupported or malformed; index classifications remain unknown: {error}")),
                    }
                }
                match openat(&fd, "logs", DIRECTORY, Mode::empty()) {
                    Ok(logs) => capture_admin_file(
                        &mut context,
                        &logs,
                        std::ffi::OsStr::new("HEAD"),
                        &admin_path.join("logs/HEAD"),
                        "own_reflog",
                        false,
                    ),
                    Err(error) if error == rustix::io::Errno::NOENT => {}
                    Err(error) => context.gap(
                        &admin_path.join("logs"),
                        format!("own reflog directory unavailable: {error}"),
                    ),
                }
                admin = Some((fd, admin_path));
            }
            Err(error) => context.gap(
                &admin_path,
                format!("administration directory unavailable: {error}"),
            ),
        },
        Err(error) => context.gap(&path, format!("administration path unavailable: {error}")),
    }
    walk(&root, &mut context);
    if index_known {
        let probes = context
            .inventory
            .entries
            .iter()
            .filter(|entry| {
                entry.category != "tracked" && entry.category != "administration_marker"
            })
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        let probe_set = probes.iter().cloned().collect::<BTreeSet<_>>();
        match bounded_git_read(
            &context,
            &root,
            &path,
            GitRead::IgnorePaths { paths: probes },
        ) {
            Ok(raw) => match parse_paths(&raw) {
                Ok(ignored) if ignored.is_subset(&probe_set) => {
                    for entry in &mut context.inventory.entries {
                        if entry.category == "unknown" {
                            entry.category = if ignored.contains(&entry.path) {
                                "ignored"
                            } else {
                                "untracked"
                            }
                            .into();
                        }
                    }
                }
                _ => context.gap(
                    &path,
                    "ignore classifications are malformed or contradict probed paths",
                ),
            },
            Err(error) => context.gap(&path, format!("ignore classification unavailable: {error}")),
        }
    }
    let head_after = bounded_git_read(
        &context,
        &root,
        &path,
        GitRead::ResolveCommit {
            reference: BytePath::from_bytes(b"HEAD"),
        },
    );
    if head_before.as_ref().ok() != head_after.as_ref().ok() || head_after.is_err() {
        context.gap(&path, "HEAD observations changed or final read failed");
    }
    if let Some((fd, admin_path)) = admin {
        let initial = context
            .inventory
            .administration_entries
            .iter()
            .filter(|e| e.category == "index" || e.category == "shared_index")
            .map(|e| (e.path.clone(), e.content_sha256.clone(), e.identity.clone()))
            .collect::<Vec<_>>();
        for (entry_path, hash, entry_identity) in initial {
            let absolute = entry_path.to_path()?;
            if let (Some(parent), Some(name)) = (absolute.parent(), absolute.file_name()) {
                let parent_fd = if parent == admin_path {
                    openat(&fd, ".", DIRECTORY, Mode::empty()).map_err(io::Error::from)
                } else {
                    open_absolute_directory(parent)
                };
                match parent_fd {
                    Ok(parent) => {
                        if let Some((entry, _)) = context.read_entry(
                            &parent,
                            name,
                            &absolute,
                            "index_revalidation".into(),
                        ) && (entry.content_sha256 != hash || entry.identity != entry_identity)
                        {
                            context.gap(
                                &absolute,
                                "raw index bytes or identity changed during collection",
                            );
                        }
                    }
                    Err(error) => context.gap(
                        &absolute,
                        format!("final index parent unavailable: {error}"),
                    ),
                }
            }
        }
    }
    match fstat(&root) {
        Ok(after) if same_stat(&root_before, &after) => {}
        Ok(_) => context.gap(&path, "worktree directory changed during local collection"),
        Err(error) => context.gap(&path, format!("final root metadata failed: {error}")),
    }
    match open_absolute_directory(&path).and_then(|fd| fstat(&fd).map_err(io::Error::from)) {
        Ok(after) if identity(&after) == identity(&root_before) => {}
        Ok(_) => context.gap(
            &path,
            "worktree pathname identifies a different directory after collection",
        ),
        Err(error) => context.gap(
            &path,
            format!("worktree pathname unavailable after collection: {error}"),
        ),
    }
    context
        .inventory
        .entries
        .sort_by(|a, b| a.path.cmp(&b.path));
    context
        .inventory
        .administration_entries
        .sort_by(|a, b| a.path.cmp(&b.path));
    context
        .inventory
        .gaps
        .sort_by(|a, b| (&a.path, &a.reason).cmp(&(&b.path, &b.reason)));
    context.inventory.not_collected.sort();
    context.inventory.content_digest_sha256 = content_digest(&context.inventory)?;
    context.inventory.finished_unix_ms = now_ms();
    Ok(context.inventory)
}

pub(crate) mod summary;
pub use summary::{SummaryLimits, collect_summary, collect_summary_with};

pub(crate) use summary::{SummaryOutcome, collect_summary_for_review};
