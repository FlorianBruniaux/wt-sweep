"""Opt-in replay of authorized historical exports, without tracked private data.

Set WT_SWEEP_HISTORICAL_FORGE_DIR explicitly to the historical forge directory.
This verifies imported observations, not present Git objects or current server state.
"""

import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from inventory_support import BINARY, clean_environment, filesystem_manifest


HISTORICAL_DIRECTORY = os.environ.get("WT_SWEEP_HISTORICAL_FORGE_DIR")


def historical_pages(path):
    """Decode raw concatenated JSON pages independently of the product parser."""
    text = path.read_text()
    decoder = json.JSONDecoder()
    offset = 0
    pages = []
    while offset < len(text):
        while offset < len(text) and text[offset].isspace():
            offset += 1
        if offset == len(text):
            break
        page, offset = decoder.raw_decode(text, offset)
        if not isinstance(page, list):
            raise AssertionError("historical export page must be an array")
        pages.append(page)
    return pages


@unittest.skipUnless(HISTORICAL_DIRECTORY, "requires explicitly authorized historical forge directory")
class HistoricalReplayTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="wt-sweep-historical-replay-", dir="/private/tmp")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = Path(HISTORICAL_DIRECTORY)

    def ingest(self, filename):
        raw = self.source / filename
        before = filesystem_manifest(raw)
        pages = historical_pages(raw)
        destination = self.root / "normalized.json"
        result = subprocess.run([str(BINARY), "forge", "ingerer", "--raw", str(raw),
                                 "--out", str(destination)], env=clean_environment(self.root),
                                capture_output=True, timeout=30)
        self.assertEqual(before, filesystem_manifest(raw), "historical source changed (atime excluded)")
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        normalized = json.loads(destination.read_bytes())
        self.assertEqual(normalized["schema"], "wt-sweep.forge/1")
        self.assertEqual(len(normalized["requests"]), sum(map(len, pages)))
        self.assertFalse(normalized["coverage"]["absence_proven"])
        self.assertFalse(normalized["coverage"]["pagination_complete"])
        self.assertEqual(normalized["coverage"]["temporal_consistency"], "not_guaranteed")
        self.assertTrue(normalized["coverage"]["gaps"], "missing executor provenance became complete evidence")
        self.assertFalse(normalized["sources"][0]["receipt_verified"])
        self.assertEqual(normalized["sources"][0]["sha256"], hashlib.sha256(raw.read_bytes()).hexdigest(),
                         "historical positive facts are not bound to their raw bytes")
        return pages, normalized

    def replay_missing_merge_case(self, filename, iid, state):
        pages, normalized = self.ingest(filename)
        raw_matches = [row for page in pages for row in page if row.get("iid") == iid]
        imported = [row for row in normalized["requests"] if row["iid"] == iid]
        self.assertEqual(len(raw_matches), 1, "historical IID is absent or ambiguous within selected project")
        self.assertEqual(len(imported), 1, "selected positive historical fact was dropped or duplicated")
        raw = raw_matches[0]
        fact = imported[0]
        self.assertEqual(raw["state"], state)
        self.assertEqual(fact["state"], state)
        self.assertIn(len(raw["sha"]), (40, 64), "historical MR head is not a complete object identity")
        self.assertEqual(fact["head_oid"], raw["sha"])
        self.assertFalse(raw.get("merge_commit_sha"), "selected historical case no longer has missing merge evidence")
        self.assertFalse(raw.get("squash_commit_sha"))
        self.assertIsNone(fact["merge_oid"], "missing merge SHA invented from source head or merged state")
        self.assertTrue(raw["source_branch"])
        self.assertTrue(raw["target_branch"])
        self.assertEqual(fact["source_branch"], raw["source_branch"])
        self.assertEqual(fact["target_branch"], raw["target_branch"])

    def test_stats_mr91_intermediate_target_keeps_missing_merge_unknown(self):
        self.replay_missing_merge_case("gl-msds-stats-api-20261003b.json", 91, "merged")

    def test_stats_mr92_replaced_local_version_keeps_missing_merge_unknown(self):
        self.replay_missing_merge_case("gl-msds-stats-api-20261003b.json", 92, "merged")

    def test_lead_app_mr43_worktree_maps_to_closed_mr383(self):
        # The historical worktree name ends in mr43; documented forge identity is !383.
        self.replay_missing_merge_case("gl-lead-app-20261003b.json", 383, "closed")

    def test_lead_app_all_430_positive_records_remain_partial(self):
        pages, normalized = self.ingest("gl-lead-app-20261003b.json")
        self.assertEqual(len(pages), 5)
        self.assertEqual(sum(map(len, pages)), 430)
        self.assertEqual(len(normalized["requests"]), 430)


if __name__ == "__main__":
    unittest.main()
