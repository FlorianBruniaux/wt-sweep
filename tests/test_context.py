"""Independent T07 context oracles from owned Git objects and graph facts.

No production modules are imported. Every CLI invocation checks conservation
of file bytes, modes, index, administration and sentinel metadata (except atime).
"""

import hashlib
import json
import os
from pathlib import Path
import unittest
import zlib

from inventory_support import BINARY as DEFAULT_BINARY, decode_path
from test_private_history import PrivateHistoryFixture, bounded_process


BINARY = Path(os.environ.get("WT_SWEEP_CONTEXT_BINARY", str(DEFAULT_BINARY)))
DEFAULT_CHANGELOGS = (b"CHANGELOG.md", b"CHANGELOG", b"CHANGES.md", b"CHANGES", b"HISTORY.md", b"HISTORY")


def digest(value):
    body = {key: item for key, item in value.items() if key != "digest_sha256"}
    return hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":"),
                                     ensure_ascii=False).encode()).hexdigest()


class ContextFixture(PrivateHistoryFixture):
    def object(self, kind, payload):
        literal = hashlib.sha1(kind.encode() + b" " + str(len(payload)).encode() + b"\0" + payload).hexdigest()
        oid = self.git(self.repo, "hash-object", "-w", "-t", kind, "--stdin", data=payload).strip().decode()
        assert oid == literal, "fixture object must match independently computed Git SHA1"
        return oid

    def tree(self, entries):
        # Literal raw tree format avoids paths being interpreted by revision syntax.
        payload = b"".join(mode.encode() + b" " + name + b"\0" + bytes.fromhex(oid)
                           for mode, name, oid in sorted(entries, key=lambda row: row[1] + (b"/" if row[0] == "40000" else b"")))
        return self.object("tree", payload)

    def commit_tree(self, tree, parents, subject):
        self.tick()
        timestamp = str(self.clock).encode()
        payload = (b"tree " + tree.encode() + b"\n" +
                   b"".join(b"parent " + oid.encode() + b"\n" for oid in parents) +
                   b"author Fixture <fixture@example.invalid> " + timestamp + b" +0000\n" +
                   b"committer Fixture <fixture@example.invalid> " + timestamp + b" +0000\n\n" + subject + b"\n")
        return self.object("commit", payload)

    def install_head(self, tree, subject=b"Synthetic context source", parents=None):
        head = self.commit_tree(tree, parents or [self.base], subject)
        self.git(self.linked, "reset", "--hard", head)
        self.expected_heads[self.linked] = head
        return head

    def changelog(self, payload, path=b"CHANGELOG.md", mode="100644", subject=b"Synthetic context source"):
        blob = self.object("blob", payload)
        pieces = path.split(b"/")
        tree = self.tree([(mode, pieces[-1], blob)])
        leaf_tree = tree
        for piece in reversed(pieces[:-1]):
            tree = self.tree([("40000", piece, tree)])
        return self.install_head(tree, subject), tree, leaf_tree, blob

    def graph(self):
        tree = self.oid(self.repo, "HEAD^{tree}")
        left = self.commit_tree(tree, [self.base], b"Left synthetic\xff\x1b branch")
        right = self.commit_tree(tree, [self.base], b"Right synthetic branch")
        merge = self.commit_tree(tree, [left, right], b"Synthetic merge")
        head = self.commit_tree(tree, [merge], b"Synthetic head")
        target = self.commit_tree(tree, [self.base], b"Synthetic unrelated target")
        self.git(self.repo, "update-ref", "refs/heads/main", target)
        self.git(self.repo, "update-ref", "refs/heads/left-target", left)
        self.git(self.repo, "update-ref", "refs/heads/right-target", right)
        self.git(self.linked, "reset", "--hard", head)
        self.expected_heads.update({self.repo: target, self.linked: head})
        return {"left": left, "right": right, "merge": merge, "head": head, "target": target}

    def uncovered(self, head, targets):
        return set(self.git(self.repo, "rev-list", head, "--not", *targets).decode().splitlines())


class ContextFixtureFactsTests(unittest.TestCase):
    def setUp(self):
        self.f = ContextFixture()
        self.addCleanup(self.f.close)

    def test_merge_union_graph_has_hand_checked_oid_sets(self):
        f = self.f
        graph = f.graph()
        before = f.manifest()
        self.assertEqual(f.uncovered(graph["head"], [graph["target"]]),
                         {graph["head"], graph["merge"], graph["left"], graph["right"]})
        self.assertEqual(f.uncovered(graph["head"], [graph["left"], graph["right"]]),
                         {graph["head"], graph["merge"]})
        self.assertEqual(f.uncovered(graph["head"], [graph["head"]]), set())
        self.assertEqual(before, f.manifest(), "independent graph premise reads mutated fixture")

    def test_nested_raw_blob_and_tree_are_known_recorded_sources(self):
        f = self.f
        payload = b"Merged and safe to delete is only synthetic prose\nraw\xff\x1b\t\r\n"
        head, tree, leaf, blob = f.changelog(payload, b"doc/release notes/CHANGELOG.md")
        before = f.manifest()
        self.assertEqual(f.git(f.repo, "cat-file", "blob", blob), payload)
        self.assertEqual(f.oid(f.repo, head + "^{tree}"), tree)
        self.assertEqual(f.git(f.repo, "ls-tree", "-z", leaf), b"100644 blob " + blob.encode() + b"\tCHANGELOG.md\0")
        self.assertEqual(before, f.manifest(), "independent object premise reads mutated fixture")


class ContextCliTests(unittest.TestCase):
    def setUp(self):
        self.f = ContextFixture()
        self.addCleanup(self.f.close)

    def cli_result(self, *arguments, expected=0):
        f = self.f
        before = f.manifest()
        try:
            result = bounded_process([BINARY, *arguments], f.env, timeout=45)
        finally:
            self.assertEqual(before, f.manifest(),
                             "context CLI changed fixture/source/index/admin/sentinel/inventory bytes or metadata; atime excluded")
        self.assertEqual(result.returncode, expected, result.stderr.decode(errors="replace"))
        return result

    def inventory(self):
        f = self.f
        result = json.loads(self.cli_result("scan", "--repo", f.repo, "--format", "json").stdout)
        self.assertEqual(result["schema"], "wt-sweep.inventory/1")
        f.sequence += 1
        path = f.root / f"context-inventory-{f.sequence}.json"
        path.write_text(json.dumps(result))
        f.manifest_roots.add(path)
        return path

    def plan_arguments(self, inventory, targets, extra):
        arguments = ["plan", "--inventory", inventory, "--format", "json"]
        for target in targets:
            arguments.extend(("--target", target))
        return [*arguments, *extra]

    def report(self, *extra, targets=("refs/heads/main",), inventory=None):
        result = json.loads(self.cli_result(*self.plan_arguments(inventory or self.inventory(), targets, extra)).stdout)
        if result["schema"] == "wt-sweep.review-view/1":
            self.assertEqual(result["digest_sha256"], digest(result), "view digest failed to bind full context source")
            source = result["source"]
        else:
            source = result
        self.assertEqual(source["schema"], "wt-sweep.review/2")
        self.assertEqual(source["digest_sha256"], digest(source), "review digest failed to bind raw context source")
        items = {decode_path(item["path"]): item for item in source["items"]}
        self.assertEqual(set(items), {os.fsencode(path) for path in self.f.expected_heads})
        for path, oid in self.f.expected_heads.items():
            self.assertEqual(items[os.fsencode(path)]["head"], oid, "review changed recorded full OID")
            self.assertNotIn("context", items[os.fsencode(path)], "context polluted the decision-item contract")
        return result, source, items

    def context(self, source, repo=None):
        self.assertIsInstance(source.get("context"), dict, "default review omitted the required T07 optional context collection")
        context = source["context"]
        self.assertEqual(set(context), {"started_unix_ms", "finished_unix_ms", "items", "omitted_items", "gaps"})
        self.assertLessEqual(source["finished_unix_ms"], context["started_unix_ms"], "context extended decision-evidence time")
        self.assertLessEqual(context["started_unix_ms"], context["finished_unix_ms"])
        self.assertEqual(context["omitted_items"], 0)
        rows = {decode_path(row["path"]): row for row in context["items"]}
        self.assertEqual(len(rows), len(context["items"]), "duplicate context worktree")
        self.assertEqual(set(rows), {os.fsencode(path) for path in self.f.expected_heads})
        row = rows[os.fsencode(repo or self.f.linked)]
        self.assertEqual(set(row), {"path", "common_dir", "family_identity", "head_oid", "uncovered", "changelogs", "gaps", "remote"})
        remote = row["remote"]
        self.assertEqual(set(remote), {"started_unix_ms", "finished_unix_ms", "remotes", "gaps"})
        self.assertLessEqual(context["started_unix_ms"], remote["started_unix_ms"])
        self.assertLessEqual(remote["started_unix_ms"], remote["finished_unix_ms"])
        self.assertLessEqual(remote["finished_unix_ms"], context["finished_unix_ms"])
        self.assertEqual(remote["remotes"], [], "fixture has no configured remotes")
        self.assertEqual(remote["gaps"], [])
        metadata = (self.f.repo / ".git").stat()
        self.assertEqual(decode_path(row["common_dir"]), os.fsencode(self.f.repo / ".git"))
        self.assertEqual(row["family_identity"], {"device": metadata.st_dev, "inode": metadata.st_ino})
        self.assertEqual(row["head_oid"], self.f.expected_heads[repo or self.f.linked])
        return row

    def changelog(self, row, path):
        entries = {decode_path(entry["path"]): entry for entry in row["changelogs"]}
        self.assertEqual(len(entries), len(row["changelogs"]))
        self.assertIn(path, entries, "selected changelog path was silently omitted")
        entry = entries[path]
        self.assertEqual(set(entry), {"revision_oid", "path", "tree_oid", "blob_oid", "status", "blob_bytes", "excerpt", "excerpt_truncated", "gaps"})
        self.assertEqual(entry["revision_oid"], row["head_oid"], "changelog source was read from another/live revision")
        return entry

    def assert_uncovered(self, row, expected, targets):
        graph = row["uncovered"]
        self.assertEqual(set(graph), {"targets", "observed", "complete", "samples", "samples_omitted", "gaps"})
        self.assertEqual({decode_path(item["reference"]): item["oid"] for item in graph["targets"]}, targets,
                         "graph used intermediate anchor or re-resolved another target")
        self.assertTrue(graph["complete"], "complete owned graph was always refused")
        self.assertEqual(graph["observed"], len(expected), "merge count or target union membership is wrong")
        self.assertEqual({sample["oid"] for sample in graph["samples"]}, expected)
        self.assertEqual(len(graph["samples"]), len(expected), "duplicate OID samples")
        self.assertEqual(graph["samples_omitted"], 0)

    def decision_projection(self, items):
        return {path: {key: item[key] for key in ("classification", "decision", "risks", "unknowns")}
                for path, item in items.items()}

    def test_default_context_has_exact_zero_and_default_absent_paths(self):
        _, source, items = self.report()
        row = self.context(source)
        self.assert_uncovered(row, set(), {b"refs/heads/main": self.f.base})
        self.assertEqual([decode_path(entry["path"]) for entry in row["changelogs"]], list(DEFAULT_CHANGELOGS))
        for path in DEFAULT_CHANGELOGS:
            entry = self.changelog(row, path)
            self.assertEqual(entry["status"], "absent")
            self.assertIsNone(entry["blob_oid"])
            self.assertIsNone(entry["excerpt"])
        self.assertEqual(items[os.fsencode(self.f.linked)]["classification"], "cleanup_candidate")

    def test_no_context_preserves_actual_candidate_and_decision_conditions(self):
        inventory = self.inventory()
        _, on, on_items = self.report(inventory=inventory)
        self.context(on)
        _, off, off_items = self.report("--no-context", inventory=inventory)
        self.assertNotIn("context", off, "disabled context should preserve pre-context serialized contract")
        self.assertEqual(self.decision_projection(on_items), self.decision_projection(off_items), "context changed judgement")
        self.assertEqual(off_items[os.fsencode(self.f.linked)]["classification"], "cleanup_candidate", "always-refuse policy passed invariance vacuously")

    def test_no_context_flag_is_accepted_without_collecting_context(self):
        _, source, items = self.report("--no-context")
        self.assertNotIn("context", source)
        self.assertEqual(items[os.fsencode(self.f.linked)]["classification"], "cleanup_candidate")

    def test_divergent_merge_counts_once_and_subject_preserves_raw_bytes(self):
        graph = self.f.graph()
        _, source, items = self.report()
        row = self.context(source)
        expected = self.f.uncovered(graph["head"], [graph["target"]])
        self.assertEqual(expected, {graph[key] for key in ("head", "merge", "left", "right")})
        self.assert_uncovered(row, expected, {b"refs/heads/main": graph["target"]})
        samples = {sample["oid"]: sample for sample in row["uncovered"]["samples"]}
        self.assertEqual(decode_path(samples[graph["left"]]["subject"]), b"Left synthetic\xff\x1b branch")
        self.assertFalse(samples[graph["left"]]["subject_truncated"])
        self.assertEqual(items[os.fsencode(self.f.linked)]["classification"], "keep")

    def test_union_of_two_selected_targets_excludes_both_sides(self):
        graph = self.f.graph()
        refs = ("refs/heads/left-target", "refs/heads/right-target")
        _, source, _ = self.report(targets=refs)
        expected = self.f.uncovered(graph["head"], [graph["left"], graph["right"]])
        self.assertEqual(expected, {graph["head"], graph["merge"]})
        self.assert_uncovered(self.context(source), expected, {refs[0].encode(): graph["left"], refs[1].encode(): graph["right"]})

    def test_no_targets_is_unknown_with_no_nonmembership_samples(self):
        self.f.graph()
        _, source, _ = self.report(targets=())
        graph = self.context(source)["uncovered"]
        self.assertFalse(graph["complete"], "no targets became an empty covered union")
        self.assertEqual(graph["observed"], 0)
        self.assertEqual(graph["samples"], [])
        self.assertTrue(graph["gaps"])

    def test_missing_target_blocks_union_nonmembership_claims(self):
        self.f.graph()
        _, source, _ = self.report(targets=("refs/heads/main", "refs/heads/missing"))
        graph = self.context(source)["uncovered"]
        self.assertFalse(graph["complete"], "unknown target became a complete union")
        self.assertEqual(graph["observed"], 0)
        self.assertEqual(graph["samples"], [], "incomplete target closure emitted nonmembership sample")
        self.assertEqual({decode_path(item["reference"]): item["oid"] for item in graph["targets"]},
                         {b"refs/heads/main": self.f.expected_heads[self.f.repo], b"refs/heads/missing": None})

    def test_identical_target_shortcut_proves_zero_even_with_missing_other_target(self):
        _, source, items = self.report(targets=("refs/heads/main", "refs/heads/missing"))
        row = self.context(source)
        self.assert_uncovered(row, set(), {b"refs/heads/main": self.f.base, b"refs/heads/missing": None})
        self.assertEqual(items[os.fsencode(self.f.linked)]["classification"], "cleanup_candidate")

    def test_recorded_changelog_bytes_ignore_changed_live_file_and_misleading_text(self):
        payload = b"Merged. Integrated. Safe to delete. Synthetic prose only.\nraw\xff\x1b\t\r\n"
        head, _, _, blob = self.f.changelog(payload)
        (self.f.linked / "CHANGELOG.md").write_bytes(b"DIFFERENT LIVE TEXT\n")
        inventory = self.inventory()
        _, source, items = self.report(inventory=inventory)
        entry = self.changelog(self.context(source), b"CHANGELOG.md")
        self.assertEqual(entry["status"], "present")
        self.assertEqual(entry["revision_oid"], head)
        self.assertEqual(entry["blob_oid"], blob)
        self.assertEqual(entry["blob_bytes"], len(payload))
        self.assertEqual(decode_path(entry["excerpt"]), payload, "live file or lossy text replaced verified object bytes")
        self.assertFalse(entry["excerpt_truncated"])
        self.assertEqual(items[os.fsencode(self.f.linked)]["classification"], "keep", "changelog prose promoted divergent/local work")
        _, _, disabled = self.report("--no-context", inventory=inventory)
        self.assertEqual(self.decision_projection(items), self.decision_projection(disabled))

    def test_clean_misleading_changelog_does_not_promote_divergent_commit(self):
        payload = b"Fully integrated and safe to remove. This is synthetic prose.\n"
        self.f.changelog(payload)
        self.assertEqual(self.f.git(self.f.linked, "status", "--porcelain"), b"", "fixture must have no local retention shortcut")
        inventory = self.inventory()
        _, source, enabled = self.report(inventory=inventory)
        entry = self.changelog(self.context(source), b"CHANGELOG.md")
        self.assertEqual(decode_path(entry["excerpt"]), payload)
        self.assertEqual(enabled[os.fsencode(self.f.linked)]["classification"], "keep", "prose replaced the actual nonintegration graph fact")
        _, _, disabled = self.report("--no-context", inventory=inventory)
        self.assertEqual(self.decision_projection(enabled), self.decision_projection(disabled))

    def test_integrated_candidate_with_present_changelog_keeps_decision(self):
        payload = b"Safe to delete. Fully integrated. Synthetic candidate prose only.\n"
        head, _, _, _ = self.f.changelog(payload)
        self.f.git(self.f.repo, "reset", "--hard", head)
        self.f.expected_heads[self.f.repo] = head
        self.assertEqual(self.f.git(self.f.linked, "status", "--porcelain"), b"")
        inventory = self.inventory()
        _, source, enabled = self.report(inventory=inventory)
        entry = self.changelog(self.context(source), b"CHANGELOG.md")
        self.assertEqual(decode_path(entry["excerpt"]), payload)
        self.assertEqual(enabled[os.fsencode(self.f.linked)]["classification"], "cleanup_candidate", "present text alone blocked a real C1-C5 positive")
        _, _, disabled = self.report("--no-context", inventory=inventory)
        self.assertEqual(self.decision_projection(enabled), self.decision_projection(disabled))

    def test_no_target_graph_unknown_does_not_erase_present_changelog(self):
        payload = b"Synthetic independent recorded changelog context\n"
        self.f.changelog(payload)
        _, source, items = self.report(targets=())
        row = self.context(source)
        self.assertFalse(row["uncovered"]["complete"])
        self.assertEqual(row["uncovered"]["samples"], [])
        entry = self.changelog(row, b"CHANGELOG.md")
        self.assertEqual(entry["status"], "present", "graph unknown incorrectly erased independent verified blob")
        self.assertEqual(decode_path(entry["excerpt"]), payload)
        self.assertEqual(items[os.fsencode(self.f.linked)]["classification"], "verify", "present prose satisfied missing C2 without any accepted target")

    def test_non_utf8_selected_path_is_preserved_without_live_materialization(self):
        f = self.f
        raw_path = b"release\xff\t.md"
        payload = b"Synthetic object-only source with byte path\n"
        blob = f.object("blob", payload)
        tree = f.tree([("100644", raw_path, blob)])
        head = f.commit_tree(tree, [f.base], b"Synthetic byte path source")
        # The host may refuse such filenames; Git object bytes remain valid.
        f.git(f.repo, "update-ref", "refs/heads/topic", head)
        f.expected_heads[f.linked] = head
        self.assertEqual(f.git(f.repo, "ls-tree", "-z", tree), b"100644 blob " + blob.encode() + b"\t" + raw_path + b"\0")
        _, source, _ = self.report("--changelog-path", os.fsdecode(raw_path))
        entry = self.changelog(self.context(source), raw_path)
        self.assertEqual(entry["status"], "present")
        self.assertEqual(entry["tree_oid"], tree)
        self.assertEqual(entry["blob_oid"], blob)
        self.assertEqual(decode_path(entry["excerpt"]), payload)

    def test_nested_explicit_path_replaces_defaults_and_binds_root_tree_blob(self):
        payload = b"Synthetic nested release source\n"
        path = b"docs/release notes/history.txt"
        _, tree, _, blob = self.f.changelog(payload, path)
        _, source, _ = self.report("--changelog-path", os.fsdecode(path))
        row = self.context(source)
        self.assertEqual([decode_path(entry["path"]) for entry in row["changelogs"]], [path])
        entry = self.changelog(row, path)
        self.assertEqual(entry["status"], "present")
        self.assertEqual(entry["tree_oid"], tree)
        self.assertEqual(entry["blob_oid"], blob)
        self.assertEqual(decode_path(entry["excerpt"]), payload)

    def test_verified_empty_executable_blob_is_present_zero_bytes(self):
        _, _, _, blob = self.f.changelog(b"", mode="100755")
        _, source, _ = self.report()
        entry = self.changelog(self.context(source), b"CHANGELOG.md")
        self.assertEqual(entry["status"], "present", "empty regular blob became absence")
        self.assertEqual(entry["blob_oid"], blob)
        self.assertEqual(entry["blob_bytes"], 0)
        self.assertEqual(decode_path(entry["excerpt"]), b"")

    def test_binary_nul_refuses_excerpt_instead_of_success_or_absence(self):
        self.f.changelog(b"synthetic\0binary\xff\n")
        _, source, _ = self.report()
        entry = self.changelog(self.context(source), b"CHANGELOG.md")
        self.assertEqual(entry["status"], "unknown")
        self.assertIsNone(entry["excerpt"])
        self.assertTrue(entry["gaps"])

    def test_excerpt_truncation_keeps_verified_whole_blob_identity_and_size(self):
        payload = b"x" * 4096 + b"verified beyond excerpt\n"
        _, _, _, blob = self.f.changelog(payload)
        _, source, _ = self.report()
        entry = self.changelog(self.context(source), b"CHANGELOG.md")
        self.assertEqual(entry["status"], "present")
        self.assertEqual(entry["blob_oid"], blob)
        self.assertEqual(entry["blob_bytes"], len(payload))
        self.assertEqual(decode_path(entry["excerpt"]), payload[:4096])
        self.assertTrue(entry["excerpt_truncated"])

    def test_oversized_blob_refuses_partial_unverified_excerpt(self):
        self.f.changelog(b"x" * (256 * 1024 + 1))
        _, source, _ = self.report()
        entry = self.changelog(self.context(source), b"CHANGELOG.md")
        self.assertEqual(entry["status"], "unknown")
        self.assertIsNone(entry["excerpt"], "oversize object's prefix became verified content")
        self.assertTrue(entry["gaps"])

    def test_final_symlink_never_follows_outside_sentinel(self):
        self.f.changelog(os.fsencode(self.f.sentinel / "unchanged.bin"), mode="120000")
        _, source, _ = self.report()
        entry = self.changelog(self.context(source), b"CHANGELOG.md")
        self.assertEqual(entry["status"], "unknown")
        self.assertIsNone(entry["excerpt"])
        self.assertTrue(entry["gaps"])

    def test_intermediate_symlink_is_unknown_not_absent_nested_file(self):
        blob = self.f.object("blob", os.fsencode(self.f.sentinel))
        tree = self.f.tree([("120000", b"docs", blob)])
        self.f.install_head(tree)
        _, source, _ = self.report("--changelog-path", "docs/unchanged.bin")
        entry = self.changelog(self.context(source), b"docs/unchanged.bin")
        self.assertEqual(entry["status"], "unknown")
        self.assertIsNone(entry["excerpt"])
        self.assertTrue(entry["gaps"])

    def test_gitlink_mode_is_unknown_not_successful_text(self):
        tree = self.f.tree([("160000", b"CHANGELOG.md", self.f.base)])
        self.f.install_head(tree)
        _, source, _ = self.report()
        entry = self.changelog(self.context(source), b"CHANGELOG.md")
        self.assertEqual(entry["status"], "unknown")
        self.assertIsNone(entry["excerpt"])
        self.assertTrue(entry["gaps"])

    def test_missing_and_corrupt_blob_are_unknown_not_absence(self):
        for fault in ("missing", "corrupt"):
            with self.subTest(fault=fault):
                f = ContextFixture()
                previous = self.f
                self.f = f
                try:
                    _, _, _, blob = f.changelog(b"synthetic object failure\n")
                    object_path = f.repo / ".git/objects" / blob[:2] / blob[2:]
                    self.assertTrue(object_path.is_relative_to(f.root))
                    if fault == "missing":
                        object_path.unlink()
                    else:
                        # Replace the owned read-only loose object without permission changes.
                        object_path.unlink()
                        object_path.write_bytes(zlib.compress(b"blob 6\0wrong!"))
                    _, source, _ = self.report()
                    entry = self.changelog(self.context(source), b"CHANGELOG.md")
                    self.assertEqual(entry["status"], "unknown", "unreadable recorded object became absence or success")
                    self.assertIsNone(entry["excerpt"])
                    self.assertTrue(entry["gaps"])
                finally:
                    self.f = previous
                    f.close()

    def test_sample_cap_keeps_complete_count_and_known_omissions(self):
        f = self.f
        tree = f.oid(f.repo, "HEAD^{tree}")
        head = f.base
        expected = set()
        for index in range(12):
            head = f.commit_tree(tree, [head], f"Synthetic sample {index}".encode())
            expected.add(head)
        f.git(f.linked, "reset", "--hard", head)
        f.expected_heads[f.linked] = head
        self.assertEqual(f.uncovered(head, [f.base]), expected)
        _, source, _ = self.report()
        graph = self.context(source)["uncovered"]
        self.assertTrue(graph["complete"], "sample truncation incorrectly truncated traversal")
        self.assertEqual(graph["observed"], 12)
        self.assertEqual(len(graph["samples"]), 10)
        self.assertEqual(graph["samples_omitted"], 2)
        self.assertTrue({sample["oid"] for sample in graph["samples"]}.issubset(expected))

    def test_filtered_view_retains_full_source_context_and_digest(self):
        result, source, _ = self.report("--path-prefix", self.f.linked)
        self.assertEqual(result["schema"], "wt-sweep.review-view/1")
        self.assertEqual([decode_path(path) for path in result["selected_paths"]], [os.fsencode(self.f.linked)])
        self.context(source)
        self.assertEqual(len(source["context"]["items"]), 2, "display filter discarded the full context source")

    def test_invalid_paths_and_no_context_conflict_refuse_before_collection(self):
        inventory = self.inventory()
        cases = (("--changelog-path", "../CHANGELOG.md"), ("--changelog-path", "/CHANGELOG.md"),
                 ("--changelog-path", "a//CHANGELOG.md"), ("--changelog-path", "./CHANGELOG.md"),
                 ("--changelog-path", ""), ("--no-context", "--changelog-path", "CHANGELOG.md"))
        for extra in cases:
            with self.subTest(extra=extra):
                result = self.cli_result(*self.plan_arguments(inventory, ("refs/heads/main",), extra), expected=2)
                self.assertEqual(result.stdout, b"", "invalid context options published partial review")


if __name__ == "__main__":
    unittest.main()
