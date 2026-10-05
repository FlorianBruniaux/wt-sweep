"""Independent T04 Git fixtures and CLI oracles; no product code is imported."""

import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import unittest

from inventory_support import BINARY as DEFAULT_BINARY, GIT, clean_environment, decode_path, filesystem_manifest


BINARY = Path(os.environ.get("WT_SWEEP_PRIVATE_HISTORY_BINARY", str(DEFAULT_BINARY)))


def bounded_process(argv, env, data=b"", timeout=15):
    """Stop the owned process group and reap the direct child on every timeout."""
    process = subprocess.Popen(list(map(str, argv)), env=env, stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               start_new_session=True)
    try:
        stdout, stderr = process.communicate(input=data, timeout=timeout)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.kill()
        process.communicate(timeout=5)
        raise AssertionError("owned fixture child exceeded deadline; group stopped and child reaped") from None
    return subprocess.CompletedProcess(process.args, process.returncode, stdout, stderr)


class PrivateHistoryFixture:
    """Synthetic commit topology and source bytes constructed independently."""
    def __init__(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="wt-sweep-t04-cli-", dir="/private/tmp")
        self.root = Path(self.temporary.name)
        self.env = clean_environment(self.root)
        self.env.update(GIT_CONFIG_SYSTEM=os.devnull, GIT_ATTR_NOSYSTEM="1",
                        GIT_OPTIONAL_LOCKS="0", GIT_NO_LAZY_FETCH="1",
                        GIT_ALLOW_PROTOCOL="", GIT_NO_REPLACE_OBJECTS="1", TMPDIR="/private/tmp")
        self.clock = 1700000000
        self.sequence = 0
        self.repo = self.root / "principal"
        self.linked = self.root / "linked topic"
        self.git(self.root, "init", "--template=", "-q", "-b", "main", self.repo)
        (self.repo / "tracked.txt").write_bytes(b"synthetic base\n")
        self.base = self.commit(self.repo, "Synthetic base", "tracked.txt")
        self.git(self.repo, "worktree", "add", "-q", "-b", "topic", self.linked)
        self.expected_heads = {self.repo: self.base, self.linked: self.base}
        self.manifest_roots = {self.repo, self.linked}
        self.sentinel = self.root / "outside sentinel"
        self.sentinel.mkdir()
        (self.sentinel / "unchanged.bin").write_bytes(b"synthetic sentinel\x00\xff\n")
        self.manifest_roots.add(self.sentinel)
        self.stashes = []

    def tick(self):
        self.clock += 1
        self.env.update(GIT_AUTHOR_DATE=f"{self.clock} +0000", GIT_COMMITTER_DATE=f"{self.clock} +0000")

    def git(self, repo, *arguments, data=b"", expected=0):
        argv = [GIT]
        for setting in ("core.hooksPath=/dev/null", "core.fsmonitor=false", "core.untrackedCache=false",
                        "gc.auto=0", "maintenance.auto=false", "protocol.allow=never",
                        "core.autocrlf=false", "core.filemode=true"):
            argv.extend(("-c", setting))
        result = bounded_process([*argv, "-C", repo, *arguments], self.env, data)
        if result.returncode != expected:
            raise AssertionError((arguments, result.returncode, result.stderr.decode(errors="replace")))
        return result.stdout

    def oid(self, repo, revision="HEAD"):
        return self.git(repo, "rev-parse", "--verify", revision).strip().decode("ascii")

    def commit(self, repo, message, *paths):
        self.tick()
        self.git(repo, "add", "--", *paths)
        self.git(repo, "commit", "-q", "-m", message)
        return self.oid(repo)

    def own_gitdir(self, repo):
        return Path(os.fsdecode(self.git(repo, "rev-parse", "--absolute-git-dir").removesuffix(b"\n")))

    def detached(self, name, revision=None):
        path = self.root / name
        self.git(self.repo, "worktree", "add", "-q", "--detach", path, revision or self.base)
        self.expected_heads[path] = revision or self.base
        self.manifest_roots.add(path)
        return path

    def exclusive_same_tree(self, repo=None):
        repo = repo or self.linked
        self.tick()
        tree = self.oid(repo, "HEAD^{tree}")
        return self.git(repo, "commit-tree", tree, "-p", self.base,
                        "-m", "Exclusive synthetic history with identical tree").strip().decode("ascii")

    def reset_detached_history(self):
        detached = self.detached("reset detached")
        (detached / "tracked.txt").write_bytes(b"exclusive detached content\n")
        exclusive = self.commit(detached, "Exclusive detached history", "tracked.txt")
        self.git(detached, "reset", "--hard", self.base)
        self.expected_heads[detached] = self.base
        return detached, exclusive

    def two_stashes(self):
        (self.repo / "tracked.txt").write_bytes(b"stashed main bytes\n")
        self.tick()
        self.git(self.repo, "stash", "push", "-q", "-m", "branch topic is only synthetic text")
        first = self.oid(self.repo, "refs/stash")
        self.stashes.append({"oid": first, "base": self.base,
                             "message": "On main: branch topic is only synthetic text",
                             "files": {b"tracked.txt"}})
        raw_name = b"untracked space\nname.txt"
        (self.linked / os.fsdecode(raw_name)).write_bytes(b"synthetic stashed untracked\x00\xff\n")
        self.tick()
        self.git(self.linked, "stash", "push", "-q", "--include-untracked", "-m", "branch main is only synthetic text")
        second = self.oid(self.repo, "refs/stash")
        self.stashes.insert(0, {"oid": second, "base": self.base,
                               "message": "On topic: branch main is only synthetic text",
                               "files": {raw_name}})
        return self.stashes

    def private_ref(self, repo=None, namespace="refs/worktree"):
        repo = repo or self.linked
        exclusive = self.exclusive_same_tree(repo)
        name = namespace + "/exclusive"
        self.git(repo, "update-ref", name, exclusive)
        return name, exclusive

    def orphan_private_reflog(self, repo=None, root_ref=False):
        repo = repo or self.linked
        exclusive = self.exclusive_same_tree(repo)
        name = "ORIG_HEAD" if root_ref else "refs/rewritten/orphan"
        # Keep the exclusive OID only in the OLD slot. This catches new-only parsers.
        path = self.own_gitdir(repo) / "logs" / name
        path.parent.mkdir(parents=True, exist_ok=True)
        payload = (f"{exclusive} {'0' * len(exclusive)} Fixture <fixture@example.invalid> "
                   "1700000042 +0000\tsynthetic orphan reflog\n").encode("ascii")
        path.write_bytes(payload)
        return name, exclusive, path

    def distinct_clone(self):
        # Recreate locally without transport: same object and URL, different physical identity.
        other = self.root / "distinct clone"
        self.git(self.root, "init", "--template=", "-q", "-b", "main", other)
        (other / "tracked.txt").write_bytes(b"synthetic base\n")
        saved_dates = {key: self.env[key] for key in ("GIT_AUTHOR_DATE", "GIT_COMMITTER_DATE")}
        self.env.update(GIT_AUTHOR_DATE="1700000001 +0000", GIT_COMMITTER_DATE="1700000001 +0000")
        self.git(other, "add", "--", "tracked.txt")
        self.git(other, "commit", "-q", "-m", "Synthetic base")
        self.env.update(saved_dates)
        assert self.oid(other) == self.base, "fixture must share full OID as well as URL"
        for repo in (self.repo, other):
            self.git(repo, "config", "remote.origin.url", "https://synthetic.example.invalid/same/project.git")
        self.expected_heads[other] = self.base
        self.manifest_roots.add(other)
        return other

    def manifest(self):
        return {os.fsencode(path): filesystem_manifest(path) for path in self.manifest_roots}

    def close(self):
        self.temporary.cleanup()


class PrivateHistoryFixtureFactsTests(unittest.TestCase):
    """Validate fixture facts with Git before connecting a product field contract."""
    def setUp(self):
        self.fixture = PrivateHistoryFixture()
        self.addCleanup(self.fixture.close)

    def assert_not_anchored(self, oid):
        fixture = self.fixture
        refs = fixture.git(fixture.repo, "for-each-ref", "--format=%(refname)", "refs/heads", "refs/remotes").splitlines()
        self.assertEqual(set(refs), {b"refs/heads/main", b"refs/heads/topic"})
        for ref in refs:
            fixture.git(fixture.repo, "merge-base", "--is-ancestor", oid, ref.decode(), expected=1)

    def test_two_worktrees_observe_identical_nonempty_two_stash_lists(self):
        fixture = self.fixture
        expected = fixture.two_stashes()
        before = fixture.manifest()
        outputs = [fixture.git(repo, "stash", "list", "--format=%H") for repo in (fixture.repo, fixture.linked)]
        self.assertEqual(outputs[0], outputs[1])
        self.assertEqual(outputs[0].splitlines(), [entry["oid"].encode() for entry in expected])
        for entry in expected:
            self.assertEqual(fixture.oid(fixture.repo, entry["oid"] + "^1"), fixture.base)
        first_files = fixture.git(fixture.repo, "diff-tree", "--no-commit-id", "--name-only", "-r", "-z",
                                  fixture.base, expected[1]["oid"]).split(b"\0")[:-1]
        second_files = fixture.git(fixture.repo, "ls-tree", "-r", "--name-only", "-z",
                                   expected[0]["oid"] + "^3").split(b"\0")[:-1]
        self.assertEqual(set(first_files), expected[1]["files"])
        self.assertEqual(set(second_files), expected[0]["files"])
        self.assertEqual(before, fixture.manifest(), "independent fact reads changed fixture")

    def test_reset_detached_retains_exclusive_oid_in_own_old_reflog_slot(self):
        fixture = self.fixture
        detached, exclusive = fixture.reset_detached_history()
        self.assertEqual(fixture.oid(detached), fixture.base)
        self.assertEqual(fixture.git(detached, "status", "--porcelain"), b"")
        reflog = (fixture.own_gitdir(detached) / "logs/HEAD").read_bytes()
        self.assertTrue(any(line.startswith(exclusive.encode() + b" ") for line in reflog.splitlines()))
        self.assert_not_anchored(exclusive)
        self.assertNotIn(exclusive.encode(), (fixture.own_gitdir(fixture.linked) / "logs/HEAD").read_bytes())

    def test_clean_detached_head_has_real_named_anchor(self):
        fixture = self.fixture
        detached = fixture.detached("covered detached")
        self.assertEqual(fixture.git(detached, "status", "--porcelain"), b"")
        fixture.git(detached, "merge-base", "--is-ancestor", fixture.oid(detached), "refs/heads/main")
        self.assertEqual(fixture.oid(detached), fixture.base)

    def test_private_ref_and_orphan_reflog_are_distinct_uncovered_same_tree_roots(self):
        fixture = self.fixture
        _, private = fixture.private_ref()
        _, orphan, path = fixture.orphan_private_reflog()
        self.assertNotEqual(private, orphan)
        self.assert_not_anchored(private)
        self.assert_not_anchored(orphan)
        self.assertEqual(fixture.oid(fixture.repo, private + "^{tree}"), fixture.oid(fixture.repo, "main^{tree}"))
        self.assertEqual(path.read_bytes().split(b" ", 1)[0], orphan.encode())
        self.assertFalse((fixture.own_gitdir(fixture.linked) / "refs/rewritten/orphan").exists())

    def test_other_worktree_head_is_not_a_named_common_anchor(self):
        fixture = self.fixture
        exclusive = fixture.exclusive_same_tree()
        other = fixture.detached("other detached", exclusive)
        fixture.private_ref()  # another independent private root, no common anchor added
        self.assertEqual(fixture.oid(other), exclusive)
        self.assert_not_anchored(exclusive)

    def test_linked_root_ref_orphan_reflog_is_in_own_logs_directory(self):
        fixture = self.fixture
        _, exclusive, path = fixture.orphan_private_reflog(root_ref=True)
        self.assertEqual(path.parent, fixture.own_gitdir(fixture.linked) / "logs")
        generic = Path(os.fsdecode(fixture.git(fixture.linked, "rev-parse", "--path-format=absolute",
                                               "--git-path", "logs/ORIG_HEAD").removesuffix(b"\n")))
        self.assertNotEqual(path, generic, "fixture must expose Git generic routing caveat")
        self.assertFalse(generic.exists())
        self.assert_not_anchored(exclusive)

    def test_same_remote_and_same_oid_are_two_physical_families(self):
        fixture = self.fixture
        other = fixture.distinct_clone()
        self.assertEqual(fixture.oid(other), fixture.base)
        urls = [fixture.git(repo, "config", "--get", "remote.origin.url") for repo in (fixture.repo, other)]
        self.assertEqual(urls[0], urls[1])
        metadata = [fixture.own_gitdir(repo).stat() for repo in (fixture.repo, other)]
        self.assertNotEqual((metadata[0].st_dev, metadata[0].st_ino), (metadata[1].st_dev, metadata[1].st_ino))


class PrivateHistoryCliTests(unittest.TestCase):
    def setUp(self):
        self.fixture = PrivateHistoryFixture()
        self.addCleanup(self.fixture.close)

    def run_cli_result(self, *arguments, expected=0):
        fixture = self.fixture
        before = fixture.manifest()
        try:
            result = bounded_process([BINARY, *arguments], fixture.env, timeout=40)
        finally:
            self.assertEqual(before, fixture.manifest(),
                             "CLI changed fixture/source/index/admin/sentinel/inventory bytes or metadata (atime excluded)")
        self.assertEqual(result.returncode, expected, result.stderr.decode(errors="replace"))
        return result

    def run_cli(self, *arguments):
        return json.loads(self.run_cli_result(*arguments).stdout)

    def inventory(self, root_scope=False):
        fixture = self.fixture
        scope = ("--root", fixture.root) if root_scope else ("--repo", fixture.repo)
        inventory = self.run_cli("scan", *scope, "--format", "json")
        self.assertEqual(inventory["schema"], "wt-sweep.inventory/1")
        fixture.sequence += 1
        path = fixture.root / f"input-inventory-{fixture.sequence}.json"
        path.write_text(json.dumps(inventory))
        fixture.manifest_roots.add(path)
        return path, inventory

    def report(self, inventory=None, root_scope=False, targets=("refs/heads/main",), target_map=None):
        if inventory is None:
            inventory, _ = self.inventory(root_scope)
        arguments = ["plan", "--inventory", inventory, "--format", "json"]
        if target_map is not None:
            self.assertFalse(targets, "explicit map callers must omit simple targets")
            arguments.extend(("--target-map", target_map))
        for target in targets:
            arguments.extend(("--target", target))
        report = self.run_cli(*arguments)
        self.assertEqual(report["schema"], "wt-sweep.review/2")
        body = {key: value for key, value in report.items() if key != "digest_sha256"}
        digest = hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":"),
                                          ensure_ascii=False).encode()).hexdigest()
        self.assertEqual(report["digest_sha256"], digest, "review digest must bind new private and stash evidence")
        items = {decode_path(item["path"]): item for item in report["items"]}
        self.assertEqual(len(items), len(report["items"]), "duplicate physical worktree review")
        self.assertEqual(set(items), {os.fsencode(path) for path in self.fixture.expected_heads},
                         "review omitted or invented fixture worktrees")
        return report, items

    def validate_sources(self, collection):
        sources = collection["sources"]
        self.assertTrue(sources, "source-less enumeration is not auditable")
        for source in sources:
            self.assertEqual(set(source), {"path", "kind", "exists", "sha256", "bytes", "complete"})
            raw = decode_path(source["path"])
            self.assertTrue(os.path.isabs(raw))
            self.assertTrue(Path(os.fsdecode(raw)).is_relative_to(self.fixture.root), "source escaped fixture root")
            path = Path(os.fsdecode(raw))
            if source["exists"] is False:
                self.assertFalse(os.path.lexists(path), "claimed absent source actually exists")
                self.assertTrue(source["complete"], "absence must be a successful anchored observation")
                self.assertIsNone(source["sha256"])
                self.assertIsNone(source["bytes"])
            elif source["exists"] is True and source["complete"] and path.is_file() and not path.is_symlink():
                payload = path.read_bytes()
                self.assertEqual(source["sha256"], hashlib.sha256(payload).hexdigest(),
                                 "source SHA256 does not bind current fixture bytes")
                self.assertEqual(source["bytes"], len(payload))

    def history(self, items, repo=None):
        repo = repo or self.fixture.linked
        item = items[os.fsencode(repo)]
        self.assertTrue("private_history" in item, "T04 requires current private_history in each review item")
        history = item["private_history"]
        self.assertIsInstance(history, dict, "present fixture worktree requires collected private history")
        self.assertEqual(set(history), {"git_dir", "git_dir_identity", "started_unix_ms", "finished_unix_ms",
                                       "enumeration_complete", "anchors_complete", "temporal_consistency",
                                       "sources", "roots", "gaps"})
        gitdir = self.fixture.own_gitdir(repo)
        self.assertEqual(decode_path(history["git_dir"]), os.fsencode(gitdir))
        metadata = gitdir.stat()
        self.assertEqual(history["git_dir_identity"], {"device": metadata.st_dev, "inode": metadata.st_ino})
        self.assertLessEqual(history["started_unix_ms"], history["finished_unix_ms"])
        self.assertEqual(history["temporal_consistency"], "not_guaranteed")
        self.validate_sources(history)
        seen = set()
        for root in history["roots"]:
            self.assertEqual(set(root), {"oid", "provenance", "result", "anchor", "observed_result", "reason"})
            self.assertNotIn(root["oid"], seen, "same private root duplicated instead of collecting provenance")
            seen.add(root["oid"])
            self.assertEqual(len(root["oid"]), len(self.fixture.base), "truncated or lossy private OID")
            self.assertTrue(root["provenance"], "private root lost its source")
            for provenance in root["provenance"]:
                self.assertEqual(set(provenance), {"source", "kind"})
                self.assertTrue(os.path.isabs(decode_path(provenance["source"])))
            self.assertIn(root["result"], {"covered", "not_preserved_by_retained_refs", "unknown"})
            self.assertIsNone(root["observed_result"], "stable fixture must not present invalidated observation")
            if root["result"] == "covered":
                self.assertIsInstance(root["anchor"], dict)
                self.assertEqual(set(root["anchor"]), {"reference", "oid"})
                reference = decode_path(root["anchor"]["reference"])
                self.assertTrue(reference.startswith(b"refs/"), "HEAD/reflog used as retained named anchor")
                self.assertFalse(reference.startswith((b"refs/worktree/", b"refs/bisect/", b"refs/rewritten/")),
                                 "private ref became a retained common anchor")
                self.assertEqual(self.fixture.oid(repo, os.fsdecode(reference)), root["anchor"]["oid"])
                self.fixture.git(repo, "merge-base", "--is-ancestor", root["oid"], root["anchor"]["oid"])
            else:
                self.assertIsNone(root["anchor"], "noncovered stable root claimed covering anchor")
        if any(root["result"] == "not_preserved_by_retained_refs" for root in history["roots"]):
            self.assertEqual(item["classification"], "keep")
            self.assertIn("private_history_not_preserved", item["decision"]["keep_reasons"])
        return history

    def root(self, history, oid):
        matches = [root for root in history["roots"] if root["oid"] == oid]
        self.assertEqual(len(matches), 1, "known real fixture private root omitted or duplicated")
        return matches[0]

    def assert_complete_covered(self, history):
        self.assertTrue(history["enumeration_complete"], history["gaps"])
        self.assertTrue(history["anchors_complete"], history["gaps"])
        self.assertTrue(history["roots"], "always-empty collector cannot satisfy positive coverage")
        self.assertTrue(all(root["result"] == "covered" for root in history["roots"]), history)
        self.assertEqual(history["gaps"], [])

    def stashes(self, report):
        self.assertTrue("shared_stashes" in report, "T04 requires family-level shared stash contexts")
        for family in report["shared_stashes"]:
            self.assertEqual(set(family), {"common_dir", "identity", "started_unix_ms", "finished_unix_ms",
                                          "enumeration_complete", "temporal_consistency", "sources", "entries", "gaps"})
            path = Path(os.fsdecode(decode_path(family["common_dir"])))
            metadata = path.stat()
            self.assertEqual(family["identity"], {"device": metadata.st_dev, "inode": metadata.st_ino})
            self.assertLessEqual(family["started_unix_ms"], family["finished_unix_ms"])
            self.assertEqual(family["temporal_consistency"], "not_guaranteed")
            self.validate_sources(family)
            for entry in family["entries"]:
                self.assertEqual(set(entry), {"oid", "base_oid", "timestamp_unix", "message", "message_truncated",
                                             "files", "metadata_complete", "files_complete", "gaps"},
                                 "stash may not acquire ownership from branch message")
        return report["shared_stashes"]

    def test_shared_stashes_are_one_exact_family_list_without_branch_attribution(self):
        fixture = self.fixture
        expected = fixture.two_stashes()
        report, items = self.report()
        families = self.stashes(report)
        self.assertEqual(len(families), 1, "linked worktrees duplicated their common stash context")
        family = families[0]
        self.assertEqual(decode_path(family["common_dir"]), os.fsencode(fixture.repo / ".git"))
        self.assertTrue(family["enumeration_complete"], family["gaps"])
        self.assertEqual({entry["oid"] for entry in family["entries"]}, {entry["oid"] for entry in expected})
        self.assertEqual(len(family["entries"]), 2)
        actual = {entry["oid"]: entry for entry in family["entries"]}
        for known in expected:
            entry = actual[known["oid"]]
            self.assertTrue(entry["metadata_complete"], entry["gaps"])
            self.assertTrue(entry["files_complete"], entry["gaps"])
            self.assertEqual(entry["base_oid"], known["base"])
            self.assertIsInstance(entry["timestamp_unix"], int)
            self.assertEqual(decode_path(entry["message"]), known["message"].encode())
            self.assertFalse(entry["message_truncated"])
            self.assertEqual({decode_path(path) for path in entry["files"]}, known["files"])
        for repo in (fixture.repo, fixture.linked):
            self.assert_complete_covered(self.history(items, repo))
            expected = "keep" if repo == fixture.repo else "cleanup_candidate"
            self.assertEqual(items[os.fsencode(repo)]["classification"], expected,
                             "shared stash context changed independent classification")
            self.assertNotIn("stashes", items[os.fsencode(repo)], "stashes were attributed to a worktree")

    def test_clean_detached_head_and_all_own_reflog_roots_are_covered(self):
        detached = self.fixture.detached("covered detached")
        _, items = self.report()
        history = self.history(items, detached)
        self.assert_complete_covered(history)
        root = self.root(history, self.fixture.base)
        self.assertEqual(root["result"], "covered")
        self.assertTrue(any(decode_path(p["source"]) == os.fsencode(self.fixture.own_gitdir(detached) / "HEAD")
                            for p in root["provenance"]))

    def test_reset_detached_old_exclusive_reflog_root_remains_known_negative(self):
        detached, exclusive = self.fixture.reset_detached_history()
        _, items = self.report()
        history = self.history(items, detached)
        self.assertTrue(history["enumeration_complete"], history["gaps"])
        self.assertTrue(history["anchors_complete"], history["gaps"])
        self.assertEqual(self.root(history, self.fixture.base)["result"], "covered")
        root = self.root(history, exclusive)
        self.assertEqual(root["result"], "not_preserved_by_retained_refs")
        self.assertTrue(any(decode_path(p["source"]) == os.fsencode(self.fixture.own_gitdir(detached) / "logs/HEAD")
                            for p in root["provenance"]), "old-only exclusive root lost own reflog provenance")
        self.assert_complete_covered(self.history(items, self.fixture.linked))

    def test_private_refs_in_all_three_namespaces_are_not_covered_by_tree_equality(self):
        known = [self.fixture.private_ref(namespace=namespace)
                 for namespace in ("refs/bisect", "refs/worktree", "refs/rewritten")]
        _, items = self.report()
        history = self.history(items)
        self.assertTrue(history["enumeration_complete"], history["gaps"])
        self.assertTrue(history["anchors_complete"], history["gaps"])
        for name, exclusive in known:
            root = self.root(history, exclusive)
            self.assertEqual(root["result"], "not_preserved_by_retained_refs")
            self.assertTrue(any(decode_path(p["source"]).endswith(os.fsencode(name)) for p in root["provenance"]))

    def test_orphan_private_reflog_old_only_root_without_live_ref_is_retained(self):
        _, exclusive, path = self.fixture.orphan_private_reflog()
        _, items = self.report()
        history = self.history(items)
        self.assertTrue(history["enumeration_complete"], history["gaps"])
        root = self.root(history, exclusive)
        self.assertEqual(root["result"], "not_preserved_by_retained_refs")
        self.assertTrue(any(decode_path(p["source"]) == os.fsencode(path) for p in root["provenance"]))

    def test_orphan_linked_root_ref_log_is_read_from_own_directory(self):
        _, exclusive, path = self.fixture.orphan_private_reflog(root_ref=True)
        _, items = self.report()
        history = self.history(items)
        root = self.root(history, exclusive)
        self.assertEqual(root["result"], "not_preserved_by_retained_refs")
        self.assertTrue(any(decode_path(p["source"]) == os.fsencode(path) for p in root["provenance"]))

    def test_other_worktree_head_is_not_a_retained_anchor(self):
        fixture = self.fixture
        exclusive = fixture.exclusive_same_tree()
        fixture.git(fixture.linked, "update-ref", "refs/worktree/exclusive", exclusive)
        other = fixture.detached("exclusive other head", exclusive)
        _, items = self.report()
        for repo in (fixture.linked, other):
            history = self.history(items, repo)
            self.assertEqual(self.root(history, exclusive)["result"], "not_preserved_by_retained_refs")

    def test_common_stored_private_namespace_ref_is_excluded_as_anchor(self):
        _, exclusive = self.fixture.private_ref(repo=self.fixture.repo)
        _, items = self.report()
        history = self.history(items, self.fixture.repo)
        self.assertEqual(self.root(history, exclusive)["result"], "not_preserved_by_retained_refs")

    def test_retained_named_descendant_genuinely_covers_private_root(self):
        fixture = self.fixture
        _, exclusive = fixture.private_ref()
        fixture.tick()
        descendant = fixture.git(fixture.repo, "commit-tree", fixture.oid(fixture.repo, "HEAD^{tree}"),
                                 "-p", exclusive, "-m", "Retained synthetic descendant").strip().decode()
        fixture.git(fixture.repo, "update-ref", "refs/heads/retained", descendant)
        _, items = self.report()
        history = self.history(items)
        self.assert_complete_covered(history)
        root = self.root(history, exclusive)
        self.assertEqual(root["anchor"]["oid"], descendant)
        self.assertEqual(decode_path(root["anchor"]["reference"]), b"refs/heads/retained")

    def test_identical_remote_and_oid_do_not_merge_distinct_stash_families(self):
        fixture = self.fixture
        expected = fixture.two_stashes()
        other = fixture.distinct_clone()
        (other / "tracked.txt").write_bytes(b"independent other clone stash\n")
        fixture.tick()
        fixture.git(other, "stash", "push", "-q", "-m", "independent family")
        other_stash = fixture.oid(other, "refs/stash")
        inventory, _ = self.inventory(root_scope=True)
        # This scenario intentionally accepts main in each known physical family.
        mapping = {"schema": "wt-sweep.targets/1", "families": []}
        for repo in (fixture.repo, other):
            common = repo / ".git"
            identity = common.stat()
            mapping["families"].append({
                "common_dir": {"hex": os.fsencode(common).hex()},
                "identity": {"device": identity.st_dev, "inode": identity.st_ino},
                "targets": [{"hex": b"refs/heads/main".hex()}],
            })
        target_map = fixture.root / "explicit-stash-family-targets.json"
        target_map.write_text(json.dumps(mapping))
        fixture.manifest_roots.add(target_map)
        report, items = self.report(inventory, targets=(), target_map=target_map)
        families = self.stashes(report)
        self.assertEqual(len(families), 2, "same URL and commit fused independent physical families")
        by_common = {decode_path(family["common_dir"]): family for family in families}
        self.assertEqual(set(by_common), {os.fsencode(fixture.repo / ".git"), os.fsencode(other / ".git")})
        self.assertEqual({entry["oid"] for entry in by_common[os.fsencode(fixture.repo / ".git")]["entries"]},
                         {entry["oid"] for entry in expected})
        self.assertEqual([entry["oid"] for entry in by_common[os.fsencode(other / ".git")]["entries"]], [other_stash])
        self.assert_complete_covered(self.history(items, other))

    def test_absent_optional_sources_are_auditable_complete_absences(self):
        report, items = self.report()
        history = self.history(items)
        self.assert_complete_covered(history)
        absent = [source for source in history["sources"] if source["exists"] is False]
        self.assertTrue(absent, "missing optional source was silently omitted")
        families = self.stashes(report)
        self.assertEqual(len(families), 1)
        self.assertTrue(families[0]["enumeration_complete"], families[0]["gaps"])
        self.assertEqual(families[0]["entries"], [])

    def assert_incomplete_source(self, kind):
        fixture = self.fixture
        inventory, _ = self.inventory()
        path = fixture.own_gitdir(fixture.linked) / "logs/refs/rewritten/bad"
        path.parent.mkdir(parents=True, exist_ok=True)
        if kind == "malformed":
            path.write_bytes(b"not an OID\tmalformed synthetic reflog\n")
        elif kind == "oversized":
            path.write_bytes(b"x" * (8 * 1024 * 1024 + 1))
        elif kind == "symlink":
            path.symlink_to(fixture.sentinel / "unchanged.bin")
        elif kind == "fifo":
            os.mkfifo(path)
        elif kind == "directory":
            path.mkdir()
        else:
            raise AssertionError(kind)
        _, items = self.report(inventory)
        history = self.history(items)
        self.assertFalse(history["enumeration_complete"], "bad private source became complete known emptiness")
        self.assertTrue(history["gaps"], "private refusal reason omitted")
        matching = [source for source in history["sources"] if decode_path(source["path"]) == os.fsencode(path)]
        self.assertTrue(matching, "refused source omitted from evidence")
        self.assertTrue(all(source["exists"] is not False for source in matching), "refused source reported absent")
        self.assert_complete_covered(self.history(items, fixture.repo))

    def test_malformed_private_reflog_blocks_complete_enumeration(self):
        self.assert_incomplete_source("malformed")

    def test_oversized_private_reflog_blocks_complete_enumeration(self):
        self.assert_incomplete_source("oversized")

    def test_symlink_private_reflog_is_refused_without_following_sentinel(self):
        self.assert_incomplete_source("symlink")

    def test_fifo_private_reflog_is_refused_and_owned_children_are_reaped(self):
        self.assert_incomplete_source("fifo")

    def test_missing_private_commit_object_is_unknown_not_complete_negative(self):
        fixture = self.fixture
        _, exclusive = fixture.private_ref()
        inventory, _ = self.inventory()
        object_file = fixture.repo / ".git/objects" / exclusive[:2] / exclusive[2:]
        self.assertTrue(object_file.is_file(), "missing-object injection needs owned real loose commit")
        object_file.unlink()
        _, items = self.report(inventory)
        history = self.history(items)
        self.assertTrue(history["enumeration_complete"], "object failure erased complete source enumeration")
        root = self.root(history, exclusive)
        self.assertEqual(root["result"], "unknown", "failed object verification inferred unpreserved history")
        self.assertTrue(root["reason"] or history["gaps"], "missing object failure reason omitted")
        self.assertEqual(self.root(history, fixture.base)["result"], "covered")

    def test_noncommit_auto_merge_tree_root_stays_unknown(self):
        fixture = self.fixture
        tree = fixture.oid(fixture.linked, "HEAD^{tree}")
        fixture.git(fixture.linked, "update-ref", "AUTO_MERGE", tree)
        _, items = self.report()
        history = self.history(items)
        self.assertEqual(self.root(history, tree)["result"], "unknown", "noncommit root was promoted by identity/tree equality")
        self.assertEqual(self.root(history, fixture.base)["result"], "covered")

    def test_bad_shared_stash_log_does_not_erase_independent_private_coverage(self):
        fixture = self.fixture
        fixture.two_stashes()
        inventory, _ = self.inventory()
        path = fixture.repo / ".git/logs/refs/stash"
        path.write_bytes(path.read_bytes() + b"synthetic malformed stash log\n")
        report, items = self.report(inventory)
        families = self.stashes(report)
        self.assertEqual(len(families), 1)
        self.assertFalse(families[0]["enumeration_complete"], "bad shared stash source became complete context")
        self.assertTrue(families[0]["gaps"])
        for repo in (fixture.repo, fixture.linked):
            self.assert_complete_covered(self.history(items, repo))

    def test_bad_anchor_blocks_negative_but_preserves_independent_coverage(self):
        fixture = self.fixture
        _, exclusive = fixture.private_ref()
        # A shared named tree is unsupported as a retained anchor in commit-only T04.
        fixture.git(fixture.repo, "update-ref", "refs/tags/tree-anchor", fixture.oid(fixture.repo, "HEAD^{tree}"))
        _, items = self.report()
        history = self.history(items)
        self.assertFalse(history["anchors_complete"], "unsupported named anchor silently skipped")
        self.assertEqual(self.root(history, exclusive)["result"], "unknown", "unsupported anchor manufactured complete negative")
        self.assertEqual(self.root(history, fixture.base)["result"], "covered", "unrelated anchor failure erased valid positive")

    def test_explicit_private_target_is_refused_by_existing_cli_contract(self):
        name, _ = self.fixture.private_ref()
        inventory, _ = self.inventory()
        result = self.run_cli_result("plan", "--inventory", inventory, "--target", name,
                                     "--format", "json", expected=2)
        self.assertIn(b"invalid_inventory", result.stderr)
        self.assertIn(b"integration targets must be full branch refs", result.stderr,
                      "private target refusal must explain full branch ref requirement")
        self.assertEqual(result.stdout, b"", "refused private target emitted a successful review")

    def test_detached_current_exclusive_head_is_known_unpreserved(self):
        fixture = self.fixture
        exclusive = fixture.exclusive_same_tree()
        detached = fixture.detached("current exclusive", exclusive)
        _, items = self.report()
        history = self.history(items, detached)
        self.assertTrue(history["enumeration_complete"], history["gaps"])
        self.assertEqual(self.root(history, exclusive)["result"], "not_preserved_by_retained_refs")



if __name__ == "__main__":
    unittest.main()
