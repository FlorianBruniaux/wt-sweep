"""Independent A1/A4 large-review oracles over uniquely owned real Git roots.

No collector or judgement module is imported. WT_SWEEP_LARGE_REVIEW_BINARY can
freeze the CLI under test; WT_SWEEP_LARGE_REVIEW_BUDGET=legacy reproduces the
pre-profile binary without passing its as-yet unsupported --budget argument.
"""

import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import unittest
import zlib

from inventory_support import BINARY as DEFAULT_BINARY, GIT, clean_environment, decode_path, filesystem_manifest

BINARY = Path(os.environ.get("WT_SWEEP_LARGE_REVIEW_BINARY", str(DEFAULT_BINARY)))
PROFILE = os.environ.get("WT_SWEEP_LARGE_REVIEW_BUDGET", "thorough")


class LargeReviewFixture:
    """Write canonical Git objects directly, then have real Git check them out."""

    def __init__(self, blobs=48, commits=8, retained_refs=4, private_roots=4):
        self.temporary = tempfile.TemporaryDirectory(prefix="wt-sweep-a1-oracle-", dir="/private/tmp")
        self.root = Path(self.temporary.name)
        self.repo = self.root / "principal"
        self.linked = self.root / "clean linked"
        self.env = clean_environment(self.root)
        self.env.update(GIT_CONFIG_SYSTEM=os.devnull, GIT_ATTR_NOSYSTEM="1",
                        GIT_OPTIONAL_LOCKS="0", GIT_NO_LAZY_FETCH="1",
                        GIT_ALLOW_PROTOCOL="", GIT_NO_REPLACE_OBJECTS="1")
        self.blobs = {}
        self.chain = []
        self.private_roots = set()
        self.named_refs = {}
        self.paths = [self.repo, self.linked]
        self.manifest_roots = {self.repo, self.linked}
        try:
            self.repo.mkdir()
            self.git(self.repo, "init", "--template=", "-q", "-b", "main")
            entries = []
            for number in range(blobs):
                name = f"file-{number:05}.txt"
                payload = f"synthetic unique content {number:05}\n".encode()
                oid = self.object("blob", payload)
                self.blobs[name] = (oid, payload)
                entries.append(b"100644 " + name.encode() + b"\0" + bytes.fromhex(oid))
            self.tree = self.object("tree", b"".join(entries))
            for number in range(commits):
                parent = (b"parent " + self.chain[-1].encode() + b"\n") if self.chain else b""
                date = f"{1700000000 + number} +0000".encode()
                payload = (b"tree " + self.tree.encode() + b"\n" + parent +
                           b"author Fixture <fixture@example.invalid> " + date + b"\n" +
                           b"committer Fixture <fixture@example.invalid> " + date +
                           b"\n\nSynthetic chain " + str(number).encode() + b"\n")
                self.chain.append(self.object("commit", payload))
            self.head, self.target = self.chain[0], self.chain[-1]
            self.git(self.repo, "update-ref", "refs/heads/main", self.target)
            self.git(self.repo, "reset", "--hard", "--quiet", self.target)
            for number in range(retained_refs):
                reference = f"refs/heads/retained-{number:03}"
                self.git(self.repo, "update-ref", reference, self.target)
                self.named_refs[reference] = self.target
            self.named_refs["refs/heads/main"] = self.target
            self.git(self.repo, "worktree", "add", "-q", "--detach", self.linked, self.head)
            self.gitdir = Path(os.fsdecode(self.git(self.linked, "rev-parse", "--absolute-git-dir").strip()))
            for number in range(private_roots):
                oid = self.chain[number]
                self.git(self.linked, "update-ref", f"refs/worktree/covered-{number:03}", oid)
                self.private_roots.add(oid)
            self.sentinel = self.root / "outside sentinel"
            self.sentinel.write_bytes(b"synthetic external sentinel\x00\xff\n")
            self.manifest_roots.add(self.sentinel)
        except BaseException:
            self.close()
            raise

    def object(self, kind, payload):
        raw = kind.encode() + b" " + str(len(payload)).encode() + b"\0" + payload
        oid = hashlib.sha1(raw).hexdigest()
        path = self.object_path(oid)
        path.parent.mkdir(exist_ok=True)
        path.write_bytes(zlib.compress(raw))
        return oid

    def object_path(self, oid):
        return self.repo / ".git/objects" / oid[:2] / oid[2:]

    def command(self, argv, data=b"", timeout=30):
        child = subprocess.Popen(list(map(str, argv)), env=self.env, stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                 start_new_session=True)
        try:
            stdout, stderr = child.communicate(data, timeout=timeout)
        except BaseException:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.communicate(timeout=5)
            raise
        return subprocess.CompletedProcess(child.args, child.returncode, stdout, stderr)

    def git(self, repo, *args, data=b"", expected=0):
        settings = ("core.hooksPath=/dev/null", "core.fsmonitor=false", "core.untrackedCache=false",
                    "gc.auto=0", "maintenance.auto=false", "protocol.allow=never",
                    "core.autocrlf=false", "core.filemode=true")
        argv = [GIT]
        for setting in settings:
            argv.extend(("-c", setting))
        result = self.command([*argv, "-C", repo, *args], data)
        if result.returncode != expected:
            raise AssertionError((args, result.returncode, result.stderr.decode(errors="replace")))
        return result.stdout

    def exclusive_root(self):
        # Identical tree is deliberately insufficient to preserve exclusive history.
        payload = (b"tree " + self.tree.encode() + b"\nparent " + self.head.encode() +
                   b"\nauthor Fixture <fixture@example.invalid> 1800000000 +0000\n" +
                   b"committer Fixture <fixture@example.invalid> 1800000000 +0000\n\nExclusive\n")
        oid = self.object("commit", payload)
        self.git(self.linked, "update-ref", "refs/worktree/exclusive", oid)
        self.private_roots.add(oid)
        for reference in self.named_refs:
            self.git(self.repo, "merge-base", "--is-ancestor", oid, reference, expected=1)
        return oid

    def manifest(self):
        return {os.fsencode(path): filesystem_manifest(path) for path in self.manifest_roots}

    def close(self):
        self.temporary.cleanup()


class LargeReviewTests(unittest.TestCase):
    def fixture(self, **dimensions):
        fixture = LargeReviewFixture(**dimensions)
        self.addCleanup(fixture.close)
        return fixture

    def review(self, fixture, focused=True):
        before = fixture.manifest()
        binary_digest = hashlib.sha256(BINARY.read_bytes()).hexdigest()
        arguments = [BINARY, "review", "--repo", fixture.repo,
                     "--target", "refs/heads/main", "--format", "json"]
        if focused:
            arguments.extend(("--worktree", fixture.linked))
        if PROFILE != "legacy":
            arguments.extend(("--budget", PROFILE))
        try:
            result = fixture.command(arguments, timeout=180)
        finally:
            self.assertEqual(before, fixture.manifest(), "review mutated source/admin/index or sentinel")
            self.assertEqual(hashlib.sha256(BINARY.read_bytes()).hexdigest(), binary_digest,
                             "CLI binary changed during collection")
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        report = json.loads(result.stdout)
        self.assertEqual(report["schema"], "wt-sweep.review/2")
        actual = {decode_path(item["path"]): item for item in report["items"]}
        self.assertEqual(len(actual), len(report["items"]), "duplicate paths in review")
        self.assertEqual(set(actual), {os.fsencode(path) for path in fixture.paths},
                         "collection must retain the complete independently known inventory")
        return report, actual[os.fsencode(fixture.linked)]

    def assert_clean_candidate(self, fixture, item):
        summary = item["local_summary"]
        for domain in ("head_complete", "index_complete", "disk_complete"):
            self.assertTrue(summary[domain], (domain, summary["gaps"]))
        for domain in ("staged", "unstaged", "conflicts", "untracked", "ignored"):
            self.assertEqual(summary[domain]["observed"], 0, domain)
            self.assertTrue(summary[domain]["complete"], (domain, summary["gaps"]))
        history = item["private_history"]
        self.assertTrue(history["enumeration_complete"], history["gaps"])
        self.assertTrue(history["anchors_complete"], history["gaps"])
        roots = {root["oid"]: root for root in history["roots"]}
        self.assertEqual(set(roots), fixture.private_roots,
                         "known private ref/HEAD roots omitted or invented")
        for root in roots.values():
            self.assertEqual(root["result"], "covered", root)
            ref = os.fsdecode(decode_path(root["anchor"]["reference"]))
            self.assertIn(ref, fixture.named_refs, "private ref or detached HEAD used as named anchor")
            self.assertEqual(root["anchor"]["oid"], fixture.named_refs[ref])
        self.assertTrue(any(o["target_role"] == "integration" and o["relation"] == ("identical_commit" if fixture.head == fixture.target else "ancestor")
                            and o["source_oid"] == fixture.head and o["target_oid"] == fixture.target
                            for o in item["observations"]), "candidate lost independently known deep ancestry")
        self.assertEqual(item["classification"], "cleanup_candidate", item["decision"])
        self.assertEqual(item["decision"]["missing_conditions"], [])

    def test_thorough_completes_4200_distinct_blobs_600_commits_and_32_private_roots(self):
        # Detects fixed small caps, per-root repeated walks and blanket refusal.
        fixture = self.fixture(blobs=4200, commits=600, retained_refs=32, private_roots=32)
        self.assertEqual(len({oid for oid, _ in fixture.blobs.values()}), 4200)
        self.assertEqual(int(fixture.git(fixture.repo, "rev-list", "--count", "main")), 600)
        self.assertEqual(len(fixture.git(fixture.repo, "for-each-ref", "--format=%(refname)",
                                       "refs/heads").splitlines()), 33)
        self.assertEqual(fixture.git(fixture.linked, "status", "--porcelain"), b"")
        fixture.git(fixture.repo, "merge-base", "--is-ancestor", fixture.head, fixture.target)
        for oid in fixture.private_roots:
            fixture.git(fixture.repo, "merge-base", "--is-ancestor", oid, fixture.target)
        fixture.git(fixture.repo, "fsck", "--full", "--no-reflogs")
        report, item = self.review(fixture)
        self.assert_clean_candidate(fixture, item)
        if PROFILE != "legacy":
            phases = self.metrics(report)
            deep = phases["deep"]["counters"]
            self.assertGreaterEqual(deep["object_reads"], 4200,
                                    "metrics omitted the independently known distinct blob reads")
            self.assertGreater(deep["git_processes"], 0)
            self.assertLess(deep["git_processes"], 2100,
                            "one Git child per blob regressed; batching must cut that cost at least in half")

    def metrics(self, report):
        metrics = report["collection_metrics"]
        self.assertEqual(metrics["budget_profile"], PROFILE)
        self.assertTrue(metrics["applied_limits"], "report omitted applied finite budget limits")
        for name, value in metrics["applied_limits"].items():
            self.assertIsInstance(value, int, name)
            self.assertGreater(value, 0, name)
        phases = {phase["name"]: phase for phase in metrics["phases"]}
        self.assertEqual(len(phases), len(metrics["phases"]), "duplicate phase metrics")
        self.assertTrue({"controls", "history", "local", "deep", "private_history", "revalidation"}
                        <= set(phases), "report omitted an evidence/revalidation phase")
        for phase in phases.values():
            self.assertGreaterEqual(phase["elapsed_ms"], 0)
            for name, value in phase["counters"].items():
                self.assertIsInstance(value, int, name)
                self.assertGreaterEqual(value, 0, name)
        return phases

    def test_direct_private_root_does_not_walk_33_equivalent_named_refs(self):
        # Detects per-ref graph/process fanout; metadata path checks are required.
        work = []
        for retained_refs in (0, 32):
            with self.subTest(named_refs=retained_refs + 1):
                fixture = self.fixture(blobs=3, commits=1, retained_refs=retained_refs, private_roots=1)
                report, item = self.review(fixture)
                self.assert_clean_candidate(fixture, item)
                if PROFILE != "legacy":
                    private = self.metrics(report)["private_history"]["counters"]
                    self.assertEqual(private["roots_enumerated"], 1)
                    self.assertEqual(private["object_reads"], 1,
                                     "directly named root requires exactly one verified commit")
                    self.assertEqual(private["traversal_steps"], 0,
                                     "directly named root must not trigger an ancestry walk")
                    work.append((private["object_reads"], private["git_processes"],
                                 private["traversal_steps"]))
        if PROFILE != "legacy":
            self.assertEqual(work[0], work[1],
                             "33 equivalent named refs increased object/process/graph work")

    def test_50_worktrees_dirty_early_row_does_not_starve_48_small_candidates(self):
        # Detects whole-family budget starvation or discarded inventory rows.
        fixture = self.fixture(blobs=3, commits=3, retained_refs=2, private_roots=1)
        stress = fixture.linked / ".synthetic-work"
        stress.mkdir()
        for number in range(4200):
            (stress / f"note-{number:05}.txt").write_bytes(b"local work to retain\n")
        for number in range(48):
            path = fixture.root / f"simple-{number:03}"
            fixture.git(fixture.repo, "worktree", "add", "-q", "--detach", path, fixture.head)
            fixture.paths.append(path)
            fixture.manifest_roots.add(path)
        self.assertEqual(len(fixture.paths), 50)
        report, dirty = self.review(fixture, focused=False)
        self.assertEqual(dirty["classification"], "keep")
        self.assertEqual(dirty["local_summary"]["untracked"]["observed"], 4200)
        actual = {decode_path(item["path"]): item for item in report["items"]}
        for path in fixture.paths[2:]:
            with self.subTest(path=path.name):
                self.assert_clean_candidate(fixture, actual[os.fsencode(path)])

    def test_thorough_small_clean_control_is_candidate(self):
        # Detects implementations that satisfy fault cases by always refusing.
        fixture = self.fixture()
        _, item = self.review(fixture)
        self.assert_clean_candidate(fixture, item)

    def test_corrupt_blob_preserves_graph_but_refuses_deep_candidate(self):
        # Detects reuse of unverified object bytes or graph-only deep proof.
        fixture = self.fixture()
        oid, _ = fixture.blobs["file-00000.txt"]
        raw = b"blob 8\0corrupt\n"
        fixture.object_path(oid).write_bytes(zlib.compress(raw))
        _, item = self.review(fixture)
        self.assertFalse(item["local_summary"]["head_complete"])
        self.assertTrue(item["local_summary"]["gaps"])
        self.assertIn("C3", item["decision"]["missing_conditions"])
        self.assertEqual(item["classification"], "verify")
        self.assertTrue(any(h["result"] == "ancestor" for h in item["history"]))
        self.assertFalse(any(o["target_role"] == "integration" for o in item["observations"]))

    def test_missing_blob_is_unknown_and_never_candidate(self):
        # Detects accepting a successful empty read or treating absence as zero work.
        fixture = self.fixture()
        fixture.object_path(fixture.blobs["file-00000.txt"][0]).unlink()
        _, item = self.review(fixture)
        self.assertFalse(item["local_summary"]["head_complete"])
        self.assertTrue(item["local_summary"]["gaps"])
        self.assertEqual(item["classification"], "verify")
        self.assertIn("C3", item["decision"]["missing_conditions"])
        self.assertFalse(any(o["target_role"] == "integration" for o in item["observations"]))

    def test_missing_private_commit_preserves_integration_but_requires_c4(self):
        # Detects treating an unavailable private root as absent/preserved history.
        fixture = self.fixture()
        missing = fixture.exclusive_root()
        fixture.object_path(missing).unlink()
        _, item = self.review(fixture)
        roots = {root["oid"]: root for root in item["private_history"]["roots"]}
        self.assertEqual(roots[missing]["result"], "unknown")
        self.assertTrue(roots[missing]["reason"])
        self.assertIsNone(roots[missing]["anchor"])
        self.assertEqual(item["classification"], "verify")
        self.assertIn("C4", item["decision"]["missing_conditions"])
        self.assertTrue(any(o["target_role"] == "integration" and o["relation"] == "ancestor"
                            for o in item["observations"]),
                        "private-root failure must preserve independent deep integration")

    def test_exclusive_same_tree_private_commit_is_kept(self):
        # Detects using tree equality or a private ref as history preservation.
        fixture = self.fixture()
        exclusive = fixture.exclusive_root()
        _, item = self.review(fixture)
        roots = {root["oid"]: root for root in item["private_history"]["roots"]}
        self.assertEqual(roots[exclusive]["result"], "not_preserved_by_retained_refs")
        self.assertIsNone(roots[exclusive]["anchor"])
        self.assertEqual(item["classification"], "keep")
        self.assertIn("private_history_not_preserved", item["decision"]["keep_reasons"])

    def test_dirty_same_size_resident_file_is_kept(self):
        # Detects trusting cached HEAD/index identity instead of current disk bytes.
        fixture = self.fixture()
        path = fixture.linked / "file-00000.txt"
        metadata = path.stat()
        path.write_bytes(b"X" * metadata.st_size)
        os.utime(path, ns=(metadata.st_atime_ns, metadata.st_mtime_ns))
        _, item = self.review(fixture)
        self.assertEqual(item["local_summary"]["unstaged"]["observed"], 1)
        self.assertEqual(item["classification"], "keep")
        self.assertIn("local_unstaged", item["decision"]["keep_reasons"])


if __name__ == "__main__":
    unittest.main()
