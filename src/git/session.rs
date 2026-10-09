//! Finite adaptive commit reads. Returned payloads are provisional until finish succeeds.
use super::*;

pub(crate) struct CommitSession {
    child: RunningChild,
    input: UnixStream,
    output: UnixStream,
    errors: UnixStream,
    out: Vec<u8>,
    err: Vec<u8>,
    consumed: usize,
    wire_limit: usize,
    requested: usize,
    max_objects: usize,
    started: Instant,
    timeout: Duration,
    path: PathBuf,
    failed: Option<SweepError>,
    #[cfg(test)]
    journal_index: usize,
}
impl ControlledGit {
    pub(crate) fn commit_session(
        &self,
        repo: &Path,
        max_objects: usize,
        payload_allowance: usize,
    ) -> Result<CommitSession, SweepError> {
        crate::residency::ensure_no_materialization()?;
        if max_objects == 0 || max_objects > 128 || payload_allowance > 128 * 1024 * 1024 {
            return Err(SweepError::new(
                "git_argument",
                "commit_batch",
                repo,
                "commit sessions require 1..128 objects",
            ));
        }
        let mut command = self.command(repo);
        command.args(["cat-file", "--batch-command"]);
        CommitSession::spawn(
            command,
            repo,
            self.timeout,
            payload_allowance,
            max_objects,
            Some(&self.spawned),
        )
    }
}
impl CommitSession {
    fn spawn(
        mut command: Command,
        path: &Path,
        timeout: Duration,
        payload_limit: usize,
        max_objects: usize,
        counter: Option<&Cell<u64>>,
    ) -> Result<Self, SweepError> {
        let io_error =
            |e: io::Error| SweepError::new("git_io", "commit_batch", path, e.to_string());
        let (input, child_input) = UnixStream::pair().map_err(io_error)?;
        let (output, child_output) = UnixStream::pair().map_err(io_error)?;
        let (errors, child_errors) = UnixStream::pair().map_err(io_error)?;
        input.set_nonblocking(true).map_err(io_error)?;
        output.set_nonblocking(true).map_err(io_error)?;
        errors.set_nonblocking(true).map_err(io_error)?;
        command
            .stdin(Stdio::from(OwnedFd::from(child_input)))
            .stdout(Stdio::from(OwnedFd::from(child_output)))
            .stderr(Stdio::from(OwnedFd::from(child_errors)))
            .process_group(0);
        let child = command
            .spawn()
            .map_err(|e| SweepError::new("git_spawn", "commit_batch", path, e.to_string()))?;
        if let Some(counter) = counter {
            counter.set(counter.get().saturating_add(1));
        }
        drop(command);
        #[cfg(test)]
        let journal_index = CHILD_READ_JOURNAL.with(|journal| {
            let mut journal = journal.borrow_mut();
            let index = journal.len();
            journal.push((
                BytePath::from_path(path),
                GitRead::CommitBatch { oids: Vec::new() },
            ));
            index
        });
        Ok(Self {
            child: RunningChild {
                child,
                completed: false,
            },
            input,
            output,
            errors,
            out: Vec::new(),
            err: Vec::new(),
            consumed: 0,
            // Payload budget stays separate from bounded protocol headers and stderr.
            wire_limit: payload_limit
                .saturating_add(max_objects * 320)
                .saturating_add(64 * 1024),
            requested: 0,
            max_objects,
            started: Instant::now(),
            timeout,
            path: path.into(),
            failed: None,
            #[cfg(test)]
            journal_index,
        })
    }
    fn error(&self, code: &str, message: impl Into<String>) -> SweepError {
        SweepError::new(code, "commit_batch", &self.path, message)
    }
    fn deadline(&self) -> Result<(), SweepError> {
        if self.started.elapsed() >= self.timeout {
            Err(self.error("git_timeout", "Git read deadline exceeded"))
        } else {
            Ok(())
        }
    }
    fn drain(&mut self) -> Result<(bool, bool), SweepError> {
        let out_done = read_ready(
            &mut self.output,
            &mut self.out,
            self.wire_limit
                .saturating_sub(self.consumed + self.err.len()),
        )
        .map_err(|e| {
            self.error(
                if e.kind() == io::ErrorKind::Other {
                    "git_output_limit"
                } else {
                    "output_io"
                },
                e.to_string(),
            )
        })?;
        let err_done = read_ready(
            &mut self.errors,
            &mut self.err,
            (64 * 1024).min(
                self.wire_limit
                    .saturating_sub(self.consumed + self.out.len()),
            ),
        )
        .map_err(|e| {
            self.error(
                if e.kind() == io::ErrorKind::Other {
                    "git_output_limit"
                } else {
                    "output_io"
                },
                e.to_string(),
            )
        })?;
        Ok((out_done, err_done))
    }
    fn wait(&self, writing: bool, out_done: bool, err_done: bool) -> Result<(), SweepError> {
        self.deadline()?;
        wait_for_io(
            (!out_done).then_some(&self.output),
            (!err_done).then_some(&self.errors),
            writing.then_some(&self.input),
            self.timeout
                .saturating_sub(self.started.elapsed())
                .min(Duration::from_millis(5)),
        )
        .map(|_| ())
        .map_err(|e| self.error("git_io", e.to_string()))
    }
    fn exchange(
        &mut self,
        request: &[u8],
        oid: &str,
        contents: bool,
    ) -> Result<Vec<u8>, SweepError> {
        let result = self.exchange_inner(request, oid, contents);
        if let Err(error) = &result {
            self.failed = Some(error.clone());
        }
        result
    }
    fn exchange_inner(
        &mut self,
        request: &[u8],
        oid: &str,
        contents: bool,
    ) -> Result<Vec<u8>, SweepError> {
        let mut written = 0;
        loop {
            self.deadline()?;
            if written < request.len() {
                match self.input.write(&request[written..]) {
                    Ok(0) => return Err(self.error("input_io", "Git input pipe closed early")),
                    Ok(count) => written += count,
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) => return Err(self.error("input_io", e.to_string())),
                }
            }
            let (out_done, err_done) = self.drain()?;
            if let Some(end) = self.out.iter().take(160).position(|b| *b == b'\n') {
                let infos = parse_object_info(&self.out[..=end], &[oid.into()])?;
                let extra = match &infos[0] {
                    Ok(info) if contents => info
                        .size
                        .checked_add(1)
                        .ok_or_else(|| self.error("git_batch", "object size overflow"))?,
                    _ => 0,
                };
                let expected = (end + 1)
                    .checked_add(extra)
                    .ok_or_else(|| self.error("git_batch", "frame size overflow"))?;
                if expected
                    > self
                        .wire_limit
                        .saturating_sub(self.consumed + self.err.len())
                {
                    return Err(self.error(
                        "git_output_limit",
                        "commit frame exceeds remaining transport allowance",
                    ));
                }
                if self.out.len() >= expected {
                    if self.out.len() != expected || written != request.len() {
                        return Err(self.error(
                            "git_batch",
                            "unexpected bytes outside requested commit frame",
                        ));
                    }
                    self.consumed += self.out.len();
                    return Ok(std::mem::take(&mut self.out));
                }
            } else if self.out.len() >= 160 {
                return Err(self.error("git_batch", "invalid commit response header"));
            }
            if out_done {
                return Err(self.error("git_batch", "Git closed an incomplete commit response"));
            }
            self.wait(written < request.len(), out_done, err_done)?;
        }
    }
    pub(crate) fn read(
        &mut self,
        oid: &str,
        remaining_payload: usize,
        remaining_time: Duration,
    ) -> Result<Vec<u8>, SweepError> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        self.timeout = self
            .timeout
            .min(self.started.elapsed().saturating_add(remaining_time));
        let result = self.read_inner(oid, remaining_payload);
        if let Err(error) = &result
            && error.code == "git_timeout"
        {
            self.failed = Some(error.clone());
        }
        result
    }
    fn read_inner(&mut self, oid: &str, remaining_payload: usize) -> Result<Vec<u8>, SweepError> {
        self.deadline()?;
        if self.requested >= self.max_objects
            || !matches!(oid.len(), 40 | 64)
            || !oid.bytes().all(|b| b.is_ascii_hexdigit())
            || oid.bytes().all(|b| b == b'0')
        {
            return Err(self.error(
                "git_argument",
                "commit request exceeds session limits or has invalid OID",
            ));
        }
        self.requested += 1;
        #[cfg(test)]
        CHILD_READ_JOURNAL.with(|journal| {
            if let Some((_, GitRead::CommitBatch { oids })) =
                journal.borrow_mut().get_mut(self.journal_index)
            {
                oids.push(oid.into());
            }
        });
        let info_raw = self.exchange(format!("info {oid}\n").as_bytes(), oid, false)?;
        let info = parse_object_info(&info_raw, &[oid.into()])?
            .remove(0)
            .map_err(|message| self.error("git_batch", message))?;
        if info.kind != "commit" {
            return Err(self.error("git_batch", "requested object is not a commit"));
        }
        if info.size > remaining_payload.min(8 * 1024 * 1024) {
            return Err(self.error(
                "git_output_limit",
                "commit exceeds remaining payload byte allowance",
            ));
        }
        let raw = self.exchange(format!("contents {oid}\n").as_bytes(), oid, true)?;
        let object = parse_objects(&raw, &[oid.into()])
            .inspect_err(|error| self.failed = Some(error.clone()))?
            .remove(0)
            .map_err(|message| self.error("git_batch", message))?;
        if object.kind != "commit" || object.bytes.len() != info.size {
            return Err(self.error(
                "git_batch",
                "commit contents disagree with size or type probe",
            ));
        }
        Ok(object.bytes)
    }
    /// No provisional object can become decision evidence before this returns Ok.
    pub(crate) fn finish(mut self, remaining_time: Duration) -> Result<(), SweepError> {
        if let Some(error) = &self.failed {
            return Err(error.clone());
        }
        self.timeout = self
            .timeout
            .min(self.started.elapsed().saturating_add(remaining_time));
        if let Err(error) = self.input.shutdown(std::net::Shutdown::Write)
            && error.kind() != io::ErrorKind::NotConnected
        {
            return Err(self.error("input_io", error.to_string()));
        }
        // A peer that already closed still requires exit and output validation below.
        loop {
            self.deadline()?;
            let (out_done, err_done) = self.drain()?;
            let status = self
                .child
                .child
                .try_wait()
                .map_err(|e| self.error("git_wait", e.to_string()))?;
            if let Some(status) = status
                && out_done
                && err_done
            {
                self.child.completed = true;
                if !status.success() {
                    return Err(self.error(
                        "git_exit",
                        format!(
                            "Git exited with {status}: {}",
                            BytePath::from_bytes(&self.err).display()
                        ),
                    ));
                }
                if !self.out.is_empty() {
                    return Err(
                        self.error("git_batch", "unrequested output at end of commit session")
                    );
                }
                return Ok(());
            }
            self.wait(false, out_done, err_done)?;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    fn scripted(script: &str, timeout: Duration, payload_limit: usize) -> CommitSession {
        let mut command = Command::new("/bin/sh");
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .args(["-c", script]);
        CommitSession::spawn(
            command,
            Path::new("/private/tmp"),
            timeout,
            payload_limit,
            4,
            None,
        )
        .unwrap()
    }
    const OID: &str = "1111111111111111111111111111111111111111";
    #[test]
    fn provisional_payload_is_refused_when_child_exits_unsuccessfully() {
        let mut session = scripted(
            "read command oid; printf '%s commit 4\\n' \"$oid\"; read command oid; printf '%s commit 4\\ndata\\n' \"$oid\"; exit 7",
            Duration::from_secs(2),
            1024,
        );
        assert_eq!(
            session.read(OID, 1024, Duration::from_secs(2)).unwrap(),
            b"data"
        );
        assert_eq!(
            session.finish(Duration::from_secs(2)).unwrap_err().code,
            "git_exit"
        );
    }
    #[test]
    fn already_exited_child_keeps_its_exit_verdict() {
        for (exit_code, expected) in [(0, None), (7, Some("git_exit"))] {
            let mut session = scripted(
                &format!(
                    "read command oid; printf '%s commit 4\\n' \"$oid\"; read command oid; printf '%s commit 4\\ndata\\n' \"$oid\"; exit {exit_code}"
                ),
                Duration::from_secs(2),
                1024,
            );
            assert_eq!(
                session.read(OID, 1024, Duration::from_secs(2)).unwrap(),
                b"data"
            );
            // Force the peer-close ordering instead of relying on scheduler timing.
            while session.child.child.try_wait().unwrap().is_none() {
                session.deadline().unwrap();
                std::thread::sleep(Duration::from_millis(1));
            }
            let result = session.finish(Duration::from_secs(2));
            assert_eq!(
                result.as_ref().err().map(|error| error.code.as_str()),
                expected
            );
        }
    }
    #[test]
    fn expired_read_cannot_be_redeemed_by_a_successful_exit() {
        let mut session = scripted(
            "read command oid; cat >/dev/null; exit 0",
            Duration::from_millis(30),
            1024,
        );
        assert_eq!(
            session
                .read(OID, 1024, Duration::from_secs(2))
                .unwrap_err()
                .code,
            "git_timeout"
        );
        assert_eq!(
            session.finish(Duration::from_secs(2)).unwrap_err().code,
            "git_timeout"
        );
    }
    #[test]
    fn malformed_payload_terminator_cannot_be_redeemed_by_successful_exit() {
        let mut session = scripted(
            "read command oid; printf '%s commit 4\\n' \"$oid\"; read command oid; printf '%s commit 4\\ndataX' \"$oid\"; cat >/dev/null; exit 0",
            Duration::from_secs(2),
            1024,
        );
        assert_eq!(
            session
                .read(OID, 1024, Duration::from_secs(2))
                .unwrap_err()
                .code,
            "git_batch"
        );
        assert_eq!(
            session.finish(Duration::from_secs(2)).unwrap_err().code,
            "git_batch"
        );
    }
    #[test]
    fn stderr_overflow_poison_is_retained_until_finish() {
        let mut session = scripted(
            "read command oid; dd if=/dev/zero bs=65537 count=1 >&2 2>/dev/null; cat >/dev/null; exit 0",
            Duration::from_secs(2),
            1024 * 1024,
        );
        assert_eq!(
            session
                .read(OID, 1024, Duration::from_secs(2))
                .unwrap_err()
                .code,
            "git_output_limit"
        );
        assert_eq!(
            session
                .read(OID, 1024, Duration::from_secs(2))
                .unwrap_err()
                .code,
            "git_output_limit"
        );
        assert_eq!(
            session.finish(Duration::from_secs(2)).unwrap_err().code,
            "git_output_limit"
        );
    }
    #[test]
    fn size_probe_refuses_payload_before_contents_command() {
        let mut session = scripted(
            "read command oid; printf '%s commit 4096\\n' \"$oid\"; if read command oid; then exit 9; fi",
            Duration::from_secs(2),
            1024,
        );
        assert_eq!(
            session
                .read(OID, 1024, Duration::from_secs(2))
                .unwrap_err()
                .code,
            "git_output_limit"
        );
        session.finish(Duration::from_secs(2)).unwrap();
    }
    #[test]
    fn malformed_commit_frame_and_hung_child_are_refused() {
        let mut malformed = scripted(
            "read command oid; printf 'wrong commit 4\\n'; cat >/dev/null",
            Duration::from_secs(2),
            1024,
        );
        assert_eq!(
            malformed
                .read(OID, 1024, Duration::from_secs(2))
                .unwrap_err()
                .code,
            "git_batch"
        );
        drop(malformed);
        let mut hung = scripted(
            "read command oid; sleep 5",
            Duration::from_millis(100),
            1024,
        );
        assert_eq!(
            hung.read(OID, 1024, Duration::from_secs(2))
                .unwrap_err()
                .code,
            "git_timeout"
        );
    }
}
