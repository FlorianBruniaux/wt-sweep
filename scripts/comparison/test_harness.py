import importlib.util
import pathlib
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).parents[1] / "compare-worktree-tools.py"

class OracleChecks(unittest.TestCase):
    def test_oracle_detects_same_size_bytes_index_refs_and_objects(self):
        self.assertTrue(SCRIPT.exists(), "comparison harness missing")
        spec = importlib.util.spec_from_file_location("comparison", SCRIPT)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            for relative in ("work/file", ".git/index", ".git/refs/heads/main", ".git/logs/HEAD"):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"old")
            before = module.snapshot(root)
            for relative in ("work/file", ".git/index", ".git/refs/heads/main", ".git/logs/HEAD"):
                (root / relative).write_bytes(b"new")
            obj = root / ".git/objects/aa/bb"
            obj.parent.mkdir(parents=True)
            obj.write_bytes(b"object")
            changes = module.delta(before, module.snapshot(root))
            self.assertEqual(set(changes["modified"]), {"work/file", ".git/index", ".git/refs/heads/main", ".git/logs/HEAD"})
            self.assertIn(".git/objects/aa/bb", changes["added"])
            self.assertEqual(changes["removed"], [])

class FixtureChecks(unittest.TestCase):
    def test_fixture_has_positive_and_distinct_preservation_obligations(self):
        self.assertTrue(SCRIPT.exists(), "comparison harness missing")
        spec = importlib.util.spec_from_file_location("comparison", SCRIPT)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            env = module.isolated_env(root / "home")
            for scenario in ("integrated", "unintegrated", "index_disk", "ignored_untracked", "stash", "private_reflog", "missing_object", "missing_target", "unreadable", "changed_target", "merge_conflict"):
                repo, work, facts = module.fixture(root / scenario, scenario, env)
                self.assertTrue(repo.is_dir())
                if scenario == "integrated":
                    self.assertEqual(module.git(work, env, "status", "--porcelain"), "")
                    self.assertEqual(module.git(repo, env, "rev-parse", "refs/heads/main"), module.git(work, env, "rev-parse", "HEAD"))
                if scenario == "unintegrated":
                    self.assertNotEqual(module.git(repo, env, "rev-parse", "refs/heads/main"), module.git(work, env, "rev-parse", "HEAD"))
                    self.assertEqual(module.git(repo, env, "rev-parse", "refs/remotes/origin/topic"), module.git(work, env, "rev-parse", "HEAD"))
                if scenario == "index_disk":
                    self.assertEqual(module.git(work, env, "show", ":tracked.txt"), "staged\n")
                    self.assertEqual((work / "tracked.txt").read_text(), "disk\n")
                if scenario == "merge_conflict":
                    self.assertNotEqual(module.git(repo, env, "show", "HEAD:tracked.txt"), module.git(work, env, "show", "HEAD:tracked.txt"))
                if scenario == "stash":
                    self.assertTrue(module.git(repo, env, "rev-parse", "refs/stash"))
                if scenario == "private_reflog":
                    self.assertIn(facts["private_oid"], module.git(work, env, "reflog", "--format=%H"))
                    self.assertEqual(module.git(work, env, "rev-parse", "--abbrev-ref", "HEAD"), "topic\n")
                if scenario == "ignored_untracked":
                    self.assertEqual((work / "notes.txt").read_bytes(), b"untracked author bytes\n")
                    self.assertEqual((work / "ignored/cache.txt").read_bytes(), b"ignored author bytes\n")
                if scenario == "missing_object":
                    oid = facts["missing_blob_oid"]
                    self.assertFalse((repo / ".git/objects" / oid[:2] / oid[2:]).exists())
                if scenario == "missing_target":
                    self.assertEqual(facts["target_ref"], "refs/heads/absent")
                if scenario == "changed_target":
                    self.assertNotEqual(facts["base_oid"], facts["head_oid"])
                if scenario == "unreadable":
                    try:
                        record = module.snapshot(root / scenario)["work/denied"]
                        self.assertEqual(record["kind"], "error")
                        self.assertEqual(record["errno"], 13)
                    finally:
                        (work / "denied").chmod(0o700)

if __name__ == "__main__":
    unittest.main()
