"""Budget choices must be explicit, recorded and leave judgement unchanged."""
import json
import unittest

from inventory_support import BINARY, HistoryFixture, decode_path, filesystem_manifest
import test_review_command as command_oracles


class BudgetProfileTests(unittest.TestCase):
    def setUp(self):
        self.fixture = HistoryFixture()
        self.addCleanup(self.fixture.close)

    def run_cli(self, *args):
        return command_oracles.ReviewCommandTests.owned_process(
            self, [BINARY, *args], timeout=60)

    def test_profiles_record_applied_limits_and_measured_phases_with_positive_candidate(self):
        before = filesystem_manifest(self.fixture.root)
        for profile in ("standard", "thorough"):
            result = self.run_cli("review", "--repo", self.fixture.principal,
                                  "--target", "refs/heads/main", "--budget", profile,
                                  "--no-context", "--format", "json")
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            report = json.loads(result.stdout)
            rows = {decode_path(row["path"]): row for row in report["items"]}
            self.assertEqual(rows[bytes(self.fixture.linked)]["classification"], "cleanup_candidate")
            metrics = report["collection_metrics"]
            self.assertEqual(metrics["budget_profile"], profile)
            limits = metrics["applied_limits"]
            self.assertGreater(limits["local_max_objects"], 0)
            self.assertGreater(limits["private_object_reads"], 0)
            phases = {phase["name"]: phase for phase in metrics["phases"]}
            for name in ("controls", "history", "local", "deep", "private_history", "revalidation"):
                self.assertIn(name, phases)
                self.assertGreaterEqual(phases[name]["elapsed_ms"], 0)
            self.assertGreater(phases["local"]["counters"]["git_processes"], 0)
            self.assertGreater(phases["deep"]["counters"]["object_reads"], 0)
        self.assertEqual(before, filesystem_manifest(self.fixture.root))

    def test_invalid_budget_refuses_before_discovery(self):
        result = self.run_cli("review", "--repo", self.fixture.principal, "--budget", "unlimited")
        self.assertEqual(result.returncode, 2)
        self.assertIn(b"budget", result.stderr)
        self.assertNotIn(b"Discovering", result.stderr)
        self.assertEqual(result.stdout, b"")
