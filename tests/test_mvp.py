"""Independent MVP residency/concurrency oracles using owned real repositories.

Special-file fixtures exercise bounded refusal, not real TCC or cloud hydration.
"""

import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
import unittest

from inventory_support import BINARY, HistoryFixture, decode_path, filesystem_manifest


class MvpTests(unittest.TestCase):
    def setUp(self):
        self.fixture = HistoryFixture()
        self.addCleanup(self.fixture.close)

    def reap_owned_process(self, process, grace=12):
        """Drain/reap only the Popen and session actually created by this helper."""
        try:
            process.communicate(timeout=grace)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.kill()
            process.communicate(timeout=5)

    def run_cli(self, *arguments, timeout=35):
        """Bound execution and always drain/reap the directly owned CLI process."""
        before = filesystem_manifest(self.fixture.root)
        process = subprocess.Popen([str(BINARY), *map(str, arguments)],
                                   env=self.fixture.env, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True)
        try:
            stdout, stderr = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            self.reap_owned_process(process)
            self.fail(f"CLI exceeded fixture process deadline ({timeout}s); owned CLI reaped")
        finally:
            if process.poll() is None or (process.stdout and not process.stdout.closed):
                self.reap_owned_process(process)
            self.assertEqual(before, filesystem_manifest(self.fixture.root),
                             "CLI changed fixture bytes, metadata or topology (atime excluded)")
        return subprocess.CompletedProcess(process.args, process.returncode, stdout, stderr)

    def test_helper_timeout_reaps_owned_process_without_process_table(self):
        process = subprocess.Popen([sys.executable, "-B", "-c", "import time; time.sleep(30)"],
                                   env=self.fixture.env, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True)
        try:
            self.reap_owned_process(process, grace=0.01)
            self.assertIsNotNone(process.returncode)
            self.assertTrue(process.stdout.closed)
            self.assertTrue(process.stderr.closed)
        finally:
            if process.poll() is None or not process.stdout.closed:
                self.reap_owned_process(process, grace=0.01)

    def test_fixture_pack_does_not_wait_for_callers_stdin(self):
        script = """
from inventory_support import HistoryFixture

fixture = HistoryFixture()
try:
    prefix = fixture.principal / ".git/objects/pack/pack"
    result = fixture.git("-C", str(fixture.principal), "pack-objects", "--all", str(prefix))
    oid = result.stdout.decode().strip()
    fixture.git("verify-pack", str(prefix.parent / f"pack-{oid}.idx"))
    print(oid)
finally:
    fixture.close()
"""
        process = subprocess.Popen([sys.executable, "-B", "-c", script],
                                   cwd=Path(__file__).resolve().parent, env=self.fixture.env,
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True)
        try:
            # Keep stdin open and empty until the child exits. communicate()
            # would close it first and conceal the interactive-terminal bug.
            process.wait(timeout=25)
            stdout, stderr = process.communicate(timeout=5)
        except subprocess.TimeoutExpired:
            self.reap_owned_process(process, grace=0.01)
            self.fail("fixture pack waited for caller input; owned process reaped")
        finally:
            if process.poll() is None or not process.stdout.closed:
                self.reap_owned_process(process, grace=0.01)
        self.assertEqual(process.returncode, 0, stderr.decode(errors="replace"))
        self.assertRegex(stdout, rb"^[0-9a-f]{40}\n$", "fixture did not produce a verified pack")

    def inventory(self, repo=None):
        result = self.run_cli("scan", "--repo", repo or self.fixture.principal, "--format", "json")
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        return json.loads(result.stdout)

    def save_inventory(self):
        path = self.fixture.root / "inventory.json"
        path.write_text(json.dumps(self.inventory()))
        return path

    def plan(self, inventory, target="refs/heads/main"):
        return self.run_cli("plan", "--inventory", inventory, "--target", target,
                            "--format", "json")

    def assert_explicit_unknown_or_refusal(self, result):
        if result.returncode != 0:
            self.assertTrue(result.stderr.strip(), "refusal has no diagnostic")
            return
        report = json.loads(result.stdout)
        items = report["items"]
        self.assertEqual({decode_path(item["path"]) for item in items}, set(self.fixture.expected))
        self.assertTrue(items, "blank review disguised missing evidence")
        for item in items:
            self.assertNotEqual(item["classification"], "cleanup_candidate",
                                "unsafe required evidence became cleanup candidacy")
            self.assertFalse(item["observations"], "unsafe read became integration evidence")
            self.assertTrue(item["unknowns"], "read refusal became known absence")

    def assert_clean_linked_positive(self, items, principal=None, linked=None):
        principal = principal or self.fixture.principal
        linked = linked or self.fixture.linked
        by_path = {decode_path(item["path"]): item for item in items}
        main = by_path[os.fsencode(principal)]
        self.assertEqual(main["classification"], "keep")
        self.assertIn("main_checkout", main["decision"]["keep_reasons"])
        self.assertEqual(main["observations"], [], "main checkout should skip deep work")
        candidate = by_path[os.fsencode(linked)]
        self.assertEqual(candidate["classification"], "cleanup_candidate",
                         "ordinary resident Git evidence caused blanket refusal")
        self.assertTrue(any(observation["target_role"] == "integration"
                            and observation["relation"] == "identical_commit"
                            and observation["source_oid"] == observation["target_oid"] == candidate["head"]
                            for observation in candidate["observations"]),
                        "linked positive control requires actual deep object proof")

    def test_resident_external_config_include_remains_positive(self):
        include = self.fixture.root / "resident included config"
        include.write_text("[core]\n\tignorecase = false\n")
        self.fixture.git("-C", str(self.fixture.principal), "config", "include.path", str(include))
        inventory = self.save_inventory()
        result = self.plan(inventory)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        items = json.loads(result.stdout)["items"]
        self.assertEqual({decode_path(item["path"]) for item in items}, set(self.fixture.expected))
        self.assert_clean_linked_positive(items)

    def install_fifo_include(self):
        fifo = self.fixture.root / "included config fifo"
        os.mkfifo(fifo)
        # Git config would itself read the include, so append synthetic config directly.
        with (self.fixture.principal / ".git/config").open("a") as config:
            config.write(f'\n[include]\n\tpath = "{fifo}"\n')

    def test_fifo_config_include_scan_repo_refuses_with_diagnostic(self):
        self.install_fifo_include()
        result = self.run_cli("scan", "--repo", self.fixture.principal, "--format", "json")
        self.assertNotEqual(result.returncode, 0, "FIFO include silently accepted as safe")
        self.assertTrue(result.stderr.strip())
        result = self.run_cli("scan", "--root", self.fixture.principal, "--format", "json")
        if result.returncode:
            self.assertTrue(result.stderr.strip())
        else:
            inventory = json.loads(result.stdout)
            self.assertFalse(inventory["coverage"]["traversal_complete"])
            self.assertTrue(inventory["coverage"]["gaps"], "unsafe include disappeared from discovery")

    def test_fifo_config_include_plan_has_no_integration_proof(self):
        inventory = self.save_inventory()
        self.install_fifo_include()
        self.assert_explicit_unknown_or_refusal(self.plan(inventory))

    def test_required_fifo_blob_cannot_prove_integration(self):
        inventory = self.save_inventory()
        oid = self.fixture.oid(self.fixture.principal, "HEAD:tracked.txt")
        blob = self.fixture.principal / ".git/objects" / oid[:2] / oid[2:]
        blob.unlink()
        os.mkfifo(blob)
        self.assert_explicit_unknown_or_refusal(self.plan(inventory))

    def test_target_only_fifo_blob_is_attempted_after_clean_source_controls(self):
        fixture = self.fixture
        target = fixture.write_commit(fixture.principal, "target-only.txt", b"target-only FIFO blob\n")
        blob_oid = fixture.oid(fixture.principal, "HEAD:target-only.txt")
        fixture.git("-C", str(fixture.principal), "update-ref", "refs/heads/integration", target)
        fixture.git("-C", str(fixture.principal), "reset", "--hard", fixture.base)
        inventory = self.save_inventory()
        positive = self.plan(inventory, "refs/heads/integration")
        self.assertEqual(positive.returncode, 0, positive.stderr.decode(errors="replace"))
        linked = next(i for i in json.loads(positive.stdout)["items"]
                      if decode_path(i["path"]) == os.fsencode(fixture.linked))
        self.assertEqual(linked["classification"], "cleanup_candidate")
        self.assertTrue(any(o["relation"] == "ancestor" and o["target_oid"] == target
                            for o in linked["observations"]))
        blob = fixture.principal / ".git/objects" / blob_oid[:2] / blob_oid[2:]
        self.assertTrue(blob.is_file())
        blob.unlink()
        os.mkfifo(blob)
        result = self.plan(inventory, "refs/heads/integration")
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        report = json.loads(result.stdout)
        self.assertEqual({decode_path(i["path"]) for i in report["items"]}, set(fixture.expected))
        linked = next(i for i in report["items"] if decode_path(i["path"]) == os.fsencode(fixture.linked))
        self.assertEqual(linked["classification"], "verify")
        self.assertFalse(linked["observations"], "target FIFO produced unverified deep integration")
        self.assertTrue(linked["local_summary"]["index_complete"], "deep timeout erased raw-index facts")
        self.assertEqual(linked["local_summary"]["staged"]["observed"], 0)
        self.assertRegex(" ".join(linked["unknowns"]), r"(?i)(timeout|timed out)",
                         "target-only FIFO fixture must execute and time out in deep proof")
        self.assertIn(blob_oid, " ".join(linked["unknowns"]), "timed-out object identity lost")
        self.assertIn("requested blob batch", " ".join(linked["unknowns"]),
                      "batch failure must label all requested OIDs without claiming the culprit")

    def test_fifo_alternates_pointer_cannot_prove_integration(self):
        inventory = self.save_inventory()
        alternates = self.fixture.principal / ".git/objects/info/alternates"
        os.mkfifo(alternates)
        self.assert_explicit_unknown_or_refusal(self.plan(inventory))

    def test_external_resident_alternates_remain_positive(self):
        objects = self.fixture.principal / ".git/objects"
        external = self.fixture.root / "external resident object store"
        objects.rename(external)
        (objects / "info").mkdir(parents=True)
        (objects / "info/alternates").write_text(str(external) + "\n")
        inventory = self.save_inventory()
        result = self.plan(inventory)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        items = json.loads(result.stdout)["items"]
        self.assertEqual({decode_path(item["path"]) for item in items}, set(self.fixture.expected))
        self.assert_clean_linked_positive(items)

    def test_fifo_gitdir_pointer_scan_repo_and_roots_preserve_error(self):
        candidate = self.fixture.root / "special gitdir marker"
        candidate.mkdir()
        os.mkfifo(candidate / ".git")
        result = self.run_cli("scan", "--repo", candidate, "--format", "json")
        self.assertNotEqual(result.returncode, 0, "special Gitdir pointer accepted")
        self.assertTrue(result.stderr.strip())
        result = self.run_cli("scan", "--root", candidate, "--format", "json")
        if result.returncode:
            self.assertTrue(result.stderr.strip())
        else:
            inventory = json.loads(result.stdout)
            self.assertFalse(inventory["coverage"]["traversal_complete"])
            self.assertTrue(inventory["coverage"]["gaps"], "special marker silently omitted")

    def test_required_fifo_pack_cannot_prove_integration(self):
        inventory = self.save_inventory()
        objects = self.fixture.principal / ".git/objects"
        packed = self.fixture.git("-C", str(self.fixture.principal), "pack-objects", "--all",
                                  str(objects / "pack/pack")).stdout.decode().strip()
        self.assertEqual(len(packed), 40)
        for directory in objects.iterdir():
            if len(directory.name) == 2 and all(c in "0123456789abcdef" for c in directory.name):
                for entry in directory.iterdir():
                    entry.unlink()
                directory.rmdir()
        positive = self.plan(inventory)
        self.assertEqual(positive.returncode, 0, positive.stderr.decode(errors="replace"))
        self.assert_clean_linked_positive(json.loads(positive.stdout)["items"])
        pack = objects / "pack" / f"pack-{packed}.pack"
        pack.unlink()
        os.mkfifo(pack)
        self.assert_explicit_unknown_or_refusal(self.plan(inventory))

    def test_separate_gitdir_resident_positive(self):
        separate = self.fixture.root / "separate worktree"
        administration = self.fixture.root / "external administration"
        self.fixture.git("init", "-b", "main", "--separate-git-dir", str(administration), str(separate))
        (separate / "resident.txt").write_bytes(b"resident separate repository\n")
        self.fixture.git("-C", str(separate), "add", "resident.txt")
        self.fixture.git("-C", str(separate), "commit", "-m", "Synthetic separate")
        inventory = self.inventory(separate)
        self.assertEqual(len(inventory["families"]), 1)
        self.assertEqual(decode_path(inventory["families"][0]["common_dir"]), os.fsencode(administration))
        self.assertEqual({decode_path(w["path"]) for w in inventory["families"][0]["worktrees"]},
                         {os.fsencode(separate)})
        worktree = inventory["families"][0]["worktrees"][0]
        self.assertFalse(worktree["bare"])
        self.assertEqual(worktree["head"], self.fixture.oid(separate))
        result = self.run_cli("scan", "--root", separate, "--format", "json")
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        discovered = json.loads(result.stdout)
        self.assertEqual(len(discovered["families"]), 1)
        self.assertEqual({decode_path(w["path"]) for w in discovered["families"][0]["worktrees"]},
                         {os.fsencode(separate)}, "discovery invented administration as worktree")
        linked = self.fixture.root / "separate linked"
        self.fixture.git("-C", str(separate), "worktree", "add", "--detach", str(linked))
        inventory = self.inventory(separate)
        inventory_file = self.fixture.root / "separate inventory.json"
        inventory_file.write_text(json.dumps(inventory))
        result = self.plan(inventory_file)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        items = json.loads(result.stdout)["items"]
        self.assertEqual({decode_path(item["path"]) for item in items},
                         {os.fsencode(separate), os.fsencode(linked)})
        self.assert_clean_linked_positive(items, separate, linked)

    def test_bare_resident_positive(self):
        bare = self.fixture.root / "resident bare.git"
        self.fixture.git("clone", "--bare", "--no-local", str(self.fixture.principal), str(bare))
        inventory = self.inventory(bare)
        worktrees = inventory["families"][0]["worktrees"]
        self.assertEqual(len(worktrees), 1)
        self.assertTrue(worktrees[0]["bare"])
        self.assertEqual(decode_path(worktrees[0]["path"]), os.fsencode(bare))

    def test_sha256_resident_positive(self):
        self.fixture = HistoryFixture(object_format="sha256")
        self.addCleanup(self.fixture.close)
        inventory_file = self.save_inventory()
        inventory = json.loads(inventory_file.read_bytes())
        self.assertEqual(len(inventory["families"]), 1)
        self.assertEqual({decode_path(w["path"]) for w in inventory["families"][0]["worktrees"]},
                         set(self.fixture.expected))
        result = self.plan(inventory_file)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        items = json.loads(result.stdout)["items"]
        self.assertEqual({decode_path(item["path"]) for item in items}, set(self.fixture.expected))
        self.assert_clean_linked_positive(items)
        self.assertTrue(all(len(observation["source_oid"]) == 64
                            and len(observation["target_oid"]) == 64
                            for item in items for observation in item["observations"]))

    def test_workers_preserve_exact_root_results(self):
        baseline = None
        for workers in (1, 4):
            started = time.monotonic()
            result = self.run_cli("scan", "--root", self.fixture.root, "--workers", workers,
                                  "--format", "json")
            self.assertLess(time.monotonic() - started, 35)
            self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
            inventory = json.loads(result.stdout)
            families = inventory["families"]
            self.assertTrue(families, "empty parallel traversal passed equality vacuously")
            self.assertEqual({decode_path(w["path"]) for family in families
                              for w in family["worktrees"]}, set(self.fixture.expected))
            stable = {key: value for key, value in inventory.items()
                      if key not in ("started_unix_ms", "finished_unix_ms")}
            # Only execution settings and elapsed time vary; keep all observed counters.
            metrics = stable["coverage"]["discovery_metrics"]
            metrics.pop("elapsed_ms")
            metrics.pop("workers")
            if baseline is None:
                baseline = stable
            else:
                self.assertEqual(stable, baseline, "worker count changed family, marker or coverage evidence")

    def test_workers_outside_bounded_range_are_refused(self):
        for workers in (0, 9):
            with self.subTest(workers=workers):
                result = self.run_cli("scan", "--root", self.fixture.root, "--workers", workers,
                                      "--format", "json")
                self.assertNotEqual(result.returncode, 0)
                self.assertTrue(result.stderr.strip())


if __name__ == "__main__":
    unittest.main()
