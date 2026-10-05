"""Behavioral U1 oracles using only disposable Git families."""

import json
import errno
import copy
import os
from pathlib import Path
import subprocess
import unittest

from inventory_support import FamilyFixture, decode_path


class InventoryTests(unittest.TestCase):
    def setUp(self):
        self.fixture = FamilyFixture()
        self.addCleanup(self.fixture.close)

    def assert_targets_unchanged(self, before, message="target manifest changed; atime excluded"):
        after = self.fixture.target_manifest()
        changes = []
        for root in sorted(before.keys() | after.keys()):
            old = before.get(root, {})
            new = after.get(root, {})
            changes.extend(path for path in sorted(old.keys() | new.keys())
                           if old.get(path) != new.get(path))
        self.assertFalse(changes, message + ": " + repr(changes[:8]))

    def test_exact_nonempty_registered_family(self):
        result = self.fixture.scan("--format", "json")
        self.assert_family(result)

    def test_linked_and_detached_entrypoints_resolve_the_same_family(self):
        before = self.fixture.target_manifest()
        for entrypoint in (self.fixture.linked, self.fixture.detached):
            with self.subTest(entrypoint=entrypoint):
                self.assert_family(self.fixture.scan("--format", "json", repo=entrypoint))
                self.assert_targets_unchanged(before)

    def test_oracle_detects_an_omitted_extra_or_duplicate_candidate(self):
        result = self.fixture.scan("--format", "json")
        report = self.assert_family(result)
        for mutation in ("omitted", "extra", "duplicate"):
            with self.subTest(mutation=mutation):
                mutated = copy.deepcopy(report)
                worktrees = mutated["families"][0]["worktrees"]
                if mutation == "omitted":
                    worktrees.pop()
                elif mutation == "extra":
                    invented = copy.deepcopy(worktrees[0])
                    invented["path"] = {"hex": os.fsencode(self.fixture.root / "invented").hex()}
                    worktrees.append(invented)
                else:
                    worktrees.append(copy.deepcopy(worktrees[0]))
                injected = subprocess.CompletedProcess(result.args, 0, json.dumps(mutated).encode(), b"")
                with self.assertRaises(AssertionError):
                    self.assert_family(injected)

    def assert_family(self, result, expect_complete=True):
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        report = json.loads(result.stdout)
        self.assertEqual(report["schema"], "wt-sweep.inventory/1")
        self.assertEqual(len(report["families"]), 1)
        family = report["families"][0]
        self.assertEqual(decode_path(family["common_dir"]), os.fsencode(self.fixture.principal / ".git"))
        worktrees = family["worktrees"]
        actual = {decode_path(worktree["path"]): worktree for worktree in worktrees}
        self.assertEqual(len(actual), len(worktrees), "duplicate worktree identities")
        self.assertEqual(set(actual), set(self.fixture.expected), "omitted or invented worktree")
        for path, (branch, detached, state) in self.fixture.expected.items():
            with self.subTest(path=path):
                worktree = actual[path]
                self.assertEqual(worktree["head"], self.fixture.heads[path])
                self.assertEqual(None if worktree["branch"] is None else decode_path(worktree["branch"]),
                                 None if branch is None else branch.encode())
                self.assertEqual(worktree["detached"], detached)
                self.assertFalse(worktree["bare"])
                self.assertEqual(worktree["path_state"]["state"], state)
        if expect_complete:
            self.assertTrue(report["coverage"]["traversal_complete"])
        self.assertEqual(report["coverage"]["temporal_consistency"], "not_guaranteed")
        return report

    def test_paths_with_spaces_newlines_and_non_utf8_are_reversible(self):
        self.fixture.add_worktree(b"line\nbreak", "line")
        # Git's registration stores arbitrary bytes even when the host filesystem
        # rejects such a directory name. This remains a real Git parser fixture.
        registered = self.fixture.add_worktree(b"registered-byte", "bytes")
        git_dir = Path(os.fsdecode(self.fixture.git("-C", str(registered),
                                                   "rev-parse", "--git-dir").stdout.rstrip(b"\n")))
        raw_path = os.fsencode(self.fixture.root) + b"/byte-\xff"
        (git_dir / "gitdir").write_bytes(raw_path + b"/.git\n")
        del self.fixture.expected[os.fsencode(registered)]
        self.fixture.heads[raw_path] = self.fixture.heads.pop(os.fsencode(registered))
        try:
            os.stat(raw_path)
        except OSError as error:
            expected_state = "absent" if error.errno == errno.ENOENT else "read_error"
        else:
            self.fail("uniquely owned raw-byte path unexpectedly exists")
        self.fixture.expected[raw_path] = ("refs/heads/bytes", False, expected_state)
        report = self.assert_family(self.fixture.scan("--format", "json"),
                                    expect_complete=expected_state == "absent")
        if expected_state == "read_error":
            self.assertTrue(report["coverage"]["gaps"], "raw-byte filesystem error hidden from coverage")
        for family in report["families"]:
            for worktree in family["worktrees"]:
                encoded = worktree["path"]["hex"]
                self.assertEqual(bytes.fromhex(encoded).hex(), encoded, "lossy or noncanonical byte identity")

    def test_existing_non_utf8_filesystem_path_when_host_supports_it(self):
        raw_name = b"existing-byte-\xff"
        raw_path = os.fsencode(self.fixture.root) + b"/" + raw_name
        try:
            os.mkdir(raw_path)
        except OSError as error:
            if error.errno == errno.EILSEQ:
                self.skipTest("host filesystem rejects non-UTF8 names (EILSEQ); registered raw-byte case still tested")
            raise
        self.fixture.add_worktree(raw_name, "existing-bytes")
        self.assert_family(self.fixture.scan("--format", "json"))

    def test_hostile_git_environment_does_not_execute_witness(self):
        self.fixture.install_witnesses()
        fake_bin = self.fixture.root / "hostile-bin"
        fake_bin.mkdir()
        script = b"#!/bin/sh\nprintf 'PATH git executed\\n' >> " + os.fsencode(self.fixture.witness_marker) + b"\nexit 99\n"
        descriptor = os.open(fake_bin / "git", os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o700)
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(script)
        hostile = dict(self.fixture.env)
        hostile.update({"PATH": str(fake_bin), "GIT_CONFIG_COUNT": "1",
                        "GIT_CONFIG_KEY_0": "core.fsmonitor",
                        "GIT_CONFIG_VALUE_0": str(self.fixture.root / "executable-witness"),
                        "GIT_DIR": str(self.fixture.root / "invented-git-dir"),
                        "GIT_WORK_TREE": str(self.fixture.root / "invented-work-tree")})
        before = self.fixture.target_manifest()
        result = self.fixture.scan("--format", "json", env=hostile)
        self.assert_targets_unchanged(before)
        self.assertFalse(self.fixture.witness_marker.exists(), "ambient executable Git config or PATH was trusted")
        self.assert_family(result)

    def test_hostile_tmpdir_never_writes_inside_worktree_or_admin(self):
        for temp_directory in (self.fixture.principal, self.fixture.principal / ".git"):
            with self.subTest(temp_directory=temp_directory):
                hostile = dict(self.fixture.env, TMPDIR=str(temp_directory))
                before = self.fixture.target_manifest()
                result = self.fixture.scan("--format", "json", env=hostile)
                self.assert_targets_unchanged(before,
                                 "ambient TMPDIR caused temporary writes in worktree or Git administration")
                self.assert_family(result)

    def test_json_and_table_conserve_worktrees_admins_and_witnesses(self):
        self.fixture.install_witnesses()
        before = self.fixture.target_manifest()
        for arguments in (("--format", "json"), ("--format", "table"), ()):
            with self.subTest(arguments=arguments):
                result = self.fixture.scan(*arguments)
                self.assert_targets_unchanged(before,
                                 "scan altered bytes, modes, links, timestamps, flags or xattrs; atime excluded")
                self.assertFalse(self.fixture.witness_marker.exists(), "executable Git configuration ran")
                self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
                if arguments == ("--format", "json"):
                    self.assert_family(result)
                else:
                    self.assert_table(result.stdout)

    def assert_table(self, output):
        self.assertTrue(output.strip(), "empty table")
        self.assertIn(b"principal", output)
        self.assertIn(b"linked branch", output)
        self.assertIn(b"detached", output)
        self.assertIn(b"registered missing", output)

    def test_explicit_out_writes_json_and_stdout_table_without_target_changes(self):
        destination = self.fixture.root / "inventory.json"
        before = self.fixture.target_manifest()
        result = self.fixture.scan("--out", str(destination))
        self.assert_targets_unchanged(before)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        self.assert_table(result.stdout)
        report = json.loads(destination.read_bytes())
        self.assertEqual(report["schema"], "wt-sweep.inventory/1")
        self.assertEqual({decode_path(worktree["path"]) for worktree in report["families"][0]["worktrees"]},
                         set(self.fixture.expected))

    def test_outputs_inside_worktrees_or_admins_are_refused(self):
        aliases = self.fixture.root / "alias-to-principal"
        os.symlink(self.fixture.principal, aliases)
        destinations = [self.fixture.principal / "inventory.json",
                        self.fixture.principal / ".git" / "inventory.json",
                        self.fixture.linked / "inventory.json",
                        self.fixture.detached / "inventory.json",
                        self.fixture.missing / "inventory.json",
                        aliases / "inventory.json"]
        before = self.fixture.target_manifest()
        for destination in destinations:
            with self.subTest(destination=destination):
                result = self.fixture.scan("--out", str(destination))
                self.assertNotEqual(result.returncode, 0, "accepted output inside target family")
                self.assertTrue(result.stderr.strip(), "refusal has no explanation")
                self.assert_targets_unchanged(before)
                self.assertFalse(destination.exists(), "created output in protected family")

    def test_existing_output_and_symlink_ancestors_are_refused(self):
        destination = self.fixture.root / "existing.json"
        destination.write_bytes(b"preserve existing output\n")
        output_dir = self.fixture.root / "outside-family"
        output_dir.mkdir()
        alias = self.fixture.root / "outside-alias"
        os.symlink(output_dir, alias)
        file_alias = self.fixture.root / "file-alias"
        os.symlink(destination, file_alias)
        before = self.fixture.target_manifest()
        for candidate in (destination, file_alias, alias / "new.json"):
            with self.subTest(candidate=candidate):
                result = self.fixture.scan("--out", str(candidate))
                self.assertNotEqual(result.returncode, 0)
                self.assertTrue(result.stderr.strip())
                self.assertEqual(destination.read_bytes(), b"preserve existing output\n")
                self.assertFalse((output_dir / "new.json").exists())
                self.assert_targets_unchanged(before)

    def test_symlink_loop_is_error_not_absent(self):
        retained = self.fixture.root / "retained-linked"
        self.fixture.linked.rename(retained)
        self.fixture.manifest_roots.add(os.fsencode(retained))
        os.symlink(self.fixture.linked.name, self.fixture.linked)
        before = self.fixture.target_manifest()
        result = self.fixture.scan("--format", "json")
        self.assert_targets_unchanged(before)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        report = json.loads(result.stdout)
        actual = {decode_path(worktree["path"]): worktree
                  for worktree in report["families"][0]["worktrees"]}
        self.assertEqual(set(actual), set(self.fixture.expected))
        self.assertIn(actual[os.fsencode(self.fixture.linked)]["path_state"]["state"],
                      ("read_error", "unknown"))
        self.assertTrue(report["coverage"]["gaps"], "filesystem error hidden from coverage")

    def test_missing_head_evidence_is_retained_and_reported(self):
        git_dir = Path(os.fsdecode(self.fixture.git("-C", str(self.fixture.linked),
                                                   "rev-parse", "--git-dir").stdout.rstrip(b"\n")))
        (git_dir / "HEAD").write_bytes(b"invalid head evidence\n")
        before = self.fixture.target_manifest()
        result = self.fixture.scan("--format", "json")
        self.assert_targets_unchanged(before)
        if result.returncode:
            self.assertTrue(result.stderr.strip(), "Git failure silently swallowed")
            self.assertFalse(result.stdout.strip(), "failed collection emitted successful inventory")
            return
        report = json.loads(result.stdout)
        actual = {decode_path(worktree["path"]): worktree
                  for worktree in report["families"][0]["worktrees"]}
        self.assertEqual(set(actual), set(self.fixture.expected))
        self.assertIsNone(actual[os.fsencode(self.fixture.linked)]["head"])
        self.assertTrue(report["coverage"]["gaps"], "missing HEAD evidence silently discarded")

    def test_git_failure_is_never_a_successful_empty_inventory(self):
        (self.fixture.principal / ".git" / "HEAD").write_bytes(b"invalid principal head\n")
        before = self.fixture.target_manifest()
        result = self.fixture.scan("--format", "json")
        self.assert_targets_unchanged(before)
        self.assertNotEqual(result.returncode, 0, "Git error accepted as a successful inventory")
        self.assertTrue(result.stderr.strip())
        self.assertFalse(result.stdout.strip(), "failed Git emitted successful empty evidence")

    def test_empty_gitdir_registration_cannot_be_silently_omitted(self):
        git_dir = Path(os.fsdecode(self.fixture.git("-C", str(self.fixture.linked),
                                                   "rev-parse", "--git-dir").stdout.rstrip(b"\n")))
        (git_dir / "gitdir").write_bytes(b"")
        before = self.fixture.target_manifest()
        result = self.fixture.scan("--format", "json")
        self.assert_targets_unchanged(before)
        if result.returncode:
            self.assertTrue(result.stderr.strip(), "broken registration was refused without explanation")
            self.assertFalse(result.stdout.strip(), "failure emitted a successful report")
            return
        report = json.loads(result.stdout)
        self.assertTrue(report["coverage"]["gaps"], "Git omitted an admin registration without a visible gap")
        self.assertTrue(any(decode_path(gap["path"]).startswith(os.fsencode(git_dir))
                            or decode_path(gap["path"]) == os.fsencode(self.fixture.linked)
                            for gap in report["coverage"]["gaps"]),
                        "gap does not identify the broken worktree registration")


if __name__ == "__main__":
    unittest.main()
