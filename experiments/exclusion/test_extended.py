"""Counterexamples must contain real writes, not a fabricated authority flag."""
import unittest
import subprocess
import sys
from pathlib import Path
import extended


class ExtendedWitnesses(unittest.TestCase):
    def test_optimized_python_refuses_before_fixture_protocol(self):
        completed = subprocess.run([sys.executable, '-O', '-B', '-c',
                                    'import extended; extended.run()'],
                                   cwd=Path(__file__).parent, capture_output=True,
                                   timeout=25)
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn(b'optimized Python is unsupported', completed.stderr)

    def test_mapping_and_new_opens_survive_rename_and_lock(self):
        result = extended.run()
        self.assertEqual(set(result['writers']), {'mapping', 'new_open', 'hard_alias', 'symlink_alias'})
        for name, witness in result['writers'].items():
            with self.subTest(name=name):
                self.assertTrue(witness['write_observed'])
                self.assertTrue(witness['same_inode'])
                self.assertTrue(witness['outside_cwd'])

    def test_interrupted_cooperative_guard_releases_at_every_checkpoint(self):
        result = extended.run()
        self.assertEqual(len(result['guard_loss']), 4)
        for witness in result['guard_loss']:
            self.assertTrue(witness['locked_before_interrupt'])
            self.assertTrue(witness['acquired_after_interrupt'])
            self.assertTrue(witness['write_observed'])
            self.assertTrue(witness['source_retained'])
        self.assertEqual(result['independent_authority'], 'NOT_ESTABLISHED')


if __name__ == '__main__':
    unittest.main()
