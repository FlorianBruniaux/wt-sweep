#!/usr/bin/env python3
"""Owned synthetic audit comparison. Never accepts a real repository as input.

Build competitor CLIs separately; this runner only calls Holt status, AgentRinse
stateless audit and Sweep review. Reports and fixtures remain in an exclusively
created temporary run directory. No cleanup/recovery commands are exposed.
"""
import argparse
import hashlib
import json
import os
import shutil
from pathlib import Path
import stat
import subprocess
import tempfile
import time

SCENARIOS = ('integrated', 'unintegrated', 'index_disk', 'ignored_untracked',
             'stash', 'private_reflog', 'missing_object', 'missing_target',
             'unreadable', 'changed_target', 'merge_conflict')
GIT = '/usr/bin/git'


def isolated_env(home):
    home.mkdir(parents=True, exist_ok=True)
    # Allowlist avoids inheriting credentials or executable Git configuration.
    return {'PATH': '/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin', 'HOME': str(home),
            'XDG_CONFIG_HOME': str(home / 'config'), 'XDG_CACHE_HOME': str(home / 'cache'),
            'XDG_DATA_HOME': str(home / 'data'), 'XDG_STATE_HOME': str(home / 'state'),
            'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null',
            'GIT_TERMINAL_PROMPT': '0', 'GIT_OPTIONAL_LOCKS': '0',
            'GIT_NO_LAZY_FETCH': '1', 'GIT_ALLOW_PROTOCOL': '',
            'GIT_AUTHOR_NAME': 'Synthetic Test', 'GIT_AUTHOR_EMAIL': 'synthetic@example.invalid',
            'GIT_COMMITTER_NAME': 'Synthetic Test', 'GIT_COMMITTER_EMAIL': 'synthetic@example.invalid',
            'GIT_AUTHOR_DATE': '2020-01-01T00:00:00Z',
            'GIT_COMMITTER_DATE': '2020-01-01T00:00:00Z', 'LC_ALL': 'C', 'NO_COLOR': '1'}


def git(root, env, *args):
    p = subprocess.run([GIT, '--no-optional-locks', '-c', 'core.fsmonitor=false',
                        '-c', 'core.hooksPath=/dev/null', '-C', str(root), *args],
                       env=env, capture_output=True, timeout=20, check=True)
    return p.stdout.decode('utf-8')


def commit(repo, env, message):
    git(repo, env, 'add', '--', 'tracked.txt', '.gitignore')
    git(repo, env, 'commit', '-m', message)
    return git(repo, env, 'rev-parse', 'HEAD').strip()


def fixture(root, scenario, env):
    root.mkdir(parents=True)
    repo, work = root / 'repo', root / 'work'
    repo.mkdir()
    git(repo, env, 'init', '-b', 'main')
    git(repo, env, 'config', 'remote.origin.url', 'https://example.invalid/synthetic.git')
    (repo / 'tracked.txt').write_text('base\n')
    (repo / '.gitignore').write_text('ignored/\n')
    base = commit(repo, env, 'base')
    git(repo, env, 'worktree', 'add', '-b', 'topic', str(work), base)
    facts = {'scenario': scenario, 'base_oid': base, 'target_ref': 'refs/heads/main',
             'obligation': scenario, 'worktree': str(work), 'repo': str(repo)}
    if scenario in ('integrated', 'changed_target'):
        (work / 'tracked.txt').write_text('landed\n')
        head = commit(work, env, 'landed')
        git(repo, env, 'reset', '--hard', head)
    if scenario == 'unintegrated':
        (work / 'tracked.txt').write_text('private current\n')
        head = commit(work, env, 'unlanded but locally remote-covered')
        git(repo, env, 'update-ref', 'refs/remotes/origin/topic', head)
        git(work, env, 'config', 'branch.topic.remote', 'origin')
        git(work, env, 'config', 'branch.topic.merge', 'refs/heads/topic')
    else:
        git(repo, env, 'update-ref', 'refs/remotes/origin/main', git(repo, env, 'rev-parse', 'HEAD').strip())
        git(work, env, 'config', 'branch.topic.remote', 'origin')
        git(work, env, 'config', 'branch.topic.merge', 'refs/heads/main')
    if scenario == 'merge_conflict':
        (work / 'tracked.txt').write_text('topic conflict\n')
        commit(work, env, 'topic conflict')
        (repo / 'tracked.txt').write_text('main conflict\n')
        commit(repo, env, 'main conflict')
    if scenario == 'index_disk':
        (work / 'tracked.txt').write_text('staged\n')
        git(work, env, 'add', '--', 'tracked.txt')
        (work / 'tracked.txt').write_text('disk\n')
    if scenario == 'ignored_untracked':
        (work / 'notes.txt').write_text('untracked author bytes\n')
        (work / 'ignored').mkdir()
        (work / 'ignored/cache.txt').write_text('ignored author bytes\n')
    if scenario == 'stash':
        (repo / 'tracked.txt').write_text('stash tracked\n')
        git(repo, env, 'add', '--', 'tracked.txt')
        (repo / 'tracked.txt').write_text('stash unstaged\n')
        (repo / 'stash-note.txt').write_text('stash untracked\n')
        git(repo, env, 'stash', 'push', '-u', '-m', 'synthetic shared stash')
    if scenario == 'private_reflog':
        (work / 'tracked.txt').write_text('reflog-only authored\n')
        facts['private_oid'] = commit(work, env, 'private reflog')
        git(work, env, 'reset', '--hard', base)
    if scenario == 'missing_object':
        oid = git(work, env, 'rev-parse', 'HEAD:tracked.txt').strip()
        (repo / '.git/objects' / oid[:2] / oid[2:]).unlink()
        facts['missing_blob_oid'] = oid
    if scenario == 'missing_target':
        facts['target_ref'] = 'refs/heads/absent'
    if scenario == 'unreadable':
        denied = work / 'denied'
        denied.mkdir()
        (denied / 'secret.txt').write_text('unreadable synthetic bytes\n')
        denied.chmod(0)
    facts['head_oid'] = git(work, env, 'rev-parse', 'HEAD').strip()
    # Age is intentionally controlled to avoid testing only a recently-created veto.
    os.utime(work, (1577836800, 1577836800))
    return repo, work, facts


def snapshot(root):
    """Filesystem oracle, independent of all three Git collectors.

    Includes raw index/refs/reflogs and objects. Ignores atime, mtime and inode
    identity; timestamps are not byte conservation. Access errors remain records.
    """
    records = {}
    def walk(path):
        relative = os.fsdecode(os.path.relpath(os.fsencode(path), os.fsencode(root)))
        try:
            info = path.lstat()
            mode = stat.S_IMODE(info.st_mode)
            if stat.S_ISLNK(info.st_mode):
                records[relative] = {'kind': 'symlink', 'mode': mode,
                                     'target_hex': os.fsencode(os.readlink(path)).hex()}
            elif stat.S_ISDIR(info.st_mode):
                records[relative] = {'kind': 'directory', 'mode': mode}
                for child in sorted(path.iterdir()):
                    walk(child)
            elif stat.S_ISREG(info.st_mode):
                records[relative] = {'kind': 'file', 'mode': mode, 'size': info.st_size,
                                     'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
            else:
                records[relative] = {'kind': 'special', 'mode': mode}
        except OSError as error:
            records[relative] = {'kind': 'error', 'errno': error.errno}
    walk(root)
    return records


def delta(before, after):
    return {'added': sorted(after.keys() - before.keys()),
            'removed': sorted(before.keys() - after.keys()),
            'modified': sorted(k for k in before.keys() & after.keys() if before[k] != after[k])}


def run_command(argv, env, cwd, output):
    started = time.monotonic()
    try:
        p = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, timeout=120)
        code, stdout, stderr = p.returncode, p.stdout, p.stderr
    except subprocess.TimeoutExpired as error:
        code, stdout, stderr = None, error.stdout or b'', error.stderr or b''
    output.with_suffix('.stdout').write_bytes(stdout)
    output.with_suffix('.stderr').write_bytes(stderr)
    return {'argv': argv, 'cwd': str(cwd), 'exit_code': code,
            'elapsed_seconds': round(time.monotonic() - started, 6),
            'stdout': str(output.with_suffix('.stdout')), 'stderr': str(output.with_suffix('.stderr'))}


def command(tool, binary, node, repo, facts, config, output, holt_normal):
    if tool == 'holt':
        return [node, binary, 'status', '--cwd', str(repo), '--base', facts['target_ref'],
                '--json', '--no-symbols', '--include-primary'] + ([] if holt_normal else ['--strict-read-only'])
    if tool == 'agentrinse':
        return [node, binary, 'audit', '--home', str(config.parent / 'home'), '--config', str(config),
                '--json', '--no-state']
    return [binary, 'review', '--repo', str(repo), '--target', facts['target_ref'],
            '--format', 'json', '--run-dir', str(output)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--holt', help='isolated installed bin/holt.mjs')
    parser.add_argument('--agentrinse', help='isolated compiled dist/cli.js')
    parser.add_argument('--sweep', help='root-provided release binary')
    parser.add_argument('--node', default=shutil.which('node'), help='Node executable (defaults to PATH)')
    parser.add_argument('--holt-normal', action='store_true', help='allow normal Holt object-writing audit mode')
    parser.add_argument('--scenario', choices=SCENARIOS, action='append')
    args = parser.parse_args()
    binaries = {tool: getattr(args, tool) for tool in ('holt', 'agentrinse', 'sweep') if getattr(args, tool)}
    if not binaries:
        parser.error('at least one explicit binary is required')
    if any(tool in binaries for tool in ('holt', 'agentrinse')) and not args.node:
        parser.error('Node is unavailable; pass --node explicitly')
    run = Path(tempfile.mkdtemp(prefix='wt-sweep-comparison-run-', dir='/private/tmp'))
    records = []
    for tool, binary in binaries.items():
        for scenario in args.scenario or SCENARIOS:
            item = run / tool / scenario
            env = isolated_env(item / 'home')
            repo, work, facts = fixture(item / 'fixture', scenario, env)
            config = item / 'config.json'
            config.write_text(json.dumps({'schemaVersion': 1, 'adapters': {
                **{name: {'enabled': False} for name in ('codex', 'claude', 'cursor', 'copilot', 'zed', 'opencode', 'grok', 'runtime', 'docker')},
                'git': {'enabled': True, 'root': str(repo)}}, 'worktrees': {'minAgeMinutes': 0}}))
            steps = ['initial', 'changed'] if scenario == 'changed_target' else ['initial']
            for step in steps:
                if step == 'changed':
                    git(repo, env, 'update-ref', facts['target_ref'], facts['base_oid'])
                out = item / step
                out.mkdir()
                before, home_before = snapshot(item / 'fixture'), snapshot(item / 'home')
                result = run_command(command(tool, binary, args.node, repo, facts, config, out, args.holt_normal),
                                     env, item, out / 'command')
                after, home_after = snapshot(item / 'fixture'), snapshot(item / 'home')
                result.update(tool=tool, scenario=scenario, step=step, facts=facts,
                              conservation=delta(before, after), home_changes=delta(home_before, home_after),
                              oracle_gaps=[k for k, v in before.items() if v['kind'] == 'error'])
                (out / 'before.json').write_text(json.dumps(before, indent=2, sort_keys=True))
                (out / 'after.json').write_text(json.dumps(after, indent=2, sort_keys=True))
                records.append(result)
                print(tool, scenario, step, result['exit_code'], result['elapsed_seconds'], result['conservation'], flush=True)
            if scenario == 'unreadable':
                (work / 'denied').chmod(0o700)
    (run / 'results.json').write_text(json.dumps({'records': records, 'scope': 'synthetic only'}, indent=2))
    print('RESULTS', run, flush=True)


if __name__ == '__main__':
    main()
