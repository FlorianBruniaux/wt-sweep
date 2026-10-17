"""Real Git and independent before/after oracle for optional local comparison."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from inventory_support import BINARY, HistoryFixture, decode_path, filesystem_manifest


class LocalComparisonTests(unittest.TestCase):
    def setUp(self):
        self.fixture = HistoryFixture(files={"tracked": b"base\n", ".gitignore": b"node_modules/\n"})
        self.addCleanup(self.fixture.close)
        self.output = tempfile.TemporaryDirectory(prefix="wt-comparison-output-", dir="/private/tmp")
        self.addCleanup(self.output.cleanup)

    def run_cli(self, *args):
        before = filesystem_manifest(self.fixture.root)
        result = subprocess.run([str(BINARY), *map(str, args)], env=self.fixture.env,
                                capture_output=True, timeout=45)
        self.assertEqual(filesystem_manifest(self.fixture.root), before, "comparison mutated a source")
        return result

    def review(self, *args):
        result = self.run_cli("review", "--repo", self.fixture.principal,
                              "--target", "refs/heads/main", "--compare-local",
                              "--worktree", self.fixture.linked, "--html",
                              "--run-dir", self.output.name, "--format", "json", *args)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        run = max(Path(self.output.name).glob("run-*"), key=lambda p: p.stat().st_mtime_ns)
        receipt = json.loads((run / "run.json").read_bytes())
        for entry in receipt["artifacts"]:
            data = (run / entry["name"]).read_bytes()
            self.assertEqual(len(data), entry["bytes"])
            self.assertEqual(hashlib.sha256(data).hexdigest(), entry["sha256"])
        doc = json.loads((run / "local-comparison.json").read_bytes())
        review = json.loads(result.stdout)
        source = review.get("source", review)
        self.assertEqual(doc["review_sha256"], source["digest_sha256"])
        item = next(i for i in doc["items"] if decode_path(i["path"]) == os.fsencode(self.fixture.linked))
        return item, source, run

    def test_equal_target_and_one_byte_difference_are_distinct(self):
        # Target captures new bytes, while linked HEAD remains the base.
        self.fixture.write_commit(self.fixture.principal, "tracked", b"target\n")
        (self.fixture.linked / "tracked").write_bytes(b"target\n")
        item, source, run = self.review()
        self.assertTrue(item["complete"])
        self.assertEqual(item["path_totals"]["matches_target"], 1)
        self.assertEqual(item["rows"][0]["result"], "matches_target")
        self.assertIn("Que reste-t-il", (run / "report.html").read_text())
        self.assertIn("matches_target", (run / "report.txt").read_text())
        self.assertEqual(next(i for i in source["items"] if decode_path(i["path"]) == os.fsencode(self.fixture.linked))["classification"], "keep")
        (self.fixture.linked / "tracked").write_bytes(b"Target\n")
        item, _, _ = self.review()
        self.assertEqual(item["path_totals"]["different"], 1)

    def test_index_different_is_not_hidden_by_equal_disk(self):
        (self.fixture.linked / "tracked").write_bytes(b"index\n")
        self.fixture.git("-C", self.fixture.linked, "add", "tracked")
        (self.fixture.linked / "tracked").write_bytes(b"base\n")
        item, _, _ = self.review()
        self.assertEqual(item["path_totals"]["different"], 1)
        self.assertEqual({r["layer"]: r["result"] for r in item["rows"]},
                         {"index": "different", "disk": "matches_target"})

    def test_ignored_equal_checkout_and_unique_payload_are_not_discarded(self):
        for root in (self.fixture.principal, self.fixture.linked):
            (root / "node_modules").mkdir()
            (root / "node_modules" / "same").write_bytes(b"same synthetic bytes\n")
        (self.fixture.linked / "node_modules" / "unique").write_bytes(b"unrecovered\n")
        item, _, _ = self.review("--compare-reference", self.fixture.principal)
        rows = {decode_path(r["path"]): r for r in item["rows"]}
        self.assertEqual(rows[b"node_modules/same"]["result"], "matches_reference")
        self.assertEqual(rows[b"node_modules/unique"]["result"], "different")
        self.assertEqual(rows[b"node_modules/same"]["category"], "ignored")

    def test_complete_zero_requires_complete_source_and_not_requested_is_distinct(self):
        item, _, _ = self.review()
        self.assertTrue(item["complete"])
        self.assertEqual(item["rows"], [])
        doc = json.loads(next(Path(self.output.name).glob("run-*/local-comparison.json")).read_bytes())
        principal = next(i for i in doc["items"] if decode_path(i["path"]) == os.fsencode(self.fixture.principal))
        self.assertEqual(principal["collection"], "not_requested")
        self.assertFalse(principal["complete"])

    def test_options_require_a_run_and_explicit_reference(self):
        result = self.run_cli("review", "--repo", self.fixture.principal, "--compare-local")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"run", result.stderr)
        result = self.run_cli("review", "--repo", self.fixture.principal,
                              "--compare-reference", self.fixture.principal)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"--compare-local", result.stderr)

    def test_plan_compares_current_files_and_preserves_saved_inventory(self):
        inventory_path = Path(self.output.name) / "input-inventory.json"
        scanned = self.run_cli("scan", "--repo", self.fixture.principal,
                               "--format", "json", "--out", inventory_path)
        self.assertEqual(scanned.returncode, 0, scanned.stderr.decode())
        original = inventory_path.read_bytes()
        (self.fixture.linked / "new-after-scan").write_bytes(b"not in the old inventory\n")
        result = self.run_cli("plan", "--inventory", inventory_path,
                              "--target", "refs/heads/main", "--compare-local",
                              "--worktree", self.fixture.linked, "--run-dir", self.output.name)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertEqual(inventory_path.read_bytes(), original)
        run = next(Path(self.output.name).glob("run-*"))
        doc = json.loads((run / "local-comparison.json").read_bytes())
        item = next(i for i in doc["items"] if decode_path(i["path"]) == os.fsencode(self.fixture.linked))
        self.assertTrue(item["complete"])
        self.assertEqual(item["path_totals"]["different"], 1)
        self.assertEqual((run / "inventory.json").read_bytes(), original)

    def test_absence_is_not_a_zero_byte_file(self):
        self.fixture.git("-C", self.fixture.principal, "rm", "tracked")
        self.fixture.git("-C", self.fixture.principal, "commit", "-m", "delete")
        (self.fixture.linked / "tracked").unlink()
        item, _, _ = self.review()
        self.assertEqual(item["path_totals"]["matches_target"], 1)
        self.assertIsNotNone(item["rows"][0]["source"]["parent"])
        self.fixture.write_commit(self.fixture.principal, "tracked", b"")
        item, _, _ = self.review()
        self.assertEqual(item["path_totals"]["different"], 1)

    def test_reference_alias_cannot_compare_worktree_to_itself(self):
        (self.fixture.linked / "unique").write_bytes(b"only copy\n")
        result = self.run_cli("review", "--repo", self.fixture.principal,
                              "--target", "refs/heads/main", "--compare-local",
                              "--compare-reference", str(self.fixture.linked) + "/.",
                              "--worktree", self.fixture.linked, "--run-dir", self.output.name)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(list(Path(self.output.name).iterdir()), [])

    def test_bare_repository_inside_reference_is_an_unknown_boundary(self):
        cache = self.fixture.principal / "cache"
        self.fixture.git("init", "--bare", cache)
        (self.fixture.linked / "cache").mkdir()
        payload = (cache / "config").read_bytes()
        (self.fixture.linked / "cache" / "config").write_bytes(payload)
        item, _, _ = self.review("--compare-reference", self.fixture.principal)
        row = next(r for r in item["rows"] if decode_path(r["path"]) == b"cache/config")
        self.assertEqual(row["result"], "unknown")
        self.assertFalse(item["complete"])

    def test_reference_alias_is_skipped_physically_in_whole_family_comparison(self):
        for root in (self.fixture.principal, self.fixture.linked):
            (root / "unique").write_bytes(b"two copies\n")
        result = self.run_cli("review", "--repo", self.fixture.principal,
                              "--target", "refs/heads/main", "--compare-local",
                              "--compare-reference", str(self.fixture.linked) + "/.",
                              "--run-dir", self.output.name, "--format", "json")
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        run = next(Path(self.output.name).glob("run-*"))
        doc = json.loads((run / "local-comparison.json").read_bytes())
        linked = next(i for i in doc["items"] if decode_path(i["path"]) == os.fsencode(self.fixture.linked))
        self.assertEqual(linked["collection"], "not_requested")
        principal = next(i for i in doc["items"] if decode_path(i["path"]) == os.fsencode(self.fixture.principal))
        self.assertEqual(principal["path_totals"]["matches_reference"], 1)
        self.assertIsNotNone(principal["references"][-1]["administration"])

    def test_mode_only_change_is_not_an_equal_file(self):
        os.chmod(self.fixture.linked / "tracked", 0o755)
        item, _, _ = self.review()
        self.assertEqual(item["path_totals"]["different"], 1)

    def test_positive_target_survives_missing_second_target_with_partial_coverage(self):
        self.fixture.write_commit(self.fixture.principal, "tracked", b"target\n")
        (self.fixture.linked / "tracked").write_bytes(b"target\n")
        item, _, _ = self.review("--target", "refs/heads/missing")
        self.assertEqual(item["path_totals"]["matches_target"], 1)
        self.assertFalse(item["complete"])
        self.assertEqual(len(item["rows"][0]["lookups"]), 2)

    def test_raw_names_and_synthetic_secret_do_not_enter_html_as_markup_or_content(self):
        name = b'<img-onerror>'
        with open(os.fsencode(self.fixture.linked) + b'/' + name, 'wb') as stream:
            stream.write(b'SYNTHETIC_SECRET_DO_NOT_PRINT')
        item, _, run = self.review()
        self.assertIn(name, {decode_path(r["path"]) for r in item["rows"]})
        html = (run / "report.html").read_bytes()
        self.assertNotIn(b'<img-onerror>', html)
        for path in run.iterdir():
            self.assertNotIn(b'SYNTHETIC_SECRET_DO_NOT_PRINT', path.read_bytes())

    def test_non_utf8_git_path_survives_even_when_host_filesystem_refuses_the_name(self):
        # Raw Git index names are supported independently of macOS filename encoding.
        name = b'raw-\xff'
        oid = self.fixture.git("-C", self.fixture.linked, "rev-parse", "HEAD:tracked").stdout.strip()
        result = subprocess.run(["/usr/bin/git", "-C", str(self.fixture.linked), "update-index", "--index-info"],
                                input=b'100644 ' + oid + b'\t' + name + b'\n', env=self.fixture.env,
                                capture_output=True, timeout=15)
        self.assertEqual(result.returncode, 0)
        item, _, _ = self.review()
        self.assertIn(name, {decode_path(r["path"]) for r in item["rows"]})

    def test_conflict_stages_are_retained_and_cannot_be_complete(self):
        self.fixture.write_commit(self.fixture.principal, "tracked", b"main\n")
        self.fixture.write_commit(self.fixture.linked, "tracked", b"topic\n")
        merged = subprocess.run(["/usr/bin/git", "-C", str(self.fixture.linked), "merge", "main"],
                                env=self.fixture.env, capture_output=True, timeout=15)
        self.assertEqual(merged.returncode, 1)
        item, _, _ = self.review()
        self.assertFalse(item["complete"])
        conflicts = [r for r in item["rows"] if r["category"] == "conflicts"]
        self.assertEqual({r["stage"] for r in conflicts}, {1, 2, 3})

    def test_output_inside_unregistered_explicit_reference_is_refused_without_writes(self):
        reference = self.fixture.root / "explicit copy"
        reference.mkdir()
        (reference / ".git").write_text("gitdir: " + str(self.fixture.principal / ".git") + "\n")
        output = reference / "reports"
        output.mkdir()
        result = self.run_cli("review", "--repo", self.fixture.principal,
                              "--target", "refs/heads/main", "--compare-local",
                              "--compare-reference", reference, "--worktree", self.fixture.linked,
                              "--run-dir", output)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(list(output.iterdir()), [])

    def test_comparison_is_discoverable_in_navigation_and_command_search(self):
        (self.fixture.linked / "tracked").write_bytes(b"changed\n")
        _, _, run = self.review()
        html = (run / "report.html").read_text()
        self.assertTrue('href="#local-comparison"' in html, 'comparison missing from navigation')
        self.assertRegex(html, r'<details[^>]*class="report-section"[^>]*id="local-comparison"')

    def test_node_version_difference_is_kept_as_data_and_not_normalized(self):
        self.fixture.write_commit(self.fixture.principal, "runtime", b'{"node":24,"port":8000}')
        (self.fixture.linked / "runtime").write_bytes(b'{"node":22,"port":8000}')
        item, _, run = self.review()
        row = next(r for r in item["rows"] if decode_path(r["path"]) == b'runtime')
        self.assertEqual(row["result"], "different")
        self.assertNotIn(b'{"node":22', (run / "local-comparison.json").read_bytes())

    def test_symlink_compares_link_bytes_without_reading_its_target(self):
        for root in (self.fixture.principal, self.fixture.linked):
            os.symlink("missing-target", root / "local-link")
        item, _, _ = self.review("--compare-reference", self.fixture.principal)
        self.assertEqual(item["path_totals"]["matches_reference"], 1)
        (self.fixture.linked / "local-link").unlink()
        os.symlink("another-missing-target", self.fixture.linked / "local-link")
        item, _, _ = self.review("--compare-reference", self.fixture.principal)
        self.assertEqual(item["path_totals"]["different"], 1)

    def test_symlink_intermediate_directory_does_not_escape_reference(self):
        outside = self.fixture.root / "outside"
        outside.mkdir()
        (outside / "payload").write_bytes(b"same\n")
        os.symlink(outside, self.fixture.principal / "alias")
        (self.fixture.linked / "alias").mkdir()
        (self.fixture.linked / "alias" / "payload").write_bytes(b"same\n")
        item, _, _ = self.review("--compare-reference", self.fixture.principal)
        row = next(r for r in item["rows"] if decode_path(r["path"]) == b'alias/payload')
        self.assertEqual(row["result"], "unknown")
        self.assertFalse(item["complete"])

    def test_feature_preserves_verdicts_and_preexisting_local_facts(self):
        (self.fixture.linked / "tracked").write_bytes(b"changed\n")
        before = self.run_cli("review", "--repo", self.fixture.principal,
                              "--target", "refs/heads/main", "--worktree", self.fixture.linked,
                              "--format", "json")
        self.assertEqual(before.returncode, 0)
        _, after, _ = self.review()
        from test_review_command import timeless
        fields = ("path", "classification", "decision", "local_summary")
        def stable(review):
            return [{field: timeless(item[field]) for field in fields} for item in review["items"]]
        self.assertEqual(stable(json.loads(before.stdout)), stable(after))

    def test_sha256_git_objects_have_complete_oids_and_verified_content(self):
        self.fixture = HistoryFixture(files={"tracked": b"base\n"}, object_format="sha256")
        self.addCleanup(self.fixture.close)
        self.fixture.write_commit(self.fixture.principal, "tracked", b"target\n")
        (self.fixture.linked / "tracked").write_bytes(b"target\n")
        item, _, _ = self.review()
        self.assertEqual(item["path_totals"]["matches_target"], 1)
        self.assertEqual(len(item["references"][0]["commit_oid"]), 64)
        self.assertEqual(len(item["rows"][0]["lookups"][0]["object_oid"]), 64)

    def test_reference_checkout_from_another_family_is_refused(self):
        other = HistoryFixture()
        self.addCleanup(other.close)
        before = filesystem_manifest(other.root)
        result = self.run_cli("review", "--repo", self.fixture.principal,
                              "--target", "refs/heads/main", "--compare-local",
                              "--compare-reference", other.principal, "--worktree", self.fixture.linked,
                              "--run-dir", self.output.name)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(filesystem_manifest(other.root), before)
        self.assertEqual(list(Path(self.output.name).iterdir()), [])

    def test_intent_to_add_and_nested_repository_keep_partial_coverage(self):
        (self.fixture.linked / "new").write_bytes(b"uncommitted\n")
        self.fixture.git("-C", self.fixture.linked, "add", "-N", "new")
        self.fixture.git("init", "-b", "inner", str(self.fixture.linked / "nested"))
        item, _, _ = self.review()
        self.assertFalse(item["complete"])
        self.assertGreater(item["path_totals"]["unknown"], 0)

    def test_squash_equivalent_tree_does_not_change_history_integration_verdict(self):
        self.fixture.write_commit(self.fixture.linked, "tracked", b"same final tree\n")
        self.fixture.git("-C", self.fixture.principal, "merge", "--squash", "topic")
        self.fixture.git("-C", self.fixture.principal, "commit", "-m", "squashed")
        # A staged local change has bytes present in the squashed target.
        (self.fixture.linked / "tracked").write_bytes(b"local difference\n")
        self.fixture.git("-C", self.fixture.linked, "add", "tracked")
        (self.fixture.linked / "tracked").write_bytes(b"same final tree\n")
        item, source, _ = self.review()
        self.assertEqual(item["path_totals"]["different"], 1)
        self.assertIn("matches_target", {r["result"] for r in item["rows"]})
        row = next(r for r in source["items"] if decode_path(r["path"]) == os.fsencode(self.fixture.linked))
        self.assertEqual(row["classification"], "keep")

    def test_corrupt_target_blob_is_unknown_instead_of_equal(self):
        self.fixture.write_commit(self.fixture.principal, "tracked", b"target\n")
        oid = self.fixture.git("-C", self.fixture.principal, "rev-parse", "HEAD:tracked").stdout.decode().strip()
        object_path = self.fixture.principal / ".git" / "objects" / oid[:2] / oid[2:]
        # Git's immutable loose object is read-only. Replace only this owned
        # fixture entry; do not relax permissions or touch a real repository.
        object_path.unlink()
        object_path.write_bytes(b"synthetic corrupt compressed object")
        (self.fixture.linked / "tracked").write_bytes(b"target\n")
        item, _, _ = self.review()
        self.assertEqual(item["path_totals"]["unknown"], 1)
        self.assertFalse(item["complete"])


if __name__ == "__main__":
    unittest.main()
