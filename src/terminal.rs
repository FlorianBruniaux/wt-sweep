//! Terminal-only decoration. Persisted artifacts never pass through this module.
use crate::Options;
use std::io::{IsTerminal, Write};

fn styled(opts: &Options, is_terminal: bool) -> bool {
    match opts.get("--color").and_then(|v| v[0].to_str()) {
        Some("always") => true,
        Some("never") => false,
        _ => {
            is_terminal
                && std::env::var_os("TERM").is_none_or(|v| v != "dumb")
                && std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty())
        }
    }
}

pub fn decorate(opts: &Options, plain: &str) -> String {
    if !styled(opts, std::io::stdout().is_terminal()) {
        return plain.into();
    }
    let mut output = String::with_capacity(plain.len());
    for line in plain.split_inclusive('\n') {
        let (icon, color) = if line.starts_with("WORKTREE SWEEP") {
            ("📋 ", "1;36")
        } else if line.starts_with("  Keep:") {
            ("  📌 ", "36")
        } else if line.starts_with("  Cleanup candidates:") {
            ("  🔎 ", "33")
        } else if line.starts_with("  Needs verification:") {
            ("  ⚠ ", "33")
        } else if line.starts_with("DISK USAGE") {
            ("💾 ", "1;36")
        } else if line.starts_with("COVERAGE") {
            ("🔎 ", "1;36")
        } else if line.starts_with("NEXT CHECK") {
            ("👉 ", "1;33")
        } else if line.trim_end() == "REPORT" {
            ("📄 ", "1;36")
        } else if line.starts_with("RESULTS") {
            ("", "1")
        } else {
            output.push_str(line);
            continue;
        };
        output.push_str(&format!("\x1b[{color}m{icon}{}\x1b[0m", line.trim()));
        if line.ends_with('\n') {
            output.push('\n');
        }
    }
    output
}

pub struct Progress {
    verbose: bool,
    live: bool,
    drawn: bool,
}
impl Progress {
    pub fn new(opts: &Options) -> Self {
        let verbose = opts.contains_key("--verbose");
        let tty = std::io::stderr().is_terminal();
        Self {
            verbose,
            live: !verbose
                && tty
                && styled(opts, tty)
                && std::env::var_os("TERM").is_none_or(|v| v != "dumb"),
            drawn: false,
        }
    }

    pub fn verbose(&self) -> bool {
        self.verbose
    }

    /// Milestones remain visible in pipes; transient counters only update a TTY.
    pub fn update(&mut self, message: &str, milestone: bool) {
        let mut stderr = std::io::stderr().lock();
        if self.verbose {
            let _ = writeln!(stderr, "{}", message.trim_end());
        } else if self.live {
            // Read the live width so a narrow pane does not turn updates into log lines.
            let columns = rustix::termios::tcgetwinsize(&stderr)
                .ok()
                .map(|size| usize::from(size.ws_col))
                .filter(|width| *width > 0)
                .unwrap_or(80);
            if columns < 4 {
                let _ = write!(stderr, "\r\x1b[2K");
                let _ = stderr.flush();
                self.drawn = false;
                return;
            }
            // Labels are ASCII; reserve two cells for the icon, a space and the last column.
            let line: String = message.trim().chars().take((columns - 4).min(68)).collect();
            let _ = write!(stderr, "\r\x1b[2K\x1b[36m🔎 {line}\x1b[0m");
            self.drawn = true;
        } else if milestone {
            let _ = writeln!(stderr, "{}", message.trim_end());
        }
        let _ = stderr.flush();
    }

    pub fn finish(&mut self) {
        if self.drawn {
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "\r\x1b[2K");
            let _ = stderr.flush();
            self.drawn = false;
        }
    }
}
impl Drop for Progress {
    fn drop(&mut self) {
        self.finish();
    }
}
