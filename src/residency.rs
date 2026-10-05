//! Process-local refusal of macOS dataless-file materialization.
//!
//! Apple TN3150 recommends disabling IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES
//! and treating EDEADLK as an ordinary refusal to access unavailable content:
//! https://developer.apple.com/documentation/technotes/tn3150-getting-ready-for-data-less-files
//! setiopolicy_np(3) specifies that new processes inherit their parent's policy.
//! The process policy therefore also covers Git's indirect reads (includes,
//! alternates, loose objects, packs and administration pointers). A thread may
//! override its process policy, so every calling thread is explicitly set OFF.
//!
//! This prevents OS dataless materialization, not arbitrary network I/O, a
//! filesystem snapshot, concurrent writes, or another component changing the
//! policy. The caller must refuse work when this guard fails and must invoke it
//! before reading content or spawning Git. No previous policy is restored.
use crate::model::SweepError;
use std::{fs, io, path::Path};

pub trait MaterializationPolicy {
    fn set_process_off(&self) -> io::Result<()>;
    fn set_thread_off(&self) -> io::Result<()>;
    fn process_is_off(&self) -> io::Result<bool>;
    fn thread_is_off(&self) -> io::Result<bool>;
}

fn unavailable(message: impl Into<String>) -> SweepError {
    SweepError {
        code: "residency_unavailable".into(),
        operation: "residency".into(),
        path: None,
        message: message.into(),
    }
}

/// Disable materialization for this process and calling thread, then verify both.
/// On unsupported platforms the MVP refuses rather than claiming protection.
pub fn ensure_no_materialization() -> Result<(), SweepError> {
    #[cfg(target_os = "macos")]
    {
        ensure_no_materialization_with(&ffi::NativePolicy)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err(unavailable("dataless materialization guard requires macOS"))
    }
}

/// Injectable policy boundary for error and readback-mismatch tests.
pub fn ensure_no_materialization_with(
    policy: &dyn MaterializationPolicy,
) -> Result<(), SweepError> {
    policy
        .set_process_off()
        .map_err(|error| unavailable(format!("cannot disable process materialization: {error}")))?;
    policy
        .set_thread_off()
        .map_err(|error| unavailable(format!("cannot disable thread materialization: {error}")))?;
    let process_off = policy
        .process_is_off()
        .map_err(|error| unavailable(format!("cannot verify process materialization: {error}")))?;
    let thread_off = policy
        .thread_is_off()
        .map_err(|error| unavailable(format!("cannot verify thread materialization: {error}")))?;
    if !process_off || !thread_off {
        return Err(unavailable(
            "materialization policy readback did not confirm OFF in both scopes",
        ));
    }
    Ok(())
}

/// Metadata diagnostics only. The native policy protects indirect reads.
/// The default uses lstat; symlink metadata is not evidence about its target.
pub trait ResidencyMetadata {
    fn metadata(&self, path: &Path) -> io::Result<fs::Metadata> {
        fs::symlink_metadata(path)
    }
    fn is_dataless(&self, metadata: &fs::Metadata) -> bool;
}

pub struct LocalMetadata;
impl ResidencyMetadata for LocalMetadata {
    fn is_dataless(&self, metadata: &fs::Metadata) -> bool {
        #[cfg(target_os = "macos")]
        {
            use std::os::macos::fs::MetadataExt;
            // SF_DATALESS, from the macOS SDK sys/stat.h.
            metadata.st_flags() & 0x4000_0000 != 0
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = metadata;
            false
        }
    }
}

pub fn resident_metadata_with(
    provider: &dyn ResidencyMetadata,
    path: &Path,
) -> io::Result<fs::Metadata> {
    let metadata = provider.metadata(path)?;
    if provider.is_dataless(&metadata) {
        return Err(io::Error::other("dataless metadata: content read refused"));
    }
    Ok(metadata)
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod ffi {
    use super::MaterializationPolicy;
    use libc::c_int;
    use std::io;

    // Constants and scalar signatures from macOS SDK sys/resource.h.
    // libc does not currently expose these two functions on this target.
    const MATERIALIZE_DATALESS: c_int = 3;
    const PROCESS: c_int = 0;
    const THREAD: c_int = 1;
    const OFF: c_int = 1;

    unsafe extern "C" {
        fn setiopolicy_np(iotype: c_int, scope: c_int, policy: c_int) -> c_int;
        fn getiopolicy_np(iotype: c_int, scope: c_int) -> c_int;
    }

    pub(super) struct NativePolicy;

    fn set_off(scope: c_int) -> io::Result<()> {
        // SAFETY: sys/resource.h declares exactly these three int arguments
        // and int result. All arguments are documented enum constants, with
        // scope restricted by the two private callers below. The API accesses
        // no Rust memory and retains no pointers. It changes only the current
        // process/thread policy; errno is consumed immediately on failure.
        let result = unsafe { setiopolicy_np(MATERIALIZE_DATALESS, scope, OFF) };
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    fn is_off(scope: c_int) -> io::Result<bool> {
        // SAFETY: sys/resource.h declares exactly two int arguments and an
        // int result. Constants are valid and the private callers restrict
        // scope to PROCESS/THREAD. No pointers, aliasing or lifetime contract
        // exists. A negative result is failure; every value other than OFF
        // fails the guard, including DEFAULT and unknown future policies.
        let policy = unsafe { getiopolicy_np(MATERIALIZE_DATALESS, scope) };
        if policy < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(policy == OFF)
        }
    }

    impl MaterializationPolicy for NativePolicy {
        fn set_process_off(&self) -> io::Result<()> {
            set_off(PROCESS)
        }
        fn set_thread_off(&self) -> io::Result<()> {
            set_off(THREAD)
        }
        fn process_is_off(&self) -> io::Result<bool> {
            is_off(PROCESS)
        }
        fn thread_is_off(&self) -> io::Result<bool> {
            is_off(THREAD)
        }
    }
}
