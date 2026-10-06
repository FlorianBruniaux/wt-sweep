//! Process-local SIGINT latch. Library callers opt in by installing the handler.
use std::{
    io,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

static REQUESTED: AtomicBool = AtomicBool::new(false);
static INSTALLED: OnceLock<Result<(), i32>> = OnceLock::new();

pub fn requested() -> bool {
    REQUESTED.load(Ordering::Relaxed)
}

extern "C" fn interrupt(_signal: libc::c_int) {
    // Only a lock-free atomic store is performed in the signal handler.
    REQUESTED.store(true, Ordering::Relaxed);
}

/// Install once before CLI workers or Git children start. No handler is installed
/// merely by importing the library or reading the latch.
pub fn install() -> io::Result<()> {
    match *INSTALLED.get_or_init(native_install) {
        Ok(()) => Ok(()),
        Err(code) => Err(io::Error::from_raw_os_error(code)),
    }
}

#[allow(unsafe_code)]
fn native_install() -> Result<(), i32> {
    // SAFETY: sigaction is initialized locally, its mask is initialized by libc,
    // and the static C handler has the required signature and process lifetime.
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = interrupt as *const () as usize;
        action.sa_flags = libc::SA_RESTART;
        if libc::sigemptyset(&mut action.sa_mask) != 0
            || libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut()) != 0
        {
            return Err(io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        process::Command,
        time::{Duration, Instant},
    };

    #[test]
    #[allow(clippy::expect_used)]
    fn real_sigint_sets_latch_in_isolated_process() {
        const CHILD: &str = "WT_SWEEP_TEST_SIGNAL_CHILD";
        if std::env::var_os(CHILD).is_some() {
            install().expect("install native handler");
            install().expect("installation is idempotent");
            assert!(!requested());
            assert!(
                Command::new("/bin/kill")
                    .args(["-INT", &std::process::id().to_string()])
                    .status()
                    .expect("send owned signal")
                    .success()
            );
            // kill's successful exit acknowledges sending the process signal,
            // not completion of the handler on another test-runner thread.
            let deadline = Instant::now() + Duration::from_secs(2);
            while !requested() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(1));
            }
            assert!(
                requested(),
                "owned SIGINT was not delivered before deadline"
            );
            return;
        }
        let output = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "cancel::tests::real_sigint_sets_latch_in_isolated_process",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .expect("isolated test process");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !requested(),
            "child signal must not contaminate the test runner"
        );
    }
}
