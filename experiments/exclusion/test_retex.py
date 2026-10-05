"""Counterexamples to session cleanup assumptions, using only owned fixtures.

These tests exercise Git and the copied scripts' recipes, not product cleanup.
No personal inventory, credentials, backup or real worktree is read.
"""

import hashlib
import multiprocessing
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from experiment import child, git, receive


class SessionAssumptions(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="wt-sweep-retex-", dir="/private/tmp")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        for name in ("outside", "home", "empty", "target", "repo"):
            (self.root / name).mkdir()
        self.repo = self.root / "repo"
        git(self.root, "init", "--template=" + str(self.root / "empty"), "-b", "main")
        (self.repo / "tracked").write_bytes(b"base\n")
        (self.repo / ".gitignore").write_bytes(b"ignored/\n")
        git(self.root, "add", "--", "tracked", ".gitignore")
        git(self.root, "commit", "-m", "Synthetic base")
        self.tree = self.root / "linked"
        git(self.root, "worktree", "add", "-b", "topic", str(self.tree))

    def tree_git(self, *args, raw=False):
        return git(self.root, "-C", str(self.tree), *args, raw=raw)

    def session_fingerprint(self):
        # Exact successful Git output bytes hashed by wt-plan.sh / wt-apply.sh.
        head = self.tree_git("rev-parse", "HEAD", raw=True)
        status = self.tree_git("status", "--porcelain=v1", "--untracked-files=all",
                               "--ignored=matching", raw=True)
        return hashlib.sha1(head + status).hexdigest()[:16]

    def test_unforced_removal_deletes_ignored_bytes_but_preserves_named_branch(self):
        ignored = self.tree / "ignored"
        ignored.mkdir()
        payload = ignored / "unique-fixture-data"
        payload.write_bytes(b"synthetic ignored content, not a secret\n")
        head = self.tree_git("rev-parse", "HEAD")
        sentinel = self.root / "outside" / "sentinel"
        sentinel.write_bytes(b"keep outside\n")
        self.assertEqual(self.tree_git("status", "--porcelain"), "")
        self.assertIn("!! ignored/", self.tree_git("status", "--porcelain", "--ignored=matching"))
        git(self.root, "worktree", "remove", str(self.tree))
        self.assertFalse(self.tree.exists())
        self.assertFalse(payload.exists(), "Git behavior changed: revisit H1")
        self.assertEqual(git(self.root, "rev-parse", "refs/heads/topic"), head)
        self.assertEqual(sentinel.read_bytes(), b"keep outside\n")

    def test_unforced_removal_refuses_an_untracked_positive_control(self):
        payload = self.tree / "untracked"
        payload.write_bytes(b"unique untracked fixture\n")
        with self.assertRaises(subprocess.CalledProcessError) as failure:
            git(self.root, "worktree", "remove", str(self.tree))
        self.assertNotEqual(failure.exception.returncode, 0)
        self.assertIn(b"untracked", failure.exception.stderr)
        self.assertEqual(payload.read_bytes(), b"unique untracked fixture\n")
        self.assertIn(str(self.tree), git(self.root, "worktree", "list", "--porcelain"))

    def test_status_fingerprint_misses_changed_tracked_untracked_and_ignored_bytes(self):
        (self.tree / "ignored").mkdir()
        paths = (self.tree / "tracked", self.tree / "untracked", self.tree / "ignored" / "payload")
        for path in paths:
            path.write_bytes(b"first version\n")
        before = self.session_fingerprint()
        for path in paths:
            with self.subTest(kind=path.name):
                digest = hashlib.sha256(path.read_bytes()).hexdigest()
                path.write_bytes(b"other version\n")
                self.assertNotEqual(hashlib.sha256(path.read_bytes()).hexdigest(), digest)
                self.assertEqual(self.session_fingerprint(), before,
                                 "Session recipe changed: revisit content-blind fingerprint")

    def test_patch_from_head_restores_disk_bytes_but_loses_distinct_index_version(self):
        tracked = self.tree / "tracked"
        tracked.write_bytes(b"staged version\n")
        self.tree_git("add", "--", "tracked")
        staged = self.tree_git("show", ":tracked")
        tracked.write_bytes(b"working version\n")
        patch = self.root / "saved.patch"
        patch.write_bytes(self.tree_git("diff", "HEAD", "--binary", raw=True))
        restore = self.root / "restored"
        git(self.root, "worktree", "add", "--detach", str(restore), "HEAD")
        git(self.root, "-C", str(restore), "apply", str(patch))
        self.assertEqual((restore / "tracked").read_bytes(), tracked.read_bytes())
        restored_index = git(self.root, "-C", str(restore), "show", ":tracked")
        self.assertEqual(restored_index, "base")
        self.assertNotEqual(restored_index, staged, "Patch unexpectedly preserved the index")

    def test_real_lsof_cwd_view_misses_a_proven_open_writer_outside_target(self):
        lsof = Path("/usr/sbin/lsof")
        if not lsof.exists():
            self.skipTest("native lsof unavailable; H4 native observation UNKNOWN")
        data = self.root / "target" / "data"
        data.write_bytes(b"before")
        with child(self.root, "fd") as (channel, cwd):
            # Select only the owned child, never enumerate other users' processes.
            # The existing handshake provides cwd; /proc or ps is not used.
            channel_pid = None
            owned = multiprocessing.active_children()
            if len(owned) == 1:
                channel_pid = owned[0].pid
            self.assertIsNotNone(channel_pid, "cannot unambiguously identify owned writer")
            def observe(descriptors):
                result = subprocess.run(
                    [str(lsof), "-nP", "-a", "-p", str(channel_pid), "-d", descriptors, "-Fn"],
                    env={"PATH": "/usr/bin:/bin", "LC_ALL": "C", "HOME": str(self.root)},
                    stdin=subprocess.DEVNULL, capture_output=True, timeout=10,
                )
                if result.returncode != 0:
                    self.skipTest("owned-process lsof refused; H4 native observation UNKNOWN")
                self.assertLessEqual(len(result.stdout) + len(result.stderr), 65536)
                return {os.fsdecode(line[1:]) for line in result.stdout.splitlines() if line.startswith(b"n")}
            cwd_paths = observe("cwd")
            self.assertIn(str(Path(cwd)), cwd_paths, "empty lsof output is not positive visibility evidence")
            self.assertNotIn(str(self.root / "target"), cwd_paths)
            all_paths = observe("0-1024")
            self.assertIn(str(data), all_paths, "owned writer's file descriptor must be observable")
            channel.send("GO")
            self.assertEqual(receive(channel)[0], "DONE")
            self.assertEqual(data.read_bytes(), b"after!")


if __name__ == "__main__":
    unittest.main()
