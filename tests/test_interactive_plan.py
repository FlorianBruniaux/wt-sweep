"""Guided plan on real Git families, using a terminal and independent oracles."""

from collections import deque
import errno
import json
import os
from pathlib import Path
import pty
import selectors
import signal
import subprocess
import termios
import tempfile
import time
import unittest

from inventory_support import BINARY, decode_path
from test_targets import TargetsFixture


class InteractivePlanTests(unittest.TestCase):
    def setUp(self):
        self.f = TargetsFixture()
        self.addCleanup(self.f.close)
        result = subprocess.run(
            [BINARY, "scan", "--root", self.f.root, "--disk-usage", "--format", "json"],
            env=self.f.env, capture_output=True, timeout=30, check=True)
        self.inventory = json.loads(result.stdout)
        self.path = self.f.save(self.inventory)
        output = tempfile.TemporaryDirectory(prefix="wt-sweep-guided-output-", dir="/private/tmp")
        self.addCleanup(output.cleanup)
        self.output = Path(output.name)

    def guided(self, answers, *extra):
        before = self.f.manifest()
        master, slave = pty.openpty()
        attributes = termios.tcgetattr(slave)
        attributes[3] &= ~termios.ECHO
        termios.tcsetattr(slave, termios.TCSANOW, attributes)
        process = subprocess.Popen(
            [BINARY, "plan", "--inventory", self.path, "--interactive", "--no-context",
             "--format", "json", *extra], env=self.f.env,
            stdin=slave, stderr=slave, stdout=subprocess.PIPE)
        os.close(slave)
        todo = deque(answers)
        transcript = bytearray()
        stdout = bytearray()
        pending = bytearray()
        selector = selectors.DefaultSelector()
        selector.register(master, selectors.EVENT_READ, "terminal")
        selector.register(process.stdout, selectors.EVENT_READ, "stdout")
        deadline = time.monotonic() + 90
        try:
            while selector.get_map():
                self.assertLess(time.monotonic(), deadline, bytes(transcript[-4000:]))
                for key, _ in selector.select(0.1):
                    try:
                        chunk = os.read(key.fd, 65536)
                    except OSError as error:
                        if error.errno != errno.EIO:
                            raise
                        chunk = b""
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    if key.data == "stdout":
                        stdout.extend(chunk)
                    else:
                        transcript.extend(chunk)
                        pending.extend(chunk)
                        if todo and todo[0][0] in pending:
                            _, answer = todo.popleft()
                            pending.clear()
                            if callable(answer):
                                if answer(process, master):
                                    # An explicit test-owned mutation while the
                                    # user is deciding establishes a new oracle.
                                    before = self.f.manifest()
                            else:
                                os.write(master, answer)
            status = process.wait(timeout=5)
            self.assertFalse(todo, bytes(transcript[-4000:]))
            return status, bytes(stdout), bytes(transcript)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            selector.close()
            process.stdout.close()
            os.close(master)
            self.assertEqual(before, self.f.manifest(), "guided review mutated inspected roots or inventory")

    def test_confirmed_family_targets_drive_real_verdicts_and_preserve_disk(self):
        status, stdout, transcript = self.guided([
            (b"Targets> ", b"refs/heads/main\n"),
            (b"Targets> ", b"refs/heads/develop\n"),
            (b"Type yes to confirm> ", b"yes\n"),
        ], "--run-dir", self.output, "--html", "--group-by", "repo")
        self.assertEqual(status, 0, transcript.decode(errors="replace"))
        source = json.loads(stdout)["source"]
        items = {decode_path(row["path"]): row for row in source["items"]}
        for path in (self.f.linked, self.f.other_linked):
            self.assertEqual(items[os.fsencode(path)]["classification"], "cleanup_candidate")
        self.assertEqual(source["disk_usage"], self.inventory["disk_usage"])
        self.assertIn(b"refs/heads/main", transcript)
        self.assertIn(b"refs/heads/develop", transcript)
        runs = list(self.output.glob("run-*"))
        self.assertEqual(len(runs), 1)
        self.assertTrue((runs[0] / "report.html").is_file())
        self.assertEqual(json.loads((runs[0] / "inventory.json").read_bytes()), self.inventory)

    def test_blank_invalid_and_skip_never_select_main_implicitly(self):
        status, stdout, transcript = self.guided([
            (b"Targets> ", b"\n"),
            (b"Targets> ", b"refs/heads/does-not-exist\n"),
            (b"Targets> ", b"skip\n"),
            (b"Targets> ", b"refs/heads/develop\n"),
            (b"Type yes to confirm> ", b"yes\n"),
        ])
        self.assertEqual(status, 0, transcript.decode(errors="replace"))
        items = {decode_path(row["path"]): row for row in json.loads(stdout)["items"]}
        self.assertEqual(items[os.fsencode(self.f.linked)]["classification"], "verify")
        self.assertEqual(items[os.fsencode(self.f.other_linked)]["classification"], "cleanup_candidate")

    def test_declining_final_confirmation_never_publishes(self):
        status, stdout, transcript = self.guided([
            (b"Targets> ", b"refs/heads/main\n"),
            (b"Targets> ", b"refs/heads/develop\n"),
            (b"Type yes to confirm> ", b"no\n"),
        ], "--run-dir", self.output, "--html")
        self.assertEqual(status, 2, transcript.decode(errors="replace"))
        self.assertEqual(stdout, b"")
        self.assertEqual(list(self.output.iterdir()), [])

    def test_eof_and_interrupt_abort_while_waiting_without_another_enter(self):
        for answer, expected in [(b"\x04", 2), (lambda process, _: process.send_signal(signal.SIGINT), 130)]:
            status, stdout, transcript = self.guided([(b"Targets> ", answer)])
            self.assertEqual(status, expected, transcript.decode(errors="replace"))
            self.assertEqual(stdout, b"")

    def test_noninteractive_input_requires_explicit_targets(self):
        result = subprocess.run(
            [BINARY, "plan", "--inventory", self.path, "--interactive"],
            input=b"yes\n", env=self.f.env, capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 2)
        self.assertIn(b"terminal", result.stderr)
        self.assertNotIn(b"unsupported option", result.stderr)
        self.assertEqual(result.stdout, b"")

    def test_numeric_choices_preserve_non_utf8_refs_and_multiple_targets(self):
        # A: develop=1, main=2, raw-ff=3, topic=4. The numeric choice is
        # necessary because the terminal renders non-UTF-8 bytes escaped.
        status, stdout, transcript = self.guided([
            (b"Targets> ", b"2 3\n"),
            (b"Targets> ", b"1\n"),
            (b"Type yes to confirm> ", b"yes\n"),
        ])
        self.assertEqual(status, 0, transcript.decode(errors="replace"))
        items = {decode_path(row["path"]): row for row in json.loads(stdout)["items"]}
        targets = {decode_path(h["target_ref"]) for h in items[os.fsencode(self.f.linked)]["history"]}
        self.assertEqual(targets, {b"refs/heads/main", b"refs/heads/raw-\xff"})
        self.assertEqual(items[os.fsencode(self.f.linked)]["classification"], "cleanup_candidate")

    def test_stale_family_identity_is_not_offered_and_can_be_skipped(self):
        self.inventory["families"][0]["identity"]["inode"] += 1
        self.path = self.f.save(self.inventory)
        status, stdout, transcript = self.guided([
            (b"Targets> ", b"skip\n"),
            (b"Targets> ", b"refs/heads/develop\n"),
            (b"Type yes to confirm> ", b"yes\n"),
        ])
        self.assertEqual(status, 0, transcript.decode(errors="replace"))
        self.assertIn(b"Cannot propose targets", transcript)
        items = {decode_path(row["path"]): row for row in json.loads(stdout)["items"]}
        self.assertNotEqual(items[os.fsencode(self.f.linked)]["classification"], "cleanup_candidate")
        self.assertEqual(items[os.fsencode(self.f.other_linked)]["classification"], "cleanup_candidate")

    def test_conflicting_explicit_target_modes_are_rejected_before_reading_map(self):
        for option, value in [("--target", "refs/heads/main"), ("--target-map", "/must/not/read.json")]:
            result = subprocess.run(
                [BINARY, "plan", "--inventory", self.path, "--interactive", option, value],
                input=b"", env=self.f.env, capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 2)
            self.assertIn(b"--interactive cannot be combined", result.stderr)
            self.assertEqual(result.stdout, b"")

    def test_target_removed_during_confirmation_refuses_publication(self):
        def remove_target_and_confirm(process, terminal):
            self.f.git(self.f.other, "update-ref", "-d", "refs/heads/develop")
            os.write(terminal, b"yes\n")
            return True

        status, stdout, transcript = self.guided([
            (b"Targets> ", b"refs/heads/main\n"),
            (b"Targets> ", b"refs/heads/develop\n"),
            (b"Type yes to confirm> ", remove_target_and_confirm),
        ], "--run-dir", self.output, "--html")
        self.assertEqual(status, 2, transcript.decode(errors="replace"))
        self.assertIn(b"disappeared", transcript)
        self.assertEqual(stdout, b"")
        self.assertEqual(list(self.output.iterdir()), [])

    def test_failed_git_listing_is_visible_and_does_not_poison_other_family(self):
        (self.f.repo / ".git/packed-refs").write_bytes(b"invalid packed refs\n")
        status, stdout, transcript = self.guided([
            (b"Targets> ", b"skip\n"),
            (b"Targets> ", b"refs/heads/develop\n"),
            (b"Type yes to confirm> ", b"yes\n"),
        ])
        self.assertEqual(status, 0, transcript.decode(errors="replace"))
        self.assertIn(b"Cannot propose targets", transcript)
        items = {decode_path(row["path"]): row for row in json.loads(stdout)["items"]}
        self.assertNotEqual(items[os.fsencode(self.f.linked)]["classification"], "cleanup_candidate")
        self.assertEqual(items[os.fsencode(self.f.other_linked)]["classification"], "cleanup_candidate")

    def test_later_page_and_remote_tracking_ref_remain_selectable(self):
        for number in range(22):
            self.f.git(self.f.repo, "update-ref", f"refs/heads/extra-{number:02}", self.f.base)
        self.f.git(self.f.repo, "update-ref", "refs/remotes/origin/main", self.f.base)
        self.f.git(self.f.repo, "symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/main")
        status, stdout, transcript = self.guided([
            (b"Targets> ", b"more\n"),
            (b"Targets> ", b"refs/remotes/origin/main\n"),
            (b"Targets> ", b"refs/heads/develop\n"),
            (b"Type yes to confirm> ", b"yes\n"),
        ])
        self.assertEqual(status, 0, transcript.decode(errors="replace"))
        self.assertIn(b"extra-21", transcript)
        self.assertNotIn(b"refs/remotes/origin/HEAD", transcript)
        items = {decode_path(row["path"]): row for row in json.loads(stdout)["items"]}
        targets = {decode_path(h["target_ref"]) for h in items[os.fsencode(self.f.linked)]["history"]}
        self.assertEqual(targets, {b"refs/remotes/origin/main"})
        self.assertEqual(items[os.fsencode(self.f.linked)]["classification"], "cleanup_candidate")


if __name__ == "__main__":
    unittest.main()
