"""Compare refusal records to the actual product, on uniquely owned Git fixtures."""
import json
import os
import pathlib
import shutil
import subprocess
import tempfile
import unittest

from test_walk import collect

PRODUCT = os.environ.get("SWEEP_NATIVE_PRODUCT")


@unittest.skipUnless(PRODUCT, "SWEEP_NATIVE_PRODUCT must name a frozen product binary")
class ProductRefusalTests(unittest.TestCase):
    def test_missing_registered_root_remains_unknown_in_both_collectors(self):
        with tempfile.TemporaryDirectory(prefix="wt-sweep-native-product-", dir="/private/tmp") as name:
            base = pathlib.Path(name)
            repo = base / "repo"
            linked = base / "missing-worktree"
            env = {"PATH": "/usr/bin:/bin", "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
                   "GIT_AUTHOR_NAME": "Fixture", "GIT_AUTHOR_EMAIL": "fixture@example.invalid",
                   "GIT_COMMITTER_NAME": "Fixture", "GIT_COMMITTER_EMAIL": "fixture@example.invalid",
                   "HOME": name, "LC_ALL": "C"}

            def git(*argv):
                subprocess.run(["/usr/bin/git", "-c", "core.hooksPath=/dev/null", *map(str, argv)],
                               env=env, check=True, capture_output=True, timeout=10)

            git("init", "-b", "main", repo)
            git("-C", repo, "commit", "--allow-empty", "-m", "fixture")
            git("-C", repo, "worktree", "add", "-b", "fixture", linked)
            shutil.rmtree(linked)  # Only this test's owned synthetic worktree.
            p = subprocess.run([PRODUCT, "scan", "--repo", str(repo), "--disk-usage", "--format", "json"],
                               env=env, check=True, capture_output=True, timeout=30)
            usage = json.loads(p.stdout)["disk_usage"]
            self.assertFalse(usage["complete"], usage)
            row = next(r for r in usage["measurements"] if r["path"]["hex"] == os.fsencode(linked).hex())
            self.assertFalse(row["complete"])
            self.assertIsNone(row["bytes"])
            self.assertTrue(row["gaps"])
            for mode in ("scalar", "native"):
                result = collect(mode, [repo, repo / ".git", linked])
                self.assertFalse(result["complete"], result)
                missing = result["roots"][-1]
                self.assertIsNone(missing["bytes"])
                self.assertFalse(missing["complete"])
                self.assertTrue(missing["gaps"])


if __name__ == "__main__":
    unittest.main()
