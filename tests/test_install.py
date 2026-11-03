"""Installer behavior in owned directories; no changes to the user's PATH."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class InstallTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='sweep-install-')
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.package = self.base / 'download with spaces'
        self.package.mkdir()
        self.prefix = self.base / 'local with spaces'
        self.version = '0.1.0-alpha.4'
        self.make_package(self.version)

    def make_package(self, version):
        (self.package / 'VERSION').write_text(version + '\n')
        (self.package / 'wt-sweep').write_text('#!/bin/sh\nprintf "wt-sweep ' + version + '\\n"\n')
        (self.package / 'wt-sweep').chmod(0o755)
        (self.package / 'LICENSE.md').write_text('Fixture license\n')
        (self.package / 'FILES.sha256').write_text(''.join(
            hashlib.sha256((self.package / name).read_bytes()).hexdigest() + '  ' + name + '\n'
            for name in ('VERSION', 'wt-sweep', 'LICENSE.md')))

    def install(self, *args):
        return subprocess.run(['/bin/bash', str(ROOT / 'scripts/release/install.sh'),
                               '--package', str(self.package), '--prefix', str(self.prefix), *args],
                              capture_output=True, text=True, timeout=15)

    def test_installed_command_survives_removed_download(self):
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        shutil.rmtree(self.package)
        binary = self.prefix / 'bin/wt-sweep'
        self.assertEqual(subprocess.check_output([binary, '--version'], text=True).strip(),
                         'wt-sweep ' + self.version)
        self.assertEqual((binary.resolve().parent / 'LICENSE.md').read_text(), 'Fixture license\n')

    def test_existing_command_requires_explicit_replace(self):
        self.prefix.joinpath('bin').mkdir(parents=True)
        command = self.prefix / 'bin/wt-sweep'
        command.write_text('existing command')
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('--replace', result.stderr)
        self.assertEqual(command.read_text(), 'existing command')

    def test_update_keeps_previous_version_and_switches_launcher(self):
        self.assertEqual(self.install().returncode, 0)
        previous = (self.prefix / 'bin/wt-sweep').resolve()
        self.make_package('0.1.0-alpha.5')
        result = self.install('--replace')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(previous.is_file())
        self.assertEqual(subprocess.check_output([self.prefix / 'bin/wt-sweep'], text=True).strip(),
                         'wt-sweep 0.1.0-alpha.5')

    def test_corrupted_package_leaves_destination_absent(self):
        (self.package / 'wt-sweep').write_text('corrupted')
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.prefix.exists())

    def test_reinstallation_does_not_overwrite_version(self):
        self.assertEqual(self.install().returncode, 0)
        installed = (self.prefix / 'bin/wt-sweep').resolve()
        before = installed.read_bytes()
        result = self.install('--replace')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(installed.read_bytes(), before)

    def test_invalid_version_refused_before_writes(self):
        self.make_package('../outside')
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.prefix.exists())

    def test_unknown_option_refused(self):
        result = self.install('--typo')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.prefix.exists())

    def test_directory_launcher_preserved(self):
        directory = self.prefix / 'bin/wt-sweep'
        directory.mkdir(parents=True)
        (directory / 'sentinel').write_text('keep')
        result = self.install('--replace')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((directory / 'sentinel').read_text(), 'keep')

    def test_installation_inside_download_refused(self):
        self.prefix = self.package / 'local'
        result = self.install()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('outside the package', result.stderr)
        self.assertFalse(self.prefix.exists())

    def test_default_prefix_uses_home_without_shell_edits(self):
        fake_home = self.base / 'home'
        fake_home.mkdir()
        rc = fake_home / '.zshrc'
        rc.write_text('# existing configuration\n')
        result = subprocess.run(['/bin/bash', str(ROOT / 'scripts/release/install.sh'),
            '--package', str(self.package)], env={**os.environ, 'HOME': str(fake_home)},
            capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((fake_home / '.local/bin/wt-sweep').is_file())
        self.assertEqual(rc.read_text(), '# existing configuration\n')
