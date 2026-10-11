//! Volume observations on an already-acquired directory descriptor.
use super::{VolumeSpace, blocks_to_bytes};
use crate::model::{BytePath, Identity, now_ms};
use std::{io, os::fd::BorrowedFd};

#[derive(Clone)]
pub(in crate::disk) struct RawVolumeSpace {
    pub block_size: u32,
    pub blocks: u64,
    pub free_blocks: u64,
    pub available_blocks: u64,
    pub mount_path: Option<BytePath>,
    pub source: Option<BytePath>,
    pub filesystem: Option<BytePath>,
}
pub(in crate::disk) trait VolumeBackend {
    fn identity(&self, fd: BorrowedFd<'_>) -> io::Result<Identity>;
    fn snapshot(&self, fd: BorrowedFd<'_>) -> io::Result<RawVolumeSpace>;
}
pub(in crate::disk) struct KernelVolumeBackend;

/// Finite, one-attempt-per-device state; it does not own any measured bytes.
pub(in crate::disk) struct Probes<'a> {
    backend: &'a dyn VolumeBackend,
    limit: usize,
    pub(in crate::disk) spaces: std::collections::BTreeMap<u64, VolumeSpace>,
    pub(in crate::disk) skipped: std::collections::BTreeMap<u64, &'static str>,
}
impl<'a> Probes<'a> {
    pub(in crate::disk) fn new(backend: &'a dyn VolumeBackend, limit: usize) -> Self {
        Self {
            backend,
            limit,
            spaces: Default::default(),
            skipped: Default::default(),
        }
    }
    pub(in crate::disk) fn observe(
        &mut self,
        fd: BorrowedFd<'_>,
        expected: &Identity,
        path: &BytePath,
        mut within_budget: impl FnMut() -> bool,
    ) {
        let device = expected.device;
        if self.spaces.contains_key(&device) || self.skipped.contains_key(&device) {
            return;
        }
        if self.spaces.len() >= self.limit {
            self.skipped.insert(device, "capacity_probe_limit");
            return;
        }
        if !within_budget() {
            self.skipped.insert(device, "capacity_time_budget");
            return;
        }
        let volume = observe_volume(fd, expected, path, self.backend, &mut within_budget);
        self.spaces.insert(device, volume);
    }
}

impl VolumeBackend for KernelVolumeBackend {
    fn identity(&self, fd: BorrowedFd<'_>) -> io::Result<Identity> {
        let st = rustix::fs::fstat(fd)?;
        Ok(Identity {
            device: st.st_dev as u64,
            inode: st.st_ino,
        })
    }
    fn snapshot(&self, fd: BorrowedFd<'_>) -> io::Result<RawVolumeSpace> {
        #[cfg(target_os = "macos")]
        {
            ffi::snapshot(fd)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = fd;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "native volume statistics require macOS",
            ))
        }
    }
}

/// Check at both sides of every native operation; a late result stays unknown.
fn budgeted<T>(
    within_budget: &mut impl FnMut() -> bool,
    operation: impl FnOnce() -> io::Result<T>,
) -> io::Result<T> {
    if !within_budget() {
        return Err(io::Error::other("capacity_time_budget"));
    }
    let result = operation();
    if !within_budget() {
        return Err(io::Error::other("capacity_time_budget"));
    }
    result
}

pub(in crate::disk) fn observe_volume(
    fd: BorrowedFd<'_>,
    expected: &Identity,
    path: &BytePath,
    backend: &dyn VolumeBackend,
    within_budget: &mut impl FnMut() -> bool,
) -> VolumeSpace {
    let mut volume = VolumeSpace {
        anchor: expected.clone(),
        anchor_path: path.clone(),
        mount_path: None,
        source: None,
        filesystem: None,
        identity_verified: false,
        capacity_bytes: None,
        free_bytes: None,
        available_bytes: None,
        method: "fstatfs".into(),
        started_unix_ms: now_ms(),
        finished_unix_ms: 0,
        gaps: vec![],
    };
    let read = (|| -> io::Result<RawVolumeSpace> {
        let before = budgeted(within_budget, || backend.identity(fd))?;
        if (before.device, before.inode) != (expected.device, expected.inode) {
            return Err(io::Error::other(
                "descriptor identity mismatch before volume observation",
            ));
        }
        let raw = budgeted(within_budget, || backend.snapshot(fd))?;
        let after = budgeted(within_budget, || backend.identity(fd))?;
        if (after.device, after.inode) != (expected.device, expected.inode) {
            return Err(io::Error::other(
                "descriptor identity changed during volume observation",
            ));
        }
        Ok(raw)
    })();
    match read {
        Ok(raw) => {
            volume.identity_verified = true;
            volume.mount_path = raw.mount_path;
            volume.source = raw.source;
            volume.filesystem = raw.filesystem;
            volume.capacity_bytes = blocks_to_bytes(raw.blocks, raw.block_size).filter(|n| *n > 0);
            volume.free_bytes = blocks_to_bytes(raw.free_blocks, raw.block_size);
            volume.available_bytes = blocks_to_bytes(raw.available_blocks, raw.block_size);
            if let Some(capacity) = volume.capacity_bytes {
                if volume.free_bytes.is_some_and(|n| n > capacity) {
                    volume.free_bytes = None;
                }
                if volume.available_bytes.is_some_and(|n| n > capacity) {
                    volume.available_bytes = None;
                }
            }
            for (field, value) in [
                ("capacity", volume.capacity_bytes),
                ("free", volume.free_bytes),
                ("available", volume.available_bytes),
            ] {
                if value.is_none() {
                    volume
                        .gaps
                        .push(format!("invalid or unavailable native {field} count"));
                }
            }
        }
        Err(e) => {
            let mut reason = e.to_string();
            if reason.len() > 2048 {
                let mut end = 2000;
                while !reason.is_char_boundary(end) {
                    end -= 1;
                }
                reason.truncate(end);
                reason.push_str(" (diagnostic truncated)");
            }
            volume.gaps.push(reason);
        }
    }
    volume.finished_unix_ms = now_ms();
    volume
}

#[cfg(any(target_os = "macos", test))]
fn native_name<const N: usize>(name: &[libc::c_char; N]) -> io::Result<Option<BytePath>> {
    let end = name.iter().position(|c| *c == 0).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "unterminated native volume name",
        )
    })?;
    if end == 0 {
        return Ok(None);
    }
    let bytes: Vec<u8> = name[..end].iter().map(|c| *c as u8).collect();
    Ok(Some(BytePath::from_bytes(&bytes)))
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod ffi {
    use super::*;
    use std::os::fd::AsRawFd;
    pub(super) fn snapshot(fd: BorrowedFd<'_>) -> io::Result<RawVolumeSpace> {
        // SAFETY: libc's Darwin statfs contains integer fields and integer arrays;
        // an all-zero value is valid and initializes every byte read by this boundary.
        let mut raw: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: fd remains borrowed for the entire call; raw is live, aligned,
        // writable SDK storage. The syscall does not retain either argument.
        let status = unsafe { libc::fstatfs(fd.as_raw_fd(), &mut raw) };
        if status != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(RawVolumeSpace {
            block_size: raw.f_bsize,
            blocks: raw.f_blocks,
            free_blocks: raw.f_bfree,
            available_blocks: raw.f_bavail,
            mount_path: native_name(&raw.f_mntonname)?,
            source: native_name(&raw.f_mntfromname)?,
            filesystem: native_name(&raw.f_fstypename)?,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use std::{
        cell::Cell,
        fs,
        os::fd::AsFd,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf, fs::File);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "wt-sweep-volume-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("owned directory");
            let fd = fs::File::open(&path).expect("directory descriptor");
            Self(path, fd)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir(&self.0).expect("owned empty fixture");
        }
    }
    struct Fake {
        ids: Vec<Identity>,
        read: Cell<usize>,
        calls: Cell<usize>,
        fail: bool,
        expired: Cell<bool>,
        expire_after_identity: bool,
        expire_after_snapshot: bool,
        raw: RawVolumeSpace,
    }
    impl Fake {
        fn new() -> Self {
            Self {
                ids: vec![
                    Identity {
                        device: 7,
                        inode: 8,
                    },
                    Identity {
                        device: 7,
                        inode: 8,
                    },
                ],
                read: Cell::new(0),
                calls: Cell::new(0),
                fail: false,
                expired: Cell::new(false),
                expire_after_identity: false,
                expire_after_snapshot: false,
                raw: RawVolumeSpace {
                    block_size: 4096,
                    blocks: 100,
                    free_blocks: 80,
                    available_blocks: 50,
                    mount_path: Some(BytePath::from_bytes(b"/fixture")),
                    source: None,
                    filesystem: Some(BytePath::from_bytes(b"apfs")),
                },
            }
        }
    }
    impl VolumeBackend for Fake {
        fn identity(&self, _: BorrowedFd<'_>) -> io::Result<Identity> {
            let n = self.read.get();
            self.read.set(n + 1);
            if self.expire_after_identity {
                self.expired.set(true);
            }
            self.ids
                .get(n)
                .cloned()
                .ok_or_else(|| io::Error::other("identity sequence exhausted"))
        }
        fn snapshot(&self, _: BorrowedFd<'_>) -> io::Result<RawVolumeSpace> {
            self.calls.set(self.calls.get() + 1);
            if self.expire_after_snapshot {
                self.expired.set(true);
            }
            if self.fail {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "fixture refusal",
                ))
            } else {
                Ok(self.raw.clone())
            }
        }
    }
    fn observe(f: &Fixture, backend: &Fake) -> VolumeSpace {
        observe_volume(
            f.1.as_fd(),
            &Identity {
                device: 7,
                inode: 8,
            },
            &BytePath::from_path(&f.0),
            backend,
            &mut || true,
        )
    }
    #[test]
    fn descriptor_mismatch_refuses_capacity() {
        let f = Fixture::new();
        let mut backend = Fake::new();
        backend.ids[0].device = 9;
        let volume = observe(&f, &backend);
        assert!(!volume.identity_verified);
        assert!(volume.capacity_bytes.is_none());
        assert!(!volume.gaps.is_empty());
        assert_eq!(backend.calls.get(), 0);
    }
    #[test]
    fn descriptor_changes_after_snapshot_refuses_capacity() {
        let f = Fixture::new();
        let mut backend = Fake::new();
        backend.ids[1].inode = 9;
        let volume = observe(&f, &backend);
        assert!(!volume.identity_verified);
        assert!(volume.capacity_bytes.is_none());
        assert!(volume.available_bytes.is_none());
        assert!(!volume.gaps.is_empty());
    }
    #[test]
    fn snapshot_error_preserves_unknown() {
        let f = Fixture::new();
        let mut backend = Fake::new();
        backend.fail = true;
        let volume = observe(&f, &backend);
        assert!(volume.capacity_bytes.is_none());
        assert!(volume.free_bytes.is_none());
        assert!(volume.gaps.iter().any(|s| s.contains("fixture refusal")));
    }
    #[test]
    fn valid_descriptor_captures_independent_fields() {
        let f = Fixture::new();
        let backend = Fake::new();
        let volume = observe(&f, &backend);
        assert!(volume.identity_verified);
        assert_eq!(volume.capacity_bytes, Some(409600));
        assert_eq!(volume.free_bytes, Some(327680));
        assert_eq!(volume.available_bytes, Some(204800));
    }
    #[test]
    fn invalid_counters_preserve_independent_fields() {
        let f = Fixture::new();
        let mut backend = Fake::new();
        backend.raw.free_blocks = 101;
        let volume = observe(&f, &backend);
        assert_eq!(volume.capacity_bytes, Some(409600));
        assert!(volume.free_bytes.is_none());
        assert_eq!(volume.available_bytes, Some(204800));
        assert!(!volume.gaps.is_empty());
        let mut backend = Fake::new();
        backend.raw.blocks = u64::MAX;
        let volume = observe(&f, &backend);
        assert!(volume.capacity_bytes.is_none());
        let mut backend = Fake::new();
        backend.raw.blocks = 0;
        let volume = observe(&f, &backend);
        assert!(volume.capacity_bytes.is_none());
    }
    #[test]
    fn unterminated_native_names_are_not_lossy_paths() {
        let invalid = [b'x' as libc::c_char; 4];
        assert!(native_name(&invalid).is_err());
        let valid = [b'/' as libc::c_char, 0xff_u8 as libc::c_char, 0, 1];
        assert_eq!(
            native_name(&valid).expect("name").expect("nonempty").hex,
            "2fff"
        );
    }

    #[test]
    fn one_probe_per_device_even_after_failure() {
        let f = Fixture::new();
        let mut backend = Fake::new();
        backend.fail = true;
        let mut probes = Probes::new(&backend, 2);
        let id = Identity {
            device: 7,
            inode: 8,
        };
        probes.observe(f.1.as_fd(), &id, &BytePath::from_path(&f.0), || true);
        probes.observe(f.1.as_fd(), &id, &BytePath::from_path(&f.0), || true);
        assert_eq!(backend.calls.get(), 1);
        assert!(probes.spaces[&7].capacity_bytes.is_none());
        assert!(!probes.spaces[&7].gaps.is_empty());
    }
    #[test]
    fn probe_limit_and_expired_budget_leave_explicit_unknowns() {
        let f = Fixture::new();
        let backend = Fake::new();
        let mut probes = Probes::new(&backend, 1);
        probes.observe(
            f.1.as_fd(),
            &Identity {
                device: 7,
                inode: 8,
            },
            &BytePath::from_path(&f.0),
            || true,
        );
        probes.observe(
            f.1.as_fd(),
            &Identity {
                device: 8,
                inode: 8,
            },
            &BytePath::from_path(&f.0),
            || true,
        );
        assert_eq!(probes.skipped[&8], "capacity_probe_limit");
        assert!(probes.spaces[&7].capacity_bytes.is_some());
        let backend = Fake::new();
        let mut probes = Probes::new(&backend, 1);
        probes.observe(
            f.1.as_fd(),
            &Identity {
                device: 7,
                inode: 8,
            },
            &BytePath::from_path(&f.0),
            || false,
        );
        assert_eq!(probes.skipped[&7], "capacity_time_budget");
        assert_eq!(backend.calls.get(), 0);
    }

    #[test]
    fn identity_exhausting_budget_prevents_snapshot() {
        let f = Fixture::new();
        let mut backend = Fake::new();
        backend.expire_after_identity = true;
        let mut probes = Probes::new(&backend, 1);
        probes.observe(
            f.1.as_fd(),
            &Identity {
                device: 7,
                inode: 8,
            },
            &BytePath::from_path(&f.0),
            || !backend.expired.get(),
        );
        assert_eq!(backend.read.get(), 1);
        assert_eq!(backend.calls.get(), 0, "no snapshot after expired identity");
        let volume = &probes.spaces[&7];
        assert!(volume.capacity_bytes.is_none());
        assert!(volume.gaps.iter().any(|r| r == "capacity_time_budget"));
    }
    #[test]
    fn late_snapshot_is_not_published_as_valid() {
        let f = Fixture::new();
        let mut backend = Fake::new();
        backend.expire_after_snapshot = true;
        let mut probes = Probes::new(&backend, 1);
        probes.observe(
            f.1.as_fd(),
            &Identity {
                device: 7,
                inode: 8,
            },
            &BytePath::from_path(&f.0),
            || !backend.expired.get(),
        );
        assert_eq!(
            backend.read.get(),
            1,
            "no final identity after expired snapshot"
        );
        assert_eq!(backend.calls.get(), 1);
        let volume = &probes.spaces[&7];
        assert!(volume.capacity_bytes.is_none());
        assert!(volume.available_bytes.is_none());
        assert!(volume.gaps.iter().any(|r| r == "capacity_time_budget"));
    }
}
