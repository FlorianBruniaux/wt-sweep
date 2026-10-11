"""Opt-in black-box qualification of Worktrunk dispatch. Creates only owned fixtures.

WORKTRUNK_BINARY and WT_SWEEP_BINARY must name explicit executables. No install,
network, real repository selection or host configuration is performed.
"""
import hashlib
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest

WT = os.environ.get('WORKTRUNK_BINARY')
SWEEP = os.environ.get('WT_SWEEP_BINARY')


@unittest.skipUnless(WT and SWEEP, 'set explicit WORKTRUNK_BINARY and WT_SWEEP_BINARY')
class WorktrunkDispatch(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='wt-sweep-dispatch-', dir='/private/tmp')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bin = self.root / 'bin'
        self.bin.mkdir()
        self.repo = self.root / 'repo with spaces'
        self.config = self.root / 'worktrunk.toml'
        self.config.write_text('')
        self.env = {
            'PATH': f'{self.bin}:/usr/bin:/bin', 'HOME': str(self.root / 'home'),
            'XDG_CONFIG_HOME': str(self.root / 'config'),
            'XDG_DATA_HOME': str(self.root / 'data'),
            'XDG_STATE_HOME': str(self.root / 'state'),
            'XDG_CACHE_HOME': str(self.root / 'cache'),
            'WORKTRUNK_APPROVALS_PATH': str(self.root / 'approvals.toml'),
            'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null',
            'GIT_OPTIONAL_LOCKS': '0', 'GIT_TERMINAL_PROMPT': '0',
            'GIT_NO_LAZY_FETCH': '1', 'GIT_ALLOW_PROTOCOL': '', 'LC_ALL': 'C',
            'GIT_AUTHOR_NAME': 'Fixture', 'GIT_COMMITTER_NAME': 'Fixture',
            'GIT_AUTHOR_EMAIL': 'fixture@example.invalid',
            'GIT_COMMITTER_EMAIL': 'fixture@example.invalid', 'NO_COLOR': '1',
        }
        (self.root / 'home').mkdir()
        self.git('init', '-b', 'main', str(self.repo))
        (self.repo / 'tracked.txt').write_text('committed fixture\n')
        self.git('-C', str(self.repo), 'add', 'tracked.txt')
        self.git('-C', str(self.repo), 'commit', '-m', 'fixture')

    def git(self, *args):
        return subprocess.run(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null',
                               '-c', 'init.templateDir=', *args], env=self.env,
                              cwd=self.root, capture_output=True, check=True, timeout=30)

    def wt(self, *args, cwd=None):
        return subprocess.run([WT, '--config', str(self.config), *args], env=self.env,
                              cwd=cwd or self.root, capture_output=True, timeout=30)

    def probe(self, name='wt-sweep', directory=None):
        path = (directory or self.bin) / name
        path.write_text(f'#!{sys.executable}\n' + '''import json,os,sys
print(json.dumps({'argv_hex':[os.fsencode(x).hex() for x in sys.argv[1:]],'cwd':os.getcwd()}))
sys.stderr.write('child stderr\\n')
sys.exit(int(os.environ.get('PROBE_EXIT','0')))
''')
        path.chmod(0o700)
        return path

    def real_sweep(self):
        (self.bin / 'wt-sweep').symlink_to(Path(SWEEP).resolve())

    def fingerprint(self, root=None):
        root = root or self.repo
        result = {}
        for p in sorted(root.rglob('*')):
            s = p.lstat()
            payload = hashlib.sha256(p.read_bytes()).hexdigest() if stat.S_ISREG(s.st_mode) else None
            result[str(p.relative_to(root))] = [s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns,payload]
        return result

    def test_literal_arguments_and_streams(self):
        self.probe()
        args = ['space value', "quote'\"", '$(touch MUST_NOT_EXIST)', '; echo bad', '', '\n', 'é', '--target']
        p = self.wt('sweep', *args)
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertEqual(json.loads(p.stdout)['argv_hex'], [os.fsencode(x).hex() for x in args])
        self.assertEqual(p.stderr, b'child stderr\n')
        self.assertFalse((self.root / 'MUST_NOT_EXIST').exists())

    def test_raw_non_utf8_argument(self):
        self.probe()
        p = self.wt('sweep', b'raw-\xff-name')
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertEqual(json.loads(p.stdout)['argv_hex'], [b'raw-\xff-name'.hex()])

    def test_inherited_and_explicit_cwd(self):
        self.probe()
        for args, expected in [(('sweep',),self.root),(('-C',str(self.repo),'sweep'),self.repo)]:
            with self.subTest(args=args):
                p = self.wt(*args)
                self.assertEqual(p.returncode, 0, p.stderr)
                self.assertEqual(Path(json.loads(p.stdout)['cwd']).resolve(),expected.resolve())

    def test_nonzero_exit_is_preserved(self):
        self.probe()
        self.env['PROBE_EXIT'] = '23'
        p = self.wt('sweep')
        self.assertEqual(p.returncode,23)
        self.assertEqual(p.stderr,b'child stderr\n')

    def test_alias_collision_takes_precedence(self):
        self.probe()
        self.config.write_text('[aliases]\nsweep = "printf alias-selected"\n')
        p = self.wt('-C',str(self.repo),'sweep')
        self.assertEqual(p.returncode,0,p.stderr)
        self.assertIn(b'alias-selected',p.stdout)
        self.assertNotIn(b'argv_hex',p.stdout)

    def test_first_path_binary_wins(self):
        self.probe()
        second = self.root / 'second'
        second.mkdir()
        self.probe(directory=second)
        (second / 'wt-sweep').write_text('#!/bin/sh\nprintf wrong-binary\n')
        self.env['PATH'] += f':{second}'
        p = self.wt('sweep')
        self.assertEqual(p.returncode,0,p.stderr)
        self.assertIn('argv_hex',json.loads(p.stdout))

    def test_builtin_collision_and_missing_command(self):
        self.probe(name='wt-list')
        p = self.wt('list','--help')
        self.assertEqual(p.returncode,0,p.stderr)
        self.assertNotIn(b'argv_hex',p.stdout)
        absent = self.wt('sweep')
        self.assertEqual(absent.returncode,2)
        self.assertIn(b'unrecognized subcommand',absent.stderr)

    def test_real_sweep_help_and_invalid_argument(self):
        self.real_sweep()
        for args in [('review','--help'),('review','--not-a-sweep-option')]:
            direct = subprocess.run([SWEEP,*args],env=self.env,cwd=self.root,
                                    capture_output=True,timeout=30)
            routed = self.wt('sweep',*args)
            self.assertEqual((routed.returncode,routed.stdout,routed.stderr),
                             (direct.returncode,direct.stdout,direct.stderr))
        self.assertNotEqual(direct.returncode,0)

    def test_real_scan_nonempty_and_read_only(self):
        self.real_sweep()
        before = self.fingerprint()
        p = self.wt('-C',str(self.repo),'sweep','scan','--repo',str(self.repo),'--format','json')
        self.assertEqual(p.returncode,0,p.stderr)
        document = json.loads(p.stdout)
        self.assertEqual(len(document['families']),1)
        self.assertEqual(len(document['families'][0]['worktrees']),1)
        self.assertEqual(self.fingerprint(),before)

    def test_real_review_retains_local_work(self):
        self.real_sweep()
        linked = self.root / 'linked worktree'
        clean = self.root / 'clean worktree'
        self.git('-C',str(self.repo),'worktree','add','-b','topic',str(linked))
        self.git('-C',str(self.repo),'worktree','add','-b','clean',str(clean))
        (linked / 'notes.txt').write_text('must retain\n')
        linked_before, clean_before = self.fingerprint(linked), self.fingerprint(clean)
        out = self.root / 'reports'
        out.mkdir()
        before = self.fingerprint()
        p = self.wt('-C',str(self.repo),'sweep','review','--repo',str(self.repo),
                    '--target','refs/heads/main','--run-dir',str(out),'--summary')
        self.assertEqual(p.returncode,0,p.stderr)
        reviews = list(out.glob('run-*/review.json'))
        self.assertEqual(len(reviews),1)
        document = json.loads(reviews[0].read_bytes())
        rows = {row['path']['hex']: row for row in document['items']}
        self.assertEqual(len(rows),3)
        self.assertEqual(rows[os.fsencode(clean).hex()]['classification'],'cleanup_candidate')
        dirty = rows[os.fsencode(linked).hex()]
        self.assertEqual(dirty['classification'],'keep')
        self.assertTrue(any('untracked' in reason for reason in dirty['decision']['keep_reasons']))
        self.assertEqual(self.fingerprint(),before)
        self.assertEqual(self.fingerprint(linked),linked_before)
        self.assertEqual(self.fingerprint(clean),clean_before)


if __name__ == '__main__':
    unittest.main(verbosity=2)
