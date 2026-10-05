//! Bounded observations of the kernel mount table, without visiting mountpoints.
use crate::model::BytePath;
use std::collections::BTreeSet;
use std::io;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MountRecord {
    pub path: BytePath,
    pub source: BytePath,
    pub filesystem: BytePath,
    pub flags: u32,
    pub filesystem_type: u32,
    pub filesystem_subtype: u32,
}

#[derive(Clone, Debug)]
pub struct MountTable {
    pub records: Vec<MountRecord>,
    pub temporal_consistency: String,
}

#[derive(Clone, Copy, Debug)]
pub struct MountLimits {
    pub max_records: usize,
    pub max_attempts: usize,
}
impl Default for MountLimits {
    fn default() -> Self {
        Self {
            max_records: 4096,
            max_attempts: 8,
        }
    }
}

/// Testable kernel boundary. `read` may return exactly capacity when truncated.
pub trait MountBackend {
    fn count(&self) -> io::Result<usize>;
    fn read(&self, capacity: usize) -> io::Result<Vec<MountRecord>>;
}

/// Observations cover kernel-listed mounts only. Unmounted/unexposed volumes,
/// mountpoint accessibility and filesystem residency are not established here.
pub fn enumerate() -> io::Result<MountTable> {
    #[cfg(target_os = "macos")]
    {
        enumerate_with(&ffi::KernelMounts, MountLimits::default())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "native mount table is implemented for macOS only",
        ))
    }
}

pub fn enumerate_with(backend: &dyn MountBackend, limits: MountLimits) -> io::Result<MountTable> {
    if !(2..=4096).contains(&limits.max_records) || !(2..=32).contains(&limits.max_attempts) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mount table limits must be 2..4096 records and 2..32 attempts",
        ));
    }
    let mut previous = None;
    for _ in 0..limits.max_attempts {
        let before = backend.count()?;
        if before == 0 {
            return Err(invalid("kernel returned an empty mount table"));
        }
        // One spare entry is necessary to distinguish a complete fill from a
        // buffer that may have truncated a mount created after the count call.
        let capacity = before
            .checked_add(1)
            .filter(|n| *n <= limits.max_records)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::OutOfMemory,
                    "mount table record limit reached",
                )
            })?;
        let mut records = backend.read(capacity)?;
        if records.is_empty() || records.len() > capacity {
            return Err(invalid("kernel mount record count is invalid"));
        }
        validate(&records)?;
        records.sort();
        let after = backend.count()?;
        if after >= limits.max_records {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "mount table record limit reached",
            ));
        }
        if records.len() == capacity || before != after || records.len() != before {
            previous = None;
            continue;
        }
        if previous.as_ref() == Some(&records) {
            return Ok(MountTable {
                records,
                temporal_consistency: "observations_equal_snapshot_unknown".into(),
            });
        }
        previous = Some(records);
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "mount table changed or remained truncated within attempt limit",
    ))
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn validate(records: &[MountRecord]) -> io::Result<()> {
    let mut paths = BTreeSet::new();
    for record in records {
        let path = record
            .path
            .bytes()
            .map_err(|_| invalid("invalid mount path encoding"))?;
        let source = record
            .source
            .bytes()
            .map_err(|_| invalid("invalid mount source encoding"))?;
        let filesystem = record
            .filesystem
            .bytes()
            .map_err(|_| invalid("invalid filesystem encoding"))?;
        if path.first() != Some(&b'/')
            || path.contains(&0)
            || source.contains(&0)
            || filesystem.is_empty()
            || filesystem.contains(&0)
        {
            return Err(invalid("malformed mount table string"));
        }
        if !paths.insert(path) {
            return Err(invalid(
                "duplicate mountpoint observations remain ambiguous",
            ));
        }
    }
    Ok(())
}

// This unsafe exception is confined to the native table boundary.
// libc supplies the Darwin SDK layout and symbol selection; no ABI is copied.
// MNT_NOWAIT requests retained kernel metadata, without filesystem refresh or
// traversing/hydrating mountpoint content. It does not promise fresh metadata.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod ffi {
    use super::{MountBackend, MountRecord, invalid};
    use crate::model::BytePath;
    use std::io;

    pub(super) struct KernelMounts;
    impl MountBackend for KernelMounts {
        fn count(&self) -> io::Result<usize> {
            // SAFETY: Darwin explicitly permits NULL with bufsize 0 to obtain
            // the count. No pointer is dereferenced; NOWAIT is a valid flag.
            let count = unsafe { libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT) };
            if count < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(count as usize)
        }

        fn read(&self, capacity: usize) -> io::Result<Vec<MountRecord>> {
            if !(1..=4096).contains(&capacity) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "native mount buffer capacity exceeds limit",
                ));
            }
            let byte_size = capacity
                .checked_mul(std::mem::size_of::<libc::statfs>())
                .and_then(|size| libc::c_int::try_from(size).ok())
                .ok_or_else(|| invalid("native mount buffer size overflow"))?;
            // SAFETY: libc's Darwin statfs contains only integer fields and
            // integer arrays (including fsid), for which all-zero is valid.
            // Every element is initialized before any syscall or Rust read.
            let empty: libc::statfs = unsafe { std::mem::zeroed() };
            let mut buffer = vec![empty; capacity];
            // SAFETY: buffer is live, aligned, writable, and initialized, with
            // exactly capacity statfs elements. byte_size is its checked size
            // and fits the SDK c_int parameter. getfsstat does not retain the
            // pointer. The kernel may fill at most the supplied byte_size.
            let count =
                unsafe { libc::getfsstat(buffer.as_mut_ptr(), byte_size, libc::MNT_NOWAIT) };
            if count < 0 {
                return Err(io::Error::last_os_error());
            }
            let count = count as usize;
            if count > capacity {
                return Err(invalid("native mount count exceeds supplied buffer"));
            }
            buffer[..count]
                .iter()
                .map(|entry| {
                    Ok(MountRecord {
                        path: BytePath::from_bytes(&field(&entry.f_mntonname)?),
                        source: BytePath::from_bytes(&field(&entry.f_mntfromname)?),
                        filesystem: BytePath::from_bytes(&field(&entry.f_fstypename)?),
                        flags: entry.f_flags,
                        filesystem_type: entry.f_type,
                        filesystem_subtype: entry.f_fssubtype,
                    })
                })
                .collect()
        }
    }

    fn field(chars: &[libc::c_char]) -> io::Result<Vec<u8>> {
        let end = chars
            .iter()
            .position(|c| *c == 0)
            .ok_or_else(|| invalid("unterminated native mount table string"))?;
        // c_char signedness changes representation, not the original byte.
        Ok(chars[..end].iter().map(|c| *c as u8).collect())
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn native_string_decoding_is_bounded_and_byte_exact() {
            assert_eq!(
                super::field(&[47, -1, 10, 0, 42]).unwrap_or_else(|e| panic!("field: {e}")),
                b"/\xff\n"
            );
            assert!(super::field(&[47, 65]).is_err());
        }
    }
}
