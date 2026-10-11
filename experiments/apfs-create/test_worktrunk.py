"""Optional installed Worktrunk alternative on an owned synthetic repository."""
import os
from pathlib import Path
import unittest
import worktrunk_reference


@unittest.skipUnless(os.environ.get('WT_APFS_WORKTRUNK'), 'set WT_APFS_WORKTRUNK to pinned native binary')
class WorktrunkAlternative(unittest.TestCase):
    def test_selected_dependency_copy_preserves_target_and_source(self):
        result = worktrunk_reference.run(Path(os.environ['WT_APFS_WORKTRUNK']))
        for key in ('copied_dependency', 'excluded_env', 'target_index_clean',
                    'source_unchanged', 'isolated_write', 'repeat_preserves_destination'):
            self.assertTrue(result[key], key)


if __name__ == '__main__':
    unittest.main()
