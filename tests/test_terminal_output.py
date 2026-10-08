"""Terminal presentation on disposable Git repositories, including real PTYs."""
import errno
import fcntl
import re
import struct
import termios
import json
import os
from pathlib import Path
import pty
import subprocess
import tempfile
import threading
import unittest

from inventory_support import BINARY, HistoryFixture, filesystem_manifest


class TerminalOutputTests(unittest.TestCase):
    def setUp(self):
        self.fixture = HistoryFixture()
        self.addCleanup(self.fixture.close)
        self.output = tempfile.TemporaryDirectory(prefix="wt-sweep-terminal-", dir="/private/tmp")
        self.addCleanup(self.output.cleanup)
        self.env = dict(self.fixture.env, TERM="xterm-256color")

    def arguments(self, *extra):
        return [str(BINARY), "review", "--repo", str(self.fixture.principal),
                "--target", "refs/heads/main", "--no-context", *map(str, extra)]

    def run_cli(self, *extra, env=None):
        before = filesystem_manifest(self.fixture.root)
        result = subprocess.run(self.arguments(*extra), env=env or self.env,
                                capture_output=True, timeout=30)
        self.assertEqual(before, filesystem_manifest(self.fixture.root))
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        return result

    def test_default_recap_is_short_without_a_log_line_per_disk_root(self):
        result = self.run_cli("--summary", "--disk-usage", "--run-dir", self.output.name)
        self.assertLessEqual(len(result.stderr.splitlines()), 8)
        self.assertEqual(result.stderr.count(b"Observed history:"), 1)
        self.assertEqual(result.stderr.count(b"Observed local:"), 1)
        self.assertIn(b"provisional", result.stderr)
        self.assertNotIn(b"\x1b", result.stdout + result.stderr)
        self.assertNotIn(b"\r", result.stdout + result.stderr)
        self.assertLessEqual(len(result.stdout.splitlines()), 32)
        for label in (b"Keep: 1", b"Cleanup candidates: 1", b"Needs verification: 0",
                      b"not reclaimable space", b"no removal authorized", b"report.txt"):
            self.assertIn(label, result.stdout)
        self.assertNotIn(b"Git commands", result.stdout)
        self.assertNotIn(b"DISCOVERY PERFORMANCE", result.stdout)
        report = next(Path(self.output.name).glob("run-*/report.txt")).read_bytes()
        self.assertIn(b"DISCOVERY PERFORMANCE", report)
        self.assertIn(os.fsencode(self.fixture.linked), report)

    def test_verbose_restores_diagnostic_metrics_and_disk_paths(self):
        result = self.run_cli("--summary", "--disk-usage", "--verbose")
        self.assertIn(os.fsencode(self.fixture.linked), result.stderr)
        self.assertIn(b"DISCOVERY PERFORMANCE", result.stdout)
        self.assertIn(b"COLLECTION TIME", result.stdout)
        self.assertIn(b"Checking local files", result.stderr)

    def test_forced_color_never_decorates_saved_artifacts_or_json_stdout(self):
        result = self.run_cli("--summary", "--color", "always", "--run-dir", self.output.name)
        self.assertIn(b"\x1b[", result.stdout)
        self.assertIn("📋".encode(), result.stdout)
        run = next(Path(self.output.name).glob("run-*"))
        for name in ("inventory.json", "review.json", "report.txt", "run.json"):
            self.assertNotIn(b"\x1b", (run / name).read_bytes())
        document = json.loads((run / "review.json").read_bytes())
        self.assertEqual(sorted(row["classification"] for row in document["items"]),
                         ["cleanup_candidate", "keep"])
        result = self.run_cli("--format", "json", "--color", "always")
        self.assertEqual(json.loads(result.stdout)["schema"], "wt-sweep.review/2")
        self.assertNotIn(b"\x1b", result.stdout)

    def terminal(self, *, terminal_stdout, env=None, extra=(), expected=0, columns=80):
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, columns, 0, 0))
        try:
            process = subprocess.Popen(self.arguments("--summary", *extra), env=env or self.env,
                stdin=subprocess.DEVNULL, stdout=slave if terminal_stdout else subprocess.PIPE,
                stderr=subprocess.PIPE if terminal_stdout else slave)
            os.close(slave)
            slave = None
            chunks = []
            def drain():
                while True:
                    try:
                        chunk = os.read(master, 65536)
                    except OSError as error:
                        if error.errno == errno.EIO:
                            break
                        raise
                    if not chunk:
                        break
                    chunks.append(chunk)
            reader = threading.Thread(target=drain)
            reader.start()
            try:
                stdout, stderr = process.communicate(timeout=30)
            except BaseException:
                process.kill()
                process.communicate()
                raise
            finally:
                reader.join(timeout=5)
            self.assertFalse(reader.is_alive(), "terminal reader did not finish")
            self.assertEqual(process.returncode, expected, stderr)
            return b"".join(chunks), stderr if terminal_stdout else stdout
        finally:
            os.close(master)
            if slave is not None:
                os.close(slave)

    def test_auto_style_follows_each_stream_terminal_independently(self):
        terminal, pipe = self.terminal(terminal_stdout=True)
        self.assertIn(b"\x1b[", terminal)
        self.assertIn("📋".encode(), terminal)
        self.assertNotIn(b"\x1b", pipe)
        terminal, pipe = self.terminal(terminal_stdout=False)
        self.assertIn(b"\r\x1b[2K", terminal)
        self.assertLessEqual(terminal.count(b"\n"), 1, "live progress should reuse one line")
        self.assertTrue(terminal.endswith(b"\r\x1b[2K"), "progress cleared before recap")
        self.assertNotIn(b"\x1b", pipe)

    def test_live_progress_fits_a_narrow_terminal_without_wrapping(self):
        for columns in (2, 4, 32, 80, 0):
            with self.subTest(columns=columns):
                terminal, _ = self.terminal(terminal_stdout=False, columns=columns, expected=2,
                    extra=("--worktree", str(self.fixture.root / "not-registered")))
                frames = [part for part in terminal.decode().split("\r\x1b[2K")
                          if part.startswith("\x1b[36m")]
                if 0 < columns < 4:
                    self.assertFalse(frames, "tiny terminals must not wrap even the icon")
                    continue
                self.assertTrue(frames)
                for frame in frames:
                    visible = re.sub(r"\x1b\[[0-9;]*m", "", frame)
                    # Labels are ASCII with one two-cell magnifying-glass icon.
                    self.assertLessEqual(len(visible.replace("🔎", "xx")), (columns or 80) - 1, visible)

    def test_progress_is_cleared_before_an_error_diagnostic(self):
        terminal, stdout = self.terminal(terminal_stdout=False, expected=2,
            extra=("--worktree", str(self.fixture.root / "not-registered")))
        self.assertFalse(stdout)
        self.assertIn(b"\r\x1b[2Kwt-sweep:", terminal)
        self.assertTrue(terminal.endswith(b"\n"))

    def test_plain_mode_no_color_and_dumb_terminal_disable_decoration(self):
        for env, extra in ((self.env, ("--color", "never")),
                           (dict(self.env, NO_COLOR="1"), ()),
                           (dict(self.env, TERM="dumb"), ())):
            with self.subTest(extra=extra, term=env.get("TERM"), no_color=env.get("NO_COLOR")):
                terminal, _ = self.terminal(terminal_stdout=True, env=env, extra=extra)
                self.assertNotIn(b"\x1b", terminal)
                self.assertNotIn("📋".encode(), terminal)

    def test_invalid_style_refuses_before_reading_repository(self):
        result = subprocess.run([str(BINARY), "review", "--repo", "/must/not/read",
                                 "--color", "rainbow"], env=self.env, capture_output=True, timeout=5)
        self.assertEqual(result.returncode, 2)
        self.assertIn(b"--color must be auto, always or never", result.stderr)
        self.assertNotIn(b"Discovering", result.stderr)
        self.assertFalse(result.stdout)


if __name__ == "__main__":
    unittest.main()
