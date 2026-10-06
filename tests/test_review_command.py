"""Independent T09 review-command oracles using owned real Git fixtures.

Expected paths, work, graph topology and digests are built without product imports.
Set WT_SWEEP_REVIEW_COMMAND_BINARY to an immutable binary for exact replay.
"""

import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import unittest

from inventory_support import BINARY as DEFAULT_BINARY, GIT, HistoryFixture, decode_path, filesystem_manifest
import test_discovery_publication as discovery_oracles

BINARY = Path(os.environ.get("WT_SWEEP_REVIEW_COMMAND_BINARY", str(DEFAULT_BINARY)))


def digest(document):
    body = {key: value for key, value in document.items() if key != "digest_sha256"}
    return hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":"),
                                     ensure_ascii=False).encode()).hexdigest()


def timeless(value):
    """Compare complete substantive facts while discarding interval-bound digests."""
    omitted = {"started_unix_ms", "finished_unix_ms", "elapsed_ms", "digest_sha256", "inventory_sha256"}
    if isinstance(value, dict):
        return {key: timeless(item) for key, item in value.items() if key not in omitted}
    if isinstance(value, list):
        return [timeless(item) for item in value]
    return value


class ReviewCommandTests(unittest.TestCase):
    def setUp(self):
        self.fixture = HistoryFixture()
        self.addCleanup(self.fixture.close)
        self.fixture.env.update(GIT_CONFIG_SYSTEM=os.devnull, GIT_ATTR_NOSYSTEM="1",
                                GIT_OPTIONAL_LOCKS="0", GIT_NO_LAZY_FETCH="1", GIT_ALLOW_PROTOCOL="")
        output = tempfile.TemporaryDirectory(prefix="wt-sweep-t09-output-", dir="/private/tmp")
        self.addCleanup(output.cleanup)
        self.output = Path(output.name)
        self.sequence = 0
        self.dirty = self.fixture.root / "dirty linked"
        self.unknown = self.fixture.root / ".hidden verify"
        for branch, path in (("dirty", self.dirty), ("unknown", self.unknown)):
            self.fixture.git("-C", self.fixture.principal, "worktree", "add", "-b", branch, path)
            self.fixture.heads[os.fsencode(path)] = self.fixture.base
        (self.dirty / "local notes").write_bytes(b"synthetic work to preserve\x00\xff\n")
        gitdir = self.fixture.git("-C", self.unknown, "rev-parse", "--absolute-git-dir").stdout.strip()
        bad_log = Path(os.fsdecode(gitdir)) / "logs/refs/rewritten/malformed"
        bad_log.parent.mkdir(parents=True)
        bad_log.write_bytes(b"synthetic invalid private reflog, no OID\n")
        self.expected = {os.fsencode(self.fixture.principal): "keep",
                         os.fsencode(self.fixture.linked): "cleanup_candidate",
                         os.fsencode(self.dirty): "keep", os.fsencode(self.unknown): "verify"}

    def owned_process(self, arguments, data=b"", timeout=30, cwd=None):
        process = subprocess.Popen(list(map(str, arguments)), env=self.fixture.env,
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True, cwd=cwd)
        try:
            stdout, stderr = process.communicate(input=data, timeout=timeout)
        except BaseException:
            # Product Git children own isolated groups. Select only this owned
            # live CLI's children, never unrelated process command lines.
            children = subprocess.run(["/usr/bin/pgrep", "-P", str(process.pid)],
                                      stdin=subprocess.DEVNULL, capture_output=True, timeout=2)
            if children.returncode == 0:
                for raw_pid in children.stdout.splitlines():
                    pid = int(raw_pid)
                    identity = subprocess.run(["/bin/ps", "-p", str(pid), "-o", "ppid=,pgid="],
                                              stdin=subprocess.DEVNULL, capture_output=True, timeout=2)
                    parts = identity.stdout.split()
                    if len(parts) == 2 and int(parts[0]) == process.pid and int(parts[1]) == pid:
                        try:
                            os.killpg(pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
            try:
                process.communicate(timeout=1)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.communicate(timeout=5)
            raise
        return subprocess.CompletedProcess(process.args, process.returncode, stdout, stderr)

    def run_cli(self, *arguments, timeout=35, cwd=None):
        before = filesystem_manifest(self.fixture.root)
        try:
            return self.owned_process([BINARY, *arguments], timeout=timeout, cwd=cwd)
        finally:
            after = filesystem_manifest(self.fixture.root)
            changed = [path for path in before.keys() | after.keys() if before.get(path) != after.get(path)]
            self.assertFalse(changed, "CLI changed source bytes/metadata/admin (atime excluded): " + repr(changed[:8]))

    def success(self, result):
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        return json.loads(result.stdout)

    def save(self, value, name="artifact"):
        self.sequence += 1
        path = self.output / f"{name}-{self.sequence}.json"
        path.write_text(json.dumps(value))
        return path

    def source(self, document):
        source = document.get("source", document)
        self.assertEqual(source["schema"], "wt-sweep.review/2")
        self.assertEqual(source["digest_sha256"], digest(source), "review digest does not bind its full facts")
        if "source" in document:
            self.assertEqual(document["schema"], "wt-sweep.review-view/1")
            self.assertEqual(document["digest_sha256"], digest(document), "filtered view digest is invalid")
        return source

    def items(self, document):
        source = self.source(document)
        items = {decode_path(item["path"]): item for item in source["items"]}
        self.assertEqual(len(items), len(source["items"]), "duplicate reviewed identity")
        self.assertEqual(set(items), set(self.expected), "full source omitted or invented a fixture worktree")
        for path, item in items.items():
            self.assertEqual(item["head"], self.fixture.heads[path], "facts are not bound to actual fixture HEAD")
        return items

    def assert_verdicts(self, document):
        items = self.items(document)
        self.assertEqual({path: item["classification"] for path, item in items.items()}, self.expected,
                         "KEEP, VERIFY and clean integrated candidate were collapsed")
        self.assertEqual(set(item["classification"] for item in items.values()), {"keep", "verify", "cleanup_candidate"})
        self.assertEqual(items[os.fsencode(self.dirty)]["local_summary"]["untracked"]["observed"], 1)
        return items

    def old_flow(self, scope=None, extra=()):
        scope = scope or ("--repo", self.fixture.principal)
        inventory = self.success(self.run_cli("scan", *scope, "--format", "json"))
        path = self.save(inventory, "inventory")
        report = self.success(self.run_cli("plan", "--inventory", path, "--target", "refs/heads/main",
                                           "--format", "json", *extra))
        self.assertEqual(self.source(report)["inventory_sha256"], digest(inventory))
        return inventory, report

    def review(self, *extra, scope=None):
        return self.success(self.run_cli("review", *(scope or ("--repo", self.fixture.principal)),
                                         "--target", "refs/heads/main", "--format", "json", *extra))

    def git_input(self, *arguments, payload=b"", expected=0):
        result = self.owned_process([GIT, "-c", "core.hooksPath=/dev/null", "-c", "gc.auto=0",
                                     "-C", self.fixture.principal, *arguments], data=payload, timeout=15)
        self.assertEqual(result.returncode, expected, result.stderr.decode(errors="replace"))
        return result.stdout

    def test_real_fixture_old_scan_plan_preserves_three_distinct_verdicts(self):
        # Independent Git premises establish the positive before new command use.
        self.assertEqual(self.fixture.oid(self.fixture.linked), self.fixture.base)
        self.assertEqual(self.fixture.git("-C", self.fixture.linked, "status", "--porcelain").stdout, b"")
        self.assertEqual(self.fixture.git("-C", self.dirty, "status", "--porcelain").stdout, b"?? \"local notes\"\n")
        inventory, report = self.old_flow()
        self.assertEqual(len(inventory["families"]), 1)
        self.assertEqual(len(inventory["families"][0]["worktrees"]), 4)
        self.assert_verdicts(report)

    def test_review_reuses_old_scan_plan_facts_for_repo_root_and_workers(self):
        for scope in (("--repo", self.fixture.principal),
                      ("--root", self.fixture.root), ("--root", self.fixture.root, "--workers", "4")):
            with self.subTest(scope=scope):
                _, previous = self.old_flow(scope)
                current = self.review(scope=scope)
                self.assert_verdicts(previous)
                self.assert_verdicts(current)
                self.assertEqual(timeless(current), timeless(previous), "fresh review changed scan-to-plan substantive facts")

    def test_review_bundle_contains_exact_full_input_review_report_and_three_artifact_hashes(self):
        document = self.review("--run-dir", self.output)
        self.assert_verdicts(document)
        directories = list(self.output.iterdir())
        self.assertEqual(len(directories), 1)
        directory = directories[0]
        self.assertEqual({p.name for p in directory.iterdir()}, {"inventory.json", "review.json", "report.txt", "run.json"})
        inventory = json.loads((directory / "inventory.json").read_bytes())
        review = json.loads((directory / "review.json").read_bytes())
        self.assertEqual(document, review, "stdout disagrees with saved review")
        self.assertEqual(self.source(review)["inventory_sha256"], digest(inventory), "review does not bind exact original saved input")
        self.assertEqual(len(inventory["families"]), 1)
        self.assertEqual({decode_path(w["path"]) for w in inventory["families"][0]["worktrees"]}, set(self.expected))
        self.assertNotIn("publication", inventory["coverage"], "fresh review saved compacted input")
        receipt = json.loads((directory / "run.json").read_bytes())
        self.assertEqual(receipt["schema"], "wt-sweep.run/1")
        self.assertEqual(receipt["command"], "review")
        self.assertEqual(receipt["run_id"], directory.name)
        self.assertIn(receipt["status"], ("complete", "partial"))
        self.assertEqual(receipt["publication"], {"completion": "requires_successful_publisher_return",
                         "durability": "unknown_from_receipt_alone", "meaning": "artifacts_synced_before_receipt"})
        self.assertEqual(len(receipt["artifacts"]), 3)
        self.assertEqual({r["name"] for r in receipt["artifacts"]}, {"inventory.json", "review.json", "report.txt"})
        for record in receipt["artifacts"]:
            payload = (directory / record["name"]).read_bytes()
            self.assertEqual(record["bytes"], len(payload))
            self.assertEqual(record["sha256"], hashlib.sha256(payload).hexdigest())
        self.assertTrue((directory / "report.txt").read_bytes().strip())
        old = self.success(self.run_cli("plan", "--inventory", directory / "inventory.json",
                                       "--target", "refs/heads/main", "--format", "json"))
        self.assertEqual(timeless(review), timeless(old), "saved original inventory cannot replay equivalent substantive facts")

    def test_filters_target_map_grouping_and_context_retain_full_source_and_match_plan(self):
        inventory, _ = self.old_flow()
        family = inventory["families"][0]
        mapping = self.save({"schema": "wt-sweep.targets/1", "families": [{"common_dir": family["common_dir"],
                            "identity": family["identity"], "targets": [{"hex": b"refs/heads/main".hex()}]}]}, "targets")
        extras = ("--filter-repo", self.fixture.principal, "--path-prefix", self.fixture.linked,
                  "--branch", "refs/heads/topic", "--local-state", "clean", "--verdict", "cleanup_candidate",
                  "--group-by", "directory")
        for context in (("--changelog-path", "tracked.txt"), ("--no-context",)):
            with self.subTest(context=context):
                saved = self.save(inventory, "input")
                old = self.success(self.run_cli("plan", "--inventory", saved, "--target-map", mapping,
                                               "--format", "json", *extras, *context))
                new = self.success(self.run_cli("review", "--root", self.fixture.root, "--target-map", mapping,
                                               "--format", "json", *extras, *context))
                self.items(new)
                self.assertEqual(set(map(decode_path, new["selected_paths"])), {os.fsencode(self.fixture.linked)})
                self.assertEqual(len(new["row_bindings"]), 4)
                self.assertEqual(new["group_by"], "directory")
                # Equivalent source requires equivalent scan scopes, not Repo vs Roots coverage.
                _, matching = self.old_flow(("--root", self.fixture.root), extra=(*extras, *context))
                self.assertEqual(timeless(new), timeless(matching))
                self.assertEqual(timeless(self.source(new)["items"]), timeless(self.source(old)["items"]))
                if "--no-context" in context:
                    self.assertIsNone(self.source(new).get("context"))
                else:
                    self.assertIsNotNone(self.source(new).get("context"))

    def test_targeted_collection_keeps_full_inventory_and_explicit_uncollected_rows(self):
        before = filesystem_manifest(self.fixture.root)
        document = self.review("--worktree", self.fixture.linked, "--path-prefix", self.fixture.linked,
                               "--run-dir", self.output)
        source = self.source(document)
        items = self.items(document)
        self.assertEqual(set(items), set(self.expected))
        chosen = items[os.fsencode(self.fixture.linked)]
        self.assertEqual(chosen["classification"], "cleanup_candidate")
        for path, item in items.items():
            if path == os.fsencode(self.fixture.linked):
                continue
            self.assertIsNone(item["local_summary"])
            self.assertIsNone(item["private_history"])
            self.assertEqual(item["history"], [])
            self.assertIn("Not selected for evidence collection (--worktree).", item["unknowns"])
            self.assertFalse(any("budget reached" in reason for reason in item["unknowns"]), item["unknowns"])
            self.assertNotEqual(item["classification"], "cleanup_candidate")
        self.assertEqual(set(map(decode_path, document["selected_paths"])), {os.fsencode(self.fixture.linked)})
        run = next(self.output.iterdir())
        inventory = json.loads((run / "inventory.json").read_bytes())
        self.assertEqual(source["inventory_sha256"], digest(inventory))
        self.assertEqual(len(inventory["families"][0]["worktrees"]), 4)
        self.assertEqual(before, filesystem_manifest(self.fixture.root))

    def test_targeted_collection_can_select_a_later_family(self):
        other = self.fixture.root / "zz-other-family"
        self.fixture.git("init", "-q", "-b", "main", str(other))
        (other / "other.txt").write_bytes(b"separate family content")
        self.fixture.git("-C", str(other), "add", ".")
        self.fixture.git("-C", str(other), "commit", "-qm", "other")
        selected = self.fixture.root / "zz-other-linked"
        self.fixture.git("-C", str(other), "worktree", "add", "-b", "other-topic", str(selected))
        inventory = self.success(self.run_cli("scan", "--root", self.fixture.root, "--format", "json"))
        mapping = self.save({"schema": "wt-sweep.targets/1", "families": [
            {"common_dir": f["common_dir"], "identity": f["identity"],
             "targets": [{"hex": b"refs/heads/main".hex()}]} for f in inventory["families"]
        ]}, "target-map")
        saved = self.save(inventory, "inventory")
        document = self.success(self.run_cli("plan", "--inventory", saved,
            "--target-map", mapping, "--worktree", selected, "--format", "json"))
        source = self.source(document)
        rows = {decode_path(i["path"]): i for i in source["items"]}
        self.assertEqual(len(rows), 6)
        self.assertEqual(rows[os.fsencode(selected)]["classification"], "cleanup_candidate")
        self.assertEqual(source["history_metrics"]["first_pass_items"], 1)
        self.assertEqual(source["inventory_sha256"], digest(inventory))
        self.assertTrue(all(i["local_summary"] is None for p, i in rows.items() if p != os.fsencode(selected)))

    def test_targeted_collection_marks_unselected_rows_even_after_identity_drift(self):
        inventory = self.success(self.run_cli("scan", "--repo", self.fixture.principal, "--format", "json"))
        saved = self.save(inventory, "inventory")
        self.fixture.git("-C", str(self.dirty), "add", ".")
        self.fixture.git("-C", str(self.dirty), "commit", "-qm", "changed after inventory")
        result = self.run_cli("plan", "--inventory", saved, "--target", "refs/heads/main",
                             "--worktree", self.fixture.linked, "--summary", "--run-dir", self.output)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertIn(b"Targeted collection: 1 worktree(s); 3 not requested", result.stdout)
        report = next(self.output.glob("run-*/review.json"))
        source = self.source(json.loads(report.read_bytes()))
        for item in source["items"]:
            if decode_path(item["path"]) == os.fsencode(self.fixture.linked):
                continue
            self.assertIn("Not selected for evidence collection (--worktree).", item["unknowns"])
            self.assertFalse(any("local summary byte budget reached" in reason for reason in item["unknowns"]))

    def test_unknown_targeted_path_refuses_before_evidence_collection(self):
        result = self.run_cli("review", "--repo", self.fixture.principal,
                              "--worktree", self.fixture.root / "not-registered", "--summary")
        self.assertEqual(result.returncode, 2)
        self.assertIn(b"worktree must match exactly one recorded path", result.stderr)
        self.assertNotIn(b"Checking local files", result.stderr)

    def test_offline_forge_positive_is_preserved_by_fresh_review(self):
        self.fixture.git("-C", self.fixture.principal, "config", "remote.origin.url",
                         "https://forge.example.invalid/team/project.git")
        raw = self.save([{"id": 901, "iid": 9, "project_id": 17, "source_project_id": 17,
                          "target_project_id": 17, "source_branch": "topic", "target_branch": "main",
                          "state": "opened", "sha": self.fixture.base, "updated_at": "2026-01-01T00:00:00Z",
                          "web_url": "https://forge.example.invalid/team/project/-/merge_requests/9"}], "raw-forge")
        forge = self.output / "normalized-forge.json"
        result = self.run_cli("forge", "ingerer", "--raw", raw, "--out", forge)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        _, old = self.old_flow(extra=("--forge", forge))
        new = self.review("--forge", forge)
        item = self.items(new)[os.fsencode(self.fixture.linked)]
        self.assertEqual(item["classification"], "keep")
        self.assertIn("open_request", item["decision"]["keep_reasons"])
        self.assertEqual(len(item["requests"]), 1)
        self.assertFalse(self.source(new)["forge"]["coverage"]["absence_proven"])
        self.assertEqual(timeless(new), timeless(old))

    def test_summary_outside_publication_and_source_containment(self):
        summary = self.run_cli("review", "--repo", self.fixture.principal,
                               "--target", "refs/heads/main", "--summary")
        self.assertEqual(summary.returncode, 0, summary.stderr.decode(errors="replace"))
        self.assertGreater(len(summary.stdout.splitlines()), 0)
        self.assertLessEqual(len(summary.stdout.splitlines()), 60)
        outside = self.output / "standalone-review.json"
        result = self.run_cli("review", "--repo", self.fixture.principal,
                              "--target", "refs/heads/main", "--out", outside)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assert_verdicts(json.loads(outside.read_bytes()))
        previous = filesystem_manifest(outside)
        repeated = self.run_cli("review", "--repo", self.fixture.principal, "--target", "refs/heads/main", "--out", outside)
        self.assertNotEqual(repeated.returncode, 0, "existing review output replaced")
        self.assertEqual(previous, filesystem_manifest(outside))
        for destination in (self.fixture.principal / "forbidden.json", self.fixture.linked / "forbidden.json"):
            result = self.run_cli("review", "--repo", self.fixture.principal,
                                  "--target", "refs/heads/main", "--out", destination)
            self.assertNotEqual(result.returncode, 0, "fresh review published inside source family")
            self.assertFalse(destination.exists())

    def test_command_help_needs_no_scope_and_emits_no_discovery_progress(self):
        for command in (("scan",), ("plan",), ("review",), ("forge",),
                        ("forge", "commandes"), ("forge", "ingerer")):
            with self.subTest(command=command):
                result = self.run_cli(*command, "--help", timeout=5, cwd=self.fixture.linked)
                self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
                self.assertTrue(result.stdout.strip(), "command help produced no content")
                self.assertIn(command[-1].encode(), result.stdout)
                self.assertNotIn(b"Discovering repositories", result.stderr)
                self.assertNotIn(b"Checking worktree", result.stderr)

    def test_invalid_pairs_refuse_before_discovery_with_valid_review_control(self):
        self.assert_verdicts(self.review())  # Always-refuse parser cannot pass this method.
        no_scope = self.run_cli("review")
        self.assertEqual(no_scope.returncode, 2)
        self.assertEqual(no_scope.stdout, b"")
        self.assertNotIn(b"Discovering repositories", no_scope.stderr, "missing scope started discovery")
        pairs = (("--root", self.fixture.root), ("--all",), ("--repo", self.fixture.linked),
                 ("--inventory", self.output / "missing.json"),
                 ("--local",), ("--summary", "--format", "json"),
                 ("--target", "refs/heads/main", "--target-map", self.output / "missing-targets.json"),
                 ("--out", self.output / "forbidden.json", "--run-dir", self.output), ("--workers", "0"),
                 ("--no-context", "--changelog-path", "tracked.txt"))
        before = filesystem_manifest(self.output)
        for pair in pairs:
            with self.subTest(pair=pair):
                result = self.run_cli("review", "--repo", self.fixture.principal, *pair)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, b"")
                self.assertTrue(result.stderr.strip())
                self.assertNotIn(b"Discovering repositories", result.stderr, "invalid options started filesystem discovery")
                self.assertNotIn(b"Checking worktree", result.stderr, "invalid options started review collection")
        self.assertEqual(before, filesystem_manifest(self.output), "invalid options created or changed outputs")

    def test_old_review_archive_refuses_plan_with_fresh_scan_diagnostic_and_no_new_C5(self):
        _, current = self.old_flow()
        archive = dict(current)
        archive["schema"] = "wt-sweep.review/1"
        path = self.save(archive, "old-review")
        before = filesystem_manifest(path)
        result = self.run_cli("plan", "--inventory", path, "--target", "refs/heads/main", "--format", "json")
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"", "old archive received current review/C5 output")
        diagnostic = result.stderr.decode(errors="replace").lower()
        self.assertIn("review/1", diagnostic, "old review schema received only generic parse failure")
        self.assertRegex(diagnostic, r"rescan|fresh scan", "old archive diagnostic lacks fresh-input advice")
        self.assertEqual(before, filesystem_manifest(path), "old archive bytes were altered")

    def test_review_sigint_direct_pid_reaps_owned_git_child_with_partial_or_binding_refusal(self):
        # Reuse proven T08 selector/cleanup methods without changing its BINARY
        # or other globals, and without importing a TestCase as a module member.
        helpers = discovery_oracles.DiscoveryPublicationTests
        self.owned_children = set()
        try:
            helpers.process_selector_preflight(self)
        except self.failureException as error:
            self.skipTest("owned process-selector preflight unavailable: " + str(error))
        config = self.fixture.principal / ".git/config"
        config.unlink()
        os.mkfifo(config)  # Owned fixture only; preflight succeeded before launch.
        before = filesystem_manifest(self.fixture.root)
        process = subprocess.Popen([str(BINARY), "review", "--repo", str(self.fixture.principal),
                                    "--target", "refs/heads/main", "--format", "json"],
                                   env=self.fixture.env, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
        try:
            progress = helpers.wait_for_stderr(self, process, b"Discovering repositories...")
            helpers.wait_for_git_child(self, process)
            os.kill(process.pid, signal.SIGINT)
            stdout, stderr = process.communicate(timeout=12)
            diagnostic = (progress + stderr).decode(errors="replace")
            self.assertEqual(process.returncode, 130, diagnostic)
            helpers.assert_children_gone(self)
            self.assertRegex(diagnostic, r"Discovery finished: [^\n]*stopped: interrupted")
            if stdout:
                report = self.source(json.loads(stdout))
                self.assertFalse(report["coverage"]["traversal_complete"])
                self.assertTrue(report["coverage"]["gaps"], "interrupted new entry omitted missing discovery evidence")
                self.assertEqual(report["coverage"]["remaining"]["reason"], "interrupted")
                self.assertEqual(report["coverage"]["remaining"]["cover"], "requested_scope")
            else:
                # Cancellation before family identity cannot bind a simple target.
                # Require this precise refusal, never accept arbitrary empty output.
                self.assertIn("wt-sweep: invalid_inventory (review): nonempty simple targets require exactly one "
                              "physical family; use an explicit target map", diagnostic)
        finally:
            helpers.stop(self, process)
            after = filesystem_manifest(self.fixture.root)
            changed = [path for path in before.keys() | after.keys() if before.get(path) != after.get(path)]
            self.assertFalse(changed, "review SIGINT changed source/admin: " + repr(changed[:8]))

    def raw_commit(self, tree, parents, name, literal=False):
        self.fixture.tick()
        date = self.fixture.clock
        payload = (f"tree {tree}\n" + "".join(f"parent {parent}\n" for parent in parents) +
                   f"author Fixture <fixture@example.invalid> {date} +0000\n" +
                   f"committer Fixture <fixture@example.invalid> {date} +0000\n\n{name}\n").encode()
        extra = ("--literally",) if literal else ()
        oid = self.git_input("hash-object", "-w", "-t", "commit", *extra, "--stdin", payload=payload).strip().decode()
        expected = hashlib.sha1(b"commit " + str(len(payload)).encode() + b"\0" + payload).hexdigest()
        self.assertEqual(oid, expected, "Git object does not match independent full commit hash")
        return oid

    def graph_target(self, mode):
        tree = self.fixture.oid(self.fixture.linked, "HEAD^{tree}")
        if mode == "negative":
            target = self.raw_commit(tree, (), "Complete unrelated target")
        else:
            self.assertNotEqual(self.fixture.base, self.fixture.base.upper(), "fixture OID lacks alphabetic hex")
            target = self.raw_commit(tree, (self.fixture.base.upper(),), "Uppercase valid parent")
            if mode == "mixed_chain":
                mixed = "".join(c.upper() if index % 2 else c.lower() for index, c in enumerate(target))
                target = self.raw_commit(tree, (mixed,), "Mixed-case valid intermediate parent")
        self.fixture.git("-C", self.fixture.principal, "update-ref", "refs/heads/main", target)
        self.fixture.heads[os.fsencode(self.fixture.principal)] = target
        self.git_input("fsck", "--no-reflogs", "--no-dangling")
        expected = 1 if mode == "negative" else 0
        self.git_input("merge-base", "--is-ancestor", self.fixture.base, target, expected=expected)
        exclusive = self.git_input("rev-list", self.fixture.base, "--not", target).splitlines()
        self.assertEqual(exclusive, [self.fixture.base.encode()] if mode == "negative" else [])
        return target

    def test_valid_uppercase_parent_is_ancestor_and_never_keep_for_nonintegration(self):
        target = self.graph_target("uppercase")
        _, report = self.old_flow()  # Shared parser regression is independent of new command availability.
        item = self.items(report)[os.fsencode(self.fixture.linked)]
        record = next(r for r in item["history"] if r["target_oid"] == target)
        self.assertEqual(record["result"], "ancestor", "valid uppercase parent produced false complete negative ancestry")
        self.assertIsNone(record["stop_reason"])
        self.assertNotIn("head_not_integrated", item["decision"]["keep_reasons"])
        self.assertEqual(item["classification"], "cleanup_candidate")

    def test_valid_mixed_case_intermediate_chain_uses_one_graph_identity(self):
        target = self.graph_target("mixed_chain")
        _, report = self.old_flow()
        item = self.items(report)[os.fsencode(self.fixture.linked)]
        record = next(r for r in item["history"] if r["target_oid"] == target)
        self.assertEqual(record["result"], "ancestor", "mixed-case full OIDs split graph/cache identity")
        self.assertIsNone(record["stop_reason"])
        self.assertNotIn("head_not_integrated", item["decision"]["keep_reasons"])

    def test_complete_valid_negative_remains_explicit_not_ancestor_keep(self):
        target = self.graph_target("negative")
        _, report = self.old_flow()
        item = self.items(report)[os.fsencode(self.fixture.linked)]
        record = next(r for r in item["history"] if r["target_oid"] == target)
        self.assertEqual(record["result"], "not_ancestor", "OID normalization invented positive ancestry")
        self.assertIsNone(record["stop_reason"])
        self.assertEqual(item["classification"], "keep")
        self.assertIn("head_not_integrated", item["decision"]["keep_reasons"])

    def test_malformed_parent_header_is_unknown_with_stop_reason_not_complete_negative(self):
        tree = self.fixture.oid(self.fixture.linked, "HEAD^{tree}")
        date = 1700000200
        payload = (f"tree {tree}\nparent\t{self.fixture.base}\n" +
                   f"author Fixture <fixture@example.invalid> {date} +0000\n" +
                   f"committer Fixture <fixture@example.invalid> {date} +0000\n\nMalformed structural parent\n").encode()
        target = self.git_input("hash-object", "-w", "-t", "commit", "--literally", "--stdin", payload=payload).strip().decode()
        self.assertEqual(target, hashlib.sha1(b"commit " + str(len(payload)).encode() + b"\0" + payload).hexdigest(),
                         "malformed structural fixture must still have a valid full content hash")
        self.fixture.git("-C", self.fixture.principal, "update-ref", "refs/heads/main", target)
        self.fixture.heads[os.fsencode(self.fixture.principal)] = target
        fsck = self.owned_process([GIT, "-C", self.fixture.principal, "fsck", "--no-reflogs", "--no-dangling"], timeout=15)
        self.assertNotEqual(fsck.returncode, 0, "malformed fixture unexpectedly passed independent Git structure validation")
        _, report = self.old_flow()
        item = self.items(report)[os.fsencode(self.fixture.linked)]
        record = next(r for r in item["history"] if r["target_oid"] == target)
        self.assertEqual(record["result"], "unknown", "malformed parent became a complete negative claim")
        self.assertTrue(record["stop_reason"], "unknown graph refusal omitted its reason")
        self.assertNotIn("head_not_integrated", item["decision"]["keep_reasons"])
        self.assertEqual(item["classification"], "verify", "malformed target was classified as established positive work or integration")


if __name__ == "__main__":
    unittest.main()
