"""Native HTML CLI artifacts, independently checked against captured JSON."""
import hashlib
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from inventory_support import BINARY as DEFAULT_BINARY, HistoryFixture, filesystem_manifest

BINARY = Path(os.environ.get("WT_SWEEP_HTML_BINARY", str(DEFAULT_BINARY)))


class Rows(HTMLParser):
    def __init__(self, source):
        super().__init__(convert_charrefs=True)
        self.rows = {}
        self.feed(source)

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if "data-path-hex" in attrs and "data-verdict" in attrs:
            key = attrs["data-path-hex"]
            if key in self.rows:
                raise AssertionError("duplicate worktree identity in HTML")
            self.rows[key] = attrs["data-verdict"]


class HtmlCliTests(unittest.TestCase):
    def setUp(self):
        self.fixture = HistoryFixture()
        self.addCleanup(self.fixture.close)
        self.tmp = tempfile.TemporaryDirectory(prefix="wt-sweep-html-cli-", dir="/private/tmp")
        self.addCleanup(self.tmp.cleanup)
        self.out = Path(self.tmp.name)

    def invoke(self, *args, success=True):
        before = filesystem_manifest(self.fixture.root)
        result = subprocess.run([str(BINARY), *map(str, args)], env=self.fixture.env,
                                capture_output=True, timeout=40)
        self.assertEqual(before, filesystem_manifest(self.fixture.root), "source changed")
        self.assertEqual(result.returncode == 0, success, result.stderr.decode(errors="replace"))
        return result

    def output(self, name):
        path = self.out / name
        path.mkdir()
        return path

    def verify_run(self, base, primary):
        run, = base.glob("run-*")
        receipt = json.loads((run / "run.json").read_bytes())
        entries = {item["name"]: item for item in receipt["artifacts"]}
        self.assertIn("report.html", entries)
        for name, item in entries.items():
            raw = (run / name).read_bytes()
            self.assertEqual(len(raw), item["bytes"])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), item["sha256"])
        html = (run / "report.html").read_text()
        self.assertNotIn("\x1b", html)
        data = json.loads((run / primary).read_bytes())
        return run, html, data

    def test_html_requires_run_directory_before_inspecting_scope(self):
        result = self.invoke("scan", "--repo", self.out / "does-not-exist",
                             "--html", success=False)
        self.assertIn(b"--html requires --run-dir", result.stderr)
        self.assertEqual(list(self.out.iterdir()), [])

    def test_scan_html_matches_saved_inventory_and_is_explicitly_unreviewed(self):
        base = self.output("scan")
        result = self.invoke("scan", "--repo", self.fixture.principal, "--disk-usage",
                             "--html", "--run-dir", base, "--format", "json")
        run, html, inventory = self.verify_run(base, "inventory.json")
        self.assertEqual(json.loads(result.stdout), inventory)
        expected = {tree["path"]["hex"]: "not_reviewed"
                    for family in inventory["families"] for tree in family["worktrees"]}
        self.assertEqual(Rows(html).rows, expected)

    def test_review_and_filtered_plan_html_preserve_verdicts_and_receipts(self):
        base = self.output("review")
        result = self.invoke("review", "--repo", self.fixture.principal,
                             "--target", "refs/heads/main", "--no-context", "--disk-usage",
                             "--html", "--run-dir", base, "--summary")
        run, html, review = self.verify_run(base, "review.json")
        expected = {row["path"]["hex"]: row["classification"] for row in review["items"]}
        self.assertEqual(sorted(expected.values()), ["cleanup_candidate", "keep"])
        self.assertEqual(Rows(html).rows, expected)
        self.assertIn(review["digest_sha256"], html)
        self.assertIn(os.fsencode(run / "report.html"), result.stdout)
        filtered = self.output("plan")
        self.invoke("plan", "--inventory", run / "inventory.json",
                    "--target", "refs/heads/main", "--no-context",
                    "--path-prefix", self.fixture.linked, "--html", "--run-dir", filtered)
        _, html, view = self.verify_run(filtered, "review.json")
        self.assertEqual(len(view["source"]["items"]), 2)
        self.assertEqual(Rows(html).rows, {os.fsencode(self.fixture.linked).hex(): "cleanup_candidate"})
        self.assertIn(view["source"]["digest_sha256"], html)
        self.assertIn(view["digest_sha256"], html)

    def test_multiple_roots_retain_external_registrations_and_inaccessible_scope(self):
        other = HistoryFixture()
        self.addCleanup(other.close)
        other_before = filesystem_manifest(other.root)
        inaccessible = self.out / "inaccessible"
        inaccessible.mkdir()
        inaccessible.chmod(0)
        self.addCleanup(inaccessible.chmod, 0o700)
        base = self.output("multi-scan")
        self.invoke("scan", "--root", self.fixture.principal,
                    "--root", other.principal, "--root", self.fixture.principal,
                    "--root", inaccessible, "--disk-usage", "--html", "--run-dir", base)
        run, html, inventory = self.verify_run(base, "inventory.json")
        self.assertEqual(len(inventory["families"]), 2)
        expected = {os.fsencode(p).hex() for p in
                    (self.fixture.principal, self.fixture.linked, other.principal, other.linked)}
        self.assertEqual(set(Rows(html).rows), expected)
        self.assertTrue(inventory["coverage"]["gaps"], "unreadable root must leave a gap")
        self.assertFalse(inventory["coverage"]["traversal_complete"])
        target_map = self.out / "targets.json"
        target_map.write_text(json.dumps({"schema": "wt-sweep.targets/1", "families": [
            {"common_dir": f["common_dir"], "identity": f["identity"],
             "targets": [{"hex": b"refs/heads/main".hex()}]} for f in inventory["families"]]}))
        output = self.output("multi-plan")
        self.invoke("plan", "--inventory", run / "inventory.json", "--target-map", target_map,
                    "--group-by", "repo", "--no-context", "--html", "--run-dir", output)
        report_run, html, view = self.verify_run(output, "review.json")
        source = view.get("source", view)
        self.assertEqual(Rows(html).rows,
                         {r["path"]["hex"]: r["classification"] for r in source["items"]})
        text = (report_run / "report.txt").read_text()
        for row in source["items"]:
            self.assertIn(os.fsdecode(bytes.fromhex(row["path"]["hex"])), text)
        self.assertEqual(other_before, filesystem_manifest(other.root), "second source changed")

    def test_without_html_flag_keeps_existing_artifact_set(self):
        base = self.output("plain")
        self.invoke("scan", "--repo", self.fixture.principal, "--run-dir", base)
        run, = base.glob("run-*")
        self.assertEqual({p.name for p in run.iterdir()},
                         {"inventory.json", "report.txt", "run.json"})


if __name__ == "__main__":
    unittest.main()
