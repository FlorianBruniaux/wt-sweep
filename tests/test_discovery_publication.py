"""Independent T08 CLI oracles using only owned real Git fixtures.

WT_SWEEP_DISCOVERY_BINARY may point at a frozen binary. No product implementation
is imported and no shared helper is mutated.
"""

import hashlib
import json
import os
from pathlib import Path
import selectors
import signal
import subprocess
import tempfile
import time
import unittest

from inventory_support import BINARY as DEFAULT_BINARY, GIT, clean_environment, decode_path, filesystem_manifest

BINARY = Path(os.environ.get("WT_SWEEP_DISCOVERY_BINARY", str(DEFAULT_BINARY)))


class DiscoveryFixture:
    def __init__(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="wt-sweep-t08-oracle-", dir="/private/tmp")
        self.root = Path(self.temporary.name)
        self.env = clean_environment(self.root)
        self.env.update(GIT_CONFIG_SYSTEM=os.devnull, GIT_OPTIONAL_LOCKS="0",
                        GIT_NO_LAZY_FETCH="1", GIT_ALLOW_PROTOCOL="", GIT_ATTR_NOSYSTEM="1")
        self.scope = self.root / "scope"
        self.scope.mkdir()
        self.outer = self.scope / "ordinary"
        self.nested = self.outer / "nested"
        self.hidden = self.scope / ".hidden/repo"
        self.ignored = self.outer / "ignored/cache"
        self.separate = self.scope / "separate"
        self.admin = self.root / "external-admin"
        self.bare = self.scope / "arbitrary-name"
        self.linked = self.root / "outside linked"
        try:
            for repo in (self.outer, self.nested, self.hidden, self.ignored):
                self.repo(repo)
            (self.outer / ".gitignore").write_bytes(b"ignored/\n")
            self.git("init", "--bare", self.bare)
            self.repo(self.separate, "--separate-git-dir", self.admin)
            self.git("-C", self.outer, "worktree", "add", "-b", "linked", self.linked)
            self.expected_families = {os.fsencode(p / ".git") for p in
                                      (self.outer, self.nested, self.hidden, self.ignored)}
            self.expected_families.update(map(os.fsencode, (self.bare, self.admin)))
            self.expected_worktrees = set(map(os.fsencode, (self.outer, self.nested, self.hidden,
                                                          self.ignored, self.separate, self.bare, self.linked)))
            self.outside_sentinel = self.root / "outside-sentinel"
            self.outside_sentinel.write_bytes(b"preserve synthetic bytes\x00\xff\n")
        except BaseException:
            self.close()
            raise

    def git(self, *arguments):
        argv = [GIT, "-c", "core.hooksPath=/dev/null", "-c", "gc.auto=0",
                "-c", "maintenance.auto=false", *map(str, arguments)]
        process = subprocess.Popen(argv, env=self.env, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
        try:
            stdout, stderr = process.communicate(timeout=15)
        except BaseException:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.communicate(timeout=5)
            raise
        if process.returncode != 0:
            raise AssertionError((argv, process.returncode, stderr.decode(errors="replace")))
        return stdout

    def repo(self, path, *extra):
        path.mkdir(parents=True)
        self.git("init", "-b", "main", *extra, path)
        (path / "tracked.txt").write_bytes(b"synthetic fixture\n")
        self.git("-C", path, "add", "tracked.txt")
        self.git("-C", path, "commit", "-m", "Synthetic fixture")

    def close(self):
        self.temporary.cleanup()


class DiscoveryPublicationTests(unittest.TestCase):
    def setUp(self):
        self.fixture = DiscoveryFixture()
        self.addCleanup(self.fixture.close)
        output = tempfile.TemporaryDirectory(prefix="wt-sweep-t08-output-", dir="/private/tmp")
        self.addCleanup(output.cleanup)
        self.output = Path(output.name)
        self.owned_children = set()

    def stop(self, process):
        # Product Git children have isolated groups. Snapshot only children of
        # the owned live CLI even if the progress handshake itself failed.
        if process.poll() is None:
            snapshot = subprocess.run(["/usr/bin/pgrep", "-P", str(process.pid)],
                                      env=self.fixture.env, stdin=subprocess.DEVNULL,
                                      capture_output=True, timeout=2)
            if snapshot.returncode == 0:
                self.owned_children.update(int(pid) for pid in snapshot.stdout.splitlines())
        for pid in self.owned_children:
            try:
                os.killpg(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        if self.owned_children and process.poll() is None:
            try:
                process.communicate(timeout=2)  # Allow parent to reap children first.
            except subprocess.TimeoutExpired:
                pass
        if process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        process.communicate(timeout=5)

    def start(self, *arguments, cwd=None):
        return subprocess.Popen([str(BINARY), *map(str, arguments)], env=self.fixture.env,
                                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, start_new_session=True, cwd=cwd)

    def run_cli(self, *arguments, cwd=None):
        before = filesystem_manifest(self.fixture.root)
        process = self.start(*arguments, cwd=cwd)
        try:
            stdout, stderr = process.communicate(timeout=35)
        except BaseException:
            self.stop(process)
            raise
        finally:
            self.assertEqual(before, filesystem_manifest(self.fixture.root),
                             "CLI altered source bytes, metadata, links or Git administration (atime excluded)")
        return subprocess.CompletedProcess(process.args, process.returncode, stdout, stderr)

    def json_success(self, result):
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        return json.loads(result.stdout)

    def assert_family_set(self, inventory):
        actual = [decode_path(f["common_dir"]) for f in inventory["families"]]
        self.assertEqual(set(actual), self.fixture.expected_families,
                         "root discovery omitted hidden/ignored/nested/bare/separate family or invented one")
        self.assertEqual(len(actual), len(self.fixture.expected_families), "duplicate physical family")
        trees = {decode_path(w["path"]) for f in inventory["families"] for w in f["worktrees"]}
        self.assertEqual(trees, self.fixture.expected_worktrees,
                         "family scope lost registered outside-root worktree or invented a registration")

    def assert_receipt(self, directory, partial):
        self.assertEqual({p.name for p in directory.iterdir()}, {"inventory.json", "report.txt", "run.json"})
        receipt = json.loads((directory / "run.json").read_bytes())
        self.assertEqual(receipt["schema"], "wt-sweep.run/1")
        self.assertEqual(receipt["command"], "scan")
        self.assertEqual(receipt["status"], "partial" if partial else "complete")
        self.assertEqual(receipt["publication"]["completion"], "requires_successful_publisher_return")
        self.assertEqual(receipt["publication"]["durability"], "unknown_from_receipt_alone")
        self.assertEqual(receipt["publication"]["meaning"], "artifacts_synced_before_receipt")
        self.assertEqual(receipt["run_id"], directory.name)
        self.assertLessEqual(receipt["started_unix_ms"], receipt["finished_unix_ms"])
        records = receipt["artifacts"]
        self.assertEqual(len(records), 2)
        self.assertEqual({r["name"] for r in records}, {"inventory.json", "report.txt"})
        for record in records:
            payload = (directory / record["name"]).read_bytes()
            self.assertEqual(record["sha256"], hashlib.sha256(payload).hexdigest())
            self.assertEqual(record["bytes"], len(payload))
        self.assertTrue((directory / "report.txt").read_bytes().strip())
        return json.loads((directory / "inventory.json").read_bytes())

    def test_exact_roots_include_hidden_ignored_nested_bare_separate_and_outside_registration(self):
        for workers in (1, 4):
            with self.subTest(workers=workers):
                inventory = self.json_success(self.run_cli("scan", "--root", self.fixture.scope,
                                                          "--workers", workers, "--format", "json"))
                self.assert_family_set(inventory)
                self.assertTrue(inventory["coverage"]["traversal_complete"], inventory["coverage"]["gaps"])
                self.assertEqual([decode_path(p) for p in inventory["coverage"]["roots"]],
                                 [os.fsencode(self.fixture.scope)])
                outside = [m for m in inventory["markers"] if decode_path(m["path"]) == os.fsencode(self.fixture.linked)]
                self.assertTrue(outside)
                self.assertTrue(any(m["origin"] == "git_registry_outside_roots" for m in outside))

    def test_progress_keeps_stdout_json_and_emits_start_progress_terminal(self):
        result = self.run_cli("scan", "--root", self.fixture.scope, "--format", "json")
        self.assert_family_set(self.json_success(result))
        progress = result.stderr.decode(errors="replace")
        for phase in ("start", "progress", "finished"):
            self.assertIn("discovery: phase=" + phase, progress,
                          "discovery stderr omitted an observable lifecycle phase")
        self.assertIn("stop=none", progress, "complete walk lacks explicit terminal stop state")
        self.assertRegex(progress, r"families[=: ]+6", "progress does not identify known family count")
        self.assertRegex(progress, r"pending[=: ]+[0-9]+", "progress omits known pending count")

    def test_complete_publication_is_verifiable_and_keeps_prior_runs(self):
        sentinel = self.output / "existing-artifact"
        sentinel.write_bytes(b"previous output\n")
        before = filesystem_manifest(sentinel)
        first = self.json_success(self.run_cli("scan", "--root", self.fixture.scope,
                                               "--format", "json", "--run-dir", self.output))
        added = {p for p in self.output.iterdir() if p != sentinel}
        self.assertEqual(len(added), 1)
        first_directory = next(iter(added))
        self.assertEqual(first, self.assert_receipt(first_directory, partial=False))
        first_manifest = filesystem_manifest(first_directory)
        second = self.json_success(self.run_cli("scan", "--repo", self.fixture.outer,
                                                "--format", "json", "--run-dir", self.output))
        added = set(self.output.iterdir()) - {sentinel, first_directory}
        self.assertEqual(len(added), 1, "second run reused a prior output identity")
        self.assertEqual(second, self.assert_receipt(added.pop(), partial=False))
        self.assertEqual(before, filesystem_manifest(sentinel), "prior artifact overwritten")
        self.assertEqual(first_manifest, filesystem_manifest(first_directory), "prior run changed")

    def test_missing_root_broken_marker_and_alias_preserve_positive_family_and_gaps(self):
        broken = self.fixture.scope / "broken"
        broken.mkdir()
        (broken / ".git").write_bytes(b"gitdir: /synthetic/missing/admin\n")
        alias = self.fixture.scope / "alias"
        alias.symlink_to(self.fixture.outer, target_is_directory=True)
        missing = self.fixture.root / "missing requested root"
        result = self.run_cli("scan", "--root", self.fixture.scope, "--root", missing,
                              "--format", "json", "--run-dir", self.output)
        inventory = self.json_success(result)
        self.assert_family_set(inventory)
        self.assertFalse(inventory["coverage"]["traversal_complete"])
        gaps = {decode_path(g["path"]): g["reason"] for g in inventory["coverage"]["gaps"]}
        self.assertIn(os.fsencode(missing), gaps, "missing requested root silently disappeared")
        aliases = {(decode_path(a["path"]), decode_path(a["canonical"]))
                   for a in inventory["coverage"]["aliases"]}
        self.assertIn((os.fsencode(alias), os.fsencode(self.fixture.outer)), aliases,
                      "in-root alias identity disappeared from coverage")
        self.assertTrue(any(p.startswith(os.fsencode(broken)) for p in gaps), "broken marker evidence omitted")
        directories = list(self.output.iterdir())
        self.assertEqual(len(directories), 1)
        self.assertEqual(inventory, self.assert_receipt(directories[0], partial=True))

    def test_publication_refuses_inside_repository_and_symlinked_destination(self):
        link = self.output / "output alias"
        link.symlink_to(self.fixture.outer, target_is_directory=True)
        for destination in (self.fixture.outer, self.fixture.admin, link):
            with self.subTest(destination=destination):
                result = self.run_cli("scan", "--root", self.fixture.scope, "--format", "json",
                                      "--run-dir", destination)
                self.assertNotEqual(result.returncode, 0, "unsafe destination accepted")
                self.assertTrue(result.stderr.strip(), "refusal lacks a diagnostic")

    def test_existing_output_file_is_refused_without_replacing_prior_bytes(self):
        output = self.output / "existing.json"
        output.write_bytes(b"previous output bytes\n")
        before = filesystem_manifest(output)
        result = self.run_cli("scan", "--root", self.fixture.scope,
                              "--format", "json", "--out", output)
        self.assertNotEqual(result.returncode, 0, "existing output file was accepted")
        self.assertTrue(result.stderr.strip(), "no-overwrite refusal lacks diagnostic")
        self.assertEqual(before, filesystem_manifest(output), "prior bytes or metadata replaced")
        self.assertEqual({p.name for p in self.output.iterdir()}, {"existing.json"},
                         "failed existing-output publication left extra files")

    def relative_empty_inventory(self):
        scan_root = self.fixture.root / "original empty scan root"
        replay_cwd = self.fixture.root / "different replay cwd"
        scan_root.mkdir()
        replay_cwd.mkdir()
        artifact = self.output / "relative-input-inventory.json"
        result = self.run_cli("scan", "--root", ".", "--format", "json",
                              "--out", artifact, cwd=scan_root)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        inventory = json.loads(artifact.read_bytes())
        self.assertTrue(inventory["coverage"]["traversal_complete"])
        self.assertEqual(inventory["families"], [], "empty scoped fixture invented a family")
        return scan_root, replay_cwd, artifact, inventory

    def test_relative_root_replay_preserves_original_boundary_and_allows_outside_publication(self):
        scan_root, replay_cwd, artifact, inventory = self.relative_empty_inventory()
        outside = self.output / "outside-review.json"
        result = self.run_cli("plan", "--inventory", artifact, "--format", "json",
                              "--out", outside, cwd=replay_cwd)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertEqual(json.loads(outside.read_bytes())["schema"], "wt-sweep.review/2")
        before_output = filesystem_manifest(self.output)
        inside = scan_root / "forbidden-review.json"
        result = self.run_cli("plan", "--inventory", artifact, "--format", "json",
                              "--out", inside, cwd=replay_cwd)
        self.assertNotEqual(result.returncode, 0, "changed cwd redirected the saved protected root")
        self.assertFalse(inside.exists(), "plan wrote review JSON inside the original scan root")
        self.assertTrue(result.stderr.strip(), "original-boundary refusal has no diagnostic")
        self.assertEqual(before_output, filesystem_manifest(self.output), "replay changed prior output")
        self.assertEqual(list(scan_root.iterdir()), [], "original empty root changed")
        self.assertEqual([decode_path(p) for p in inventory["scope"]["paths"]], [os.fsencode(scan_root)],
                         "fresh CLI inventory retained cwd-dependent root identity")
        self.assertEqual([decode_path(p) for p in inventory["coverage"]["roots"]], [os.fsencode(scan_root)])

    def test_legacy_relative_inventory_refuses_publication_with_rescan_advice_but_stdout_is_valid(self):
        scan_root, replay_cwd, _, inventory = self.relative_empty_inventory()
        inventory["scope"]["paths"] = [{"hex": os.fsencode(".").hex()}]
        inventory["coverage"]["roots"] = [{"hex": os.fsencode(".").hex()}]
        legacy = self.output / "legacy-relative.json"
        legacy.write_text(json.dumps(inventory))
        result = self.run_cli("plan", "--inventory", legacy, "--format", "json", cwd=replay_cwd)
        self.assertEqual(self.json_success(result)["schema"], "wt-sweep.review/2")
        output = self.output / "legacy-forbidden-review.json"
        before = filesystem_manifest(self.output)
        result = self.run_cli("plan", "--inventory", legacy, "--format", "json",
                              "--out", output, cwd=replay_cwd)
        self.assertNotEqual(result.returncode, 0, "ambiguous legacy relative scope authorized publication")
        self.assertIn("relative", result.stderr.decode(errors="replace").lower())
        self.assertIn("rescan", result.stderr.decode(errors="replace").lower(),
                      "legacy relative-scope refusal omits actionable rescan advice")
        self.assertFalse(output.exists(), "relative legacy inventory published filesystem artifact")
        self.assertEqual(before, filesystem_manifest(self.output), "refusal modified existing output")
        self.assertEqual(list(scan_root.iterdir()), [])

    def wait_for_stderr(self, process, token, timeout=5):
        observed = bytearray()
        deadline = time.monotonic() + timeout
        with selectors.DefaultSelector() as selector:
            selector.register(process.stderr, selectors.EVENT_READ)
            while token not in observed and time.monotonic() < deadline:
                for key, _ in selector.select(max(0, deadline - time.monotonic())):
                    chunk = os.read(key.fd, 8192)
                    if not chunk:
                        self.fail(f"CLI exited before progress handshake: {bytes(observed)!r}")
                    observed.extend(chunk)
        self.assertIn(token, observed, "no discovery progress handshake before bounded deadline")
        return bytes(observed)

    def wait_for_git_child(self, process, timeout=4):
        deadline = time.monotonic() + timeout
        while process.poll() is None and time.monotonic() < deadline:
            result = subprocess.run(["/usr/bin/pgrep", "-P", str(process.pid)],
                                    env=self.fixture.env, stdin=subprocess.DEVNULL,
                                    capture_output=True, timeout=2)
            self.assertIn(result.returncode, (0, 1), result.stderr)
            for raw_pid in result.stdout.splitlines():
                pid = int(raw_pid)
                child = subprocess.run(["/bin/ps", "-p", str(pid), "-o", "ppid=,pgid=,comm="],
                                       env=self.fixture.env, stdin=subprocess.DEVNULL,
                                       capture_output=True, timeout=2)
                if child.returncode != 0:  # Child exited between the two identity reads.
                    continue
                columns = child.stdout.split(None, 2)
                if len(columns) == 3 and int(columns[0]) == process.pid and columns[2].strip().endswith(b"git"):
                    group = int(columns[1])
                    self.assertEqual(pid, group, "owned Git child does not have its expected isolated group")
                    self.owned_children.add(pid)
                    return pid
            time.sleep(0.02)  # Poll an actual process event; this delay never chooses signal timing.
        self.fail("blocked synthetic Git child not observed; this is fixture/setup failure, not product RED")

    def assert_children_gone(self):
        for pid in tuple(self.owned_children):
            try:
                os.kill(pid, 0)
            except ProcessLookupError:
                self.owned_children.remove(pid)
                continue
            self.fail(f"observed owned Git child {pid} survived or was not reaped")

    def process_selector_preflight(self):
        # Refuse setup before launching a blocking FIFO CLI if the host cannot
        # identify this helper's owned children. No process command lines read.
        result = subprocess.run(["/usr/bin/pgrep", "-P", str(os.getpid())],
                                env=self.fixture.env, stdin=subprocess.DEVNULL,
                                capture_output=True, timeout=2)
        self.assertIn(result.returncode, (0, 1),
                      "owned process selector unavailable; no FIFO CLI launched: " +
                      result.stderr.decode(errors="replace"))
        result = subprocess.run(["/bin/ps", "-p", str(os.getpid()), "-o", "pid=,pgid="],
                                env=self.fixture.env, stdin=subprocess.DEVNULL,
                                capture_output=True, timeout=2)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertEqual(int(result.stdout.split()[0]), os.getpid(), "process selector returned wrong PID")

    def interrupted(self, arguments, grouped=False, git_child=False):
        if git_child:
            self.process_selector_preflight()
        before = filesystem_manifest(self.fixture.root)
        process = self.start(*arguments)
        try:
            progress = self.wait_for_stderr(process, b"discovery: phase=start")
            if git_child:
                self.wait_for_git_child(process)
            if grouped:
                os.killpg(process.pid, signal.SIGINT)
            else:
                os.kill(process.pid, signal.SIGINT)
            stdout, stderr = process.communicate(timeout=12)
            self.assert_children_gone()
            return subprocess.CompletedProcess(process.args, process.returncode, stdout, progress + stderr)
        finally:
            self.stop(process)
            self.assertEqual(before, filesystem_manifest(self.fixture.root),
                             "interruption modified the source fixture")

    def assert_interrupted_json(self, result):
        self.assertEqual(result.returncode, 130, result.stderr.decode(errors="replace"))
        inventory = json.loads(result.stdout)
        self.assertFalse(inventory["coverage"]["traversal_complete"])
        self.assertTrue(inventory["coverage"]["gaps"], "interruption omitted missing evidence")
        remaining = inventory["coverage"]["remaining"]
        self.assertEqual(remaining["reason"], "interrupted")
        self.assertEqual(remaining["cover"], "requested_scope",
                         "remaining scope must conservatively cover every original requested root")
        self.assertIsInstance(remaining["known_pending_paths"], int)
        self.assertGreater(remaining["known_pending_paths"], 0,
                           "current operation/pending root falsely counted complete")
        self.assertIn(b"discovery: phase=finished", result.stderr)
        self.assertIn(b"stop=interrupted", result.stderr)
        return inventory

    def fifo_repo(self, name):
        repo = self.fixture.root / name
        self.fixture.repo(repo)
        config = repo / ".git/config"
        config.unlink()
        os.mkfifo(config)
        return repo

    def test_sigint_direct_pid_and_owned_group_reap_real_git_child_and_keep_partial_stdout(self):
        # A real Git child blocks in an owned FIFO. Signal follows observed
        # stderr plus child identity, never guessed fixture duration.
        for grouped in (False, True):
            with self.subTest(grouped=grouped):
                repo = self.fifo_repo("interrupt-group" if grouped else "interrupt-pid")
                result = self.interrupted(("scan", "--repo", repo, "--format", "json"),
                                          grouped=grouped, git_child=True)
                inventory = self.assert_interrupted_json(result)
                self.assertEqual(inventory["scope"]["mode"], "repo")
                self.assertEqual(decode_path(inventory["scope"]["path"]), os.fsencode(repo))

    def test_interrupted_unreadable_repo_refuses_filesystem_publication_without_receipt(self):
        repo = self.fifo_repo("unsafe interrupted repo")
        result = self.interrupted(("scan", "--repo", repo, "--format", "json",
                                   "--run-dir", self.output), git_child=True)
        self.assertNotEqual(result.returncode, 0, "unknown repository boundary permitted publication")
        self.assertTrue(result.stderr.strip(), "unsafe publication refusal lacks diagnostic")
        self.assertEqual(list(self.output.iterdir()), [],
                         "unreadable repository published files or a success receipt")

    def test_sigint_roots_publish_partial_json_receipt_and_conservative_remaining_scope(self):
        # Root authorizes this large, bounded synthetic traversal. Signal is sent
        # immediately on the flushed before-I/O start event, with no guessed
        # exact visited count. FIFO test above separately proves child reaping.
        for grouped in (False, True):
            with self.subTest(grouped=grouped):
                scope = self.fixture.root / ("wide-group" if grouped else "wide-pid")
                scope.mkdir()
                for shard in range(32):
                    parent = scope / f"shard-{shard:02d}"
                    parent.mkdir()
                    for entry in range(64):
                        (parent / f"directory-{entry:02d}").mkdir()
                second_root = self.fixture.root / ("pending-group" if grouped else "pending-pid")
                second_root.mkdir()
                output = self.output / ("group" if grouped else "pid")
                output.mkdir()
                result = self.interrupted(("scan", "--root", scope, "--root", second_root,
                                           "--format", "json", "--run-dir", output), grouped=grouped)
                inventory = self.assert_interrupted_json(result)
                self.assertEqual(set(map(decode_path, inventory["coverage"]["roots"])),
                                 set(map(os.fsencode, (scope, second_root))))
                directories = list(output.iterdir())
                self.assertEqual(len(directories), 1, "interrupt did not publish one partial run")
                self.assertEqual(inventory, self.assert_receipt(directories[0], partial=True))


if __name__ == "__main__":
    unittest.main()
