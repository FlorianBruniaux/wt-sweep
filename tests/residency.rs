#![allow(clippy::expect_used)]
use std::{
    cell::RefCell,
    fs,
    io::{self, Read},
    os::{fd::OwnedFd, unix::net::UnixStream},
    path::PathBuf,
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use wt_sweep::residency::{
    LocalMetadata, MaterializationPolicy, ResidencyMetadata, ensure_no_materialization,
    ensure_no_materialization_with, resident_metadata_with,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn python() -> Command {
    let mut command = Command::new("/usr/bin/python3");
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("TMPDIR", "/private/tmp")
        .args(["-I", "-S", "-B"]);
    command
}

fn bounded_output(mut command: Command, timeout: Duration) -> io::Result<Output> {
    let (mut reader, writer) = UnixStream::pair()?;
    reader.set_nonblocking(true)?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(OwnedFd::from(writer.try_clone()?)))
        .stderr(Stdio::from(OwnedFd::from(writer)));
    let mut child = command.spawn()?;
    drop(command);
    let started = Instant::now();
    let result = (|| {
        let mut bytes = Vec::new();
        let mut eof = false;
        loop {
            let mut buffer = [0; 4096];
            // Bound each drain, including a child that writes continuously.
            for _ in 0..16 {
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        eof = true;
                        break;
                    }
                    Ok(count) => {
                        if bytes.len() + count > 64 * 1024 {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "subprocess output limit",
                            ));
                        }
                        bytes.extend_from_slice(&buffer[..count]);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(error),
                }
            }
            if let Some(status) = child.try_wait()?
                && eof
            {
                return Ok(Output {
                    status,
                    stdout: bytes,
                    stderr: Vec::new(),
                });
            }
            if started.elapsed() >= timeout {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "subprocess deadline",
                ));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    })();
    if result.is_err() {
        let _ = child.kill();
        let reap_started = Instant::now();
        while matches!(child.try_wait(), Ok(None))
            && reap_started.elapsed() < Duration::from_secs(1)
        {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    result
}

#[test]
fn subprocess_timeout_and_output_limit_refuse() {
    let fixture = Fixture::new();
    let mut command = python();
    command
        .args(["-c", "import time; time.sleep(0.5)"])
        .current_dir(&fixture.0);
    assert_eq!(
        bounded_output(command, Duration::from_millis(50))
            .expect_err("timeout")
            .kind(),
        io::ErrorKind::TimedOut
    );
    let mut command = python();
    command
        .args(["-c", "print('x'*100000)"])
        .current_dir(&fixture.0);
    assert_eq!(
        bounded_output(command, Duration::from_secs(10))
            .expect_err("output limit")
            .kind(),
        io::ErrorKind::InvalidData
    );
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/wt-sweep-residency-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("owned fixture");
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Policy {
    fail: Option<&'static str>,
    mismatch: Option<&'static str>,
    calls: RefCell<Vec<&'static str>>,
}
impl Policy {
    fn step(&self, name: &'static str) -> io::Result<()> {
        self.calls.borrow_mut().push(name);
        if self.fail == Some(name) {
            Err(io::Error::other("injected policy failure"))
        } else {
            Ok(())
        }
    }
}
impl MaterializationPolicy for Policy {
    fn set_process_off(&self) -> io::Result<()> {
        self.step("set_process")
    }
    fn set_thread_off(&self) -> io::Result<()> {
        self.step("set_thread")
    }
    fn process_is_off(&self) -> io::Result<bool> {
        self.step("get_process")?;
        Ok(self.mismatch != Some("get_process"))
    }
    fn thread_is_off(&self) -> io::Result<bool> {
        self.step("get_thread")?;
        Ok(self.mismatch != Some("get_thread"))
    }
}

#[test]
fn accepts_only_confirmed_process_and_thread_refusal() {
    let policy = Policy {
        fail: None,
        mismatch: None,
        calls: RefCell::new(Vec::new()),
    };
    ensure_no_materialization_with(&policy).expect("both scopes confirmed off");
    assert_eq!(
        *policy.calls.borrow(),
        ["set_process", "set_thread", "get_process", "get_thread"]
    );
    ensure_no_materialization_with(&policy).expect("idempotent guard");
    assert_eq!(policy.calls.borrow().len(), 8);
}

#[test]
fn every_policy_error_and_readback_mismatch_refuses() {
    for operation in ["set_process", "set_thread", "get_process", "get_thread"] {
        let policy = Policy {
            fail: Some(operation),
            mismatch: None,
            calls: RefCell::new(Vec::new()),
        };
        let error = ensure_no_materialization_with(&policy).expect_err(operation);
        assert_eq!(error.code, "residency_unavailable");
        assert_eq!(error.operation, "residency");
        assert!(error.path.is_none());
    }
    for operation in ["get_process", "get_thread"] {
        let policy = Policy {
            fail: None,
            mismatch: Some(operation),
            calls: RefCell::new(Vec::new()),
        };
        assert!(ensure_no_materialization_with(&policy).is_err());
    }
}

struct Dataless;
impl ResidencyMetadata for Dataless {
    fn is_dataless(&self, _: &fs::Metadata) -> bool {
        true
    }
}

#[test]
fn metadata_provider_refuses_dataless_without_reading_content() {
    let fixture = Fixture::new();
    let path = fixture.0.join("placeholder");
    fs::write(&path, b"resident bytes used only for injected metadata").expect("fixture");
    assert!(resident_metadata_with(&Dataless, &path).is_err());
    assert!(resident_metadata_with(&LocalMetadata, &path).is_ok());
    assert!(resident_metadata_with(&LocalMetadata, &fixture.0.join("absent")).is_err());
}

#[cfg(target_os = "macos")]
#[test]
fn native_guard_overrides_enabled_policy_and_is_inherited_by_exec() {
    let fixture = Fixture::new();
    let mut command = python();
    command.args([
            "-c",
            "import ctypes,os,sys\nc=ctypes.CDLL(None,use_errno=True)\nfor scope in (0,1):\n assert c.setiopolicy_np(3,scope,2)==0,ctypes.get_errno()\n assert c.getiopolicy_np(3,scope)==2\nos.execv(sys.argv[1],[sys.argv[1],'--exact','native_policy_child','--ignored','--nocapture'])",
        ])
        .arg(std::env::current_exe().expect("test executable"))
        .env("WT_SWEEP_RESIDENCY_CHILD", "1")
        .current_dir(&fixture.0);
    let output =
        bounded_output(command, Duration::from_secs(10)).expect("isolated policy subprocess");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "subprocess entry point for isolated native policy test"]
fn native_policy_child() {
    assert_eq!(
        std::env::var("WT_SWEEP_RESIDENCY_CHILD").as_deref(),
        Ok("1")
    );
    ensure_no_materialization().expect("native refusal enabled");
    ensure_no_materialization().expect("native refusal idempotent");
    let fixture = Fixture::new();
    let mut command = python();
    command.args([
            "-c",
            "import ctypes\nc=ctypes.CDLL(None)\nassert c.getiopolicy_np(3,0)==1,'child process must inherit OFF'\nassert c.getiopolicy_np(3,1) in (0,1),'child thread must follow process or be OFF'",
        ]).current_dir(&fixture.0);
    let output =
        bounded_output(command, Duration::from_secs(5)).expect("exec descendant policy check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn unsupported_os_refuses() {
    assert!(ensure_no_materialization().is_err());
}
