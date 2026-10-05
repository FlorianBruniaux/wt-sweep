"""Independent U3 relation oracles from disposable real Git histories."""

import json
import hashlib
import os
import subprocess
import unittest
import zlib

from inventory_support import BINARY, GIT, FamilyFixture, HistoryFixture, decode_path


class ReviewTests(unittest.TestCase):
    def setUp(self):
        self.fixture = FamilyFixture()
        self.addCleanup(self.fixture.close)
        self.forge_sequence = 0

    def new_history(self, files=None, object_format=None):
        self.fixture = HistoryFixture(files, object_format)
        self.addCleanup(self.fixture.close)
        return self.fixture

    def plan(self, targets=("refs/heads/main",), inventory=None, extra=(), env=None):
        if inventory is None:
            scan = self.fixture.scan("--format", "json")
            self.assertEqual(scan.returncode, 0, scan.stderr.decode(errors="replace"))
            inventory = json.loads(scan.stdout)
        inventory_file = self.fixture.root / "inventory.json"
        inventory_file.write_text(json.dumps(inventory))
        self.expected_inventory_digest = canonical_digest(inventory)
        args = [str(BINARY), "plan", "--inventory", str(inventory_file), "--format", "json"]
        for target in targets:
            args.extend(("--target", target))
        before = self.fixture.target_manifest()
        result = subprocess.run([*args, *extra], env=env or self.fixture.env, capture_output=True, timeout=30)
        after = self.fixture.target_manifest()
        changed = [path for root in before.keys() | after.keys()
                   for path in before.get(root, {}).keys() | after.get(root, {}).keys()
                   if before.get(root, {}).get(path) != after.get(root, {}).get(path)]
        self.assertFalse(changed, "plan modified target/admin manifest (atime excluded): " + repr(changed[:8]))
        return result

    def report(self, result):
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        report = json.loads(result.stdout)
        self.assertEqual(report["schema"], "wt-sweep.review/1")
        self.assertEqual(report["inventory_sha256"], self.expected_inventory_digest)
        claimed = report["digest_sha256"]
        body = {key: value for key, value in report.items() if key != "digest_sha256"}
        self.assertEqual(claimed, canonical_digest(body), "review digest does not bind the report")
        actual = {decode_path(item["path"]): item for item in report["items"]}
        self.assertEqual(len(actual), len(report["items"]), "duplicate review identity")
        self.assertEqual(set(actual), set(self.fixture.expected), "omitted or invented review worktree")
        return report, actual

    def assert_not_integrated(self, item):
        self.assertEqual(item["classification"], "to_triage")
        self.assertFalse(any(observation["target_role"] == "integration"
                             for observation in item["observations"]), "unsupported integration proof")

    def test_explicit_target_has_nonempty_ancestor_observation(self):
        self.fixture.git("-C", str(self.fixture.principal), "merge", "--no-ff", "topic", "-m", "Integrate fixture")
        target = self.fixture.git("-C", str(self.fixture.principal), "rev-parse", "refs/heads/main").stdout.decode().strip()
        _, items = self.report(self.plan())
        item = items[os.fsencode(self.fixture.linked)]
        self.assertEqual(item["classification"], "integration_observed", "always-triage implementation")
        self.assertTrue(any(observation["relation"] == "ancestor"
                            and observation["source_oid"] == self.fixture.heads[os.fsencode(self.fixture.linked)]
                            and observation["target_oid"] == target
                            and decode_path(observation["target_ref"]) == b"refs/heads/main"
                            and observation["target_role"] == "integration"
                            for observation in item["observations"]), "missing independently expected target relation")

    def test_identical_commit_and_identical_tree_have_explicit_target_oids(self):
        fixture = self.new_history()
        _, items = self.report(self.plan())
        source = items[os.fsencode(fixture.linked)]
        self.assertEqual(source["classification"], "integration_observed")
        self.assertTrue(any(observation["relation"] == "identical_commit"
                            and observation["source_oid"] == fixture.base
                            and observation["target_oid"] == fixture.base
                            for observation in source["observations"]))
        source_oid = fixture.write_commit(fixture.linked, "tracked.txt", b"identical content\n")
        target_oid = fixture.write_commit(fixture.principal, "tracked.txt", b"identical content\n")
        self.assertNotEqual(source_oid, target_oid, "fixture accidentally recreated the same commit")
        _, items = self.report(self.plan())
        source = items[os.fsencode(fixture.linked)]
        self.assertEqual(source["classification"], "integration_observed")
        self.assertTrue(any(observation["relation"] == "identical_tree"
                            and observation["source_oid"] == source_oid
                            and observation["target_oid"] == target_oid
                            and decode_path(observation["target_ref"]) == b"refs/heads/main"
                            for observation in source["observations"]))

    def test_no_explicit_target_preserves_intermediate_relations_and_forge_unknown(self):
        fixture = self.new_history()
        _, items = self.report(self.plan(targets=()))
        source = items[os.fsencode(fixture.linked)]
        self.assert_not_integrated(source)
        self.assertTrue(source["observations"], "known intermediate relation was discarded")
        self.assertTrue(all(observation["target_role"] == "intermediate"
                            for observation in source["observations"]))
        self.assertTrue(source["unknowns"], "no target/forge evidence was presented as known absence")

    def test_new_commit_after_integration_is_not_inferred_complete(self):
        fixture = self.new_history()
        fixture.write_commit(fixture.linked, "tracked.txt", b"first work\n")
        fixture.tick()
        fixture.git("-C", str(fixture.principal), "merge", "--no-ff", "topic", "-m", "Integrate old head")
        fixture.write_commit(fixture.linked, "tracked.txt", b"new unintegrated work\n")
        _, items = self.report(self.plan())
        self.assert_not_integrated(items[os.fsencode(fixture.linked)])

    def test_replace_object_cannot_forge_ancestry_or_tree_identity(self):
        fixture = self.new_history()
        source_oid = fixture.write_commit(fixture.linked, "tracked.txt", b"unique source\n")
        target_oid = fixture.write_commit(fixture.principal, "tracked.txt", b"different target\n")
        fixture.git("-C", str(fixture.principal), "replace", target_oid, source_oid)
        _, items = self.report(self.plan())
        self.assert_not_integrated(items[os.fsencode(fixture.linked)])

    def test_gitlink_changes_are_not_hidden_by_submodule_ignore(self):
        fixture = self.new_history()
        (fixture.principal / ".gitmodules").write_bytes(b'[submodule "s"]\n path = s\n url = /irrelevant\n')
        fixture.git("-C", str(fixture.principal), "update-index", "--add", "--cacheinfo", "160000," + fixture.base + ",s")
        fixture.commit(fixture.principal, "Base gitlink", ".gitmodules")
        target = fixture.oid(fixture.principal)
        fixture.git("-C", str(fixture.linked), "reset", "--hard", target)
        fixture.git("-C", str(fixture.linked), "update-index", "--cacheinfo", "160000," + target + ",s")
        fixture.tick()
        fixture.git("-C", str(fixture.linked), "commit", "-m", "Unique gitlink")
        fixture.git("-C", str(fixture.principal), "config", "submodule.s.ignore", "all")
        _, items = self.report(self.plan())
        self.assert_not_integrated(items[os.fsencode(fixture.linked)])

    def test_missing_commit_object_is_unknown_not_empty_integration(self):
        fixture = self.new_history()
        source = fixture.write_commit(fixture.linked, "tracked.txt", b"unique source\n")
        scan = fixture.scan("--format", "json")
        self.assertEqual(scan.returncode, 0)
        inventory = json.loads(scan.stdout)
        object_file = fixture.principal / ".git" / "objects" / source[:2] / source[2:]
        object_file.unlink()  # Disposable synthetic object only; never a development repository.
        result = self.plan(inventory=inventory)
        _, items = self.report(result)
        item = items[os.fsencode(fixture.linked)]
        self.assert_not_integrated(item)
        self.assertTrue(item["unknowns"], "missing source object concealed")

    def test_missing_blob_cannot_support_an_identical_tree_observation(self):
        fixture = self.new_history()
        source = fixture.write_commit(fixture.linked, "tracked.txt", b"same tree but unavailable blob\n")
        target = fixture.write_commit(fixture.principal, "tracked.txt", b"same tree but unavailable blob\n")
        self.assertNotEqual(source, target)
        self.assertEqual(fixture.oid(fixture.linked, "HEAD^{tree}"),
                         fixture.oid(fixture.principal, "HEAD^{tree}"))
        blob = fixture.oid(fixture.linked, "HEAD:tracked.txt")
        scan = fixture.scan("--format", "json")
        self.assertEqual(scan.returncode, 0)
        inventory = json.loads(scan.stdout)
        (fixture.principal / ".git" / "objects" / blob[:2] / blob[2:]).unlink()
        _, items = self.report(self.plan(inventory=inventory))
        item = items[os.fsencode(fixture.linked)]
        self.assert_not_integrated(item)
        self.assertTrue(item["unknowns"], "tree OID equality hid an unavailable referenced blob")

    def test_inventory_head_and_path_identity_drift_are_unknown(self):
        fixture = self.new_history()
        scan = fixture.scan("--format", "json")
        self.assertEqual(scan.returncode, 0)
        inventory = json.loads(scan.stdout)
        fixture.write_commit(fixture.linked, "tracked.txt", b"late head\n")
        _, items = self.report(self.plan(inventory=inventory))
        item = items[os.fsencode(fixture.linked)]
        self.assert_not_integrated(item)
        self.assertTrue(item["unknowns"], "HEAD drift hidden")
        retained = fixture.root / "retained-source"
        fixture.linked.rename(retained)
        fixture.manifest_roots.add(os.fsencode(retained))
        fixture.linked.mkdir()
        _, items = self.report(self.plan(inventory=inventory))
        item = items[os.fsencode(fixture.linked)]
        self.assert_not_integrated(item)
        self.assertTrue(item["unknowns"], "directory identity replacement hidden")

    def test_suggestions_and_action_fields_are_refused(self):
        scan = self.fixture.scan("--format", "json")
        self.assertEqual(scan.returncode, 0)
        inventory = json.loads(scan.stdout)
        inventory["actions"] = [{"delete": "synthetic"}]
        result = self.plan(inventory=inventory)
        self.assertNotEqual(result.returncode, 0, "accepted action fields in a review input")
        result = self.plan(extra=("--suggestions", str(self.fixture.root / "missing-suggestions.json")))
        self.assertNotEqual(result.returncode, 0, "accepted suggestions as judgement authority")

    def test_s1_to_s9_rewritten_history_is_not_commit_integration_proof(self):
        # Consolidated U3 forbids the old squash/patch/simulated-merge cascade.
        # Each target below differs in both history and final tree from source.
        for scenario in range(1, 10):
            with self.subTest(scenario=f"S{scenario}"):
                lines = [str(number).encode() for number in range(1, 41)]
                original = b"\n".join(lines) + b"\n"
                fixture = self.new_history({"a.txt": original, "c.txt": b"x\n"})
                first = fixture.write_commit(fixture.linked, "a.txt", original.replace(b"\n10\n", b"\n10-feat\n"))
                second_bytes = (fixture.linked / "a.txt").read_bytes().replace(b"\n20\n", b"\n20-feat\n")
                second = fixture.write_commit(fixture.linked, "a.txt", second_bytes)
                third = fixture.write_commit(fixture.linked, "d.txt", b"new feature file\n")
                if scenario in (1, 6):
                    fixture.write_commit(fixture.principal, "c.txt", b"x\ny\n")
                if scenario == 3:
                    fixture.write_commit(fixture.principal, "a.txt", original.replace(b"\n35\n", b"\n35-main\n"))
                    fixture.tick()
                    fixture.git("-C", str(fixture.principal), "cherry-pick", first, second, third)
                elif scenario in (7, 9):
                    fixture.tick()
                    fixture.git("-C", str(fixture.principal), "cherry-pick", first)
                    if scenario == 9:
                        payload = (fixture.principal / "a.txt").read_bytes().replace(b"\n20\n", b"\n  20-feat\n")
                        fixture.write_commit(fixture.principal, "a.txt", payload)
                        fixture.tick()
                        fixture.git("-C", str(fixture.principal), "cherry-pick", third)
                elif scenario != 6:
                    payload = second_bytes
                    if scenario == 4:
                        payload = payload.replace(b"\n12\n", b"\n12-main\n")
                    if scenario == 5:
                        payload = payload.replace(b"20-feat", b"20-feat-reviewed")
                    if scenario == 8:
                        payload = payload.replace(b"20-feat", b"  20-feat")
                    (fixture.principal / "a.txt").write_bytes(payload)
                    (fixture.principal / "d.txt").write_bytes(b"new feature file\n")
                    fixture.commit(fixture.principal, "Synthetic squash", "a.txt", "d.txt")
                    if scenario in (2, 4):
                        payload = (fixture.principal / "a.txt").read_bytes().replace(b"10-feat", b"10-main-retouch")
                        fixture.write_commit(fixture.principal, "a.txt", payload)
                source_tree = fixture.oid(fixture.linked, "HEAD^{tree}")
                target_tree = fixture.oid(fixture.principal, "HEAD^{tree}")
                self.assertNotEqual(source_tree, target_tree, "fixture does not distinguish source content")
                _, items = self.report(self.plan())
                self.assert_not_integrated(items[os.fsencode(fixture.linked)])

    def test_equal_patch_ids_in_different_repeated_blocks_are_not_proof(self):
        lines = [b"anchor A"] + [b"padding"] * 10 + [b"old"] + [b"padding"] * 10
        lines += [b"anchor B"] + [b"padding"] * 10 + [b"old"] + [b"padding"] * 10 + [b"end"]
        fixture = self.new_history({"tracked.txt": b"\n".join(lines) + b"\n"})
        source_lines, target_lines = list(lines), list(lines)
        source_lines[33] = b"new"
        target_lines[11] = b"new"
        source = fixture.write_commit(fixture.linked, "tracked.txt", b"\n".join(source_lines) + b"\n")
        target = fixture.write_commit(fixture.principal, "tracked.txt", b"\n".join(target_lines) + b"\n")
        patch_ids = []
        for oid in (source, target):
            diff = fixture.git("-C", str(fixture.principal), "diff-tree", "-r", "-p", fixture.base, oid)
            self.assertTrue(diff.stdout, "counterexample unexpectedly has an empty patch")
            patch = subprocess.run([GIT, "patch-id", "--verbatim"], input=diff.stdout, env=fixture.env,
                                   capture_output=True, check=True, timeout=15)
            patch_ids.append(patch.stdout.split()[0])
        self.assertEqual(patch_ids[0], patch_ids[1], "counterexample does not have equal patch IDs")
        _, items = self.report(self.plan())
        self.assert_not_integrated(items[os.fsencode(fixture.linked)])

    def test_fsmonitor_filters_and_merge_driver_remain_inactive(self):
        (self.fixture.principal / "tracked.txt").write_bytes(b"target conflict\n")
        self.fixture.git("-C", str(self.fixture.principal), "add", "--", "tracked.txt")
        self.fixture.git("-C", str(self.fixture.principal), "commit", "-m", "Divergent target")
        self.fixture.install_witnesses()
        for path in (self.fixture.principal, self.fixture.linked, self.fixture.detached):
            (path / ".gitattributes").write_bytes(b"*.txt filter=witness merge=keep\n")
        driver = "printf 'merge driver executed\\n' >> " + str(self.fixture.witness_marker) + "; true"
        self.fixture.git("-C", str(self.fixture.principal), "config", "merge.keep.driver", driver)
        _, items = self.report(self.plan(env=dict(self.fixture.env, TMPDIR=str(self.fixture.principal / ".git"))))
        self.assert_not_integrated(items[os.fsencode(self.fixture.linked)])
        self.assertFalse(self.fixture.witness_marker.exists(), "Git executable configuration ran during plan")

    def test_grafts_cannot_forge_ancestry(self):
        fixture = self.new_history()
        source = fixture.write_commit(fixture.linked, "tracked.txt", b"unique source\n")
        target = fixture.write_commit(fixture.principal, "tracked.txt", b"different target\n")
        (fixture.principal / ".git" / "info" / "grafts").write_text(target + " " + source + "\n")
        _, items = self.report(self.plan())
        self.assert_not_integrated(items[os.fsencode(fixture.linked)])

    def test_shallow_marker_does_not_discard_proven_raw_object_ancestry(self):
        fixture = self.new_history()
        fixture.write_commit(fixture.linked, "tracked.txt", b"integrated source\n")
        fixture.write_commit(fixture.principal, "main-only.txt", b"target has additional data\n")
        fixture.tick()
        fixture.git("-C", str(fixture.principal), "merge", "--no-ff", "topic", "-m", "Merge")
        target = fixture.oid(fixture.principal)
        (fixture.principal / ".git" / "shallow").write_text(target + "\n")
        _, items = self.report(self.plan())
        item = items[os.fsencode(fixture.linked)]
        self.assertEqual(item["classification"], "integration_observed")
        self.assertTrue(any(observation["relation"] == "ancestor"
                            and observation["target_oid"] == target
                            and observation["target_role"] == "integration"
                            for observation in item["observations"]), "actual raw parents were ignored")

    def test_shallow_missing_parent_is_unknown_not_fabricated_ancestry(self):
        fixture = self.new_history()
        fixture.write_commit(fixture.linked, "tracked.txt", b"integrated source\n")
        fixture.write_commit(fixture.principal, "main-only.txt", b"target has additional data\n")
        fixture.tick()
        fixture.git("-C", str(fixture.principal), "merge", "--no-ff", "topic", "-m", "Merge")
        parent = fixture.oid(fixture.principal)
        target = fixture.write_commit(fixture.principal, "main-only.txt", b"additional target work\n")
        (fixture.principal / ".git" / "shallow").write_text(target + "\n")
        scan = fixture.scan("--format", "json")
        self.assertEqual(scan.returncode, 0)
        inventory = json.loads(scan.stdout)
        (fixture.principal / ".git" / "objects" / parent[:2] / parent[2:]).unlink()
        _, items = self.report(self.plan(inventory=inventory))
        item = items[os.fsencode(fixture.linked)]
        self.assert_not_integrated(item)
        self.assertTrue(item["unknowns"], "missing raw parent object concealed by shallow traversal")

    def test_recent_worktree_without_scoped_open_request_is_not_active(self):
        fixture = self.new_history()
        fixture.write_commit(fixture.linked, "tracked.txt", b"recent unique work\n")
        _, items = self.report(self.plan())
        self.assert_not_integrated(items[os.fsencode(fixture.linked)])

    def import_forge(self, requests):
        self.forge_sequence += 1
        raw = self.fixture.root / f"raw-forge-{self.forge_sequence}.json"
        export = self.fixture.root / f"normalized-forge-{self.forge_sequence}.json"
        raw.write_text(json.dumps(requests))
        before = self.fixture.target_manifest()
        result = subprocess.run([str(BINARY), "forge", "ingerer", "--raw", str(raw), "--out", str(export)],
                                env=self.fixture.env, capture_output=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assertEqual(self.fixture.target_manifest(), before)
        normalized = json.loads(export.read_bytes())
        self.assertEqual(normalized["schema"], "wt-sweep.forge/1")
        self.assertTrue(normalized["requests"], "positive request dropped from partial export")
        self.assertFalse(normalized["coverage"]["pagination_complete"])
        self.assertFalse(normalized["coverage"]["absence_proven"])
        return raw, export

    def request(self, state="opened"):
        return {"id": 101, "iid": 1, "project_id": 17,
                "source_project_id": 17, "target_project_id": 17,
                "source_branch": "topic", "target_branch": "main", "state": state,
                "sha": self.fixture.git("-C", str(self.fixture.linked), "rev-parse", "HEAD").stdout.decode().strip(),
                "updated_at": "2026-01-01T00:00:00Z",
                "web_url": "https://forge.example.invalid/team/project/-/merge_requests/1"}

    def test_scoped_open_request_overrides_integration_even_in_partial_export(self):
        self.fixture.git("-C", str(self.fixture.principal), "config", "remote.origin.url",
                         "https://forge.example.invalid/team/project.git")
        self.fixture.git("-C", str(self.fixture.principal), "merge", "--no-ff", "topic", "-m", "Merge old work")
        _, export = self.import_forge([self.request()])
        report, items = self.report(self.plan(extra=("--forge", str(export))))
        item = items[os.fsencode(self.fixture.linked)]
        self.assertEqual(item["classification"], "active", "scoped open request lost to local integration")
        self.assertTrue(item["requests"], "matching positive request omitted")
        self.assertTrue(any(observation["target_role"] == "integration" for observation in item["observations"]))
        self.assertFalse(report["forge"]["coverage"]["absence_proven"], "partial export became proof of absence")

    def test_contradictory_raw_project_or_head_cannot_establish_activity(self):
        self.fixture.git("-C", str(self.fixture.principal), "config", "remote.origin.url",
                         "https://forge.example.invalid/team/project.git")
        for contradiction in ("project", "head", "both"):
            with self.subTest(contradiction=contradiction):
                request = self.request()
                if contradiction in ("project", "both"):
                    request["project_id"] = 18  # Target URL and target_project_id still say 17.
                if contradiction in ("head", "both"):
                    request["diff_refs"] = {"head_sha": self.fixture.head}
                    self.assertNotEqual(request["sha"], self.fixture.head)
                _, export = self.import_forge([request])
                _, items = self.report(self.plan(extra=("--forge", str(export))))
                item = items[os.fsencode(self.fixture.linked)]
                self.assert_not_integrated(item)
                self.assertTrue(item["unknowns"], "contradictory raw identities were silently resolved")

    def test_closed_request_source_head_is_not_an_integration_target(self):
        self.fixture.git("-C", str(self.fixture.principal), "config", "remote.origin.url",
                         "https://forge.example.invalid/team/project.git")
        for state in ("closed", "merged"):
            with self.subTest(state=state):
                _, export = self.import_forge([self.request(state=state)])
                _, items = self.report(self.plan(extra=("--forge", str(export))))
                item = items[os.fsencode(self.fixture.linked)]
                self.assert_not_integrated(item)
                self.assertTrue(item["requests"], "scoped closed/merged request positive fact was discarded")

    def test_homonymous_fork_or_different_head_open_request_is_not_active(self):
        self.fixture.git("-C", str(self.fixture.principal), "config", "remote.origin.url",
                         "https://forge.example.invalid/team/project.git")
        for mismatch in ("host", "project", "fork", "head", "branch"):
            with self.subTest(mismatch=mismatch):
                request = self.request()
                if mismatch == "host":
                    request["web_url"] = "https://other.example.invalid/team/project/-/merge_requests/1"
                elif mismatch == "project":
                    request["web_url"] = "https://forge.example.invalid/other/project/-/merge_requests/1"
                elif mismatch == "fork":
                    request["source_project_id"] = 19
                elif mismatch == "head":
                    request["sha"] = self.fixture.head
                else:
                    request["source_branch"] = "homonymous-other-branch"
                _, export = self.import_forge([request])
                _, items = self.report(self.plan(extra=("--forge", str(export))))
                self.assert_not_integrated(items[os.fsencode(self.fixture.linked)])

    def test_forged_normalized_request_cannot_override_original_raw_fact(self):
        self.fixture.git("-C", str(self.fixture.principal), "config", "remote.origin.url",
                         "https://forge.example.invalid/team/project.git")
        _, export = self.import_forge([self.request(state="closed")])
        normalized = json.loads(export.read_bytes())
        normalized["requests"][0]["state"] = "opened"
        export.write_text(json.dumps(normalized))
        result = self.plan(extra=("--forge", str(export)))
        self.assertNotEqual(result.returncode, 0, "unverified normalized forged request became authority")
        self.assertTrue(result.stderr.strip())
        self.assertFalse(result.stdout.strip(), "forged export emitted a successful review")

    def test_replaced_raw_export_cannot_reuse_normalized_provenance(self):
        self.fixture.git("-C", str(self.fixture.principal), "config", "remote.origin.url",
                         "https://forge.example.invalid/team/project.git")
        raw, export = self.import_forge([self.request()])
        raw.write_text("[]")
        result = self.plan(extra=("--forge", str(export)))
        self.assertNotEqual(result.returncode, 0, "old provenance accepted new raw bytes")
        self.assertTrue(result.stderr.strip())
        self.assertFalse(result.stdout.strip())

    def test_plan_out_persists_digest_and_table_outside_the_family(self):
        destination = self.fixture.root / "review.json"
        result = self.plan(extra=("--out", str(destination)))
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        report = json.loads(destination.read_bytes())
        claimed = report.pop("digest_sha256")
        self.assertEqual(claimed, canonical_digest(report))
        self.assertIn(claimed.encode(), result.stdout)
        self.assertIn(b"principal", result.stdout)
        forbidden = self.fixture.linked / "review.json"
        result = self.plan(extra=("--out", str(forbidden)))
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(forbidden.exists())

    def test_stale_inventory_cannot_publish_inside_a_moved_live_worktree(self):
        scan = self.fixture.scan("--format", "json")
        self.assertEqual(scan.returncode, 0)
        inventory = json.loads(scan.stdout)
        moved = self.fixture.root / "moved-live-worktree"
        self.fixture.git("-C", str(self.fixture.principal), "worktree", "move", str(self.fixture.linked), str(moved))
        self.fixture.manifest_roots.add(os.fsencode(moved))
        destination = moved / "review.json"
        result = self.plan(inventory=inventory, extra=("--out", str(destination)))
        self.assertNotEqual(result.returncode, 0, "stale inventory permitted output in a live moved worktree")
        self.assertTrue(result.stderr.strip())
        self.assertFalse(destination.exists(), "review written into a live worktree")

    def test_forged_middle_commit_body_is_unknown_not_an_ancestor_proof(self):
        fixture = self.new_history()
        source = fixture.write_commit(fixture.linked, "tracked.txt", b"unrelated source\n")
        middle = fixture.write_commit(fixture.principal, "main-only.txt", b"unrelated target middle\n")
        fixture.write_commit(fixture.principal, "main-only.txt", b"unrelated target head\n")
        scan = fixture.scan("--format", "json")
        self.assertEqual(scan.returncode, 0)
        inventory = json.loads(scan.stdout)
        object_file = fixture.principal / ".git" / "objects" / middle[:2] / middle[2:]
        original = zlib.decompress(object_file.read_bytes())
        self.assertEqual(hashlib.sha1(original).hexdigest(), middle, "fixture object started corrupt")
        _, body = original.split(b"\0", 1)
        forged = body.replace(b"parent " + fixture.base.encode() + b"\n",
                              b"parent " + source.encode() + b"\n", 1)
        self.assertNotEqual(forged, body, "fixture failed to change original parent")
        object_file.unlink()  # Replace synthetic read-only object without permission changes.
        object_file.write_bytes(zlib.compress(b"commit " + str(len(forged)).encode() + b"\0" + forged))
        _, items = self.report(self.plan(inventory=inventory))
        item = items[os.fsencode(fixture.linked)]
        self.assert_not_integrated(item)
        self.assertTrue(item["unknowns"], "corrupt intermediate object was trusted as ancestry")

    def test_corrupt_tree_content_is_unknown_even_with_a_valid_commit(self):
        fixture = self.new_history()
        fixture.write_commit(fixture.linked, "tracked.txt", b"source tree content\n")
        fixture.write_commit(fixture.principal, "tracked.txt", b"target tree content\n")
        source_tree = fixture.oid(fixture.linked, "HEAD^{tree}")
        target_tree = fixture.oid(fixture.principal, "HEAD^{tree}")
        self.assertNotEqual(source_tree, target_tree)
        objects = fixture.principal / ".git" / "objects"
        source_file = objects / source_tree[:2] / source_tree[2:]
        target_file = objects / target_tree[:2] / target_tree[2:]
        original = zlib.decompress(target_file.read_bytes())
        self.assertEqual(hashlib.sha1(original).hexdigest(), target_tree)
        replacement = source_file.read_bytes()
        target_file.unlink()
        target_file.write_bytes(replacement)
        _, items = self.report(self.plan())
        item = items[os.fsencode(fixture.linked)]
        self.assert_not_integrated(item)
        self.assertTrue(item["unknowns"], "tree bytes did not match their complete object ID")

    def test_sha256_real_objects_preserve_full_oids_and_positive_integration(self):
        fixture = self.new_history(object_format="sha256")
        source = fixture.write_commit(fixture.linked, "tracked.txt", b"same SHA256 tree\n")
        target = fixture.write_commit(fixture.principal, "tracked.txt", b"same SHA256 tree\n")
        self.assertEqual(len(source), 64)
        self.assertEqual(len(target), 64)
        self.assertNotEqual(source, target)
        _, items = self.report(self.plan())
        item = items[os.fsencode(fixture.linked)]
        self.assertEqual(item["classification"], "integration_observed")
        self.assertTrue(any(observation["relation"] == "identical_tree"
                            and observation["source_oid"] == source
                            and observation["target_oid"] == target
                            for observation in item["observations"]), "SHA256 identity shortened or refused")

    def test_unreadable_live_boundaries_refuse_out_but_keep_stdout_review_unknown(self):
        scan = self.fixture.scan("--format", "json")
        self.assertEqual(scan.returncode, 0)
        inventory = json.loads(scan.stdout)
        retained = self.fixture.root / "retained-principal"
        self.fixture.principal.rename(retained)
        self.fixture.manifest_roots.add(os.fsencode(retained))
        _, items = self.report(self.plan(inventory=inventory))
        self.assertTrue(all(item["unknowns"] for item in items.values()))
        destination = self.fixture.root / "unsafe-boundary-report.json"
        result = self.plan(inventory=inventory, extra=("--out", str(destination)))
        self.assertNotEqual(result.returncode, 0, "unknown current family boundaries allowed file publication")
        self.assertTrue(result.stderr.strip())
        self.assertFalse(destination.exists())


def canonical_digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"),
                                     ensure_ascii=False).encode()).hexdigest()


if __name__ == "__main__":
    unittest.main()
