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

    def add_skill(self):
        (self.root / "harness/skills.json").write_text(json.dumps([{
            "name": "fixture-skill", "files": ["SKILL.md", "references/commands.md", "evals/scenarios.json"]
        }]))
        skill = self.root / "harness/skills/fixture-skill"
        (skill / "references").mkdir(parents=True)
        (skill / "evals").mkdir()
        (skill / "SKILL.md").write_text('---\nname: fixture-skill\ndescription: "Inspect owned fixtures"\n---\nRead [commands](references/commands.md).\n')
        (skill / "references/commands.md").write_text("Fixture commands.\n")
        (skill / "evals/scenarios.json").write_text(json.dumps({"skill": "fixture-skill", "positive": [f"fixture {n}" for n in range(8)], "negative": ["other", "unrelated"]}))
        return skill

    def test_skill_bundle_reaches_both_hosts_and_updates_from_one_source(self):
        skill = self.add_skill()
        self.generate()
        for host in (".agents", ".claude"):
            for relative in ("SKILL.md", "references/commands.md", "evals/scenarios.json"):
                self.assertEqual((self.root / host / "skills/fixture-skill" / relative).read_bytes(),
                                 (skill / relative).read_bytes())
        (skill / "references/commands.md").write_text("Updated command.\n")
        self.assertNotEqual(self.run_sync("--check").returncode, 0)
        self.assertEqual(self.run_sync("--write").returncode, 0)
        self.assertEqual((self.root / ".agents/skills/fixture-skill/references/commands.md").read_text(), "Updated command.\n")

    def test_skill_invalid_manifest_and_evaluations_refuse_before_writing(self):
        skill = self.add_skill()
        manifest = self.root / "harness/skills.json"
        original = manifest.read_text()
        for resource in ("../instructions.md", "/private/tmp/outside", "references//commands.md"):
            with self.subTest(resource=resource):
                data = json.loads(original)
                data[0]["files"].append(resource)
                manifest.write_text(json.dumps(data))
                self.assertNotEqual(self.run_sync("--write").returncode, 0)
                self.assertFalse((self.root / "AGENTS.md").exists())
        manifest.write_text(original)
        scenarios = skill / "evals/scenarios.json"
        data = json.loads(scenarios.read_text())
        data["positive"] = ["only one"]
        scenarios.write_text(json.dumps(data))
        self.assertNotEqual(self.run_sync("--write").returncode, 0)
        self.assertFalse((self.root / "AGENTS.md").exists())

    def test_skill_conflicting_projection_refuses_entire_batch(self):
        self.add_skill()
        self.generate()
        (self.root / ".claude/skills/fixture-skill/SKILL.md").write_text("Local edit\n")
        (self.root / "harness/instructions.md").write_text("Changed instructions\n")
        before = self.snapshot()
        self.assertNotEqual(self.run_sync("--write").returncode, 0)
        self.assertEqual(self.snapshot(), before)

    def test_skill_missing_resource_or_escaped_link_refuses_before_writing(self):
        skill = self.add_skill()
        for target in ("references/missing.md", "../outside.md"):
            with self.subTest(target=target):
                (skill / "SKILL.md").write_text('---\nname: fixture-skill\ndescription: "Inspect owned fixtures"\n---\nRead [required](' + target + ').\n')
                self.assertNotEqual(self.run_sync("--write").returncode, 0)
                self.assertFalse((self.root / "AGENTS.md").exists())

    def test_skill_symlinked_resource_refuses_without_following(self):
        skill = self.add_skill()
        resource = skill / "references/commands.md"
        resource.unlink()
        outside = Path(self.temp.name) / "outside.md"
        outside.write_text("Do not change\n")
        resource.symlink_to(outside)
        self.assertNotEqual(self.run_sync("--write").returncode, 0)
        self.assertFalse((self.root / "AGENTS.md").exists())
        self.assertEqual(outside.read_text(), "Do not change\n")

    def test_skill_output_directory_symlink_is_refused(self):
        self.add_skill()
        outside = Path(self.temp.name) / "outside"
        outside.mkdir()
        (self.root / ".agents").symlink_to(outside, target_is_directory=True)
        self.assertNotEqual(self.run_sync("--write").returncode, 0)
        self.assertEqual(list(outside.iterdir()), [])
        self.assertFalse((self.root / "AGENTS.md").exists())

    def test_unrelated_skills_are_preserved_but_extra_owned_files_refuse(self):
        self.add_skill()
        unrelated = self.root / ".agents/skills/another/SKILL.md"
        unrelated.parent.mkdir(parents=True)
        unrelated.write_text("Unrelated skill\n")
        self.generate()
        self.assertEqual(unrelated.read_text(), "Unrelated skill\n")
        extra = self.root / ".claude/skills/fixture-skill/extra.md"
        extra.write_text("Preserve this file\n")
        before = self.snapshot()
        self.assertNotEqual(self.run_sync("--write").returncode, 0)
        self.assertEqual(self.snapshot(), before)

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
