"""Behavior checks for disposable E1 witnesses, not a product exclusion gate."""

import unittest

from experiment import run


class ExclusionWitnesses(unittest.TestCase):
    def test_real_writers_defeat_the_three_candidate_checks(self):
        result = run()
        self.assertEqual(result["decision"], "NON_DEMONTREE")
        self.assertTrue(result["fd_after_rename"]["outside_cwd"])
        self.assertTrue(result["fd_after_rename"]["cooperative_lock_refused"])
        self.assertTrue(result["fd_after_rename"]["write_succeeded"])
        self.assertEqual(result["partial_process_view"]["injected_visible_pids"], [])
        self.assertTrue(result["partial_process_view"]["empty_view_despite_write"])
        self.assertTrue(result["same_stat"]["same_size_and_mtime"])
        self.assertTrue(result["same_stat"]["different_bytes"])
        self.assertTrue(result["late_detached_commit"]["head_changed"])
        self.assertTrue(result["late_detached_commit"]["branch_unchanged"])
        self.assertTrue(result["late_detached_commit"]["object_readable"])
        self.assertEqual(result["process_visibility"], "UNKNOWN")
        self.assertEqual(result["durability"], "UNKNOWN")
        for category in ("modes", "symlinks", "hardlinks", "acl", "xattrs", "special_types"):
            self.assertIn(category, result["metadata"])
            self.assertEqual(result["metadata"][category]["restoration"], "UNKNOWN")


if __name__ == "__main__":
    unittest.main()
