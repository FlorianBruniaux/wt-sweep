"""Exercise the documented skill recipes against disposable Git worktrees."""

import collections
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

from inventory_support import BINARY, HistoryFixture, filesystem_manifest


ROOT = Path(__file__).resolve().parents[1]


class SkillWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.output = tempfile.TemporaryDirectory(prefix="wt-sweep-skill-", dir="/private/tmp")
        self.addCleanup(self.output.cleanup)
        self.output_path = Path(self.output.name)
        reference = (ROOT / "harness/skills/wt-sweep/references/commands.md").read_text()
        self.reader = re.search(r"<<'PYCODE'\n(.*?)\nPYCODE", reference, re.S)[1]

    def page(self, path, offset=0, limit=1):
        return subprocess.run(
            [sys.executable, "-", str(path), str(offset), str(limit)],
            input=self.reader, text=True, capture_output=True, timeout=10,
        )

    @unittest.skipUnless(sys.platform == "darwin", "CLI collector requires macOS")
    def test_scoped_collection_and_paged_review_preserve_real_worktrees(self):
        fixture = HistoryFixture()
        self.addCleanup(fixture.close)
        before = filesystem_manifest(fixture.root)
        for args in (["--version"], ["--help"], ["review", "--help"], ["scan", "--help"]):
            subprocess.run([str(BINARY), *args], capture_output=True, check=True,
                           env=fixture.env, timeout=15)
        discovery = self.output_path / "discovery.json"
        subprocess.run([str(BINARY), "scan", "--root", str(fixture.root), "--out", str(discovery)],
                       capture_output=True, check=True, env=fixture.env, timeout=30)
        self.assertTrue(discovery.is_file())
        self.assertEqual(filesystem_manifest(fixture.root), before)
        for name, dirty, extra, expected_selected in (
            ("clean", False, [], 2),
            ("filtered", False, ["--verdict", "cleanup_candidate"], 1),
            ("dirty", True, [], 2),
        ):
            with self.subTest(case=name):
                if dirty:
                    (fixture.linked / "notes.txt").write_text("Synthetic unsaved work\n")
                before = filesystem_manifest(fixture.root)
                base = self.output_path / name
                base.mkdir()
                result = subprocess.run(
                    [str(BINARY), "review", "--repo", str(fixture.principal),
                     "--target", "refs/heads/main", "--group-by", "directory",
                     "--format", "json", "--run-dir", str(base), *extra],
                    capture_output=True, env=fixture.env, timeout=60,
                )
                self.assertEqual(result.returncode, 0, result.stderr.decode())
                run, = base.glob("run-*")
                receipt = json.loads((run / "run.json").read_bytes())
                for artifact in receipt["artifacts"]:
                    self.assertIn(artifact["name"], ("inventory.json", "review.json", "report.txt"))
                    data = (run / artifact["name"]).read_bytes()
                    self.assertEqual(len(data), artifact["bytes"])
                    self.assertEqual(hashlib.sha256(data).hexdigest(), artifact["sha256"])
                pages = []
                for offset in range(expected_selected):
                    read = self.page(run / "review.json", offset)
                    self.assertEqual(read.returncode, 0, read.stderr)
                    pages.append(json.loads(read.stdout))
                rows = [row for page in pages for row in page["rows"]]
                self.assertEqual(len({row["path"]["hex"] for row in rows}), expected_selected)
                for page in pages:
                    self.assertEqual(page["source_total"], 2)
                    self.assertEqual(page["selected_total"], expected_selected)
                    self.assertEqual(sum(page["source_verdicts"].values()), 2)
                by_path = {bytes.fromhex(row["path"]["hex"]): row for row in rows}
                linked = by_path[bytes(fixture.linked)]
                self.assertEqual(linked["classification"], "keep" if dirty else "cleanup_candidate")
                if not extra:
                    expected = {"keep": 2} if dirty else {"keep": 1, "cleanup_candidate": 1}
                    self.assertEqual(dict(collections.Counter(r["classification"] for r in rows)), expected)
                self.assertIsNone(pages[-1]["next_offset"])
                self.assertEqual(filesystem_manifest(fixture.root), before)

    def test_reader_refuses_historical_review_and_invalid_page(self):
        path = self.output_path / "historical.json"
        path.write_text(json.dumps({"schema": "wt-sweep.review/1"}))
        result = self.page(path)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Unsupported review", result.stderr)
        for offset, limit in ((-1, 5), (0, 0), (0, 21)):
            result = self.page(path, offset, limit)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("page size", result.stderr)


if __name__ == "__main__":
    unittest.main()
