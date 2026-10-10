//! Desktop access stays separate from evidence collection and publication.
use std::{
    path::{Component, Path, PathBuf},
    time::Duration,
};

pub(crate) fn default_base() -> Result<PathBuf, String> {
    if !cfg!(target_os = "macos") {
        return Err(
            "--html needs --run-dir BASE on this platform; the Desktop default is macOS-only"
                .into(),
        );
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && !p.components().any(|c| c == Component::ParentDir))
        .ok_or("Cannot locate Desktop from HOME; choose --run-dir BASE")?;
    let desktop = home.join("Desktop");
    let metadata = std::fs::symlink_metadata(&desktop)
        .map_err(|_| "Desktop is unavailable; choose an existing directory with --run-dir BASE")?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Desktop must be a plain directory; choose --run-dir BASE".into());
    }
    Ok(desktop.join("Worktree Sweep"))
}

pub(crate) fn reopen_command(path: &Path) -> Option<String> {
    if !cfg!(target_os = "macos") || !path.is_absolute() {
        return None;
    }
    let value = path.to_str()?;
    if value.chars().any(|c| {
        c.is_control()
            || ('\u{202a}'..='\u{202e}').contains(&c)
            || ('\u{2066}'..='\u{2069}').contains(&c)
    }) {
        return None;
    }
    Some(format!("/usr/bin/open '{}'", value.replace('\'', "'\\''")))
}

pub(crate) fn open(path: &Path) -> Result<(), String> {
    launch(Path::new("/usr/bin/open"), path, Duration::from_secs(5))
}

fn launch(opener: &Path, path: &Path, timeout: Duration) -> Result<(), String> {
    use std::{
        process::{Command, Stdio},
        time::Instant,
    };
    if !path.is_absolute() {
        return Err("HTML report path must be absolute".into());
    }
    // Fixed native executable in production; paths are one argument, never shell code.
    let mut child = Command::new(opener)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not start macOS opener: {e}"))?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(format!("macOS opener exited with {status}"))
                };
            }
            Ok(None) if started.elapsed() < timeout && !wt_sweep::cancel::requested() => {
                std::thread::sleep(Duration::from_millis(10));
            }
            result => {
                child
                    .kill()
                    .map_err(|e| format!("could not stop macOS opener: {e}"))?;
                child
                    .wait()
                    .map_err(|e| format!("could not reap macOS opener: {e}"))?;
                return Err(match result {
                    Err(e) => format!("could not wait for macOS opener: {e}"),
                    _ => "macOS opener timed out or was interrupted".into(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::PermissionsExt,
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
        time::Instant,
    };

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::fs::canonicalize(std::env::temp_dir())?.join(format!(
                "wt-sweep-opener-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path)?;
            Ok(Self(path))
        }
        fn script(&self, body: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
            let path = self.0.join("open-test");
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
            Ok(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn opener_preserves_one_literal_path_and_checks_exit_status()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = Fixture::new()?;
        let output = fixture.0.join("arguments");
        let executable = fixture.script(&format!(
            "printf '%s\\n' \"$#\" \"$1\" > '{}'",
            output.display()
        ))?;
        let path = Path::new("/fixture/it's $(printf nope) work/report.html");
        launch(&executable, path, Duration::from_secs(1))?;
        assert_eq!(
            std::fs::read_to_string(output)?,
            "1\n/fixture/it's $(printf nope) work/report.html\n"
        );
        fixture.script("exit 7")?;
        assert!(launch(&executable, path, Duration::from_secs(1)).is_err());
        assert!(launch(&fixture.0.join("missing"), path, Duration::from_secs(1)).is_err());
        Ok(())
    }

    #[test]
    fn opener_timeout_is_bounded() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = Fixture::new()?;
        let executable = fixture.script("while :; do :; done")?;
        let started = Instant::now();
        let error = launch(
            &executable,
            Path::new("/fixture/report.html"),
            Duration::from_millis(80),
        )
        .err()
        .ok_or("opener should time out")?;
        assert!(error.contains("timed out"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(2));
        Ok(())
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn reopen_command_round_trips_quotes_and_refuses_controls()
    -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new("/fixture/it's $(printf nope) work/report.html");
        let command = reopen_command(path).ok_or("expected reopen command")?;
        let result = Command::new("/bin/sh")
            .arg("-c")
            .arg(format!("set -- {command}; printf '%s\\n' \"$@\""))
            .output()?;
        assert!(result.status.success());
        assert_eq!(
            result.stdout,
            b"/usr/bin/open\n/fixture/it's $(printf nope) work/report.html\n"
        );
        assert!(reopen_command(Path::new("/fixture/control\n/report.html")).is_none());
        Ok(())
    }
}
