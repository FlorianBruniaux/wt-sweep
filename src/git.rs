//! Controlled, read-only Git collection. This module makes no integration judgement.
use crate::model::{
    BytePath, DirState, Family, Gap, Identity, Inventory, ScanScope, SweepError, Worktree,
};
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

mod batch;
mod session;
pub(crate) use batch::{parse_object_info, parse_objects};
pub(crate) use session::CommitSession;

/// Closed set: new reads require reviewing their executable and network behavior.
#[derive(Clone, Debug)]
pub enum GitRead {
    ObjectInfo { oids: Vec<String> },
    Objects { oids: Vec<String> },
    CommitBatch { oids: Vec<String> },
    CommonDir,
    GitDir,
    TopLevel,
    ObjectFormat,
    RefFormat,
    AdminPath { name: BytePath },
    IgnorePaths { paths: Vec<BytePath> },
    Worktrees,
    ResolveCommit { reference: BytePath },
    Commit { oid: String },
    LocalRefs,
    Tree { oid: String },
    RemoteUrls,
    TrackingRefs,
    Blob { oid: String },
}

/// Fixed system executable, cleared environment and bounded execution.
pub struct ControlledGit {
    timeout: Duration,
    output_limit: u64,
    spawned: Cell<u64>,
    first_timeout: RefCell<Option<GitRead>>,
}

impl Default for ControlledGit {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(10),
            output_limit: 8 * 1024 * 1024,
            spawned: Cell::new(0),
            first_timeout: RefCell::new(None),
        }
    }
}

#[cfg(test)]
thread_local! {
    static CHILD_READ_JOURNAL: RefCell<Vec<(BytePath, GitRead)>> = const { RefCell::new(Vec::new()) };
}
#[cfg(test)]
pub(crate) fn take_child_read_journal() -> Vec<(BytePath, GitRead)> {
    CHILD_READ_JOURNAL.with(|journal| std::mem::take(&mut *journal.borrow_mut()))
}

impl ControlledGit {
    pub(crate) fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout: timeout.min(Duration::from_secs(10)),
            ..Self::default()
        }
    }
    pub(crate) fn timeout_limit(&self) -> Duration {
        self.timeout
    }
    pub(crate) fn first_timeout(&self) -> Option<GitRead> {
        self.first_timeout.borrow().clone()
    }
    pub(crate) fn spawned_processes(&self) -> u64 {
        self.spawned.get()
    }
    pub(crate) fn with_output_limit(timeout: Duration, output_limit: usize) -> Self {
        Self {
            output_limit: output_limit.min(8 * 1024 * 1024) as u64,
            ..Self::with_timeout(timeout)
        }
    }
    fn command(&self, repo: &Path) -> Command {
        let mut command = Command::new("/usr/bin/git");
        command
            .env_clear()
            .env(
                "TMPDIR",
                if cfg!(target_os = "macos") {
                    "/private/tmp"
                } else {
                    "/tmp"
                },
            )
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_ALLOW_PROTOCOL", "")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C");
        for config in [
            "core.fsmonitor=false",
            "core.untrackedCache=false",
            "core.hooksPath=/dev/null",
            "gc.auto=0",
            "maintenance.auto=false",
            "protocol.allow=never",
            "core.pager=cat",
            "pager.worktree=false",
        ] {
            command.arg("-c").arg(config);
        }
        command.arg("--no-pager").arg("-C").arg(repo);
        command
    }

    pub fn read(&self, repo: &Path, read: GitRead) -> Result<Vec<u8>, SweepError> {
        self.read_inner(repo, None, repo, read)
    }

    /// Anchor the child cwd to an already-open directory. The descriptor stays
    /// live until spawn has selected cwd; CLOEXEC remains set for the exec.
    pub fn read_from_directory(
        &self,
        directory: &OwnedFd,
        display_path: &Path,
        read: GitRead,
    ) -> Result<Vec<u8>, SweepError> {
        self.read_inner(Path::new("."), Some(directory), display_path, read)
    }

    fn read_inner(
        &self,
        command_path: &Path,
        directory: Option<&OwnedFd>,
        repo: &Path,
        read: GitRead,
    ) -> Result<Vec<u8>, SweepError> {
        crate::residency::ensure_no_materialization()?;
        let observed_read = read.clone();
        let mut command = self.command(command_path);
        if let Some(directory) = directory {
            anchored_cwd::attach(&mut command, directory)
                .map_err(|e| SweepError::new("git_cwd", "anchor_directory", repo, e.to_string()))?;
        }
        let allow_empty_no_match =
            matches!(&read, GitRead::RemoteUrls | GitRead::IgnorePaths { .. });
        let mut input = Vec::new();
        let operation = match read {
            GitRead::CommitBatch { .. } => {
                return Err(SweepError::new(
                    "git_argument",
                    "commit_batch",
                    repo,
                    "adaptive commit batches require the finite session API",
                ));
            }
            GitRead::ObjectInfo { oids } | GitRead::Objects { oids } => {
                if oids.is_empty()
                    || oids.len() > 128
                    || oids.iter().any(|oid| {
                        !matches!(oid.len(), 40 | 64)
                            || !oid.bytes().all(|b| b.is_ascii_hexdigit())
                            || oid.bytes().all(|b| b == b'0')
                    })
                {
                    return Err(SweepError::new(
                        "git_argument",
                        "git_batch",
                        repo,
                        "object batches require 1..128 complete nonzero hexadecimal OIDs",
                    ));
                }
                for oid in oids {
                    input.extend_from_slice(oid.as_bytes());
                    input.push(b'\n');
                }
                let flag = if matches!(observed_read, GitRead::ObjectInfo { .. }) {
                    "--batch-check"
                } else {
                    "--batch"
                };
                command.args(["cat-file", flag]);
                "git_batch"
            }
            GitRead::ObjectFormat => {
                command.args(["rev-parse", "--show-object-format"]);
                "git_object_format"
            }
            GitRead::IgnorePaths { paths } => {
                for path in paths {
                    let bytes = path.bytes()?;
                    if bytes.is_empty()
                        || bytes.contains(&0)
                        || bytes.starts_with(b"/")
                        || bytes
                            .split(|b| *b == b'/')
                            .any(|p| p.is_empty() || p == b"." || p == b"..")
                        || input.len().saturating_add(bytes.len() + 3) > 8 * 1024 * 1024
                    {
                        return Err(SweepError::new(
                            "git_argument",
                            "git_ignore_paths",
                            repo,
                            "ignore probes require relative paths and at most 8MiB input",
                        ));
                    }
                    // check-ignore rejects PATHSPEC_LITERAL magic on older
                    // Git. A leading ./ prevents ':' filenames being magic;
                    // check-ignore tests each supplied pathname individually.
                    input.extend_from_slice(b"./");
                    input.extend_from_slice(&bytes);
                    input.push(0);
                }
                command.args(["check-ignore", "--no-index", "--stdin", "-z"]);
                "git_ignore_paths"
            }
            GitRead::Blob { oid } => {
                if !matches!(oid.len(), 40 | 64)
                    || !oid.bytes().all(|b| b.is_ascii_hexdigit())
                    || oid.bytes().all(|b| b == b'0')
                {
                    return Err(SweepError::new(
                        "git_argument",
                        "git_blob",
                        repo,
                        "blob reads require a complete nonzero hexadecimal OID",
                    ));
                }
                command.args(["cat-file", "blob", &oid]);
                "git_blob"
            }
            GitRead::RemoteUrls => {
                command.args(["config", "--null", "--get-regexp", "^remote\\..*\\.url$"]);
                "git_remote_urls"
            }
            GitRead::Tree { oid } => {
                if !matches!(oid.len(), 40 | 64)
                    || !oid.bytes().all(|b| b.is_ascii_hexdigit())
                    || oid.bytes().all(|b| b == b'0')
                {
                    return Err(SweepError::new(
                        "git_argument",
                        "git_tree",
                        repo,
                        "tree reads require a complete nonzero hexadecimal OID",
                    ));
                }
                command.args(["cat-file", "tree", &oid]);
                "git_tree"
            }
            GitRead::ResolveCommit { reference } => {
                let mut bytes = reference.bytes()?;
                bytes.extend_from_slice(b"^{commit}");
                command
                    .args(["rev-parse", "--verify", "--end-of-options"])
                    .arg(std::ffi::OsString::from_vec(bytes));
                "git_resolve_commit"
            }
            GitRead::Commit { oid } => {
                if !matches!(oid.len(), 40 | 64)
                    || !oid.bytes().all(|b| b.is_ascii_hexdigit())
                    || oid.bytes().all(|b| b == b'0')
                {
                    return Err(SweepError::new(
                        "git_argument",
                        "git_commit",
                        repo,
                        "commit reads require a complete nonzero hexadecimal OID",
                    ));
                }
                command.args(["cat-file", "commit", &oid]);
                "git_commit"
            }
            GitRead::TrackingRefs => {
                command.args([
                    "for-each-ref",
                    "--format=%(refname)%00%(objectname)%00%(upstream:remotename)%00%(upstream:remoteref)%00%(upstream)%00",
                    "refs/heads/", "refs/remotes/",
                ]);
                "git_tracking_refs"
            }
            GitRead::LocalRefs => {
                command.args([
                    "for-each-ref",
                    "--format=%(refname)%00%(objectname)%00",
                    "refs/heads/",
                    "refs/remotes/",
                ]);
                "git_local_refs"
            }
            GitRead::RefFormat => {
                command.args(["rev-parse", "--show-ref-format"]);
                "git_ref_format"
            }
            GitRead::AdminPath { name } => {
                let bytes = name.bytes()?;
                if ![
                    b"HEAD".as_slice(),
                    b"refs",
                    b"refs/bisect",
                    b"refs/worktree",
                    b"refs/rewritten",
                    b"logs/HEAD",
                    b"logs/refs/bisect",
                    b"logs/refs/worktree",
                    b"logs/refs/rewritten",
                    b"packed-refs",
                    b"refs/stash",
                    b"logs/refs/stash",
                ]
                .contains(&bytes.as_slice())
                {
                    return Err(SweepError::new(
                        "git_argument",
                        "git_admin_path",
                        repo,
                        "history path is not approved",
                    ));
                }
                command
                    .args(["rev-parse", "--path-format=absolute", "--git-path"])
                    .arg(std::ffi::OsString::from_vec(bytes));
                "git_admin_path"
            }
            GitRead::CommonDir => {
                command.args(["rev-parse", "--path-format=absolute", "--git-common-dir"]);
                "git_common_dir"
            }
            GitRead::GitDir => {
                command.args(["rev-parse", "--absolute-git-dir"]);
                "git_dir"
            }
            GitRead::TopLevel => {
                command.args(["rev-parse", "--show-toplevel"]);
                "git_top_level"
            }
            GitRead::Worktrees => {
                command.args(["worktree", "list", "--porcelain", "-z"]);
                "git_worktrees"
            }
        };
        #[cfg(test)]
        let before_spawned = self.spawned.get();
        let result = run_bounded_inner(
            command,
            repo,
            operation,
            self.timeout,
            self.output_limit,
            allow_empty_no_match,
            (&input, Some(&self.spawned)),
        );
        #[cfg(test)]
        if self.spawned.get() > before_spawned {
            CHILD_READ_JOURNAL.with(|journal| {
                journal
                    .borrow_mut()
                    .push((BytePath::from_path(repo), observed_read.clone()))
            });
        }
        if result.as_ref().is_err_and(|e| e.code == "git_timeout")
            && self.first_timeout.borrow().is_none()
        {
            *self.first_timeout.borrow_mut() = Some(observed_read);
        }
        result
    }
}

// macOS /dev/fd directory chdir returns ENOTDIR; do not fall back to pathname
// cwd. This small exception anchors the child through its open descriptor.
#[allow(unsafe_code)]
mod anchored_cwd {
    use std::io;
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::os::unix::process::CommandExt;
    use std::process::Command;
    pub(super) fn attach(command: &mut Command, directory: &OwnedFd) -> io::Result<()> {
        let fd = rustix::io::fcntl_dupfd_cloexec(directory, 3)?;
        // SAFETY: fd is duplicated in the parent before fork and captured by
        // value, so it stays live until cwd selection and has CLOEXEC set.
        // The child callback performs only the async-signal-safe fchdir syscall
        // and conversion of its integer errno to io::Error (no allocation,
        // locks, environment access or other Rust work). It never follows a
        // pathname, and the descriptor closes automatically at exec. No
        // callback runs in the parent. Runner arguments, limits, environment
        // neutralization and process-group cleanup are otherwise unchanged.
        unsafe {
            command.pre_exec(move || {
                if libc::fchdir(fd.as_raw_fd()) == 0 {
                    Ok(())
                } else {
                    Err(io::Error::last_os_error())
                }
            });
        }
        Ok(())
    }
}

struct RunningChild {
    child: Child,
    completed: bool,
}
impl Drop for RunningChild {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let _ = Command::new("/bin/kill")
            .env_clear()
            .args(["-KILL", &format!("-{}", self.child.id())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_ready(stream: &mut UnixStream, bytes: &mut Vec<u8>, budget: usize) -> io::Result<bool> {
    let mut buffer = [0; 8192];
    // Finite batches let the caller enforce the deadline even under saturation.
    for _ in 0..32 {
        match stream.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(count) => {
                if bytes.len().saturating_add(count) > budget {
                    return Err(io::Error::other("output limit exceeded"));
                }
                bytes.extend_from_slice(&buffer[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

#[allow(unsafe_code)]
fn wait_for_io(
    stdout: Option<&UnixStream>,
    stderr: Option<&UnixStream>,
    stdin: Option<&UnixStream>,
    timeout: Duration,
) -> io::Result<bool> {
    let mut descriptors: [libc::pollfd; 3] = std::array::from_fn(|_| libc::pollfd {
        fd: -1,
        events: 0,
        revents: 0,
    });
    let mut count = 0;
    for (stream, events) in [
        (stdout, libc::POLLIN),
        (stderr, libc::POLLIN),
        (stdin, libc::POLLOUT),
    ] {
        if let Some(stream) = stream {
            descriptors[count] = libc::pollfd {
                fd: stream.as_raw_fd(),
                events,
                revents: 0,
            };
            count += 1;
        }
    }
    let millis = timeout.as_millis().min(i32::MAX as u128) as i32;
    // SAFETY: the slice contains at most three initialized pollfd values, and
    // every borrowed stream remains alive throughout the syscall. No completed
    // output stream is included; persistent EOF/HUP cannot cause a busy loop.
    let ready = unsafe { libc::poll(descriptors.as_mut_ptr(), count as libc::nfds_t, millis) };
    if ready < 0 {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            // The caller rechecks its deadline before the next finite I/O batch.
            return Ok(false);
        }
        return Err(error);
    }
    Ok(ready > 0)
}

#[cfg(test)]
fn run_bounded(
    command: Command,
    path: &Path,
    operation: &str,
    timeout: Duration,
    limit: u64,
) -> Result<Vec<u8>, SweepError> {
    run_bounded_inner(command, path, operation, timeout, limit, false, (&[], None))
}

fn run_bounded_inner(
    mut command: Command,
    path: &Path,
    operation: &str,
    timeout: Duration,
    limit: u64,
    allow_empty_no_match: bool,
    input_and_spawn_counter: (&[u8], Option<&Cell<u64>>),
) -> Result<Vec<u8>, SweepError> {
    let (input, spawned) = input_and_spawn_counter;
    let error = |code: &str, message: String| SweepError::new(code, operation, path, message);
    let (mut stdout, child_stdout) =
        UnixStream::pair().map_err(|e| error("output_io", e.to_string()))?;
    let (mut stderr, child_stderr) =
        UnixStream::pair().map_err(|e| error("output_io", e.to_string()))?;
    stdout
        .set_nonblocking(true)
        .map_err(|e| error("output_io", e.to_string()))?;
    stderr
        .set_nonblocking(true)
        .map_err(|e| error("output_io", e.to_string()))?;
    let mut stdin = if input.is_empty() {
        command.stdin(Stdio::null());
        None
    } else {
        let (writer, reader) = UnixStream::pair().map_err(|e| error("input_io", e.to_string()))?;
        writer
            .set_nonblocking(true)
            .map_err(|e| error("input_io", e.to_string()))?;
        command.stdin(Stdio::from(OwnedFd::from(reader)));
        Some(writer)
    };
    command
        .stdout(Stdio::from(OwnedFd::from(child_stdout)))
        .stderr(Stdio::from(OwnedFd::from(child_stderr)))
        .process_group(0);
    let mut child = RunningChild {
        child: command
            .spawn()
            .map_err(|e| error("git_spawn", e.to_string()))?,
        completed: false,
    };
    if let Some(spawned) = spawned {
        spawned.set(spawned.get().saturating_add(1));
    }
    // Drop the parent's duplicate child descriptors before waiting for EOF.
    drop(command);
    let budget = usize::try_from(limit).unwrap_or(usize::MAX);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut status = None;
    let mut written = 0usize;
    let started = Instant::now();
    loop {
        if started.elapsed() >= timeout {
            return Err(error("git_timeout", "Git read deadline exceeded".into()));
        }
        if let Some(writer) = stdin.as_mut() {
            for _ in 0..32 {
                if written == input.len() {
                    break;
                }
                match writer.write(&input[written..input.len().min(written + 8192)]) {
                    Ok(0) => return Err(error("input_io", "Git input pipe closed early".into())),
                    Ok(count) => written += count,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(error("input_io", e.to_string())),
                }
            }
            if written == input.len() {
                writer
                    .shutdown(std::net::Shutdown::Write)
                    .map_err(|e| error("input_io", e.to_string()))?;
                stdin = None;
            }
        }
        let read_error = |e: io::Error| {
            if e.kind() == io::ErrorKind::Other {
                error(
                    "git_output_limit",
                    format!("Git output exceeded {limit} bytes"),
                )
            } else {
                error("output_io", e.to_string())
            }
        };
        let out_done = read_ready(&mut stdout, &mut out, budget.saturating_sub(err.len()))
            .map_err(read_error)?;
        let err_done = read_ready(&mut stderr, &mut err, budget.saturating_sub(out.len()))
            .map_err(read_error)?;
        if status.is_none() {
            status = child
                .child
                .try_wait()
                .map_err(|e| error("git_wait", e.to_string()))?;
        }
        if status.is_some() && out_done && err_done {
            child.completed = true;
            break;
        }
        if started.elapsed() >= timeout {
            return Err(error("git_timeout", "Git read deadline exceeded".into()));
        }
        // macOS socketpairs commonly buffer only 8KiB. Sleeping after EAGAIN
        // throttles each producer burst; readiness wakes as soon as more I/O
        // is possible while the short ceiling still polls child exit/deadline.
        let wait = timeout
            .saturating_sub(started.elapsed())
            .min(Duration::from_millis(5));
        wait_for_io(
            (!out_done).then_some(&stdout),
            (!err_done).then_some(&stderr),
            stdin.as_ref(),
            wait,
        )
        .map_err(|e| error("output_io", e.to_string()))?;
    }
    let status = status.ok_or_else(|| error("git_wait", "missing Git exit status".into()))?;
    if !status.success() {
        if allow_empty_no_match && status.code() == Some(1) && out.is_empty() && err.is_empty() {
            return Ok(Vec::new());
        }
        return Err(error(
            "git_exit",
            format!(
                "Git exited with {status}: {}",
                BytePath::from_bytes(&err).display()
            ),
        ));
    }
    Ok(out)
}

fn classify_read_error(error: &io::Error) -> DirState {
    let reason = error.to_string();
    match error.raw_os_error() {
        Some(1 | 13) => DirState::Inaccessible { reason },
        Some(2) => DirState::Unknown {
            reason: format!("absence has not been verified in a readable parent: {reason}"),
        },
        _ if error.kind() == io::ErrorKind::PermissionDenied => DirState::Inaccessible { reason },
        _ => DirState::ReadError { reason },
    }
}

fn identity(metadata: &fs::Metadata) -> Identity {
    Identity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

fn observe_path(path: &Path) -> (Option<Identity>, DirState) {
    match fs::metadata(path) {
        Ok(metadata) => {
            let id = Some(identity(&metadata));
            if !metadata.is_dir() {
                return (
                    id,
                    DirState::Unknown {
                        reason: "registered worktree path is not a directory".into(),
                    },
                );
            }
            match fs::read_dir(path) {
                Ok(_) => (id, DirState::Present),
                Err(error) => (id, classify_read_error(&error)),
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let Some(parent) = path.parent() else {
                return (None, classify_read_error(&error));
            };
            let mut found = false;
            match fs::read_dir(parent) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(entry) => {
                                if Some(entry.file_name().as_os_str()) == path.file_name() {
                                    found = true;
                                }
                            }
                            Err(error) => return (None, classify_read_error(&error)),
                        }
                    }
                    if found {
                        (
                            None,
                            DirState::Unknown {
                                reason: "directory entry exists but its target could not be read"
                                    .into(),
                            },
                        )
                    } else {
                        (None, DirState::Absent)
                    }
                }
                Err(error) => (None, classify_read_error(&error)),
            }
        }
        Err(error) => (None, classify_read_error(&error)),
    }
}

fn parse_worktrees(output: &[u8], path: &Path) -> Result<Vec<Worktree>, SweepError> {
    let malformed = || {
        SweepError::new(
            "git_parse",
            "parse_worktrees",
            path,
            "malformed or unsupported NUL-delimited worktree output",
        )
    };
    if output.is_empty() || !output.ends_with(b"\0\0") {
        return Err(malformed());
    }
    let mut records = Vec::new();
    let mut fields = Vec::new();
    let mut paths = BTreeSet::new();
    for field in output[..output.len() - 1].split(|b| *b == 0) {
        if !field.is_empty() {
            fields.push(field);
            continue;
        }
        let Some(first) = fields.first().and_then(|f| f.strip_prefix(b"worktree ")) else {
            return Err(malformed());
        };
        if first.is_empty()
            || !PathBuf::from(std::ffi::OsString::from_vec(first.to_vec())).is_absolute()
            || !paths.insert(first.to_vec())
        {
            return Err(malformed());
        }
        let mut tree = Worktree {
            path: BytePath::from_bytes(first),
            identity: None,
            head: None,
            branch: None,
            detached: false,
            bare: false,
            locked: None,
            prunable: None,
            path_state: DirState::Unknown {
                reason: "not yet inspected".into(),
            },
        };
        let mut keys = BTreeSet::new();
        for field in &fields[1..] {
            let split = field.iter().position(|b| *b == b' ').unwrap_or(field.len());
            let key = &field[..split];
            let value = if split < field.len() {
                Some(&field[split + 1..])
            } else {
                None
            };
            if !keys.insert(key) {
                return Err(malformed());
            }
            match (key, value) {
                (b"HEAD", Some(value))
                    if matches!(value.len(), 40 | 64)
                        && value.iter().all(u8::is_ascii_hexdigit) =>
                {
                    tree.head = Some(String::from_utf8(value.to_vec()).map_err(|_| malformed())?);
                }
                (b"branch", Some(value))
                    if value.starts_with(b"refs/heads/") && value.len() > 11 =>
                {
                    tree.branch = Some(BytePath::from_bytes(value))
                }
                (b"detached", None) => tree.detached = true,
                (b"bare", None) => tree.bare = true,
                (b"locked", value) => {
                    tree.locked = Some(
                        value
                            .map(|b| BytePath::from_bytes(b).display())
                            .unwrap_or_default(),
                    )
                }
                (b"prunable", value) => {
                    tree.prunable = Some(
                        value
                            .map(|b| BytePath::from_bytes(b).display())
                            .unwrap_or_default(),
                    )
                }
                _ => return Err(malformed()),
            }
        }
        if usize::from(tree.bare) + usize::from(tree.detached) + usize::from(tree.branch.is_some())
            != 1
            || (tree.bare && tree.head.is_some())
            || (!tree.bare && tree.head.is_none())
        {
            return Err(malformed());
        }
        records.push(tree);
        fields.clear();
    }
    if records.is_empty() || !fields.is_empty() {
        return Err(malformed());
    }
    Ok(records)
}

fn validate_registry(common: &Path, trees: &[Worktree]) -> Result<(), SweepError> {
    let registry = common.join("worktrees");
    let error = |path: &Path, message: String| {
        SweepError::new("git_registry_read", "validate_registry", path, message)
    };
    let entries = match fs::read_dir(&registry) {
        Ok(entries) => Some(entries),
        Err(read_error) if read_error.kind() == io::ErrorKind::NotFound => {
            if !matches!(observe_path(&registry).1, DirState::Absent) {
                return Err(error(
                    &registry,
                    "registry absence could not be verified".into(),
                ));
            }
            None
        }
        Err(read_error) => return Err(error(&registry, read_error.to_string())),
    };
    let paths: BTreeSet<_> = trees.iter().map(|tree| tree.path.clone()).collect();
    let mut expected = BTreeSet::new();
    if let Some(entries) = entries {
        for entry in entries {
            let entry = entry.map_err(|e| error(&registry, e.to_string()))?;
            if !entry
                .file_type()
                .map_err(|e| error(&entry.path(), e.to_string()))?
                .is_dir()
            {
                return Err(error(
                    &entry.path(),
                    "registry entry is not a plain directory".into(),
                ));
            }
            let gitdir = entry.path().join("gitdir");
            let metadata =
                fs::symlink_metadata(&gitdir).map_err(|e| error(&gitdir, e.to_string()))?;
            if !metadata.is_file() {
                return Err(error(
                    &gitdir,
                    "registration pointer is not a regular file".into(),
                ));
            }
            const POINTER_LIMIT: u64 = 256 * 1024;
            let mut bytes = Vec::new();
            File::open(&gitdir)
                .and_then(|file| file.take(POINTER_LIMIT + 1).read_to_end(&mut bytes))
                .map_err(|e| error(&gitdir, e.to_string()))?;
            if bytes.len() as u64 > POINTER_LIMIT {
                return Err(error(
                    &gitdir,
                    "registration pointer exceeds byte limit".into(),
                ));
            }
            let bytes = bytes.strip_suffix(b"\n").ok_or_else(|| {
                error(
                    &gitdir,
                    "registration pointer is missing its line terminator".into(),
                )
            })?;
            if bytes.is_empty() || bytes.contains(&0) {
                return Err(error(
                    &gitdir,
                    "registration pointer is empty or contains NUL".into(),
                ));
            }
            let marker = PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec()));
            if !marker.is_absolute() || marker.file_name() != Some(std::ffi::OsStr::new(".git")) {
                return Err(error(
                    &gitdir,
                    "registration pointer is not an absolute worktree marker".into(),
                ));
            }
            let tree_path = marker
                .parent()
                .ok_or_else(|| error(&gitdir, "worktree marker has no parent".into()))?;
            let tree_path = BytePath::from_path(tree_path);
            if !paths.contains(&tree_path) || !expected.insert(tree_path) {
                return Err(error(
                    &gitdir,
                    "Git output omits or contradicts an administrative registration".into(),
                ));
            }
        }
    }
    if trees.len() != expected.len() + 1 {
        return Err(error(
            &registry,
            "Git output and administrative registration counts differ".into(),
        ));
    }
    Ok(())
}

fn absolute_git_path(raw: &[u8], path: &Path, operation: &str) -> Result<PathBuf, SweepError> {
    let error = || {
        SweepError::new(
            "git_parse",
            operation,
            path,
            "Git path output must be an absolute byte path with one final line terminator",
        )
    };
    let bytes = raw.strip_suffix(b"\n").ok_or_else(error)?;
    if bytes.is_empty() || bytes.contains(&0) {
        return Err(error());
    }
    let value = PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec()));
    if !value.is_absolute() {
        return Err(error());
    }
    Ok(value)
}

/// A separate administration has no implicit backlink to its primary tree.
/// Require Git's resolved top-level plus the actual .git marker to prove it.
fn prove_primary(
    git: &ControlledGit,
    candidate: &Path,
    common: &Path,
) -> Result<PathBuf, SweepError> {
    let error = |message: String| {
        SweepError::new(
            "primary_worktree_unknown",
            "prove_primary",
            candidate,
            message,
        )
    };
    let top = absolute_git_path(
        &git.read(candidate, GitRead::TopLevel)?,
        candidate,
        "top_level",
    )?;
    let resolved_gitdir = absolute_git_path(&git.read(&top, GitRead::GitDir)?, &top, "git_dir")?;
    let resolved_common =
        absolute_git_path(&git.read(&top, GitRead::CommonDir)?, &top, "common_dir")?;
    for directory in [&resolved_gitdir, &resolved_common] {
        if fs::canonicalize(directory).map_err(|e| error(e.to_string()))? != common {
            return Err(error(
                "candidate belongs to a linked or different administration".into(),
            ));
        }
    }
    let marker = top.join(".git");
    let metadata = fs::symlink_metadata(&marker).map_err(|e| error(e.to_string()))?;
    let target = if metadata.is_dir() {
        marker.clone()
    } else if metadata.is_file() {
        let fd = rustix::fs::open(
            &marker,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|e| error(e.to_string()))?;
        let file = File::from(fd);
        if !file.metadata().map_err(|e| error(e.to_string()))?.is_file() {
            return Err(error("primary marker changed to a nonregular file".into()));
        }
        const LIMIT: u64 = 256 * 1024;
        let mut bytes = Vec::new();
        file.take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| error(e.to_string()))?;
        if bytes.len() as u64 > LIMIT {
            return Err(error("primary marker exceeds byte limit".into()));
        }
        let pointer = bytes.strip_suffix(b"\n").unwrap_or(&bytes);
        let pointer = pointer
            .strip_prefix(b"gitdir: ")
            .filter(|p| !p.is_empty() && !p.contains(&0))
            .ok_or_else(|| error("primary marker is malformed".into()))?;
        let pointer = PathBuf::from(std::ffi::OsString::from_vec(pointer.to_vec()));
        if pointer.is_absolute() {
            pointer
        } else {
            top.join(pointer)
        }
    } else {
        return Err(error(
            "primary marker is neither a regular file nor directory".into(),
        ));
    };
    if fs::canonicalize(&target).map_err(|e| error(e.to_string()))? != common {
        return Err(error(
            "primary marker does not identify the common administration".into(),
        ));
    }
    Ok(top)
}

fn reconcile_primary(
    git: &ControlledGit,
    selected: &Path,
    common: &Path,
    trees: &mut [Worktree],
) -> Result<(), SweepError> {
    let primary = trees
        .first_mut()
        .ok_or_else(|| SweepError::new("git_parse", "primary", selected, "empty family"))?;
    if primary.bare {
        return Ok(());
    }
    let selected_gitdir =
        absolute_git_path(&git.read(selected, GitRead::GitDir)?, selected, "git_dir")?;
    let selected_gitdir = fs::canonicalize(&selected_gitdir).map_err(|e| {
        SweepError::new(
            "git_dir_read",
            "canonicalize",
            &selected_gitdir,
            e.to_string(),
        )
    })?;
    let reported = primary.path.to_path()?;
    // A linked tree's gitdir differs from common, so it can never be substituted
    // as the primary, even when the porcelain primary is an external admin.
    let mut failures = Vec::new();
    let selected_primary = if selected_gitdir == common {
        match prove_primary(git, selected, common) {
            Ok(path) => Some(path),
            Err(error) if error.code == "git_timeout" => return Err(error),
            Err(error) => {
                failures.push(error.to_string());
                None
            }
        }
    } else {
        None
    };
    let proven = match selected_primary {
        Some(path) => path,
        None => match prove_primary(git, &reported, common) {
            Ok(path) => path,
            Err(error) if error.code == "git_timeout" => return Err(error),
            Err(error) => {
                failures.push(error.to_string());
                return Err(SweepError::new(
                    "primary_worktree_unknown",
                    "primary",
                    selected,
                    format!(
                        "primary worktree could not be proved from top-level, gitdir and .git marker; separate administration may lack a backlink: {}",
                        failures.join("; ")
                    ),
                ));
            }
        },
    };
    primary.path = BytePath::from_path(&proven);
    Ok(())
}

pub fn scan_family(path: &Path) -> Result<Inventory, SweepError> {
    scan_family_with_git(path, &ControlledGit::default())
}
pub(crate) fn scan_family_with_git(
    path: &Path,
    git: &ControlledGit,
) -> Result<Inventory, SweepError> {
    let mut inventory = Inventory::new(ScanScope::Repo {
        path: BytePath::from_path(path),
    });
    let raw_common = git.read(path, GitRead::CommonDir)?;
    let raw_common = raw_common.strip_suffix(b"\n").ok_or_else(|| {
        SweepError::new(
            "git_parse",
            "common_dir",
            path,
            "common directory output has no line terminator",
        )
    })?;
    if raw_common.is_empty() || raw_common.contains(&0) {
        return Err(SweepError::new(
            "git_parse",
            "common_dir",
            path,
            "common directory output is empty or contains NUL",
        ));
    }
    let common = PathBuf::from(std::ffi::OsString::from_vec(raw_common.to_vec()));
    if !common.is_absolute() {
        return Err(SweepError::new(
            "git_parse",
            "common_dir",
            path,
            "common directory is not absolute",
        ));
    }
    let common = fs::canonicalize(&common)
        .map_err(|e| SweepError::new("common_dir_read", "canonicalize", &common, e.to_string()))?;
    let metadata = fs::metadata(&common)
        .map_err(|e| SweepError::new("common_dir_read", "metadata", &common, e.to_string()))?;
    if !metadata.is_dir() {
        return Err(SweepError::new(
            "common_dir_read",
            "metadata",
            &common,
            "common directory is not a directory",
        ));
    }
    let mut worktrees = parse_worktrees(&git.read(path, GitRead::Worktrees)?, path)?;
    reconcile_primary(git, path, &common, &mut worktrees)?;
    validate_registry(&common, &worktrees)?;
    for tree in &mut worktrees {
        let tree_path = tree.path.to_path()?;
        (tree.identity, tree.path_state) = observe_path(&tree_path);
        if !matches!(tree.path_state, DirState::Present | DirState::Absent) {
            inventory.coverage.gaps.push(Gap {
                path: tree.path.clone(),
                reason: format!("{:?}", tree.path_state),
            });
        }
        if tree
            .head
            .as_ref()
            .is_some_and(|head| head.bytes().all(|b| b == b'0'))
        {
            tree.head = None;
            inventory.coverage.gaps.push(Gap {
                path: tree.path.clone(),
                reason: "Git reports an unborn or unknown HEAD".into(),
            });
        }
    }
    inventory.families.push(Family {
        common_dir: BytePath::from_path(&common),
        identity: identity(&metadata),
        worktrees,
    });
    inventory.coverage.traversal_complete = inventory.coverage.gaps.is_empty();
    inventory.finish();
    Ok(inventory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_transport_denial_overrides_repository_protocol_allow() {
        let fixture = PathBuf::from(format!(
            "/private/tmp/wt-sweep-transport-{}-{}",
            std::process::id(),
            crate::model::now_ms()
        ));
        fs::create_dir(&fixture).unwrap_or_else(|e| panic!("fixture: {e}"));
        let git = ControlledGit::default();
        let mut init = git.command(&fixture);
        init.arg("init");
        // Fixture setup shares the normal budget; CI's first Git launch can be slow.
        run_bounded(init, &fixture, "fixture_init", git.timeout, 1024 * 1024)
            .unwrap_or_else(|e| panic!("init: {e}"));
        let mut config = git.command(&fixture);
        config.args(["config", "protocol.ext.allow", "always"]);
        run_bounded(config, &fixture, "fixture_config", git.timeout, 1024)
            .unwrap_or_else(|e| panic!("config: {e}"));
        let witness = fixture.join("witness");
        let mut command = git.command(&fixture);
        command
            .arg("fetch")
            .arg(format!("ext::/usr/bin/touch {}", witness.display()));
        let denial = run_bounded(
            command,
            &fixture,
            "fixture_transport",
            git.timeout,
            1024 * 1024,
        )
        .err()
        .unwrap_or_else(|| panic!("controlled Git must refuse the ext transport"));
        let executed = witness.exists();
        fs::remove_dir_all(&fixture).unwrap_or_else(|e| panic!("cleanup owned fixture: {e}"));
        assert_eq!(
            denial.code, "git_exit",
            "a timeout is not a transport refusal"
        );
        assert!(denial.message.contains("transport 'ext' not allowed"));
        assert!(
            !executed,
            "repository protocol.ext.allow bypassed generic protocol.allow=never"
        );
    }

    #[test]
    fn primary_proof_timeout_is_typed_and_does_not_retry_reported_path() {
        let fixture = PathBuf::from(format!(
            "/private/tmp/wt-sweep-primary-timeout-{}-{}",
            std::process::id(),
            crate::model::now_ms()
        ));
        fs::create_dir(&fixture).unwrap_or_else(|e| panic!("fixture: {e}"));
        let selected = fixture.join("selected");
        let redirected = fixture.join("redirected");
        for path in [&selected, &redirected] {
            fs::create_dir(path).unwrap_or_else(|e| panic!("mkdir: {e}"));
            let git = ControlledGit::default();
            let mut init = git.command(path);
            init.args(["init", "-b", "main"]);
            run_bounded(init, path, "fixture_init", git.timeout, 1024 * 1024)
                .unwrap_or_else(|e| panic!("init: {e}"));
        }
        // Initial admin reads use selected's regular config. TopLevel redirects
        // the proof into a different real repo whose GitDir read must time out.
        let git = ControlledGit::default();
        let mut config = git.command(&selected);
        config.args(["config", "core.worktree"]).arg(&redirected);
        run_bounded(config, &selected, "fixture_config", git.timeout, 1024)
            .unwrap_or_else(|e| panic!("config: {e}"));
        let config = redirected.join(".git/config");
        fs::remove_file(&config).unwrap_or_else(|e| panic!("replace owned config: {e}"));
        assert!(
            Command::new("/usr/bin/mkfifo")
                .arg(&config)
                .status()
                .unwrap_or_else(|e| panic!("fifo: {e}"))
                .success()
        );
        let git = ControlledGit::with_timeout(Duration::from_secs(1));
        let result = scan_family_with_git(&selected, &git);
        let spawned = git.spawned_processes();
        fs::remove_dir_all(&fixture).unwrap_or_else(|e| panic!("cleanup owned fixture: {e}"));
        let failure = result
            .err()
            .unwrap_or_else(|| panic!("blocked primary proof accepted"));
        assert_eq!(
            failure.code, "git_timeout",
            "primary timeout was swallowed into a generic proof refusal"
        );
        assert_eq!(spawned, 5, "reported primary retried after a typed timeout");
    }

    #[test]
    fn nul_porcelain_preserves_bytes_and_full_sha256_heads() {
        let mut output = b"worktree /private/tmp/a \n\xff\0HEAD ".to_vec();
        output
            .extend_from_slice(b"abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789");
        output.extend_from_slice(b"\0branch refs/heads/a\xff\0locked reason\nline\0\0");
        let trees = parse_worktrees(&output, Path::new("/private/tmp"))
            .unwrap_or_else(|e| panic!("parse: {e}"));
        assert_eq!(trees.len(), 1);
        assert_eq!(
            trees[0]
                .path
                .bytes()
                .unwrap_or_else(|e| panic!("bytes: {e}")),
            b"/private/tmp/a \n\xff"
        );
        assert_eq!(
            trees[0].head.as_deref(),
            Some("abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789")
        );
        assert_eq!(
            trees[0].branch.as_ref().and_then(|b| b.bytes().ok()),
            Some(b"refs/heads/a\xff".to_vec())
        );
        assert_eq!(trees[0].locked.as_deref(), Some("reason\\nline"));
    }

    #[test]
    fn malformed_or_empty_porcelain_is_an_error() {
        for bytes in [&b""[..], b"worktree /a\0bare\0", b"worktree /a\0HEAD abc\0detached\0\0", b"worktree relative\0bare\0\0", b"worktree /a\0bare\0unexpected\0\0", b"worktree /a\0bare\0bare\0\0", b"worktree /a\0HEAD 0123456789012345678901234567890123456789\0branch refs/heads/a\0detached\0\0"] {
            assert!(parse_worktrees(bytes, Path::new("/a")).is_err(), "accepted {bytes:?}");
        }
    }

    #[test]
    fn permission_and_io_errors_are_never_absence() {
        for errno in [1, 13] {
            assert!(matches!(
                classify_read_error(&std::io::Error::from_raw_os_error(errno)),
                DirState::Inaccessible { .. }
            ));
        }
        assert!(matches!(
            classify_read_error(&std::io::Error::from_raw_os_error(5)),
            DirState::ReadError { .. }
        ));
        assert!(!matches!(
            classify_read_error(&std::io::Error::from_raw_os_error(2)),
            DirState::Absent
        ));
    }

    #[test]
    fn bounded_runner_stops_saturated_output_and_descendant_pipe_hold() {
        let path = Path::new("/private/tmp");
        let mut saturation = Command::new("/bin/sh");
        saturation
            .env_clear()
            .args(["-c", "while :; do printf 1234567890; done"]);
        let error = run_bounded(
            saturation,
            path,
            "test_saturation",
            Duration::from_secs(1),
            1024,
        )
        .err()
        .unwrap_or_else(|| panic!("saturation accepted"));
        assert_eq!(error.code, "git_output_limit");
        let mut descendant = Command::new("/bin/sh");
        descendant.env_clear().args(["-c", "/bin/sleep 30 & wait"]);
        let started = Instant::now();
        let error = run_bounded(
            descendant,
            path,
            "test_descendant",
            Duration::from_millis(50),
            1024,
        )
        .err()
        .unwrap_or_else(|| panic!("timeout accepted"));
        assert_eq!(error.code, "git_timeout");
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn bounded_runner_checks_exit_even_when_stdout_is_usable() {
        let mut command = Command::new("/bin/sh");
        command
            .env_clear()
            .args(["-c", "printf valid; printf broken >&2; exit 7"]);
        let error = run_bounded(
            command,
            Path::new("/private/tmp"),
            "test_exit",
            Duration::from_secs(1),
            1024,
        )
        .err()
        .unwrap_or_else(|| panic!("failed exit accepted"));
        assert_eq!(error.code, "git_exit");
        assert!(error.message.contains("broken"));
    }
    #[test]
    fn io_wait_selects_live_output_and_pending_input_and_excludes_completed_streams() {
        let (mut output, mut producer) = UnixStream::pair().unwrap_or_else(|e| panic!("{e}"));
        let (error, _error_producer) = UnixStream::pair().unwrap_or_else(|e| panic!("{e}"));
        producer
            .write_all(b"resident output")
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(
            wait_for_io(Some(&output), Some(&error), None, Duration::from_millis(20))
                .unwrap_or_else(|e| panic!("{e}"))
        );
        let mut bytes = [0u8; 15];
        output
            .read_exact(&mut bytes)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(&bytes, b"resident output");
        let (input, _consumer) = UnixStream::pair().unwrap_or_else(|e| panic!("{e}"));
        assert!(
            wait_for_io(None, None, Some(&input), Duration::from_millis(20))
                .unwrap_or_else(|e| panic!("{e}"))
        );
        drop(producer);
        assert!(
            wait_for_io(Some(&output), None, None, Duration::from_millis(20))
                .unwrap_or_else(|e| panic!("{e}"))
        );
        assert!(
            !wait_for_io(None, None, None, Duration::from_millis(1))
                .unwrap_or_else(|e| panic!("{e}")),
            "completed stream HUP must not keep waking the runner"
        );
    }

    #[test]
    fn bounded_runner_transfers_large_partial_input_and_aggregates_stderr_limit() {
        let input = vec![b'x'; 1024 * 1024];
        let mut command = Command::new("/bin/cat");
        command.env_clear();
        let output = run_bounded_inner(
            command,
            Path::new("/private/tmp"),
            "test_partial_input",
            Duration::from_secs(5),
            (input.len() + 1) as u64,
            false,
            (&input, None),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(output, input);
        let mut command = Command::new("/bin/sh");
        command
            .env_clear()
            .args(["-c", "printf 123456; printf abcdef >&2"]);
        let error = run_bounded(
            command,
            Path::new("/private/tmp"),
            "test_combined_limit",
            Duration::from_secs(1),
            10,
        )
        .err()
        .unwrap_or_else(|| panic!("combined stdout+stderr limit was ignored"));
        assert_eq!(error.code, "git_output_limit");
    }
    #[test]
    fn bounded_runner_streams_32mib_round_trip_within_its_deadline() {
        let mut input = vec![b'x'; 32 * 1024 * 1024];
        input[0] = 0;
        let end = input.len() - 1;
        input[end] = 255;
        let mut command = Command::new("/bin/cat");
        command.env_clear();
        let output = run_bounded_inner(
            command,
            Path::new("/private/tmp"),
            "test_streaming_deadline",
            Duration::from_secs(10),
            (input.len() + 1) as u64,
            false,
            (&input, None),
        )
        .unwrap_or_else(|e| panic!("finite binary stream did not complete: {e}"));
        assert_eq!(output, input);
    }
}
