"""Native HTML CLI artifacts, independently checked against captured JSON."""
import hashlib
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import subprocess
import shlex
import sys
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

    def desktop(self):
        home = self.out / "home with an apostrophe's"
        desktop = home / "Desktop"
        desktop.mkdir(parents=True)
        self.fixture.env["HOME"] = str(home)
        return desktop

    @unittest.skipUnless(sys.platform == "darwin", "macOS desktop default")
    def test_html_defaults_to_desktop_and_repeated_runs_preserve_first_report(self):
        desktop = self.desktop()
        result = self.invoke("scan", "--repo", self.fixture.principal, "--html")
        base = desktop / "Worktree Sweep"
        run, _, _ = self.verify_run(base, "inventory.json")
        before = filesystem_manifest(run)
        command = next(line.split("Open HTML: ", 1)[1] for line in result.stdout.decode().splitlines()
                       if "Open HTML: " in line)
        self.assertEqual(shlex.split(command), ["/usr/bin/open", str(run / "report.html")])
        self.invoke("scan", "--repo", self.fixture.principal, "--html")
        self.assertEqual(len(list(base.glob("run-*"))), 2)
        self.assertEqual(before, filesystem_manifest(run))
        self.assertEqual(base.stat().st_mode & 0o777, 0o700)

    @unittest.skipUnless(sys.platform == "darwin", "macOS desktop default")
    def test_review_and_plan_default_html_keep_json_stdout(self):
        desktop = self.desktop()
        base = desktop / "Worktree Sweep"
        result = self.invoke("review", "--repo", self.fixture.principal,
                             "--target", "refs/heads/main", "--html", "--format", "json")
        run, _, review = self.verify_run(base, "review.json")
        self.assertEqual(json.loads(result.stdout), review)
        first = run
        result = self.invoke("plan", "--inventory", first / "inventory.json",
                             "--target", "refs/heads/main", "--html", "--format", "json")
        second, = set(base.glob("run-*")) - {first}
        self.assertEqual(json.loads(result.stdout), json.loads((second / "review.json").read_bytes()))
        self.assertTrue((second / "report.html").is_file())
        self.assertTrue((second / "run.json").is_file())

    @unittest.skipUnless(sys.platform == "darwin", "macOS desktop default")
    def test_custom_run_directory_overrides_desktop_and_plain_json_creates_nothing(self):
        desktop = self.desktop()
        custom = self.output("custom")
        self.invoke("scan", "--repo", self.fixture.principal, "--html", "--run-dir", custom)
        self.verify_run(custom, "inventory.json")
        result = self.invoke("scan", "--repo", self.fixture.principal, "--format", "json")
        self.assertEqual(json.loads(result.stdout)["schema"], "wt-sweep.inventory/1")
        self.assertEqual(list(desktop.iterdir()), [])

    @unittest.skipUnless(sys.platform == "darwin", "macOS desktop default")
    def test_default_destination_refuses_symlink_or_protected_desktop_without_writing(self):
        desktop = self.desktop()
        destination = desktop / "Worktree Sweep"
        destination.symlink_to(self.fixture.principal, target_is_directory=True)
        self.invoke("scan", "--repo", self.fixture.principal, "--html", success=False)
        destination.unlink()
        # The default must not create even its base folder inside a scanned root.
        self.invoke("scan", "--root", desktop, "--html", success=False)
        self.assertEqual(list(desktop.iterdir()), [])

    def test_html_and_open_conflicts_refuse_before_collection(self):
        for flag in ("--html", "--open"):
            with self.subTest(flag=flag):
                result = self.invoke("scan", "--repo", self.out / "does-not-exist",
                                     flag, "--out", self.out / "forbidden.json", success=False)
                self.assertIn(b"--out", result.stderr)
                self.assertIn(b"incompatible", result.stderr)
        self.assertEqual(list(self.out.iterdir()), [])

    @unittest.skipUnless(sys.platform == "darwin", "macOS desktop default")
    def test_open_without_desktop_explains_custom_destination_before_collection(self):
        self.fixture.env["HOME"] = str(self.out)
        result = self.invoke("scan", "--repo", self.out / "does-not-exist", "--open", success=False)
        self.assertIn(b"Desktop", result.stderr)
        self.assertIn(b"--run-dir", result.stderr)
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

        storage = inventory["disk_usage"]["storage"]
        self.assertEqual(sum(row["bytes"]["allocated"] for row in storage["devices"]),
                         inventory["disk_usage"]["unique_bytes"]["allocated"])
        for row in storage["devices"]:
            self.assertIn(f'data-device="{row["device"]}"', html)

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
        self.assertEqual(view["source"]["disk_usage"], review["disk_usage"])

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
