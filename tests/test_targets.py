"""Independent T06 family-target/view oracles.

Expected classifications come from hand-built Git graphs, never product helpers.
Run fixture premises without a product binary:
  python3.14 -B tests/test_targets.py TargetsFixtureFactsTests -v
Set WT_SWEEP_TARGETS_BINARY to an immutable binary for behavior assertions.
"""
import copy
import hashlib
import errno
import shutil
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

# Scratch location is outside tests; when promoted to tests this is a no-op.
HERE = Path(__file__).resolve()
DEFAULT_WORKSPACE = HERE.parents[1] if HERE.parent.name == "tests" else HERE.parents[3]
WORKSPACE = Path(os.environ.get("WT_SWEEP_TARGETS_WORKSPACE", str(DEFAULT_WORKSPACE)))
sys.path.insert(0, str(WORKSPACE / "tests"))
from inventory_support import BINARY as DEFAULT_BINARY, clean_environment, decode_path
from test_private_history import PrivateHistoryFixture, bounded_process

BINARY = Path(os.environ.get("WT_SWEEP_TARGETS_BINARY", str(DEFAULT_BINARY)))


def byte_path(value):
    return {"hex": os.fsencode(value).hex()}


def digest(document):
    body = {key: value for key, value in document.items() if key != "digest_sha256"}
    return hashlib.sha256(json.dumps(body, sort_keys=True, separators=(",", ":"),
                                     ensure_ascii=False).encode()).hexdigest()


class TargetsFixture(PrivateHistoryFixture):
    """Two physical families, both refs present, incompatible accepted targets."""
    def __init__(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="wt-sweep-t06-oracle-", dir="/private/tmp")
        self.root = Path(self.temporary.name)
        try:
            self.populate()
        except BaseException:
            self.temporary.cleanup()
            raise

    def populate(self):
        self.env = clean_environment(self.root)
        self.env.update(GIT_CONFIG_SYSTEM=os.devnull, GIT_ATTR_NOSYSTEM="1", GIT_OPTIONAL_LOCKS="0",
                        GIT_NO_LAZY_FETCH="1", GIT_ALLOW_PROTOCOL="", GIT_NO_REPLACE_OBJECTS="1",
                        TMPDIR="/private/tmp")
        self.clock = 1700000000
        self.sequence = 0
        self.repo = self.root / "family-a"
        self.linked = self.root / "a"
        self.other = self.root / "family-b"
        self.other_linked = self.root / "ab"
        self.raw = self.root / "raw"
        self.raw_branch = b"refs/heads/raw-\xff"
        self.expected_heads = {}
        self.manifest_roots = set()
        self.stashes = []
        self.sentinel = self.root / "outside sentinel"
        self.sentinel.mkdir()
        (self.sentinel / "unchanged.bin").write_bytes(b"synthetic witness\x00\xff\n")
        self.manifest_roots.add(self.sentinel)
        self.bases = {}
        for repo, linked, label, accepted in ((self.repo, self.linked, b"A", "refs/heads/main"),
                                               (self.other, self.other_linked, b"B", "refs/heads/develop")):
            self.git(self.root, "init", "--template=", "-q", "-b", "main", repo)
            (repo / "tracked.txt").write_bytes(b"synthetic base " + label + b"\n")
            base = self.commit(repo, "Synthetic base " + label.decode(), "tracked.txt")
            self.bases[repo] = base
            self.git(repo, "worktree", "add", "-q", "-b", "topic", linked)
            (linked / "tracked.txt").write_bytes(b"synthetic accepted head " + label + b"\n")
            head = self.commit(linked, "Synthetic accepted " + label.decode(), "tracked.txt")
            if accepted == "refs/heads/main":
                self.git(repo, "reset", "--hard", head)
                self.git(repo, "update-ref", "refs/heads/develop", base)
            else:
                self.git(repo, "update-ref", "refs/heads/develop", head)
            self.expected_heads.update({repo: self.oid(repo), linked: head})
            self.manifest_roots.update((repo, linked))
        self.base = self.expected_heads[self.linked]
        self.git(self.repo, "worktree", "add", "-q", "--detach", self.raw, self.base)
        # APFS disallows nonUTF8 loose ref filenames. Packed refs and symbolic
        # HEAD are ordinary ASCII filenames whose contents retain exact bytes.
        packed = self.repo / ".git/packed-refs"
        packed.write_bytes(b"# pack-refs with: peeled fully-peeled sorted\n" +
                           self.base.encode() + b" " + self.raw_branch + b"\n")
        (self.own_gitdir(self.raw) / "HEAD").write_bytes(b"ref: " + self.raw_branch + b"\n")
        self.expected_heads[self.raw] = self.base
        self.manifest_roots.add(self.raw)
        self.expected_branches = {self.repo: b"refs/heads/main", self.linked: b"refs/heads/topic",
                                  self.raw: self.raw_branch, self.other: b"refs/heads/main",
                                  self.other_linked: b"refs/heads/topic"}
        self.family_for = {path: self.repo if path in (self.repo, self.linked, self.raw) else self.other
                           for path in self.expected_heads}

    def binding(self, repo, targets):
        common = repo / ".git"
        info = common.stat()
        return {"common_dir": byte_path(common), "identity": {"device": info.st_dev, "inode": info.st_ino},
                "targets": [byte_path(target) for target in targets]}

    def target_map(self):
        return {"schema": "wt-sweep.targets/1", "families": [
            self.binding(self.repo, ("refs/heads/main",)),
            self.binding(self.other, ("refs/heads/develop",))]}

    def save(self, document):
        self.sequence += 1
        path = self.root / f"t06-input-{self.sequence}.json"
        path.write_text(json.dumps(document, ensure_ascii=True))
        self.manifest_roots.add(path)
        return path

    def clean(self, case, repo):
        case.assertEqual(self.git(repo, "status", "--porcelain=v1", "-z", "--ignored"), b"")
        case.assertEqual(self.git(repo, "diff", "--exit-code"), b"")
        case.assertEqual(self.git(repo, "diff", "--cached", "--exit-code"), b"")

    def incomplete_local(self, repo=None):
        path = self.own_gitdir(repo or self.linked) / "index"
        path.write_bytes(b"synthetic truncated index, no reliable zero\n")
        return path


class TargetsFixtureFactsTests(unittest.TestCase):
    def setUp(self):
        self.f = TargetsFixture()
        self.addCleanup(self.f.close)

    def test_each_accepted_ref_covers_head_and_wrong_broadcast_does_not(self):
        f = self.f
        before = f.manifest()
        for repo, linked, accepted, wrong in ((f.repo, f.linked, "main", "develop"),
                                             (f.other, f.other_linked, "develop", "main")):
            with self.subTest(family=repo):
                self.assertEqual(set(f.git(repo, "for-each-ref", "--format=%(refname)",
                                          "refs/heads/main", "refs/heads/develop").splitlines()),
                                 {b"refs/heads/main", b"refs/heads/develop"})
                head = f.oid(linked)
                self.assertEqual(f.oid(repo, accepted), head)
                f.git(repo, "merge-base", "--is-ancestor", head, "refs/heads/" + accepted)
                f.git(repo, "merge-base", "--is-ancestor", head, "refs/heads/" + wrong, expected=1)
                self.assertNotEqual(f.oid(repo, wrong + "^{tree}"), f.oid(linked, "HEAD^{tree}"))
                f.clean(self, linked)
        self.assertNotEqual(f.oid(f.linked), f.oid(f.other_linked))
        self.assertNotEqual(f.binding(f.repo, ())['identity'], f.binding(f.other, ())['identity'])
        self.assertEqual(before, f.manifest(), "independent graph reads changed fixture")

    def test_nonutf8_branch_is_exact_and_prefix_siblings_differ(self):
        f = self.f
        before = f.manifest()
        self.assertEqual(f.git(f.raw, "symbolic-ref", "HEAD").strip(), f.raw_branch)
        self.assertFalse(f.raw.is_relative_to(f.linked))
        self.assertFalse(f.other_linked.is_relative_to(f.linked))
        self.assertTrue(os.fsencode(f.other_linked).startswith(os.fsencode(f.linked)))
        self.assertEqual(before, f.manifest())

    def test_injected_bad_index_is_not_a_clean_empty_git_status(self):
        f = self.f
        f.incomplete_local()
        before = f.manifest()
        result = bounded_process(["/usr/bin/git", "-C", f.linked, "status", "--porcelain=v1", "-z"], f.env)
        self.assertNotEqual(result.returncode, 0, "bad-index injection failed to break evidence collection")
        self.assertEqual(result.stdout, b"")
        self.assertEqual(before, f.manifest())


class TargetsCliTests(unittest.TestCase):
    def setUp(self):
        self.f = TargetsFixture()
        self.addCleanup(self.f.close)

    def cli_result(self, *arguments):
        before = self.f.manifest()
        try:
            return bounded_process([BINARY, *arguments], self.f.env, timeout=60)
        finally:
            self.assertEqual(before, self.f.manifest(), "CLI changed owned source/index/admin/input/sentinel; atime excluded")

    def cli(self, *arguments):
        result = self.cli_result(*arguments)
        self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        return json.loads(result.stdout)

    def inventory(self, repo=None):
        f = self.f
        scope = ("--repo", repo) if repo else ("--root", f.root)
        inventory = self.cli("scan", *scope, "--format", "json")
        self.assertEqual(inventory["schema"], "wt-sweep.inventory/1")
        self.assertEqual({decode_path(family["common_dir"]) for family in inventory["families"]},
                         {os.fsencode(repo / ".git")} if repo else {os.fsencode(f.repo / ".git"), os.fsencode(f.other / ".git")})
        return f.save(inventory), inventory

    def plan(self, *extra, inventory=None, mapping=None):
        path, data = inventory or self.inventory()
        mapping_path = self.f.save(mapping if mapping is not None else self.f.target_map())
        report = self.cli("plan", "--inventory", path, "--target-map", mapping_path, "--format", "json", *extra)
        source = report["source"] if report["schema"] == "wt-sweep.review-view/1" else report
        self.assert_source(source, data)
        if report is not source:
            self.assert_view(report)
        return report

    def assert_source(self, source, inventory):
        f = self.f
        self.assertEqual(source["schema"], "wt-sweep.review/2")
        self.assertEqual(source["digest_sha256"], digest(source), "source digest omits or changes reviewed facets")
        self.assertEqual(source["inventory_sha256"], digest(inventory), "source is not bound to exact saved inventory")
        self.assertEqual(source["coverage"], inventory["coverage"], "filter altered source coverage")
        items = {decode_path(item["path"]): item for item in source["items"]}
        self.assertEqual(len(items), len(source["items"]), "ambiguous duplicate source path")
        self.assertEqual(set(items), {os.fsencode(path) for path in f.expected_heads})
        for path, head in f.expected_heads.items():
            item = items[os.fsencode(path)]
            self.assertEqual(item["head"], head, "verdict detached from exact fixture HEAD")
            self.assertEqual(None if item["branch"] is None else decode_path(item["branch"]),
                             f.expected_branches[path], "raw branch identity was lossy")

    def assert_view(self, view):
        f = self.f
        self.assertEqual(view["schema"], "wt-sweep.review-view/1")
        self.assertEqual(view["digest_sha256"], digest(view), "view digest fails to bind source and selection")
        self.assertEqual(set(view), {"schema", "source", "selected_paths", "group_by", "filters", "row_bindings", "digest_sha256"})
        self.assertEqual(set(view["filters"]), {"repository", "path_prefix", "branch", "local_state", "verdict"})
        self.assertIn(view["group_by"], ("repo", "directory"))
        bindings = {decode_path(row["path"]): row for row in view["row_bindings"]}
        self.assertEqual(len(bindings), len(view["row_bindings"]), "ambiguous repeated binding path")
        self.assertEqual(set(bindings), {os.fsencode(path) for path in f.expected_heads}, "hidden rows lost historical family bindings")
        for path, repo in f.family_for.items():
            expected = f.binding(repo, ())
            row = bindings[os.fsencode(path)]
            self.assertEqual(set(row), {"path", "common_dir", "identity"})
            self.assertEqual(row["common_dir"], expected["common_dir"])
            self.assertEqual(row["identity"], expected["identity"])
        selection = [decode_path(path) for path in view["selected_paths"]]
        self.assertEqual(len(selection), len(set(selection)), "selection repeats a row")
        self.assertTrue(set(selection).issubset(bindings), "selection is not a subset of source bindings")

    def selected(self, view):
        return {decode_path(path) for path in view["selected_paths"]}

    def items(self, report):
        source = report.get("source", report)
        return {decode_path(item["path"]): item for item in source["items"]}

    def refuse(self, *extra, mapping=None, inventory=None):
        path, _ = inventory or self.inventory()
        args = ["plan", "--inventory", path, "--format", "json"]
        if mapping is not None:
            args.extend(("--target-map", self.f.save(mapping)))
        result = self.cli_result(*args, *extra)
        self.assertEqual(result.returncode, 2, result.stderr.decode(errors="replace"))
        self.assertEqual(result.stdout, b"", "refusal emitted success-shaped review JSON")
        self.assertTrue(result.stderr, "refusal lacks diagnostic")
        self.assertNotIn(b"unsupported option", result.stderr.lower(), "unsupported flag is not feature-level refusal")
        self.assertNotIn(b"unknown option", result.stderr.lower(), "unsupported flag is not feature-level refusal")
        self.assertNotIn(b"unrecognized", result.stderr.lower(), "unsupported flag is not feature-level refusal")

    def test_family_map_produces_positive_candidate_for_both_different_targets(self):
        items = self.items(self.plan())
        for linked in (self.f.linked, self.f.other_linked):
            self.assertEqual(items[os.fsencode(linked)]["classification"], "cleanup_candidate",
                             "valid family-local proof failed; an always-refuse implementation must not pass")

    def test_wrong_target_in_each_family_does_not_become_candidate(self):
        mapping = self.f.target_map()
        mapping["families"][0]["targets"] = [byte_path("refs/heads/develop")]
        mapping["families"][1]["targets"] = [byte_path("refs/heads/main")]
        items = self.items(self.plan(mapping=mapping))
        for linked in (self.f.linked, self.f.other_linked):
            self.assertNotEqual(items[os.fsencode(linked)]["classification"], "cleanup_candidate", "wrong family target was accepted")

    def test_missing_mapping_never_falls_back_to_main(self):
        mapping = self.f.target_map()
        mapping["families"] = mapping["families"][1:]
        items = self.items(self.plan(mapping=mapping))
        self.assertEqual(items[os.fsencode(self.f.linked)]["classification"], "verify", "unmapped family inherited main or another family's target")
        self.assertEqual(items[os.fsencode(self.f.other_linked)]["classification"], "cleanup_candidate", "missing mapping poisoned independent mapped family")

    def test_missing_target_ref_is_unknown_without_poisoning_other_family(self):
        mapping = self.f.target_map()
        mapping["families"][0]["targets"] = [byte_path("refs/heads/not-present")]
        items = self.items(self.plan(mapping=mapping))
        self.assertEqual(items[os.fsencode(self.f.linked)]["classification"], "verify",
                         "missing target manufactured an integration proof or clean negative")
        self.assertEqual(items[os.fsencode(self.f.other_linked)]["classification"], "cleanup_candidate")

    def test_identical_duplicate_binding_keeps_positive_result(self):
        mapping = self.f.target_map()
        mapping["families"].append(copy.deepcopy(mapping["families"][0]))
        items = self.items(self.plan(mapping=mapping))
        self.assertEqual(items[os.fsencode(self.f.linked)]["classification"], "cleanup_candidate")
        self.assertEqual(items[os.fsencode(self.f.other_linked)]["classification"], "cleanup_candidate")

    def test_simple_target_refuses_multifamily_inventory(self):
        self.refuse("--target", "refs/heads/main")

    def test_simple_target_succeeds_for_one_physical_family(self):
        path, data = self.inventory(self.f.repo)
        result = self.cli("plan", "--inventory", path, "--target", "refs/heads/main", "--format", "json")
        self.assertEqual(result["schema"], "wt-sweep.review/2")
        self.assertEqual(result["inventory_sha256"], digest(data))
        self.assertEqual(result["digest_sha256"], digest(result))
        self.assertEqual(self.items(result)[os.fsencode(self.f.linked)]["classification"], "cleanup_candidate")

    def test_one_family_prefix_view_works_without_target_map(self):
        path, data = self.inventory(self.f.repo)
        view = self.cli("plan", "--inventory", path, "--target", "refs/heads/main",
                        "--path-prefix", self.f.linked, "--format", "json")
        self.assertEqual(view["schema"], "wt-sweep.review-view/1")
        self.assertEqual(view["digest_sha256"], digest(view))
        source = view["source"]
        self.assertEqual(source["digest_sha256"], digest(source))
        self.assertEqual(source["inventory_sha256"], digest(data))
        self.assertEqual(self.selected(view), {os.fsencode(self.f.linked)})
        self.assertEqual(len(source["items"]), 3)
        self.assertEqual(len(view["row_bindings"]), 3)
        self.assertEqual(self.items(view)[os.fsencode(self.f.linked)]["classification"], "cleanup_candidate")

    def test_simple_target_and_map_cannot_combine(self):
        self.refuse("--target", "refs/heads/main", mapping=self.f.target_map())

    def test_contradictory_duplicate_binding_refuses(self):
        mapping = self.f.target_map()
        duplicate = copy.deepcopy(mapping["families"][0])
        duplicate["targets"] = [byte_path("refs/heads/develop")]
        mapping["families"].append(duplicate)
        self.refuse(mapping=mapping)

    def test_stale_identity_binding_refuses(self):
        mapping = self.f.target_map()
        mapping["families"][0]["identity"]["inode"] += 1
        self.refuse(mapping=mapping)

    def test_current_physical_identity_replacement_refuses_old_binding(self):
        inventory = self.inventory()
        mapping = self.f.target_map()
        common = self.f.repo / ".git"
        moved = self.f.root / "owned replaced administration"
        common.rename(moved)
        shutil.copytree(moved, common)
        self.f.manifest_roots.add(moved)
        self.assertNotEqual(common.stat().st_ino, mapping["families"][0]["identity"]["inode"])
        self.refuse(mapping=mapping, inventory=inventory)

    def test_map_missing_and_malformed_bytes_refuse_without_success_json(self):
        inventory, _ = self.inventory()
        for payload in (None, b"{synthetic invalid JSON\n"):
            path = self.f.root / "missing or malformed target map"
            if payload is not None:
                path.write_bytes(payload)
                self.f.manifest_roots.add(path)
            result = self.cli_result("plan", "--inventory", inventory, "--target-map", path, "--format", "json")
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, b"")
            self.assertTrue(result.stderr)
            self.assertNotIn(b"unsupported option", result.stderr.lower())
            self.assertNotIn(b"unknown option", result.stderr.lower())

    def test_strict_map_schema_paths_and_fullrefs_refuse(self):
        good = self.f.target_map()
        variants = []
        document = copy.deepcopy(good); document["extra"] = True; variants.append(document)
        document = copy.deepcopy(good); document["schema"] = "wt-sweep.targets/0"; variants.append(document)
        document = copy.deepcopy(good); document["families"][0]["common_dir"] = byte_path("relative/.git"); variants.append(document)
        document = copy.deepcopy(good); document["families"][0]["targets"] = [byte_path("main")]; variants.append(document)
        document = copy.deepcopy(good); document["families"][0]["targets"] = [byte_path("refs/worktree/private")]; variants.append(document)
        document = copy.deepcopy(good); document["families"][0]["common_dir"] = {"hex": "ff0"}; variants.append(document)
        for document in variants:
            with self.subTest(document=document):
                self.refuse(mapping=document)

    def test_prefix_matches_components(self):
        view = self.plan("--path-prefix", self.f.linked)
        self.assertEqual(self.selected(view), {os.fsencode(self.f.linked)}, "/a prefix selected /ab")
        self.assertEqual(view["filters"]["path_prefix"], byte_path(self.f.linked))
        self.assertEqual(view["group_by"], "repo")

    def test_nonutf8_physical_path_is_selected_without_loss_when_host_supports_it(self):
        raw = self.f.root / os.fsdecode(b"physical-\xff")
        try:
            raw.mkdir()
        except OSError as error:
            if error.errno == errno.EILSEQ:
                self.skipTest("host filesystem refuses nonUTF8 path creation (EILSEQ)")
            raise
        raw.rmdir()
        self.f.git(self.f.repo, "worktree", "add", "-q", "--detach", raw, self.f.base)
        self.f.expected_heads[raw] = self.f.base
        self.f.expected_branches[raw] = None
        self.f.family_for[raw] = self.f.repo
        self.f.manifest_roots.add(raw)
        view = self.plan("--path-prefix", raw)
        self.assertEqual(self.selected(view), {os.fsencode(raw)}, "nonUTF8 path was replaced or omitted")

    def test_relative_and_parent_traversal_prefixes_refuse(self):
        for prefix in ("relative", self.f.root / "a/../ab"):
            with self.subTest(prefix=prefix):
                self.refuse("--path-prefix", prefix, mapping=self.f.target_map())

    def test_exact_branch_filter_and_intersection(self):
        view = self.plan("--branch", os.fsdecode(self.f.raw_branch))
        self.assertEqual(self.selected(view), {os.fsencode(self.f.raw)})
        self.assertEqual(view["filters"]["branch"], byte_path(self.f.raw_branch))
        view = self.plan("--branch", "refs/heads/topic", "--filter-repo", self.f.repo / ".git")
        self.assertEqual(self.selected(view), {os.fsencode(self.f.linked)}, "filters failed to intersect by exact family bytes")
        self.assertEqual(view["filters"]["repository"], byte_path(self.f.repo / ".git"))

    def test_repository_checkout_paths_select_whole_historical_family(self):
        for checkout in (self.f.repo, self.f.linked):
            with self.subTest(checkout=checkout):
                view = self.plan("--filter-repo", checkout)
                self.assertEqual(self.selected(view), {os.fsencode(self.f.repo),
                                                     os.fsencode(self.f.linked), os.fsencode(self.f.raw)},
                                 "checkout repository filter did not select its entire historical family")
                self.assertEqual(view["filters"]["repository"], byte_path(checkout),
                                 "repository filter failed to retain the exact supplied bytes")

    def test_unknown_repository_path_selects_zero_without_losing_source(self):
        unknown = self.f.root / "not an observed family"
        view = self.plan("--filter-repo", unknown)
        self.assertEqual(self.selected(view), set())
        self.assertEqual(view["filters"]["repository"], byte_path(unknown))
        self.assertEqual(len(view["source"]["items"]), 5)
        self.assertEqual(len(view["row_bindings"]), 5)

    def test_grouping_preserves_full_source_and_bindings(self):
        for grouping in ("repo", "directory"):
            with self.subTest(grouping=grouping):
                view = self.plan("--group-by", grouping)
                self.assertEqual(view["group_by"], grouping)
                self.assertEqual(self.selected(view), {os.fsencode(path) for path in self.f.expected_heads})

    def test_empty_selection_keeps_every_source_row_coverage_and_binding(self):
        view = self.plan("--path-prefix", self.f.root / "no selected rows")
        self.assertEqual(self.selected(view), set())
        self.assertEqual(len(view["source"]["items"]), 5)
        self.assertEqual(len(view["row_bindings"]), 5)

    def test_empty_selection_preserves_hidden_local_failure_and_unknown_verdict(self):
        inventory = self.inventory()
        self.f.incomplete_local()
        view = self.plan("--path-prefix", self.f.root / "no selected rows", inventory=inventory)
        self.assertEqual(self.selected(view), set())
        self.assertEqual(self.items(view)[os.fsencode(self.f.linked)]["classification"], "verify",
                         "filter hid failed evidence before judgement")
        local = self.items(view)[os.fsencode(self.f.linked)]["local_summary"]
        self.assertIsNotNone(local)
        self.assertFalse(local["index_complete"], "hidden failed index read promoted to complete")

    def test_dirty_and_clean_selection_use_positive_facts(self):
        (self.f.linked / "tracked.txt").write_bytes(b"synthetic positive disk change\n")
        dirty = self.plan("--local-state", "dirty")
        self.assertEqual(self.selected(dirty), {os.fsencode(self.f.linked)})
        clean = self.plan("--local-state", "clean")
        self.assertNotIn(os.fsencode(self.f.linked), self.selected(clean))
        self.assertIn(os.fsencode(self.f.other_linked), self.selected(clean))

    def test_incomplete_zero_local_evidence_selects_unknown_and_never_clean(self):
        inventory = self.inventory()
        self.f.incomplete_local()
        unknown = self.plan("--local-state", "unknown", inventory=inventory)
        self.assertIn(os.fsencode(self.f.linked), self.selected(unknown), "failed current local reads became a clean observed zero")
        clean = self.plan("--local-state", "clean", inventory=inventory)
        self.assertNotIn(os.fsencode(self.f.linked), self.selected(clean))
        self.assertEqual(self.items(unknown)[os.fsencode(self.f.linked)]["classification"], "verify")

    def test_verdict_filter_is_after_full_judgement(self):
        view = self.plan("--verdict", "cleanup_candidate")
        self.assertEqual(self.selected(view), {os.fsencode(self.f.linked), os.fsencode(self.f.raw), os.fsencode(self.f.other_linked)})
        self.assertEqual(self.items(view)[os.fsencode(self.f.repo)]["classification"], "keep")

    def test_hidden_families_still_enforce_output_destination_boundary(self):
        inventory = self.inventory()
        destination = self.f.other_linked / "forbidden report.json"
        self.refuse("--path-prefix", self.f.root / "no selected rows", "--out", destination,
                    mapping=self.f.target_map(), inventory=inventory)
        self.assertFalse(destination.exists(), "empty selection bypassed full-source output-boundary check")


if __name__ == "__main__":
    unittest.main()
