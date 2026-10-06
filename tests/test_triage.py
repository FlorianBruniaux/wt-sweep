"""T05 decision oracles from disposable Git facts, without product imports.

Only classification's fixed three-value contract is tested here. New reason,
control and usage fields belong to the integrator's subsequent contract tests.
"""

import hashlib
import json
import os
from pathlib import Path
import unittest

from inventory_support import BINARY as DEFAULT_BINARY, decode_path
from test_private_history import PrivateHistoryFixture, bounded_process


BINARY = Path(os.environ.get("WT_SWEEP_TRIAGE_BINARY", str(DEFAULT_BINARY)))


class TriageFixture(PrivateHistoryFixture):
    def dirty(self, kind):
        if kind == "staged":
            (self.linked / "tracked.txt").write_bytes(b"synthetic staged bytes\x00\xff\n")
            self.git(self.linked, "add", "--", "tracked.txt")
        elif kind == "unstaged":
            (self.linked / "tracked.txt").write_bytes(b"synthetic disk bytes\x00\xff\n")
        elif kind == "untracked":
            (self.linked / "local notes").write_bytes(b"synthetic local notes\x00\xff\n")
        elif kind == "ignored":
            # Rules live in administration so the fixture has only ignored work.
            (self.repo / ".git/info").mkdir(exist_ok=True)
            (self.repo / ".git/info/exclude").write_bytes(b"ignored.bin\n")
            (self.linked / "ignored.bin").write_bytes(b"synthetic ignored bytes\x00\xff\n")
        else:
            raise AssertionError(kind)

    def conflicts(self):
        records = [b"0 " + b"0" * len(self.base) + b"\ttracked.txt\n"]
        expected = []
        for stage, payload in enumerate((b"synthetic ancestor\n", b"synthetic ours\n",
                                         b"synthetic theirs\n"), 1):
            oid = self.git(self.linked, "hash-object", "-w", "--stdin", data=payload).strip()
            literal = hashlib.sha1(b"blob " + str(len(payload)).encode() + b"\0" + payload).hexdigest()
            assert oid.decode() == literal, "independent blob digest disagrees with Git fixture"
            record = b"100644 " + oid + b" " + str(stage).encode() + b"\ttracked.txt\n"
            records.append(record)
            expected.append(record)
        self.git(self.linked, "update-index", "--index-info", data=b"".join(records))
        return b"".join(expected)

    def same_tree_divergence(self):
        exclusive = self.exclusive_same_tree()
        self.git(self.linked, "reset", "--hard", exclusive)
        self.expected_heads[self.linked] = exclusive
        return exclusive

    def target_with_missing_blob(self):
        payload = b"synthetic target-only unavailable blob\x00\xff\n"
        blob = self.git(self.repo, "hash-object", "-w", "--stdin", data=payload).strip().decode()
        literal = hashlib.sha1(b"blob " + str(len(payload)).encode() + b"\0" + payload).hexdigest()
        assert blob == literal, "missing-object injection must name the independently hashed blob"
        tree = self.git(self.repo, "mktree", data=f"100644 blob {blob}\ttarget-only.txt\n".encode()).strip()
        self.tick()
        target = self.git(self.repo, "commit-tree", tree.decode(), "-p", self.base,
                          "-m", "Synthetic target with missing content").strip().decode()
        ref = "refs/heads/incomplete-target"
        self.git(self.repo, "update-ref", ref, target)
        self.git(self.repo, "merge-base", "--is-ancestor", self.base, ref)
        destination = self.repo / ".git/objects" / blob[:2] / blob[2:]
        assert destination.is_file() and destination.is_relative_to(self.root)
        destination.unlink()  # exclusively owned loose object in disposable fixture
        return ref, target, blob

    def malformed_private_log(self, repo=None):
        path = self.own_gitdir(repo or self.linked) / "logs/refs/rewritten/malformed"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"synthetic invalid private reflog, no OID\n")
        return path

    def assert_clean(self, case, repo=None):
        repo = repo or self.linked
        case.assertEqual(self.git(repo, "status", "--porcelain", "--untracked-files=all"), b"")
        case.assertEqual(self.git(repo, "ls-files", "-u"), b"")
        case.assertEqual(self.git(repo, "diff", "--exit-code"), b"")
        case.assertEqual(self.git(repo, "diff", "--cached", "--exit-code"), b"")

    def assert_unanchored(self, case, oid):
        refs = self.git(self.repo, "for-each-ref", "--format=%(refname)",
                        "refs/heads", "refs/remotes").splitlines()
        case.assertEqual(set(refs), {b"refs/heads/main", b"refs/heads/topic"})
        for ref in refs:
            self.git(self.repo, "merge-base", "--is-ancestor", oid, ref.decode(), expected=1)


class TriageFixtureFactsTests(unittest.TestCase):
    """Git validates premises independently before the product is executed."""
    def setUp(self):
        self.fixture = TriageFixture()
        self.addCleanup(self.fixture.close)

    def test_main_and_linked_are_clean_identical_named_heads(self):
        f = self.fixture
        before = f.manifest()
        for repo in (f.repo, f.linked):
            f.assert_clean(self, repo)
            self.assertEqual(f.oid(repo), f.base)
            f.git(repo, "merge-base", "--is-ancestor", f.oid(repo), "refs/heads/main")
        self.assertEqual(f.own_gitdir(f.repo), f.repo / ".git")
        self.assertEqual(f.own_gitdir(f.linked).parent, f.repo / ".git/worktrees")
        self.assertEqual(before, f.manifest(), "oracle reads mutated index or administration")

    def test_local_variants_have_exact_positive_git_facts(self):
        expected = {"staged": b"M  tracked.txt\0", "unstaged": b" M tracked.txt\0",
                    "untracked": b"?? local notes\0", "ignored": b"!! ignored.bin\0"}
        for kind, status in expected.items():
            with self.subTest(kind=kind):
                f = TriageFixture()
                try:
                    f.dirty(kind)
                    before = f.manifest()
                    self.assertEqual(f.git(f.linked, "status", "--porcelain=v1", "-z",
                                           "--ignored", "--untracked-files=all"), status)
                    self.assertEqual(f.oid(f.linked), f.base)
                    self.assertEqual(before, f.manifest())
                finally:
                    f.close()

    def test_conflicts_are_three_real_index_stages_with_literal_blobs(self):
        f = self.fixture
        expected = f.conflicts()
        before = f.manifest()
        self.assertEqual(f.git(f.linked, "ls-files", "-u"), expected)
        self.assertEqual(f.git(f.linked, "status", "--porcelain=v1", "-z"), b"UU tracked.txt\0")
        self.assertEqual(f.oid(f.linked), f.base)
        self.assertEqual(before, f.manifest())

    def test_tree_equality_does_not_establish_commit_ancestry(self):
        f = self.fixture
        exclusive = f.same_tree_divergence()
        before = f.manifest()
        f.assert_clean(self)
        self.assertNotEqual(exclusive, f.base)
        self.assertEqual(f.oid(f.linked, "HEAD^{tree}"), f.oid(f.repo, "main^{tree}"))
        f.git(f.repo, "merge-base", "--is-ancestor", exclusive, "refs/heads/main", expected=1)
        self.assertEqual(before, f.manifest())

    def test_missing_target_blob_leaves_graph_ancestry_and_clean_source(self):
        f = self.fixture
        ref, target, blob = f.target_with_missing_blob()
        before = f.manifest()
        f.assert_clean(self)
        self.assertEqual(f.oid(f.repo, ref), target)
        f.git(f.repo, "merge-base", "--is-ancestor", f.base, target)
        self.assertFalse((f.repo / ".git/objects" / blob[:2] / blob[2:]).exists())
        f.git(f.repo, "cat-file", "-e", blob, expected=1)
        self.assertEqual(before, f.manifest())

    def test_old_only_orphan_log_names_an_exclusive_root(self):
        f = self.fixture
        name, exclusive, path = f.orphan_private_reflog()
        before = f.manifest()
        f.assert_clean(self)
        f.assert_unanchored(self, exclusive)
        self.assertEqual(path.read_bytes().split(b" ")[:2],
                         [exclusive.encode(), b"0" * len(exclusive)])
        self.assertFalse((f.own_gitdir(f.linked) / name).exists())
        self.assertEqual(before, f.manifest())

    def test_detached_clean_head_is_named_and_reflog_covered(self):
        f = self.fixture
        detached = f.detached("covered detached T05")
        before = f.manifest()
        f.assert_clean(self, detached)
        self.assertEqual(f.git(detached, "symbolic-ref", "-q", "HEAD", expected=1), b"")
        self.assertEqual(f.oid(detached), f.base)
        f.git(detached, "merge-base", "--is-ancestor", f.base, "refs/heads/main")
        self.assertTrue((f.own_gitdir(detached) / "logs/HEAD").read_bytes().strip())
        self.assertEqual(before, f.manifest())

    def test_absent_target_is_distinct_from_malformed_present_private_log(self):
        f = self.fixture
        path = f.malformed_private_log()
        before = f.manifest()
        f.git(f.repo, "show-ref", "--verify", "--quiet", "refs/heads/unknown", expected=1)
        self.assertTrue(path.is_file())
        self.assertEqual(path.read_bytes(), b"synthetic invalid private reflog, no OID\n")
        f.assert_clean(self)
        self.assertEqual(before, f.manifest())


class TriageCliTests(unittest.TestCase):
    def setUp(self):
        self.fixture = TriageFixture()
        self.addCleanup(self.fixture.close)

    def cli(self, *arguments):
        f = self.fixture
        before = f.manifest()
        try:
            result = bounded_process([BINARY, *arguments], f.env, timeout=40)
        finally:
            self.assertEqual(before, f.manifest(),
                             "T05 CLI changed bytes/index/admin/sentinel/inventory or metadata; atime excluded")
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        return json.loads(result.stdout)

    def inventory(self):
        f = self.fixture
        inventory = self.cli("scan", "--repo", f.repo, "--format", "json")
        self.assertEqual(inventory["schema"], "wt-sweep.inventory/1")
        f.sequence += 1
        path = f.root / f"triage-input-{f.sequence}.json"
        path.write_text(json.dumps(inventory))
        f.manifest_roots.add(path)
        return path

    def report(self, inventory=None, targets=("refs/heads/main",)):
        f = self.fixture
        argv = ["plan", "--inventory", inventory or self.inventory(), "--format", "json"]
        for target in targets:
            argv.extend(("--target", target))
        report = self.cli(*argv)
        self.assertEqual(report["schema"], "wt-sweep.review/2")
        body = {key: value for key, value in report.items() if key != "digest_sha256"}
        digest = hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":"),
                                          ensure_ascii=False).encode()).hexdigest()
        self.assertEqual(report["digest_sha256"], digest, "T05 report digest does not bind all current facets")
        items = {decode_path(item["path"]): item for item in report["items"]}
        self.assertEqual(len(items), len(report["items"]))
        self.assertEqual(set(items), {os.fsencode(path) for path in f.expected_heads})
        for repo, expected_head in f.expected_heads.items():
            self.assertEqual(items[os.fsencode(repo)]["head"], expected_head, "verdict is not bound to exact current fixture HEAD")
        return items

    def item(self, items, repo=None):
        return items[os.fsencode(repo or self.fixture.linked)]

    def assert_verdict(self, item, expected, defect):
        self.assertEqual(item["classification"], expected, defect)

    def test_main_checkout_keep(self):
        self.assert_verdict(self.item(self.report(), self.fixture.repo), "keep", "main checkout was offered as a candidate")

    def test_clean_linked_candidate_without_forge(self):
        item = self.item(self.report())
        self.assert_verdict(item, "cleanup_candidate", "always-refuse judgement or missing forge blocked C1-C5")
        self.assertEqual(item["requests"], [])

    def test_dirty_staged_unstaged_untracked_and_ignored_each_keep(self):
        for kind in ("staged", "unstaged", "untracked", "ignored"):
            with self.subTest(kind=kind):
                f = TriageFixture()
                previous = self.fixture
                self.fixture = f
                try:
                    f.dirty(kind)
                    item = self.item(self.report())
                    self.assertEqual(item["local_summary"][kind]["observed"], 1,
                                     "fixture's exact local positive was lost")
                    self.assert_verdict(item, "keep", f"{kind} work was erased by integration proof")
                finally:
                    self.fixture = previous
                    f.close()

    def test_conflicted_index_keeps_integrated_head(self):
        self.fixture.conflicts()
        item = self.item(self.report())
        self.assertEqual(item["local_summary"]["conflicts"]["observed"], 1)
        self.assert_verdict(item, "keep", "conflicted index was promoted because HEAD is integrated")

    def test_clean_detached_with_named_coverage_is_candidate(self):
        detached = self.fixture.detached("covered detached T05")
        self.assert_verdict(self.item(self.report(), detached), "cleanup_candidate",
                            "detached state alone blocked genuinely named/private-root coverage")

    def test_exclusive_old_only_reflog_keeps_clean_integrated_head(self):
        _, exclusive, _ = self.fixture.orphan_private_reflog()
        self.fixture.assert_unanchored(self, exclusive)
        self.assert_verdict(self.item(self.report()), "keep", "old reflog slot was ignored or tree equality substituted for coverage")

    def test_tree_only_integration_is_keep_for_complete_nonancestry(self):
        self.fixture.same_tree_divergence()
        self.assert_verdict(self.item(self.report()), "keep", "identical_tree incorrectly satisfied C2 or complete nonancestry was lost")

    def test_graph_only_missing_deep_blob_is_verify(self):
        ref, _, _ = self.fixture.target_with_missing_blob()
        item = self.item(self.report(targets=(ref,)))
        self.assert_verdict(item, "verify", "commit-graph ancestry promoted missing tree/blob evidence into C2")

    def test_missing_target_alongside_valid_deep_target_allows_candidate(self):
        item = self.item(self.report(targets=("refs/heads/unknown", "refs/heads/main")))
        self.assert_verdict(item, "cleanup_candidate", "one unrelated unknown target erased valid deep C2")

    def test_no_accepted_target_is_verify_not_negative_integration(self):
        self.assert_verdict(self.item(self.report(targets=())), "verify",
                            "empty target set became complete nonintegration or a vacuous candidate")

    def test_malformed_private_history_is_verify_not_complete_empty(self):
        self.fixture.malformed_private_log()
        self.assert_verdict(self.item(self.report()), "verify",
                            "present malformed private history was silently skipped or treated absent")

    def test_three_verdicts_exist_in_one_exact_fixture_review(self):
        f = self.fixture
        dirty = f.detached("dirty detached T05")
        (dirty / "tracked.txt").write_bytes(b"synthetic detached local work\n")
        unknown = f.detached("unknown detached T05")
        f.malformed_private_log(unknown)
        items = self.report()
        expected = {f.repo: "keep", f.linked: "cleanup_candidate", dirty: "keep", unknown: "verify"}
        actual = {repo: self.item(items, repo)["classification"] for repo in expected}
        self.assertEqual(actual, expected, "one report must preserve distinct keep/candidate/verify decisions")
        self.assertEqual(set(actual.values()), {"keep", "cleanup_candidate", "verify"})

    def test_positive_local_work_survives_another_partial_domain(self):
        f = self.fixture
        f.dirty("unstaged")
        f.malformed_private_log()
        item = self.item(self.report())
        self.assertEqual(item["local_summary"]["unstaged"]["observed"], 1)
        self.assertFalse(item["private_history"]["enumeration_complete"])
        self.assert_verdict(item, "keep", "private-history failure erased a current positive local keep fact")


if __name__ == "__main__":
    unittest.main()
