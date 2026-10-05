"""Independent T02 CLI oracles. No product collector or judgement is imported."""

import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import unittest

from inventory_support import BINARY as DEFAULT_BINARY, GIT, clean_environment, decode_path, filesystem_manifest


# Dedicated override lets this suite test a frozen owned copy while other
# agents run Cargo commands that may replace target/debug/wt-sweep.
BINARY = Path(os.environ.get("WT_SWEEP_LOCAL_SUMMARY_BINARY", str(DEFAULT_BINARY)))


class LocalFixture:
    def __init__(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="wt-sweep-t02-cli-", dir="/private/tmp")
        self.root = Path(self.temporary.name)
        self.repo = self.root / "principal"
        self.env = clean_environment(self.root)
        self.env.update(GIT_CONFIG_SYSTEM=os.devnull, GIT_ATTR_NOSYSTEM="1",
                        GIT_OPTIONAL_LOCKS="0", GIT_NO_LAZY_FETCH="1",
                        GIT_ALLOW_PROTOCOL="", GIT_NO_REPLACE_OBJECTS="1")
        self.manifest_roots = {self.repo}
        self.sequence = 0

    def git(self, *arguments, data=None, expected=0):
        argv = [GIT]
        for config in ["core.hooksPath=/dev/null", "core.fsmonitor=false",
                       "core.untrackedCache=false", "gc.auto=0", "maintenance.auto=false",
                       "protocol.allow=never", "core.autocrlf=false", "core.filemode=true"]:
            argv += ["-c", config]
        result = subprocess.run([*argv, "-C", str(self.repo), *map(str, arguments)],
                                input=data if data is not None else b"", env=self.env,
                                capture_output=True, timeout=30, check=False)
        if result.returncode != expected:
            raise AssertionError((arguments, result.returncode, result.stderr))
        return result.stdout

    def init(self, files=None, object_format=None, commit=True):
        self.repo.mkdir()
        extra = ["--object-format=" + object_format] if object_format else []
        self.git("init", "-q", "-b", "main", *extra)
        for name, payload in (files or {"tracked.txt": b"head\n"}).items():
            self.write(name, payload)
        if commit:
            self.commit()

    def write(self, name, payload):
        path = self.repo / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
        return path

    def commit(self, message="Fixture base"):
        self.git("add", "--all")
        self.git("commit", "-q", "-m", message)
        return self.git("rev-parse", "HEAD").strip().decode()

    def manifest(self):
        return {os.fsencode(root): filesystem_manifest(root) for root in self.manifest_roots}

    def close(self):
        self.temporary.cleanup()


class LocalSummaryTests(unittest.TestCase):
    def setUp(self):
        self.fixture = LocalFixture()
        self.addCleanup(self.fixture.close)

    def run_cli(self, *arguments):
        before = self.fixture.manifest()
        process = subprocess.Popen([str(BINARY), *map(str, arguments)], env=self.fixture.env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=True)
        try:
            stdout, stderr = process.communicate(timeout=60)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.kill()
            process.communicate(timeout=5)
            self.fail("owned CLI exceeded deadline; process group stopped and child reaped")
        finally:
            self.assertEqual(before, self.fixture.manifest(),
                             "CLI changed fixture bytes, index/shared-index or metadata (atime excluded)")
        self.assertEqual(process.returncode, 0, stderr.decode(errors="replace"))
        return stdout

    def inventory(self, local=False, scope_root=False):
        extra = ["--local"] if local else []
        scope = ["--root", self.fixture.root] if scope_root else ["--repo", self.fixture.repo]
        inventory = json.loads(self.run_cli("scan", *scope,
                                           *extra, "--format", "json"))
        self.assertEqual(inventory["schema"], "wt-sweep.inventory/1")
        self.fixture.sequence += 1
        path = self.fixture.root / f"inventory-{self.fixture.sequence}.json"
        path.write_text(json.dumps(inventory))
        self.fixture.manifest_roots.add(path)
        return path, inventory

    def plan(self, inventory, target_map=None):
        targets = ("--target", "refs/heads/main") if target_map is None else ("--target-map", target_map)
        return json.loads(self.run_cli("plan", "--inventory", inventory,
                                       *targets, "--format", "json"))

    def item(self, report, repo=None):
        repo = repo or self.fixture.repo
        matching = [item for item in report["items"] if decode_path(item["path"]) == os.fsencode(repo)]
        self.assertEqual(len(matching), 1, "expected exactly one item for the independently known worktree")
        return matching[0]

    def summary(self, report, repo=None):
        if repo is None:
            self.assertEqual(len(report["items"]), 1, "single real repository must produce one review item")
        repo = repo or self.fixture.repo
        item = self.item(report, repo)
        self.assertIn("local_summary", item,
                      "T02 requires fresh local_summary even when scan omitted --local")
        summary = item["local_summary"]
        self.assertIsInstance(summary, dict, "current local summary must be collected for present worktree")
        self.assertEqual(report["schema"], "wt-sweep.review/2")
        expected = {"worktree_identity", "git_dir", "head", "started_unix_ms", "finished_unix_ms",
                    "staged", "unstaged", "conflicts", "untracked", "ignored", "head_complete",
                    "index_complete", "disk_complete", "temporal_consistency", "sources", "gaps",
                    "details_truncated", "digest_sha256"}
        self.assertEqual(set(summary), expected, "exact T02 local summary contract")
        self.assertLessEqual(summary["started_unix_ms"], summary["finished_unix_ms"])
        self.assertEqual(summary["temporal_consistency"], "not_guaranteed")
        body = {key: value for key, value in summary.items() if key != "digest_sha256"}
        expected_digest = hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":"),
                                                   ensure_ascii=False).encode()).hexdigest()
        self.assertEqual(summary["digest_sha256"], expected_digest,
                         "summary digest must bind coverage, positives, sources AND collection interval")
        self.assertLessEqual(len(json.dumps(summary, separators=(",", ":")).encode()), 64 * 1024)
        self.assertLessEqual(len(summary["gaps"]), 32)
        for key in ["staged", "unstaged", "conflicts", "untracked", "ignored"]:
            count = summary[key]
            self.assertEqual(set(count), {"observed", "complete", "samples", "samples_omitted"})
            self.assertLessEqual(len(count["samples"]), 8)
            self.assertLessEqual(sum(len(decode_path(path)) for path in count["samples"]), 1024)
            self.assertEqual(len(count["samples"]) + count["samples_omitted"], count["observed"], key)
        for source in summary["sources"]:
            self.assertEqual(set(source), {"kind", "path", "sha256"})
            path = Path(os.fsdecode(decode_path(source["path"])))
            self.assertTrue(path.is_absolute())
            self.assertTrue(path.is_relative_to(repo / ".git"),
                            "source observation must stay inside fixture administration")
            self.assertTrue(path.is_file(), "raw source hash must name a regular readable fixture file")
            self.assertEqual(source["sha256"], hashlib.sha256(path.read_bytes()).hexdigest(),
                             "source digest does not bind raw bytes actually present")
        for key in ("staged", "unstaged", "conflicts", "untracked", "ignored"):
            if summary[key]["observed"]:
                self.assertEqual(item["classification"], "keep", "observed local work cannot become a candidate")
                self.assertIn("local_" + key, item["decision"]["keep_reasons"],
                              "positive local count lost its decision reason")
        return summary

    def assert_count(self, summary, key, observed, complete=True):
        self.assertIn(key, summary)
        count = summary[key]
        self.assertEqual(count["observed"], observed, key)
        self.assertIs(count["complete"], complete, key)

    def assert_integrated(self, report, repo=None):
        item = self.item(report, repo)
        self.assertEqual(item["classification"], "cleanup_candidate")
        self.assertTrue(any(observation["target_role"] == "integration" and
                            observation["relation"] == "identical_commit" and
                            decode_path(observation["target_ref"]) == b"refs/heads/main" and
                            observation["source_oid"] == observation["target_oid"] == item["head"]
                            for observation in item["observations"]),
                        "local failure must not erase independent positive HEAD integration evidence")

    def assert_kept_main(self, report):
        item = self.item(report)
        self.assertEqual(item["classification"], "keep")
        self.assertIn("main_checkout", item["decision"]["keep_reasons"])
        self.assertEqual(item["observations"], [], "known KEEP skips deep reads")
        self.assertTrue(any(record["result"] == "identical_commit" for record in item["history"]),
                        "local failure must not erase independent verified commit graph facts")

    def test_clean_linked_control_executes_actual_deep_commit_proof(self):
        self.fixture.init()
        linked = self.fixture.root / "clean-linked"
        self.fixture.git("worktree", "add", "--detach", linked)
        self.fixture.manifest_roots.add(linked)
        inventory, _ = self.inventory()
        report = self.plan(inventory)
        self.assert_kept_main(report)
        self.assert_integrated(report, linked)

    def test_clean_nested_tracked_file_has_current_summary_without_local_scan(self):
        self.fixture.init({"src/main.rs": b"fn main() {}\n"})
        inventory, _ = self.inventory()
        summary = self.summary(self.plan(inventory))
        for key in ["staged", "unstaged", "conflicts", "untracked", "ignored"]:
            self.assert_count(summary, key, 0)
        self.assertTrue(summary["head_complete"])
        self.assertTrue(summary["index_complete"])
        self.assertTrue(summary["disk_complete"])
        metadata = self.fixture.repo.stat()
        self.assertEqual(summary["worktree_identity"],
                         {"device": metadata.st_dev, "inode": metadata.st_ino})
        self.assertEqual(decode_path(summary["git_dir"]), os.fsencode(self.fixture.repo / ".git"))
        self.assertEqual(summary["head"], self.fixture.git("rev-parse", "HEAD").strip().decode())
        source_paths = {decode_path(source["path"]) for source in summary["sources"]}
        self.assertIn(os.fsencode(self.fixture.repo / ".git/HEAD"), source_paths)
        self.assertIn(os.fsencode(self.fixture.repo / ".git/index"), source_paths)

    def test_head_index_disk_are_separate_current_observations(self):
        self.fixture.init()
        inventory, _ = self.inventory()
        self.fixture.write("tracked.txt", b"index\n")
        self.fixture.git("add", "tracked.txt")
        self.fixture.write("tracked.txt", b"disk!\n")
        summary = self.summary(self.plan(inventory))
        self.assert_count(summary, "staged", 1)
        self.assert_count(summary, "unstaged", 1)
        self.assert_count(summary, "conflicts", 0)
        self.assert_count(summary, "untracked", 0)
        self.assert_count(summary, "ignored", 0)
        self.assertEqual({decode_path(path) for path in summary["staged"]["samples"]}, {b"tracked.txt"})
        self.assertEqual({decode_path(path) for path in summary["unstaged"]["samples"]}, {b"tracked.txt"})
        text = self.run_cli("plan", "--inventory", inventory, "--target", "refs/heads/main").decode()
        self.assertIn("Local now ", text)
        self.assertIn("staged=1; unstaged=1; conflicts=0; untracked=0; ignored=0", text)

    def test_structural_src_then_ignored_hidden_descendants(self):
        self.fixture.init({"src/main.rs": b"fn main() {}\n", ".gitignore": b"src/*.tmp\nignored/\n"})
        inventory, _ = self.inventory()
        clean = self.summary(self.plan(inventory))
        self.assert_count(clean, "untracked", 0)
        self.assert_count(clean, "ignored", 0)
        expected = {b"src/ignored.tmp"}
        self.fixture.write("src/ignored.tmp", b"preserve ignored\n")
        for number in range(12):
            name = f"ignored/.hidden/{number:02}.txt"
            self.fixture.write(name, b"hidden ignored\n")
            expected.add(name.encode())
        summary = self.summary(self.plan(inventory))
        self.assert_count(summary, "ignored", 13)
        self.assert_count(summary, "untracked", 0)
        samples = {decode_path(path) for path in summary["ignored"]["samples"]}
        self.assertEqual(len(samples), 8)
        self.assertTrue(samples <= expected)
        self.assertEqual(summary["ignored"]["samples_omitted"], 5)

    def test_deleted_staged_and_unstaged_paths_are_distinct(self):
        self.fixture.init({"staged.txt": b"staged base\n", "unstaged.txt": b"unstaged base\n"})
        inventory, _ = self.inventory()
        self.fixture.git("rm", "staged.txt")
        (self.fixture.repo / "unstaged.txt").unlink()
        summary = self.summary(self.plan(inventory))
        self.assert_count(summary, "staged", 1)
        self.assert_count(summary, "unstaged", 1)
        self.assert_count(summary, "untracked", 0)
        self.assertEqual({decode_path(path) for path in summary["staged"]["samples"]}, {b"staged.txt"})
        self.assertEqual({decode_path(path) for path in summary["unstaged"]["samples"]}, {b"unstaged.txt"})

    def test_conflict_counts_one_path_not_three_index_stages(self):
        self.fixture.init()
        self.fixture.git("checkout", "-q", "-b", "other")
        self.fixture.write("tracked.txt", b"other\n")
        self.fixture.commit("Other")
        self.fixture.git("checkout", "-q", "main")
        self.fixture.write("tracked.txt", b"ours\n")
        self.fixture.commit("Ours")
        self.fixture.git("merge", "other", expected=1)
        inventory, _ = self.inventory()
        summary = self.summary(self.plan(inventory))
        self.assert_count(summary, "conflicts", 1)
        self.assertTrue(summary["index_complete"])
        self.assertEqual({decode_path(path) for path in summary["conflicts"]["samples"]}, {b"tracked.txt"})

    def test_chmod_literal_symlink_and_file_type_changes(self):
        self.fixture.init({"mode.txt": b"mode\n", "file.txt": b"file\n"})
        outside = self.fixture.root / "outside sentinel"
        outside.write_bytes(b"never follow me\n")
        self.fixture.manifest_roots.add(outside)
        os.symlink(str(outside), self.fixture.repo / "link")
        self.fixture.commit("Tracked link")
        inventory, _ = self.inventory()
        clean = self.summary(self.plan(inventory))
        self.assert_count(clean, "unstaged", 0)
        self.assertTrue(clean["disk_complete"])
        os.chmod(self.fixture.repo / "mode.txt", 0o755)
        (self.fixture.repo / "link").unlink()
        os.symlink(str(outside) + "-different", self.fixture.repo / "link")
        (self.fixture.repo / "file.txt").unlink()
        os.symlink("mode.txt", self.fixture.repo / "file.txt")
        summary = self.summary(self.plan(inventory))
        self.assert_count(summary, "staged", 0)
        self.assert_count(summary, "unstaged", 3)
        self.assertEqual({decode_path(path) for path in summary["unstaged"]["samples"]},
                         {b"mode.txt", b"link", b"file.txt"})

    def test_control_and_unicode_path_samples_preserve_original_bytes(self):
        self.fixture.init()
        inventory, _ = self.inventory()
        names = ["odd\t\né.txt", ".hidden/note.txt"]
        for name in names:
            self.fixture.write(name, b"untracked\n")
        summary = self.summary(self.plan(inventory))
        self.assert_count(summary, "untracked", 2)
        self.assertEqual({decode_path(path) for path in summary["untracked"]["samples"]},
                         {os.fsencode(name) for name in names})
        text = self.run_cli("plan", "--inventory", inventory, "--target", "refs/heads/main").decode()
        self.assertNotIn("odd\t\né.txt", text, "raw control path injected extra terminal structure")

    def test_same_size_and_mtime_change_ignores_historical_local_manifest(self):
        self.fixture.init()
        inventory, saved = self.inventory(local=True)
        self.assertTrue(saved["local_inventories"], "fixture must carry a genuinely old manifest")
        tracked = self.fixture.repo / "tracked.txt"
        before = tracked.stat()
        self.fixture.write("tracked.txt", b"disk\n")
        os.utime(tracked, ns=(before.st_atime_ns, before.st_mtime_ns))
        self.assertEqual(tracked.stat().st_size, before.st_size)
        self.assertEqual(tracked.stat().st_mtime_ns, before.st_mtime_ns)
        summary = self.summary(self.plan(inventory))
        self.assert_count(summary, "staged", 0)
        self.assert_count(summary, "unstaged", 1)
        self.assertGreaterEqual(summary["started_unix_ms"], saved["local_inventories"][0]["finished_unix_ms"])

    def test_assume_valid_and_skip_worktree_do_not_hide_resident_changes(self):
        self.fixture.init({"assumed.txt": b"base\n", "skipped.txt": b"base\n"})
        self.fixture.git("update-index", "--assume-unchanged", "assumed.txt")
        self.fixture.git("update-index", "--skip-worktree", "skipped.txt")
        inventory, _ = self.inventory()
        self.fixture.write("assumed.txt", b"edit\n")
        self.fixture.write("skipped.txt", b"edit\n")
        summary = self.summary(self.plan(inventory))
        self.assertTrue(summary["index_complete"])
        self.assert_count(summary, "unstaged", 2)

    def test_split_index_is_explicitly_unknown_and_never_refreshed(self):
        self.fixture.init()
        self.fixture.git("update-index", "--split-index")
        self.assertTrue(list((self.fixture.repo / ".git").glob("sharedindex.*")),
                        "split positive fixture requires real shared-index bytes")
        inventory, _ = self.inventory()
        summary = self.summary(self.plan(inventory))
        self.assertFalse(summary["index_complete"])
        self.assertFalse(summary["unstaged"]["complete"])
        self.assertTrue(summary["gaps"])

    def test_valid_index_v4_is_explicitly_unknown(self):
        self.fixture.init()
        self.fixture.git("update-index", "--index-version=4")
        self.assertEqual(int.from_bytes((self.fixture.repo / ".git/index").read_bytes()[4:8], "big"), 4)
        inventory, _ = self.inventory()
        summary = self.summary(self.plan(inventory))
        self.assertFalse(summary["index_complete"])
        self.assertFalse(summary["unstaged"]["complete"])
        self.assertTrue(summary["gaps"])

    def test_corrupt_index_checksum_is_not_a_clean_empty_index(self):
        self.fixture.init()
        inventory, _ = self.inventory()
        index = self.fixture.repo / ".git/index"
        raw = bytearray(index.read_bytes())
        raw[-1] ^= 1
        index.write_bytes(raw)
        summary = self.summary(self.plan(inventory))
        self.assertFalse(summary["index_complete"])
        self.assertFalse(summary["staged"]["complete"])
        self.assertFalse(summary["unstaged"]["complete"])
        self.assertTrue(summary["gaps"])

    def test_missing_index_is_not_an_empty_successful_index(self):
        self.fixture.init()
        inventory, _ = self.inventory()
        (self.fixture.repo / ".git/index").unlink()
        summary = self.summary(self.plan(inventory))
        self.assertFalse(summary["index_complete"])
        self.assertFalse(summary["unstaged"]["complete"])
        self.assertTrue(summary["gaps"])

    def test_unborn_head_is_unknown_with_real_interpreted_index(self):
        self.fixture.init(commit=False)
        self.fixture.git("add", "tracked.txt")
        inventory, _ = self.inventory()
        summary = self.summary(self.plan(inventory))
        self.assertIsNone(summary["head"])
        self.assertFalse(summary["head_complete"])
        self.assertTrue(summary["index_complete"])
        self.assertFalse(summary["staged"]["complete"])
        self.assertTrue(summary["gaps"])

    def remove_blob(self, payload):
        oid = hashlib.sha1(b"blob " + str(len(payload)).encode() + b"\0" + payload).hexdigest()
        path = self.fixture.repo / ".git/objects" / oid[:2] / oid[2:]
        self.assertTrue(path.is_file(), "independent literal blob OID must identify a real fixture object")
        path.unlink()

    def test_missing_head_blob_prevents_complete_head_evidence(self):
        self.fixture.init()
        inventory, _ = self.inventory()
        self.remove_blob(b"head\n")
        summary = self.summary(self.plan(inventory))
        self.assertFalse(summary["head_complete"])
        self.assertTrue(summary["index_complete"])
        self.assertFalse(summary["unstaged"]["complete"])
        self.assertTrue(summary["gaps"])

    def test_missing_staged_blob_preserves_index_and_positive_staged_count(self):
        self.fixture.init()
        self.fixture.write("tracked.txt", b"staged distinct blob\n")
        self.fixture.git("add", "tracked.txt")
        inventory, _ = self.inventory()
        self.remove_blob(b"staged distinct blob\n")
        summary = self.summary(self.plan(inventory))
        self.assertTrue(summary["index_complete"], "object availability must not erase raw-index interpretation")
        self.assertEqual(summary["staged"]["observed"], 1, "known staged work must survive missing staged blob")
        self.assertFalse(summary["unstaged"]["complete"])
        self.assertTrue(summary["gaps"])

    def test_sha256_clean_and_changed_paths_use_complete_object_ids(self):
        self.fixture.init(object_format="sha256")
        inventory, _ = self.inventory()
        clean = self.summary(self.plan(inventory))
        self.assertEqual(len(clean["head"]), 64)
        self.assertTrue(clean["head_complete"])
        self.assert_count(clean, "unstaged", 0)
        self.fixture.write("tracked.txt", b"sha256 index\n")
        self.fixture.git("add", "tracked.txt")
        self.fixture.write("tracked.txt", b"sha256 disk\n")
        summary = self.summary(self.plan(inventory))
        self.assert_count(summary, "staged", 1)
        self.assert_count(summary, "unstaged", 1)

    def test_nested_repository_cannot_receive_clean_disk_coverage(self):
        self.fixture.init({"src/main.rs": b"fn main() {}\n"})
        nested = self.fixture.repo / "src/vendor"
        nested.mkdir()
        self.fixture.git("-C", nested, "init", "-q", "-b", "main")
        inventory, _ = self.inventory()
        summary = self.summary(self.plan(inventory))
        self.assertFalse(summary["disk_complete"])
        self.assertTrue(summary["gaps"])

    def test_gitlink_cannot_receive_clean_local_coverage(self):
        self.fixture.init()
        head = self.fixture.git("rev-parse", "HEAD").strip().decode()
        self.fixture.git("update-index", "--add", "--cacheinfo", f"160000,{head},module")
        inventory, _ = self.inventory()
        summary = self.summary(self.plan(inventory))
        self.assertFalse(summary["unstaged"]["complete"])
        self.assertTrue(any("gitlink" in gap["reason"] for gap in summary["gaps"]),
                        "unsupported gitlink needs an explicit gap, not a fabricated clean result")
        facets = [summary[key] for key in ["head_complete", "index_complete", "disk_complete"]]
        facets += [summary[key]["complete"] for key in
                   ["staged", "unstaged", "conflicts", "untracked", "ignored"]]
        self.assertFalse(all(facets), "uninterpreted gitlink cannot establish complete local equality")

    def test_physical_local_fifo_preserves_graph_and_keep_reason(self):
        self.fixture.init()
        os.mkfifo(self.fixture.repo / "untracked.pipe")
        inventory, _ = self.inventory()
        report = self.plan(inventory)
        summary = self.summary(report)
        self.assertFalse(summary["disk_complete"])
        self.assertTrue(summary["gaps"])
        self.assert_kept_main(report)
        self.assertNotIn("family collection stopped", " ".join(self.item(report)["unknowns"]))

    def test_ignorepaths_timeout_preserves_graph_and_keep_reason(self):
        self.fixture.init()
        os.mkfifo(self.fixture.repo / ".gitignore")
        self.fixture.write("untracked.txt", b"ordinary content forces IgnorePaths\n")
        inventory, _ = self.inventory()
        report = self.plan(inventory)
        summary = self.summary(report)
        # C3 is incomplete, so deep work is skipped. Graph facts and local
        # positives remain available independently of IgnorePaths failure.
        self.assertFalse(summary["untracked"]["complete"])
        self.assertFalse(summary["ignored"]["complete"])
        self.assertRegex(" ".join(gap["reason"] for gap in summary["gaps"]),
                         r"(?i)(ignore.*timeout|timeout.*ignore)",
                         "fixture must reproduce a real IgnorePaths timeout")
        self.assert_kept_main(report)
        self.assertNotIn("family collection stopped", " ".join(self.item(report)["unknowns"]))

    def test_git_blob_timeout_stops_only_first_family_and_second_stays_integrated(self):
        self.fixture.init()
        healthy = self.fixture.root / "zz-healthy"
        healthy.mkdir()
        self.fixture.git("-C", healthy, "init", "-q", "-b", "main")
        (healthy / "tracked.txt").write_bytes(b"healthy independent family\n")
        self.fixture.git("-C", healthy, "add", "tracked.txt")
        self.fixture.git("-C", healthy, "commit", "-q", "-m", "Healthy family")
        linked = self.fixture.root / "zz-healthy-linked"
        self.fixture.git("-C", healthy, "worktree", "add", "--detach", linked)
        self.fixture.manifest_roots.update((healthy, linked))
        inventory, saved = self.inventory(scope_root=True)
        paths = [decode_path(family["common_dir"]) for family in saved["families"]]
        self.assertEqual(paths, [os.fsencode(self.fixture.repo / ".git"), os.fsencode(healthy / ".git")],
                         "blocked family must precede healthy family in the real inventory")
        payload = b"head\n"
        oid = hashlib.sha1(b"blob " + str(len(payload)).encode() + b"\0" + payload).hexdigest()
        blocked = self.fixture.repo / ".git/objects" / oid[:2] / oid[2:]
        self.assertTrue(blocked.is_file())
        blocked.unlink()
        os.mkfifo(blocked)
        # Keep the timeout scenario's two explicit family-local main targets.
        mapping = {"schema": "wt-sweep.targets/1", "families": []}
        for repo in (self.fixture.repo, healthy):
            common = repo / ".git"
            identity = common.stat()
            mapping["families"].append({
                "common_dir": {"hex": os.fsencode(common).hex()},
                "identity": {"device": identity.st_dev, "inode": identity.st_ino},
                "targets": [{"hex": b"refs/heads/main".hex()}],
            })
        target_map = self.fixture.root / "explicit-timeout-family-targets.json"
        target_map.write_text(json.dumps(mapping))
        self.fixture.manifest_roots.add(target_map)
        report = self.plan(inventory, target_map=target_map)
        self.assertEqual({decode_path(item["path"]) for item in report["items"]},
                         {os.fsencode(self.fixture.repo), os.fsencode(healthy), os.fsencode(linked)})
        first = self.item(report)
        self.assertFalse(any(observation["target_role"] == "integration"
                             for observation in first["observations"]),
                         "blocked unverified history cannot produce an integration proof")
        self.assertRegex(" ".join(first["unknowns"]), r"(?i)(family.*stopp|stopp.*family)",
                         "first family's unknowns must name its explicit stop")
        self.assertIn("family collection stopped after Git timeout in ", " ".join(first["unknowns"]))
        self.assert_integrated(report, linked)
        summary = self.summary(report, healthy)
        self.assertTrue(summary["head_complete"])
        self.assert_count(summary, "unstaged", 0)

    def test_positive_staged_count_survives_staged_blob_timeout(self):
        self.fixture.init()
        payload = b"staged FIFO object\n"
        self.fixture.write("tracked.txt", payload)
        self.fixture.git("add", "tracked.txt")
        inventory, _ = self.inventory()
        oid = hashlib.sha1(b"blob " + str(len(payload)).encode() + b"\0" + payload).hexdigest()
        blocked = self.fixture.repo / ".git/objects" / oid[:2] / oid[2:]
        self.assertTrue(blocked.is_file())
        blocked.unlink()
        os.mkfifo(blocked)
        summary = self.summary(self.plan(inventory))
        self.assertTrue(summary["index_complete"])
        self.assertEqual(summary["staged"]["observed"], 1,
                         "positive staged comparison must survive later object timeout")
        self.assertFalse(summary["unstaged"]["complete"])
        self.assertRegex(" ".join(gap["reason"] for gap in summary["gaps"]), r"(?i)timeout",
                         "staged FIFO fixture must reproduce an actual Git object timeout")

    def test_25001_ordinary_index_entries_produce_small_exact_clean_summary(self):
        self.fixture.init()
        # This one repository is separate from all many-worktree fixtures.
        (self.fixture.repo / "tracked.txt").unlink()
        self.fixture.git("update-index", "--force-remove", "tracked.txt")
        payload = b"large fixture\n"
        oid = self.fixture.git("hash-object", "-w", "--stdin", data=payload).strip()
        names = [f"entry-{number:05}.txt" for number in range(25001)]
        for name in names:
            self.fixture.write(name, payload)
        self.fixture.git("update-index", "-z", "--index-info", data=b"".join(
            b"100644 " + oid + b"\t" + name.encode() + b"\0" for name in names))
        tree = self.fixture.git("write-tree").strip().decode()
        commit = self.fixture.git("commit-tree", tree, data=b"large fixture\n").strip().decode()
        self.fixture.git("update-ref", "refs/heads/main", commit)
        index = (self.fixture.repo / ".git/index").read_bytes()
        self.assertEqual(int.from_bytes(index[8:12], "big"), 25001, "fixture index must exceed v1 limit")
        inventory, _ = self.inventory()
        summary = self.summary(self.plan(inventory))
        for key in ["staged", "unstaged", "conflicts", "untracked", "ignored"]:
            self.assert_count(summary, key, 0)
        self.assertTrue(summary["head_complete"])
        self.assertTrue(summary["index_complete"])
        self.assertTrue(summary["disk_complete"])
        self.assertFalse(summary["gaps"])
        self.assertLess(len(json.dumps(summary, separators=(",", ":")).encode()), 64 * 1024)
        self.assertNotIn("entries", summary, "compact summary must not serialize 25001 manifest records")


if __name__ == "__main__":
    unittest.main()
