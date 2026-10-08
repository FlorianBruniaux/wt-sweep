"""Independent T03 oracles over owned real Git fixtures.

Expected ancestry comes from a synthetic DAG constructed here, not the product.
No product module is imported. Work-unit assertions wait for the shared contract.
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


BINARY = Path(os.environ.get("WT_SWEEP_SCALE_BINARY", str(DEFAULT_BINARY)))


class ScaleFixture:
    def __init__(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="wt-sweep-t03-oracle-", dir="/private/tmp")
        self.root = Path(self.temporary.name)
        self.repo = self.root / "00-costly-principal"
        self.env = clean_environment(self.root)
        self.env.update(GIT_CONFIG_SYSTEM=os.devnull, GIT_ATTR_NOSYSTEM="1",
                        GIT_OPTIONAL_LOCKS="0", GIT_NO_LAZY_FETCH="1",
                        GIT_ALLOW_PROTOCOL="", GIT_NO_REPLACE_OBJECTS="1")
        self.parents = {}
        self.trees = {}
        self.objects = {}
        self.paths = []
        self.heads = {}
        self.sequence = 0
        self.protected_files = [self.root / "outside-sentinel"]
        self.protected_files[0].write_bytes(b"owned external sentinel\n")
        self.repo.mkdir()
        try:
            self.git("init", "-q", "-b", "main")
        except BaseException:
            self.close()
            raise

    def command(self, argv, data=b"", expected=0, timeout=30):
        process = subprocess.Popen(argv, env=self.env, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=True)
        try:
            stdout, stderr = process.communicate(data, timeout=timeout)
        except BaseException:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.communicate(timeout=5)
            raise
        if process.returncode != expected:
            raise AssertionError((argv, process.returncode, stderr.decode(errors="replace")))
        self.last_stderr = stderr
        return stdout

    def git(self, *arguments, data=b"", expected=0):
        argv = [GIT]
        for config in ["core.hooksPath=/dev/null", "core.fsmonitor=false",
                       "core.untrackedCache=false", "gc.auto=0", "maintenance.auto=false",
                       "protocol.allow=never", "core.autocrlf=false", "core.filemode=true"]:
            argv += ["-c", config]
        return self.command([*argv, "-C", str(self.repo), *map(str, arguments)],
                            data=data, expected=expected)

    def object(self, kind, payload):
        encoded = kind.encode() + b" " + str(len(payload)).encode() + b"\0" + payload
        oid = hashlib.sha1(encoded).hexdigest()
        destination = self.repo / ".git/objects" / oid[:2] / oid[2:]
        destination.parent.mkdir(exist_ok=True)
        if not destination.exists():
            destination.write_bytes(zlib.compress(encoded))
        self.objects[oid] = (kind, payload)
        return oid

    def tree(self, payload):
        blob = self.object("blob", payload)
        tree = self.object("tree", b"100644 tracked.txt\0" + bytes.fromhex(blob))
        return tree, blob

    def commit(self, tree, parents=(), message="Synthetic commit"):
        self.sequence += 1
        date = str(1700000000 + self.sequence).encode() + b" +0000"
        payload = b"tree " + tree.encode() + b"\n"
        payload += b"".join(b"parent " + parent.encode() + b"\n" for parent in parents)
        payload += b"author Fixture <fixture@example.invalid> " + date + b"\n"
        payload += b"committer Fixture <fixture@example.invalid> " + date + b"\n\n"
        payload += message.encode() + b"\n"
        oid = self.object("commit", payload)
        self.parents[oid] = tuple(parents)
        self.trees[oid] = tree
        return oid

    def chain(self, count, tree, parent=None):
        chain = []
        for number in range(count):
            parent = self.commit(tree, (parent,) if parent else (), f"Chain {number}")
            chain.append(parent)
        return chain

    def ancestor(self, source, target):
        pending = [target]
        seen = set()
        while pending:
            oid = pending.pop()
            if oid in seen:
                continue
            seen.add(oid)
            if oid == source:
                return True
            pending.extend(self.parents[oid])
        return False

    def checkout(self, head):
        self.git("update-ref", "refs/heads/main", head)
        self.git("reset", "--hard", "--quiet", head)
        self.paths = [self.repo]
        self.heads[self.repo] = head

    def many_worktrees(self, count, depth=600):
        if count < 2:
            raise ValueError("costly and simple cases both required")
        costly_tree, _ = self.tree(b"costly source root\n")
        simple_tree, self.target_blob = self.tree(b"small target worktree\n")
        self.costly_head = self.commit(costly_tree, message="Divergent costly HEAD")
        self.target_chain = self.chain(depth, simple_tree)
        self.target = self.target_chain[-1]
        self.checkout(self.costly_head)
        self.git("update-ref", "refs/heads/integration", self.target)
        for number in range(1, count):
            path = self.root / f"simple-{number:03}"
            head = self.target_chain[-number]
            self.git("worktree", "add", "--quiet", "--detach", path, head)
            self.paths.append(path)
            self.heads[path] = head
        return self

    def unavailable_target(self):
        tree, _ = self.tree(b"unavailable target\n")
        oid = self.commit(tree, message="Target with deliberately unavailable commit")
        self.git("update-ref", "refs/heads/00-inaccessible", oid)
        self.remove_object(oid)
        return oid

    def remove_object(self, oid):
        path = self.repo / ".git/objects" / oid[:2] / oid[2:]
        if not path.is_file():
            raise AssertionError("fault injection requires a real owned loose object")
        path.unlink()

    def manifest(self):
        return {os.fsencode(path): filesystem_manifest(path) for path in [*self.paths, *self.protected_files]}

    def close(self):
        self.temporary.cleanup()


class ScaleFixtureTests(unittest.TestCase):
    """Fixture construction checks establish real topology, not product success."""

    def setUp(self):
        self.fixture = ScaleFixture()
        self.addCleanup(self.fixture.close)

    def test_real_46_then_100_worktrees_have_exact_independent_topology(self):
        for count in (46, 100):
            with self.subTest(worktrees=count):
                fixture = ScaleFixture()
                self.addCleanup(fixture.close)
                fixture.many_worktrees(count)
                before = fixture.manifest()
                records = fixture.git("worktree", "list", "--porcelain", "-z")
                actual = [record[len(b"worktree "):] for record in records.split(b"\0")
                          if record.startswith(b"worktree ")]
                self.assertEqual(actual, [os.fsencode(path) for path in fixture.paths],
                                 "fixture must contain exactly the costly-first and all simple worktrees")
                self.assertFalse(fixture.ancestor(fixture.costly_head, fixture.target))
                self.assertNotEqual(fixture.trees[fixture.costly_head], fixture.trees[fixture.target],
                                    "distinct trees prevent an accidental tree-identity positive")
                for path in fixture.paths[1:]:
                    self.assertTrue(fixture.ancestor(fixture.heads[path], fixture.target))
                self.assertEqual(len(set(fixture.heads.values())), count,
                                 "distinct HEADs must exercise fairness beyond identical-OID cache sharing")
                fixture.git("merge-base", "--is-ancestor", fixture.costly_head, fixture.target, expected=1)
                fixture.git("merge-base", "--is-ancestor", fixture.target, fixture.target)
                self.assertEqual(before, fixture.manifest(), "fixture topology checks must preserve sources")
                fixture.close()

    def test_history_really_exceeds_512_commits_and_contains_known_ancestor(self):
        fixture = self.fixture
        tree, _ = fixture.tree(b"long history\n")
        chain = fixture.chain(600, tree)
        fixture.checkout(chain[0])
        fixture.git("update-ref", "refs/heads/integration", chain[-1])
        before = fixture.manifest()
        self.assertEqual(int(fixture.git("rev-list", "--count", chain[-1])), 600,
                         "dedicated fixture must exceed the old 512 traversal ceiling")
        self.assertTrue(fixture.ancestor(chain[0], chain[-1]))
        fixture.git("merge-base", "--is-ancestor", chain[0], chain[-1])
        self.assertEqual(before, fixture.manifest())

    def test_missing_target_does_not_erase_second_real_covering_target(self):
        fixture = self.fixture.many_worktrees(2, depth=3)
        missing = fixture.unavailable_target()
        before = fixture.manifest()
        self.assertNotEqual(missing, fixture.target)
        result = fixture.git("cat-file", "-e", missing, expected=1)
        self.assertEqual(result, b"")
        self.assertTrue(fixture.ancestor(fixture.heads[fixture.paths[-1]], fixture.target))
        fixture.git("merge-base", "--is-ancestor", fixture.target, "refs/heads/integration")
        self.assertEqual(before, fixture.manifest())


class CliScaleTests(unittest.TestCase):
    """Default CLI history assertions, independently derived from fixture DAG."""

    def setUp(self):
        self.fixture = ScaleFixture()
        self.addCleanup(self.fixture.close)

    def run_cli(self, *arguments):
        before = self.fixture.manifest()
        binary_digest = hashlib.sha256(BINARY.read_bytes()).hexdigest()
        try:
            raw = self.fixture.command([str(BINARY), *map(str, arguments)], timeout=180)
            stderr = self.fixture.last_stderr
        finally:
            self.assertEqual(before, self.fixture.manifest(),
                             "CLI modified source/admin/index or external sentinel (atime excluded)")
            self.assertEqual(hashlib.sha256(BINARY.read_bytes()).hexdigest(), binary_digest,
                             "binary changed during independent oracle execution")
        return json.loads(raw), stderr

    def inventory(self):
        saved, _ = self.run_cli("scan", "--repo", self.fixture.repo, "--format", "json")
        actual = [decode_path(worktree["path"]) for family in saved["families"]
                  for worktree in family["worktrees"]]
        self.assertEqual(actual, [os.fsencode(path) for path in self.fixture.paths])
        return saved

    def plan(self, inventory, targets=("refs/heads/integration",)):
        path = self.fixture.root / "inventory.json"
        path.write_text(json.dumps(inventory))
        self.fixture.protected_files.append(path)
        arguments = ["plan", "--inventory", path, "--format", "json", "--verbose"]
        for target in targets:
            arguments.extend(("--target", target))
        report, stderr = self.run_cli(*arguments)
        self.assertEqual(report["schema"], "wt-sweep.review/2")
        actual = {decode_path(item["path"]): item for item in report["items"]}
        self.assertEqual(len(actual), len(report["items"]), "duplicate result paths")
        self.assertEqual(set(actual), {os.fsencode(path) for path in self.fixture.paths},
                         "history review omitted or invented a worktree")
        return report, actual, stderr

    def history(self, item, target="refs/heads/integration"):
        self.assertIn("history", item, "T03 requires graph history distinct from deep observations")
        matches = [record for record in item["history"] if decode_path(record["target_ref"]) == target.encode()]
        self.assertEqual(len(matches), 1, "each explicit target requires one outcome or precise omission")
        record = matches[0]
        self.assertEqual(record["source_oid"], item["head"])
        self.assertIn(record["result"], ("identical_commit", "ancestor", "not_ancestor", "unknown"))
        self.assertLessEqual(record["started_unix_ms"], record["finished_unix_ms"])
        self.assertEqual(item["history_omitted_targets"], 0)
        return record

    def stable_results(self, items):
        # Comparison excludes real observation clocks and traversal-dependent metrics.
        return {path: (self.history(item)["source_oid"], self.history(item)["target_oid"],
                       self.history(item)["result"], self.history(item)["observed_result"],
                       item["history_omitted_targets"])
                for path, item in items.items()}

    def decision_evidence(self, item):
        # Clocks and their derived digest vary even when all collected proofs agree.
        # Keep source hashes, identities, completeness, positives, and refusal causes.
        ignored = {"started_unix_ms", "finished_unix_ms", "digest_sha256", "classification", "decision"}
        def normalize(value):
            if isinstance(value, dict):
                return {key: normalize(child) for key, child in value.items() if key not in ignored}
            if isinstance(value, list):
                return [normalize(child) for child in value]
            return value
        return normalize(item)

    def assert_order_verdicts_follow_collected_evidence(self, first, second):
        for path in first:
            a, b = first[path], second[path]
            if self.decision_evidence(a) == self.decision_evidence(b):
                self.assertEqual(a["classification"], b["classification"],
                                 "identical collected proofs must give the same judgement")
            elif a["classification"] != b["classification"]:
                self.assertEqual({a["classification"], b["classification"]},
                                 {"cleanup_candidate", "verify"}, "positive retention must not disappear")
                partial = a if a["classification"] == "verify" else b
                complete = b if partial is a else a
                self.assertEqual(partial["decision"]["missing_conditions"], ["C4", "C5"],
                                 "only the reproduced private final-proof gap explains this difference")
                history = partial["private_history"]
                self.assertFalse(history["enumeration_complete"])
                self.assertTrue(any("final" in gap and ("validation" in gap or "check" in gap)
                                    for gap in history["gaps"]), "final refusal cause must stay visible")
                self.assertTrue(history["roots"], "unknown fallback must not erase collected private roots")
                self.assertTrue(all(root["result"] == "unknown" and root["observed_result"] == "covered"
                                    and root["reason"] for root in history["roots"]),
                                "final deadline must preserve the positive observed coverage")
                self.assertTrue(all(root["result"] == "covered"
                                    for root in complete["private_history"]["roots"]))

    def test_46_then_100_distinct_heads_costly_first_have_history_and_order_invariance(self):
        for count in (46, 100):
            with self.subTest(worktrees=count):
                self.fixture = ScaleFixture().many_worktrees(count)
                self.addCleanup(self.fixture.close)
                inventory = self.inventory()
                report, first, stderr = self.plan(inventory)
                for path in self.fixture.paths[1:]:
                    record = self.history(first[os.fsencode(path)])
                    expected = "identical_commit" if self.fixture.heads[path] == self.fixture.target else "ancestor"
                    self.assertEqual(record["result"], expected,
                                     "costly first case starved an independently known simple ancestry")
                    self.assertEqual(record["target_oid"], self.fixture.target)
                costly = self.history(first[os.fsencode(self.fixture.repo)])
                self.assertEqual(costly["result"], "unknown", "default 512 cap must stay explicit")
                self.assertTrue(costly["stop_reason"], "capped divergent walk cannot claim complete negative")
                metrics = report["history_metrics"]
                self.assertEqual(metrics["items_total"], count)
                self.assertEqual(metrics["first_pass_items"], count)
                self.assertLessEqual(metrics["object_read_attempts"], 2048)
                self.assertLessEqual(metrics["object_bytes"], 32 * 1024 * 1024)
                self.assertLessEqual(metrics["traversal_steps"], 65536)
                self.assertRegex(stderr.decode(errors="replace"), r"(?i)history",
                                 "progress must identify history phase on stderr")
                inventory["families"][0]["worktrees"].reverse()
                _, second, _ = self.plan(inventory)
                self.assertEqual(self.stable_results(first), self.stable_results(second),
                                 "equivalent commit-graph coverage must not depend on inventory order")
                self.assert_order_verdicts_follow_collected_evidence(first, second)
                self.fixture.close()

    def test_stdout_stays_json_and_stderr_records_phases_and_processed_count(self):
        self.fixture.many_worktrees(2, depth=3)
        report, _, stderr = self.plan(self.inventory())
        self.assertEqual(report["schema"], "wt-sweep.review/2", "stdout must remain strict JSON")
        linked = next(item for item in report["items"]
                      if decode_path(item["path"]) == os.fsencode(self.fixture.paths[1]))
        self.assertEqual(linked["classification"], "cleanup_candidate",
                         "small complete corpus must retain a positive final candidate")
        progress = stderr.decode(errors="replace")
        for phase in ("history", "local files", "Verifying commit", "Review finished"):
            self.assertIn(phase, progress, "CLI progress omitted an execution phase")
        self.assertRegex(progress, r"Review finished: 2 worktrees", "progress needs worktree count")

    def test_more_than_512_commits_is_explicit_unknown_not_false_negative(self):
        fixture = self.fixture
        tree, _ = fixture.tree(b"long target\n")
        other_tree, _ = fixture.tree(b"divergent source\n")
        source = fixture.commit(other_tree)
        target = fixture.chain(600, tree)[-1]
        fixture.checkout(source)
        fixture.git("update-ref", "refs/heads/integration", target)
        self.assertFalse(fixture.ancestor(source, target))
        _, items, _ = self.plan(self.inventory())
        record = self.history(items[os.fsencode(fixture.repo)])
        self.assertEqual(record["result"], "unknown")
        self.assertTrue(record["stop_reason"])
        self.assertIsNone(record["observed_result"])

    def test_complete_divergent_walk_has_negative_for_every_selected_target(self):
        fixture = self.fixture
        source_tree, _ = fixture.tree(b"source unique tree\n")
        source = fixture.commit(source_tree)
        fixture.checkout(source)
        target_oids = {}
        for number in range(2):
            tree, _ = fixture.tree(f"target {number}\n".encode())
            target = fixture.chain(3, tree)[-1]
            reference = f"refs/heads/integration-{number}"
            fixture.git("update-ref", reference, target)
            target_oids[reference] = target
            self.assertFalse(fixture.ancestor(source, target))
        _, items, stderr = self.plan(self.inventory(), tuple(target_oids))
        progress = stderr.decode(errors="replace")
        for phase in ("history", "local files", "Verifying commit", "Review finished"):
            self.assertIn(phase, progress, "CLI progress omitted an execution phase")
        self.assertRegex(progress, r"Review finished: 1 worktree;", "progress needs worktree count")
        item = items[os.fsencode(fixture.repo)]
        for reference, oid in target_oids.items():
            record = self.history(item, reference)
            self.assertEqual(record["result"], "not_ancestor")
            self.assertEqual(record["target_oid"], oid)
            self.assertIsNone(record["stop_reason"])
        self.assertFalse(any(observation["target_role"] == "integration" for observation in item["observations"]))

    def test_unavailable_first_target_preserves_second_covering_target(self):
        fixture = self.fixture.many_worktrees(2, depth=3)
        fixture.unavailable_target()
        _, items, _ = self.plan(self.inventory(), ("refs/heads/00-inaccessible", "refs/heads/integration"))
        item = items[os.fsencode(fixture.paths[-1])]
        self.assertEqual(self.history(item, "refs/heads/00-inaccessible")["result"], "unknown")
        self.assertTrue(self.history(item, "refs/heads/00-inaccessible")["stop_reason"])
        covering = self.history(item)
        self.assertEqual(covering["result"], "identical_commit")
        self.assertEqual(covering["target_oid"], fixture.target)
        self.assertEqual(item["classification"], "cleanup_candidate",
                         "unrelated target failure erased independently current coverage")
        self.assertTrue(any(o["target_role"] == "integration" and o["relation"] == "identical_commit"
                            and o["target_oid"] == fixture.target for o in item["observations"]),
                        "graph-only positive was promoted without actual deep evidence")

    def test_missing_blob_retains_graph_ancestor_without_deep_integration(self):
        fixture = self.fixture
        tree, blob = fixture.tree(b"missing deep blob\n")
        source, target = fixture.chain(2, tree)
        fixture.checkout(source)
        fixture.git("update-ref", "refs/heads/integration", target)
        linked = fixture.root / "missing-blob-linked"
        fixture.git("worktree", "add", "--quiet", "--detach", linked, source)
        fixture.paths.append(linked)
        fixture.heads[linked] = source
        inventory = self.inventory()
        fixture.remove_object(blob)
        self.assertTrue(fixture.ancestor(source, target))
        _, items, _ = self.plan(inventory)
        item = items[os.fsencode(linked)]
        record = self.history(item)
        self.assertEqual(record["result"], "ancestor", "missing blob does not erase available verified commits")
        self.assertEqual(record["target_oid"], target)
        self.assertFalse(any(observation["target_role"] == "integration" for observation in item["observations"]),
                         "graph-only evidence must not become deep content verification")
        self.assertEqual(item["classification"], "verify")

        self.assertFalse(item["local_summary"]["head_complete"])
        self.assertIn("C3", item["decision"]["missing_conditions"])


if __name__ == "__main__":
    unittest.main()
