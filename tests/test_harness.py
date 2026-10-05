"""Behavior checks for project-only generation, using disposable directories."""

import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest


REPO = Path(__file__).resolve().parents[1]
SCRIPT = REPO / "scripts/sync-ai.py"


class HarnessTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="wt-sweep-harness-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "repo"
        self.root.mkdir()
        shutil.copytree(REPO / "harness", self.root / "harness")

    def run_sync(self, mode):
        return subprocess.run(
            [sys.executable, "-B", str(SCRIPT), "--root", str(self.root), mode],
            capture_output=True, text=True, check=False,
        )

    def generate(self):
        result = self.run_sync("--write")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.root / "AGENTS.md").is_file(), "generation must produce instructions")

    def snapshot(self):
        return {str(p.relative_to(self.root)): p.read_bytes()
                for p in self.root.rglob("*") if p.is_file()}

    def test_both_hosts_get_the_canonical_instructions_and_roles(self):
        self.generate()
        source = (self.root / "harness/instructions.md").read_bytes()
        self.assertEqual((self.root / "AGENTS.md").read_bytes(), source)
        self.assertEqual((self.root / "CLAUDE.md").read_bytes(), source)
        agents = json.loads((self.root / "harness/agents.json").read_text())
        self.assertGreater(len(agents), 0)
        for agent in agents:
            name = agent["name"]
            body = (self.root / f"harness/agents/{name}.md").read_text()
            codex = tomllib.loads((self.root / f".codex/agents/{name}.toml").read_text())
            claude = (self.root / f".claude/agents/{name}.md").read_text()
            self.assertEqual(codex["developer_instructions"], body)
            self.assertTrue(claude.endswith(body))
            self.assertEqual(codex["sandbox_mode"], "workspace-write" if agent["writable"] else "read-only")
            self.assertNotIn("model", codex)
        tomllib.loads((self.root / ".codex/config.toml").read_text())

    def test_check_and_repeated_generation_leave_bytes_unchanged(self):
        self.generate()
        before = self.snapshot()
        self.assertEqual(self.run_sync("--check").returncode, 0)
        self.assertEqual(self.run_sync("--write").returncode, 0)
        self.assertEqual(self.snapshot(), before)

    def test_drift_fails_without_repairing_any_file(self):
        self.generate()
        (self.root / "CLAUDE.md").write_text("independent edit\n")
        before = self.snapshot()
        result = self.run_sync("--check")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("CLAUDE.md", result.stderr)
        self.assertEqual(self.snapshot(), before)

    def test_write_refuses_conflicting_preimage_before_other_updates(self):
        self.generate()
        (self.root / "CLAUDE.md").write_text("concurrent edit\n")
        with (self.root / "harness/instructions.md").open("a") as stream:
            stream.write("\nNew canonical instruction.\n")
        before = self.snapshot()
        self.assertNotEqual(self.run_sync("--write").returncode, 0)
        self.assertEqual(self.snapshot(), before)

    def test_canonical_update_can_be_regenerated(self):
        self.generate()
        with (self.root / "harness/instructions.md").open("a") as stream:
            stream.write("\nUpdated canonical instruction.\n")
        self.assertNotEqual(self.run_sync("--check").returncode, 0)
        self.assertEqual(self.run_sync("--write").returncode, 0)
        self.assertEqual(self.run_sync("--check").returncode, 0)

    def test_symlinked_output_directory_is_refused(self):
        outside = Path(self.temp.name) / "outside"
        outside.mkdir()
        (self.root / ".codex").symlink_to(outside, target_is_directory=True)
        self.assertNotEqual(self.run_sync("--write").returncode, 0)
        self.assertEqual(list(outside.iterdir()), [])
        self.assertFalse((self.root / "AGENTS.md").exists())

    def test_manifest_path_traversal_is_refused(self):
        path = self.root / "harness/agents.json"
        agents = json.loads(path.read_text())
        agents[0]["name"] = "../../escape"
        path.write_text(json.dumps(agents))
        self.assertNotEqual(self.run_sync("--write").returncode, 0)
        self.assertFalse((self.root / "AGENTS.md").exists())


if __name__ == "__main__":
    unittest.main()
