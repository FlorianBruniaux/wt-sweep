"""Actual Git creation and clone isolation on owned macOS fixtures."""
import sys
import unittest
import creation


@unittest.skipUnless(sys.platform == 'darwin', 'requires actual macOS clonefile')
class CreationTests(unittest.TestCase):
    def test_git_creation_needs_selected_ignored_dependencies(self):
        result = creation.run(samples=1, files=2, bytes_per_file=4096)
        self.assertTrue(result['need']['stock_worktree_missing_dependencies'])
        self.assertTrue(result['source_unchanged'])
        self.assertEqual({row['method'] for row in result['samples']}, {'copy', 'clone'})
        for row in result['samples']:
            self.assertTrue(row['dependency_bytes_equal'])
            self.assertTrue(row['isolated_write'])
            self.assertTrue(row['index_clean'])
            self.assertTrue(row['source_staging_not_inherited'])
            self.assertTrue(row['filter_correct'])
            self.assertTrue(row['unselected_ignored_absent'])
            self.assertEqual(row['source_device'], row['staging_device'])
            self.assertEqual(row['source_device'], row['destination_device'])
        self.assertEqual(result['unique_physical_bytes'], 'UNKNOWN')

    def test_clone_refuses_existing_destination_without_overwriting(self):
        import tempfile
        from pathlib import Path
        with tempfile.TemporaryDirectory(dir='/private/tmp') as directory:
            root = Path(directory)
            (root / 'src').write_bytes(b'source')
            (root / 'dst').write_bytes(b'sentinel')
            with self.assertRaises(FileExistsError):
                creation.clone(root / 'src', root / 'dst')
            self.assertEqual((root / 'dst').read_bytes(), b'sentinel')

if __name__ == '__main__':
    unittest.main()
