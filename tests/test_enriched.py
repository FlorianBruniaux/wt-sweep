"""Independent E2E oracles for descriptive local evidence and owned run artifacts."""

import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import unittest

from inventory_support import BINARY, GIT, HistoryFixture, decode_path


class EnrichedTests(unittest.TestCase):
    def setUp(self):
        self.fixture = HistoryFixture()
        self.addCleanup(self.fixture.close)

    def stop_owned_process(self, process):
        # Git reads have their own 10s bound. Drain every directly owned Popen;
        # no process-table inspection or claim about unknown descendants.
        try:
            process.communicate(timeout=12)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.kill()
            process.communicate(timeout=5)

    def run_cli(self, *arguments):
        before = self.fixture.target_manifest()
        process = subprocess.Popen([str(BINARY), *map(str, arguments)], env=self.fixture.env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=True)
        try:
            stdout, stderr = process.communicate(timeout=40)
        except subprocess.TimeoutExpired:
            self.stop_owned_process(process)
            self.fail("owned CLI stopped/reaped after bounded deadline")
        finally:
            self.assertEqual(before, self.fixture.target_manifest(),
                             "CLI modified worktree/admin manifest (atime excluded)")
        return subprocess.CompletedProcess(process.args, process.returncode, stdout, stderr)

    def successful_json(self, result):
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        return json.loads(result.stdout)

    def save_inventory(self, local=False):
        extra = ("--local",) if local else ()
        inventory = self.successful_json(self.run_cli("scan", "--repo", self.fixture.principal,
                                                       *extra, "--format", "json"))
        path = self.fixture.root / "input inventory.json"
        path.write_text(json.dumps(inventory))
        return path, inventory

    def plan(self, inventory, *extra):
        return self.run_cli("plan", "--inventory", inventory, "--target", "refs/heads/main", *extra)

    def git_input(self, payload, *arguments):
        result = subprocess.run([GIT, "-C", str(self.fixture.principal), *arguments],
                                input=payload, env=self.fixture.env, capture_output=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        return result.stdout

    def raw_subject_commit(self, subject):
        self.fixture.tick()
        tree = self.fixture.oid(self.fixture.principal, "HEAD^{tree}")
        header = (f"tree {tree}\nparent {self.fixture.base}\n"
                  f"author Fixture <fixture@example.invalid> {self.fixture.clock} +0000\n"
                  f"committer Fixture <fixture@example.invalid> {self.fixture.clock} +0000\n\n").encode()
        oid = self.git_input(header + subject + b"\nBody must not become subject.\n",
                             "hash-object", "-t", "commit", "--literally", "-w", "--stdin").decode().strip()
        self.fixture.git("-C", str(self.fixture.linked), "update-ref", "refs/heads/topic", oid)
        self.fixture.heads[os.fsencode(self.fixture.linked)] = oid
        return oid

    def local_by_worktree(self, inventory):
        local = inventory["local_inventories"]
        result = {decode_path(item["worktree"]): item for item in local}
        self.assertEqual(len(result), len(local), "duplicate local worktree identity")
        self.assertEqual(set(result), set(self.fixture.expected), "empty or omitted local inventory")
        return result

    def test_local_categories_raw_hashes_and_symlinks_are_preserved(self):
        principal = self.fixture.principal
        files = {b"tracked.txt": (b"modified tracked\n", "tracked"),
                 b".gitignore": (b"ignored/\n", "untracked"),
                 b"untracked space\nname": (b"raw\x00payload\xff\n", "untracked"),
                 b".hidden": (b"hidden\n", "untracked"),
                 b"ignored/.hidden": (b"ignored bytes\n", "ignored")}
        (principal / "ignored").mkdir()
        for raw_name, (payload, _) in files.items():
            (principal / os.fsdecode(raw_name)).write_bytes(payload)
        outside = self.fixture.root / "outside sentinel"
        outside.mkdir()
        (outside / "not traversed").write_bytes(b"symlink target sentinel\n")
        self.fixture.manifest_roots.add(os.fsencode(outside))
        os.symlink("../outside sentinel", principal / "symbolic directory")
        os.symlink("tracked.txt", principal / "symbolic file")
        _, inventory = self.save_inventory(local=True)
        local = self.local_by_worktree(inventory)[os.fsencode(principal)]
        self.assertTrue(local["traversal_complete"], local["gaps"])
        self.assertEqual(local["temporal_consistency"], "not_guaranteed")
        entries = {decode_path(entry["path"]): entry for entry in local["entries"]}
        self.assertEqual(len(entries), len(local["entries"]))
        self.assertEqual(set(entries), set(files) | {b"ignored", b"symbolic directory", b"symbolic file", b".git"},
                         "local inventory omitted or invented fixture paths")
        self.assertTrue(all(not name.startswith(b"/") for name in entries), "local entries lost relative identity")
        for name, (payload, category) in files.items():
            entry = entries[name]
            self.assertEqual(entry["kind"], "file")
            self.assertEqual(entry["category"], category)
            self.assertEqual(entry["content_sha256"], hashlib.sha256(payload).hexdigest())
            self.assertEqual(entry["size"], len(payload))
        for name, target in ((b"symbolic directory", b"../outside sentinel"),
                             (b"symbolic file", b"tracked.txt")):
            self.assertEqual(entries[name]["kind"], "symlink")
            self.assertEqual(decode_path(entries[name]["link_target"]), target)
            self.assertIsNone(entries[name]["content_sha256"])
            self.assertFalse(any(candidate.startswith(name + b"/") for candidate in entries),
                             "local walker followed symlink directory")
        index = [entry for entry in local["index_entries"] if decode_path(entry["path"]) == b"tracked.txt"]
        self.assertEqual(len(local["index_entries"]), 1)
        self.assertEqual(len(index), 1)
        self.assertEqual(index[0]["stage"], 0)
        self.assertEqual(index[0]["oid"], self.fixture.oid(principal, "HEAD:tracked.txt"))

    def test_local_index_conflict_stages_and_administration_bytes(self):
        oids = [self.git_input(payload, "hash-object", "-w", "--stdin").decode().strip()
                for payload in (b"base stage\n", b"ours stage\n", b"theirs stage\n")]
        records = b"0 " + b"0" * 40 + b"\ttracked.txt\n"
        records += b"".join(f"100644 {oid} {stage}\ttracked.txt\n".encode()
                            for stage, oid in enumerate(oids, start=1))
        self.git_input(records, "update-index", "--index-info")
        _, inventory = self.save_inventory(local=True)
        for worktree, local in self.local_by_worktree(inventory).items():
            entries = {decode_path(entry["path"]): entry for entry in local["administration_entries"]}
            self.assertTrue(all(Path(os.fsdecode(path)).is_absolute() for path in entries))
            for git_path in ("index", "logs/HEAD"):
                path = Path(self.fixture.git("-C", os.fsdecode(worktree), "rev-parse",
                                             "--path-format=absolute", "--git-path", git_path).stdout.decode().strip())
                self.assertIn(os.fsencode(path), entries)
                self.assertEqual(entries[os.fsencode(path)]["content_sha256"],
                                 hashlib.sha256(path.read_bytes()).hexdigest())
        principal = self.local_by_worktree(inventory)[os.fsencode(self.fixture.principal)]
        stages = [(entry["stage"], entry["oid"]) for entry in principal["index_entries"]
                  if decode_path(entry["path"]) == b"tracked.txt"]
        self.assertEqual(sorted(stages), list(enumerate(oids, start=1)), "index collapsed conflict stages")

    def test_split_index_shared_bytes_are_in_administration_evidence(self):
        self.fixture.git("-C", str(self.fixture.principal), "update-index", "--split-index")
        shared = list((self.fixture.principal / ".git").glob("sharedindex.*"))
        self.assertTrue(shared, "fixture did not create a real shared index")
        _, inventory = self.save_inventory(local=True)
        local = self.local_by_worktree(inventory)[os.fsencode(self.fixture.principal)]
        self.assertFalse(local["traversal_complete"])
        self.assertEqual(local["index_entries"], [], "split index stages invented without safe decoding proof")
        self.assertTrue(any("split" in gap["reason"].lower() for gap in local["gaps"]),
                        "split index decoding omission has no explicit gap")
        tracked = next(entry for entry in local["entries"] if decode_path(entry["path"]) == b"tracked.txt")
        self.assertNotIn(tracked["category"], ("tracked", "untracked", "ignored"),
                         "split index produced a known file classification without decoding")
        self.assertEqual(tracked["content_sha256"], hashlib.sha256(b"base\n").hexdigest())
        entries = {decode_path(entry["path"]): entry for entry in local["administration_entries"]}
        index = self.fixture.principal / ".git/index"
        self.assertIn(os.fsencode(index), entries)
        self.assertEqual(entries[os.fsencode(index)]["content_sha256"], hashlib.sha256(index.read_bytes()).hexdigest())
        for path in shared:
            self.assertIn(os.fsencode(path), entries)
            self.assertEqual(entries[os.fsencode(path)]["content_sha256"],
                             hashlib.sha256(path.read_bytes()).hexdigest())

    def test_content_digest_changes_for_same_status_size_and_mtime(self):
        path = self.fixture.principal / "tracked.txt"
        path.write_bytes(b"AAAA\n")
        original = os.stat(path)
        _, first = self.save_inventory(local=True)
        path.write_bytes(b"BBBB\n")
        os.utime(path, ns=(original.st_atime_ns, original.st_mtime_ns))
        self.assertEqual(os.stat(path).st_size, original.st_size)
        self.assertEqual(os.stat(path).st_mtime_ns, original.st_mtime_ns)
        _, second = self.save_inventory(local=True)
        before = self.local_by_worktree(first)[os.fsencode(self.fixture.principal)]
        after = self.local_by_worktree(second)[os.fsencode(self.fixture.principal)]
        for local, payload in ((before, b"AAAA\n"), (after, b"BBBB\n")):
            entry = next(entry for entry in local["entries"] if decode_path(entry["path"]) == b"tracked.txt")
            self.assertEqual(entry["category"], "tracked")
            self.assertEqual(entry["content_sha256"], hashlib.sha256(payload).hexdigest())
            self.assertEqual(len(local["content_digest_sha256"]), 64)
            self.assertTrue(all(character in "0123456789abcdef"
                                for character in local["content_digest_sha256"]))
        self.assertNotEqual(before["content_digest_sha256"], after["content_digest_sha256"])

    def test_local_special_file_exposes_gap_without_reading_fifo(self):
        os.mkfifo(self.fixture.principal / "filesystem fifo")
        _, inventory = self.save_inventory(local=True)
        local = self.local_by_worktree(inventory)[os.fsencode(self.fixture.principal)]
        entry = next(entry for entry in local["entries"] if decode_path(entry["path"]) == b"filesystem fifo")
        self.assertEqual(entry["kind"], "fifo")
        self.assertIsNone(entry["content_sha256"])
        self.assertFalse(local["traversal_complete"])
        self.assertTrue(local["gaps"])

    def test_local_file_budget_retains_metadata_and_explicit_gap(self):
        path = self.fixture.principal / "over budget sparse file"
        size = 64 * 1024 * 1024 + 1
        with path.open("wb") as stream:
            stream.truncate(size)
        _, inventory = self.save_inventory(local=True)
        local = self.local_by_worktree(inventory)[os.fsencode(self.fixture.principal)]
        entry = next(entry for entry in local["entries"]
                     if decode_path(entry["path"]) == b"over budget sparse file")
        self.assertEqual(entry["kind"], "file")
        self.assertEqual(entry["size"], size)
        self.assertIsNone(entry["content_sha256"], "file above advertised 64MiB bound was read fully")
        self.assertFalse(local["traversal_complete"])
        self.assertTrue(local["gaps"], "bounded omission was silently presented as complete")

    def test_raw_subject_bytes_truncation_and_nul_unknown(self):
        cases = ((b"raw subject \xff", b"raw subject \xff", False),
                 (b"x" * 239 + "é".encode(), b"x" * 239 + b"\xc3", True),
                 (b"subject\x00hidden", None, False))
        for raw, expected, truncated in cases:
            with self.subTest(subject=raw[:20]):
                head = self.raw_subject_commit(raw)
                path, _ = self.save_inventory()
                review = self.successful_json(self.plan(path, "--format", "json"))
                item = next(item for item in review["items"]
                            if decode_path(item["path"]) == os.fsencode(self.fixture.linked))
                self.assertEqual(item["head"], head)
                self.assertEqual(item["classification"], "keep",
                                 "descriptive subject cannot erase complete HEAD non-integration")
                self.assertIn("head_not_integrated", item["decision"]["keep_reasons"])
                self.assertEqual(item["subject_truncated"], truncated)
                if expected is None:
                    self.assertIsNone(item["subject"])
                    self.assertTrue(any("subject" in unknown.lower() and "nul" in unknown.lower()
                                        for unknown in item["unknowns"]), "NUL subject was silently known")
                else:
                    self.assertIsNotNone(item["subject"], "KEEP must retain verified raw subject bytes")
                    self.assertEqual(decode_path(item["subject"]), expected)

    def test_subject_and_branch_are_descriptive_verified_head_fields(self):
        path, _ = self.save_inventory()
        review = self.successful_json(self.plan(path, "--format", "json"))
        items = {decode_path(item["path"]): item for item in review["items"]}
        self.assertEqual(set(items), set(self.fixture.expected))
        for raw_path, item in items.items():
            self.assertEqual(item["head"], self.fixture.heads[raw_path])
            self.assertEqual(decode_path(item["branch"]), self.fixture.expected[raw_path][0].encode())
            self.assertIsNotNone(item["subject"], "deep gating must retain verified descriptive HEAD facts")
            self.assertEqual(decode_path(item["subject"]), b"Base")
            self.assertFalse(item["subject_truncated"])
            expected = "keep" if raw_path == os.fsencode(self.fixture.principal) else "cleanup_candidate"
            self.assertEqual(item["classification"], expected)

    def test_summary_is_bounded_and_json_combination_refused(self):
        path, _ = self.save_inventory()
        result = self.plan(path, "--summary")
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        lines = result.stdout.decode().splitlines()
        self.assertGreater(len(lines), 0)
        self.assertLessEqual(len(lines), 5)
        result = self.plan(path, "--summary", "--format", "json")
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(result.stderr.strip())

    def write_forge_raw(self, title="Readable request title", head=None):
        self.fixture.git("-C", str(self.fixture.principal), "config", "remote.origin.url",
                         "https://forge.example.invalid/team/project.git")
        raw = self.fixture.root / "synthetic raw.json"
        raw.write_text(json.dumps([{"iid": 7, "project_id": 17, "source_project_id": 17,
                                    "target_project_id": 17, "source_branch": "topic",
                                    "target_branch": "main", "sha": head or self.fixture.base,
                                    "state": "opened", "title": title,
                                    "web_url": "https://forge.example.invalid/team/project/-/merge_requests/7"}]))
        return raw

    def test_forge_title_is_descriptive_and_cannot_override_head_authority(self):
        raw = self.write_forge_raw()
        forge = self.fixture.root / "synthetic normalized.json"
        result = self.run_cli("forge", "ingerer", "--raw", raw, "--out", forge)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        inventory, _ = self.save_inventory()
        report = self.successful_json(self.plan(inventory, "--forge", forge, "--format", "json"))
        source = next(item for item in report["items"]
                      if decode_path(item["path"]) == os.fsencode(self.fixture.linked))
        self.assertEqual(source["classification"], "keep")
        self.assertIn("open_request", source["decision"]["keep_reasons"])
        self.assertTrue(source["requests"])
        self.assertEqual(source["requests"][0]["title"], "Readable request title")
        table = self.plan(inventory, "--forge", forge, "--format", "table")
        self.assertEqual(table.returncode, 0, table.stderr.decode(errors="replace"))
        text = table.stdout.decode()
        for value in ("Base", "Readable request title", "refs/heads/main", "UNKNOWN"):
            self.assertIn(value, text, "detailed table omitted subject, MR, target or unknown")
        raw = self.write_forge_raw(title="ALREADY FULLY INTEGRATED", head="a" * 40)
        alternate = self.fixture.root / "unmatched normalized.json"
        result = self.run_cli("forge", "ingerer", "--raw", raw, "--out", alternate)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        report = self.successful_json(self.plan(inventory, "--forge", alternate, "--format", "json"))
        source = next(item for item in report["items"]
                      if decode_path(item["path"]) == os.fsencode(self.fixture.linked))
        self.assertEqual(source["classification"], "cleanup_candidate", "title fabricated matching open activity")

    def validate_run(self, directory, artifact, command, status=None):
        self.assertEqual({path.name for path in directory.iterdir()}, {artifact, "report.txt", "run.json"},
                         "successful run has missing files or temporary publication residue")
        manifest = json.loads((directory / "run.json").read_bytes())
        self.assertEqual(manifest["schema"], "wt-sweep.run/1")
        self.assertIsInstance(manifest["run_id"], str)
        self.assertTrue(manifest["run_id"])
        self.assertEqual(manifest["command"], command)
        self.assertLessEqual(manifest["started_unix_ms"], manifest["finished_unix_ms"])
        self.assertIn(manifest["status"], ("complete", "partial"))
        if status:
            self.assertEqual(manifest["status"], status)
        records = manifest["artifacts"]
        self.assertEqual(len(records), 2)
        self.assertEqual({record["name"] for record in records}, {artifact, "report.txt"})
        for record in records:
            payload = (directory / record["name"]).read_bytes()
            self.assertEqual(record["sha256"], hashlib.sha256(payload).hexdigest())
            self.assertEqual(record["bytes"], len(payload))
        self.assertTrue((directory / "report.txt").read_bytes().strip())
        return manifest, json.loads((directory / artifact).read_bytes())

    def new_run(self, base, artifact, command, *arguments, status=None):
        before = set(base.iterdir())
        result = self.run_cli(*arguments, "--run-dir", base)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        added = set(base.iterdir()) - before
        self.assertEqual(len(added), 1, "command did not publish exactly one uniquely owned run")
        directory = added.pop()
        self.assertTrue(directory.is_dir())
        manifest, data = self.validate_run(directory, artifact, command, status)
        return result, directory, manifest, data

    def test_scan_plan_and_forge_publish_verifiable_independent_runs(self):
        base = self.fixture.root / "outside runs"
        base.mkdir()
        scan, scan_directory, scan_manifest, inventory = self.new_run(
            base, "inventory.json", "scan", "scan", "--repo", self.fixture.principal,
            "--format", "json", status="complete")
        self.assertEqual(json.loads(scan.stdout), inventory)
        plan, plan_directory, plan_manifest, _ = self.new_run(
            base, "review.json", "plan", "plan", "--inventory", scan_directory / "inventory.json",
            "--target", "refs/heads/main", "--summary")
        self.assertGreater(len(plan.stdout.splitlines()), 0)
        self.assertLessEqual(len(plan.stdout.splitlines()), 5)
        detail = (plan_directory / "report.txt").read_text()
        for value in ("Base", "refs/heads/main", "UNKNOWN"):
            self.assertIn(value, detail, "summary publication lost its full descriptive report")
        self.assertGreater(len(detail.splitlines()), 5)
        raw = self.write_forge_raw()
        forge, _, forge_manifest, imported = self.new_run(
            base, "forge.json", "forge ingerer", "forge", "ingerer", "--raw", raw,
            "--format", "json", status="partial")
        self.assertEqual(json.loads(forge.stdout), imported)
        self.assertEqual(len(imported["requests"]), 1)
        self.assertFalse(imported["coverage"]["absence_proven"])
        self.assertEqual(len({scan_manifest["run_id"], plan_manifest["run_id"], forge_manifest["run_id"]}), 3)

    def test_local_gap_publishes_partial_run(self):
        os.mkfifo(self.fixture.principal / "filesystem fifo")
        base = self.fixture.root / "partial runs"
        base.mkdir()
        _, _, _, inventory = self.new_run(base, "inventory.json", "scan", "scan", "--repo",
                                           self.fixture.principal, "--local", status="partial")
        self.assertFalse(self.local_by_worktree(inventory)[os.fsencode(self.fixture.principal)]["traversal_complete"])

    def test_run_directory_refuses_protected_symlink_and_conflicting_output(self):
        base = self.fixture.root / "safe runs"
        base.mkdir()
        symlink = self.fixture.root / "symbolic runs"
        os.symlink(base, symlink)
        inside = self.fixture.principal / "forbidden runs"
        inside.mkdir()
        for destination in (symlink, symlink / "nested", inside):
            with self.subTest(destination=destination.name):
                result = self.run_cli("scan", "--repo", self.fixture.principal, "--run-dir", destination)
                self.assertNotEqual(result.returncode, 0)
                self.assertTrue(result.stderr.strip())
        existing = self.fixture.root / "existing artifact.json"
        existing.write_bytes(b"sentinel\n")
        result = self.run_cli("scan", "--repo", self.fixture.principal, "--run-dir", base, "--out", existing)
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(result.stderr.strip())
        self.assertEqual(existing.read_bytes(), b"sentinel\n")
        self.assertEqual(list(base.iterdir()), [], "refused invocation created run artifacts")
        self.assertEqual(list(inside.iterdir()), [])

    def test_two_concurrent_cli_calls_publish_distinct_runs(self):
        base = self.fixture.root / "concurrent runs"
        base.mkdir()
        before = self.fixture.target_manifest()
        arguments = [str(BINARY), "scan", "--repo", str(self.fixture.principal), "--run-dir", str(base)]
        processes = []
        results = []
        try:
            for _ in range(2):
                processes.append(subprocess.Popen(arguments, env=self.fixture.env, stdout=subprocess.PIPE,
                                                   stderr=subprocess.PIPE, start_new_session=True))
            for process in processes:
                stdout, stderr = process.communicate(timeout=40)
                results.append((process.returncode, stdout, stderr))
            for returncode, stdout, stderr in results:
                self.assertEqual(returncode, 0, stderr.decode(errors="replace"))
                self.assertTrue(stdout.strip())
        finally:
            for process in processes:
                if process.poll() is None or (process.stdout and not process.stdout.closed):
                    self.stop_owned_process(process)
            self.assertEqual(before, self.fixture.target_manifest(), "concurrent publications mutated targets")
        directories = list(base.iterdir())
        self.assertEqual(len(directories), 2)
        manifests = [self.validate_run(path, "inventory.json", "scan")[0] for path in directories]
        self.assertEqual(len({manifest["run_id"] for manifest in manifests}), 2)


if __name__ == "__main__":
    unittest.main()
