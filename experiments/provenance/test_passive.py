import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from fixtures import card, DATE, GENERATION, NEXT_GENERATION, publish, write_json
from passive import read_context

NOW = "2026-10-11T12:10:00.000Z"


class PassiveTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="sweep-provenance-")
        self.addCleanup(self.temp.cleanup)
        self.main = Path(self.temp.name).resolve() / "main"
        self.path = Path(self.temp.name).resolve() / "topic"
        self.path.mkdir()
        self.common = self.main / ".git"
        self.value = card(self.path, self.common)

    def read(self, **kwargs):
        return read_context(str(self.path), str(self.common), now=NOW, **kwargs)

    def write_card(self, value=None):
        write_json(self.path / ".worktree.json", self.value if value is None else value)

    def test_present_context_preserves_creator_and_dates_without_identity_proof(self):
        self.write_card()
        result = self.read()
        observation = result["observations"][0]
        self.assertEqual(observation["state"], "present")
        self.assertEqual(observation["origin"]["creator"]["tool"], "codex")
        self.assertEqual(observation["origin"]["creator"]["evidence"], "explicit")
        self.assertEqual(observation["origin"]["timestamps"]["createdAt"], None)
        self.assertEqual(observation["origin"]["sessions"][0]["evidence"], "runtime")
        self.assertEqual(observation["freshness"], {"state": "recent", "ageSeconds": 600})
        self.assertEqual(observation["identity"]["currentWorktree"], "UNKNOWN")
        self.assertEqual(observation["source"]["capturedAt"], NOW)
        self.assertEqual(observation["source"]["updatedAt"], DATE)
        self.assertEqual(len(observation["source"]["sha256"]), 64)
        self.assertFalse(result["authorizesRemoval"])
        self.assertEqual(result["decisionEvidence"], [])
        self.assertEqual(result["usage"], "UNKNOWN")

    def test_absent_is_an_observation_not_empty_provenance(self):
        result = self.read()
        self.assertEqual(result["observations"][0]["state"], "absent")
        self.assertEqual(result["observations"][0]["identity"]["currentWorktree"], "UNKNOWN")

    def test_invalid_inputs_are_explicit_and_do_not_overwrite_source(self):
        invalid = [b"{", b"[]", b'{"schemaVersion":2}', b'{"schemaVersion":1,"schemaVersion":2}', b"\xff"]
        values = []
        for changes in ({"schemaVersion": 2}, {"extra": "secret"}, {"revision": True},
                        {"updatedAt": "2026-02-31T12:00:00.000Z"}, {"sessions": [None]}):
            value = copy.deepcopy(self.value)
            value.update(changes)
            values.append(json.dumps(value).encode())
        for raw in invalid + values:
            with self.subTest(raw=raw[:40]):
                source = self.path / ".worktree.json"
                source.write_bytes(raw)
                result = self.read()
                self.assertEqual(result["observations"][0]["state"], "invalid")
                self.assertIsNone(result["observations"][0]["origin"])
                self.assertEqual(source.read_bytes(), raw)

    def test_oversize_card_is_not_read_as_partial_valid_context(self):
        (self.path / ".worktree.json").write_bytes(b" " * 65537)
        result = self.read()
        self.assertEqual(result["observations"][0]["error"], "SOURCE_TOO_LARGE")

    def test_symlink_and_fifo_cards_are_refused_without_following_or_blocking(self):
        source = self.path / ".worktree.json"
        target = Path(self.temp.name) / "outside.json"
        write_json(target, self.value)
        source.symlink_to(target)
        self.assertEqual(self.read()["observations"][0]["error"], "UNSAFE_SOURCE")
        source.unlink()
        os.mkfifo(source)
        self.assertEqual(self.read()["observations"][0]["error"], "UNSAFE_SOURCE")

    def test_stale_and_future_dates_never_become_usage(self):
        for date, state in [("2026-10-01T12:00:00.000Z", "stale"), ("2026-10-12T12:00:00.000Z", "future")]:
            self.value["updatedAt"] = date
            self.write_card()
            result = self.read()
            self.assertEqual(result["observations"][0]["freshness"]["state"], state)
            self.assertEqual(result["usage"], "UNKNOWN")

    def test_path_and_repository_mismatches_keep_declared_origin_and_contradictions(self):
        self.value["path"] = "/synthetic/moved"
        self.value["repository"]["commonDir"] = "/synthetic/other.git"
        self.write_card()
        observation = self.read()["observations"][0]
        self.assertEqual(observation["origin"]["path"], "/synthetic/moved")
        self.assertEqual(observation["contradictions"], ["PATH_MISMATCH", "REPOSITORY_MISMATCH"])
        self.assertEqual(observation["identity"]["currentWorktree"], "contradicted")

    def test_source_replacement_during_read_is_marked_changed(self):
        self.write_card()
        def replace():
            replacement = self.path / "replacement.json"
            write_json(replacement, self.value)
            replacement.replace(self.path / ".worktree.json")
        observation = self.read(after_read=replace)["observations"][0]
        self.assertEqual(observation["state"], "changed")
        self.assertEqual(observation["error"], "SOURCE_CHANGED")
        self.assertIsNone(observation["origin"])

    def test_source_disappearance_after_read_is_changed_not_absent(self):
        self.write_card()
        observation = self.read(after_read=lambda: (self.path / ".worktree.json").unlink())["observations"][0]
        self.assertEqual(observation["error"], "SOURCE_CHANGED")
        self.assertEqual(observation["state"], "changed")

    def test_generation_lookup_keeps_source_generation_and_observation_date(self):
        publish(self.main, self.value)
        result = self.read(main=str(self.main))
        observation = result["observations"][1]
        self.assertEqual(observation["state"], "present")
        self.assertEqual(observation["source"]["generation"], GENERATION)
        self.assertEqual(observation["source"]["observedAt"], DATE)
        self.assertEqual(observation["identity"]["currentWorktree"], "UNKNOWN")

    def test_changed_generation_does_not_masquerade_as_current(self):
        publish(self.main, self.value)
        def advance():
            publish(self.main, self.value, NEXT_GENERATION)
        result = self.read(main=str(self.main), after_read=advance)
        self.assertEqual(result["observations"][1]["error"], "GENERATION_CHANGED")
        self.assertIsNone(result["observations"][1]["origin"])

    def test_generation_disappearance_after_read_is_changed_not_absent(self):
        root, _ = publish(self.main, self.value)
        result = self.read(main=str(self.main), after_read=lambda: (root / "current").unlink())
        self.assertEqual(result["observations"][1]["error"], "GENERATION_CHANGED")
        self.assertEqual(result["observations"][1]["state"], "changed")

    def test_unsafe_generation_path_is_refused(self):
        root, _ = publish(self.main, self.value)
        (root / "current").unlink()
        (root / "current").symlink_to("../../outside")
        self.assertEqual(self.read(main=str(self.main))["observations"][1]["error"], "UNSAFE_GENERATION")

    def test_index_contradiction_retains_both_origins(self):
        self.write_card()
        old = copy.deepcopy(self.value)
        old["creator"]["tool"] = "human"
        publish(self.main, old, problems=["BRANCH_CHANGED"])
        result = self.read(main=str(self.main))
        self.assertEqual([row["origin"]["creator"]["tool"] for row in result["observations"]], ["codex", "human"])
        self.assertIn("SOURCE_DISAGREEMENT", result["contradictions"])
        self.assertEqual(result["observations"][1]["contradictions"], ["BRANCH_CHANGED"])

    def test_duplicate_index_rows_are_ambiguous_and_entries_are_bounded(self):
        root, index = publish(self.main, self.value)
        source = root / "generations" / GENERATION / "index.json"
        index["entries"] *= 2
        write_json(source, index)
        self.assertEqual(self.read(main=str(self.main))["observations"][1]["error"], "AMBIGUOUS_PATH")
        index["entries"] *= 65
        write_json(source, index)
        self.assertEqual(self.read(main=str(self.main))["observations"][1]["error"], "ENTRY_LIMIT")

    def test_generation_document_must_match_pointer(self):
        root, index = publish(self.main, self.value)
        index["generation"] = NEXT_GENERATION
        write_json(root / "generations" / GENERATION / "index.json", index)
        self.assertEqual(self.read(main=str(self.main))["observations"][1]["error"], "GENERATION_MISMATCH")

    def test_publication_date_does_not_refresh_old_origin(self):
        self.value["updatedAt"] = "2026-10-01T12:00:00.000Z"
        publish(self.main, self.value)
        observation = self.read(main=str(self.main))["observations"][1]
        self.assertEqual(observation["freshness"]["state"], "stale")
        self.assertEqual(observation["publicationFreshness"]["state"], "recent")

    def test_absent_generation_and_unrecorded_path_remain_distinct(self):
        self.main.mkdir()
        observation = self.read(main=str(self.main))["observations"][1]
        self.assertEqual(observation["state"], "absent")
        self.assertEqual(observation["error"], "SOURCE_ABSENT")
        root, index = publish(self.main, self.value)
        index["entries"] = []
        write_json(root / "generations" / GENERATION / "index.json", index)
        self.assertEqual(self.read(main=str(self.main))["observations"][1]["error"], "PATH_NOT_RECORDED")

    def test_invalid_selected_card_and_partial_generation_keep_their_limits(self):
        root, index = publish(self.main, self.value)
        index["complete"] = False
        source = root / "generations" / GENERATION / "index.json"
        write_json(source, index)
        result = self.read(main=str(self.main))
        self.assertEqual(result["observations"][1]["state"], "present")
        self.assertFalse(result["observations"][1]["source"]["complete"])
        index["entries"][0]["card"]["creator"]["tool"] = "invalid-tool"
        write_json(source, index)
        self.assertEqual(self.read(main=str(self.main))["observations"][1]["error"], "INVALID_FORMAT")

    def test_oversize_index_and_relation_lists_are_refused(self):
        root, _ = publish(self.main, self.value)
        (root / "generations" / GENERATION / "index.json").write_bytes(b" " * 1048577)
        self.assertEqual(self.read(main=str(self.main))["observations"][1]["error"], "SOURCE_TOO_LARGE")
        self.value["sessions"] *= 33
        self.write_card()
        self.assertEqual(self.read()["observations"][0]["error"], "RELATION_LIMIT")

    def test_non_utf8_path_is_refused_without_lossy_identity(self):
        result = read_context(os.fsdecode(b"/synthetic/\xff"), str(self.common), now=NOW)
        self.assertEqual(result["observations"][0]["error"], "UNSUPPORTED_PATH_BYTES")

    def test_real_git_recreated_path_keeps_identity_unknown_despite_matching_metadata(self):
        env = {"PATH": os.environ["PATH"], "HOME": self.temp.name, "GIT_CONFIG_NOSYSTEM": "1",
               "GIT_CONFIG_GLOBAL": os.devnull, "GIT_TERMINAL_PROMPT": "0"}
        def git(*args):
            return subprocess.run(["/usr/bin/git", "-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false",
                                   *args], check=True, capture_output=True, env=env).stdout.decode().strip()
        self.path.rmdir()
        git("init", "-b", "main", str(self.main))
        git("-C", str(self.main), "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
            "commit", "--allow-empty", "-m", "synthetic")
        git("-C", str(self.main), "worktree", "add", "-b", "synthetic/topic", str(self.path))
        self.value["initial"]["head"] = git("-C", str(self.path), "rev-parse", "HEAD")
        self.write_card()
        before = self.read()["observations"][0]
        raw = (self.path / ".worktree.json").read_bytes()
        (self.path / ".worktree.json").unlink()
        git("-C", str(self.main), "worktree", "remove", str(self.path))
        git("-C", str(self.main), "worktree", "add", str(self.path), "synthetic/topic")
        (self.path / ".worktree.json").write_bytes(raw)
        after = self.read()["observations"][0]
        self.assertEqual(before["origin"], after["origin"])
        self.assertEqual(after["contradictions"], [])
        self.assertEqual(after["identity"]["currentWorktree"], "UNKNOWN")
        self.assertEqual(after["identity"]["reason"], "NO_INSTANCE_BINDING")


if __name__ == "__main__":
    unittest.main()
