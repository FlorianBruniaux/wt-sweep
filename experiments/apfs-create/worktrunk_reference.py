"""Replay Worktrunk copy-ignored with explicit owned roots and inert config."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

from creation import environment, git


def run(binary):
    binary = binary.resolve(strict=True)
    result = {'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest()}
    with tempfile.TemporaryDirectory(prefix='wt-sweep-apfs-worktrunk-', dir='/private/tmp') as directory:
        root = Path(directory)
        for name in ('repo', 'home', 'empty', 'staging'):
            (root / name).mkdir()
        source = root / 'repo'
        destination = root / 'destination'
        config = root / 'config.toml'
        config.write_text('')
        git(root, source, 'init', '--template=' + str(root / 'empty'), '-b', 'main')
        (source / '.gitignore').write_text('node_modules/\n.env\n')
        (source / '.worktreeinclude').write_text('node_modules/\n')
        (source / 'tracked').write_bytes(b'committed\n')
        git(root, source, 'add', '--', '.gitignore', '.worktreeinclude', 'tracked')
        git(root, source, 'commit', '-m', 'synthetic reference')
        (source / 'node_modules').mkdir()
        (source / 'node_modules' / 'dependency').write_bytes(b'dependency-source\n')
        (source / '.env').write_bytes(b'SYNTHETIC=exclude\n')
        (source / 'tracked').write_bytes(b'staged-source\n')
        git(root, source, 'add', '--', 'tracked')
        index = git(root, source, 'ls-files', '--stage', '-z')
        git(root, source, 'worktree', 'add', '-b', 'destination', str(destination), 'HEAD')
        command = [str(binary), '--config', str(config), '-C', str(source),
                   'step', 'copy-ignored', '--from', 'main', '--to', 'destination',
                   '--require-include', '--format', 'json']
        env = environment(root)
        env['NO_COLOR'] = '1'
        result['version'] = subprocess.run([str(binary), '--version'], env=env,
                                          capture_output=True, check=True, timeout=10).stdout.decode().strip()
        execution = subprocess.run(command, env=env, capture_output=True,
                                   stdin=subprocess.DEVNULL, check=True, timeout=30)
        # Preserve tool output as evidence, but not its physical-space claims.
        result['output'] = execution.stdout.decode().replace(str(root), '<fixture>')
        result['stderr'] = execution.stderr.decode().replace(str(root), '<fixture>')
        copied = destination / 'node_modules' / 'dependency'
        result['copied_dependency'] = copied.read_bytes() == b'dependency-source\n'
        result['excluded_env'] = not (destination / '.env').exists()
        result['target_index_clean'] = not git(root, destination, 'status', '--porcelain=v1')
        copied.write_bytes(b'destination-only\n')
        result['isolated_write'] = (source / 'node_modules' / 'dependency').read_bytes() == b'dependency-source\n'
        subprocess.run(command, env=env, capture_output=True,
                       stdin=subprocess.DEVNULL, check=True, timeout=30)
        result['repeat_preserves_destination'] = copied.read_bytes() == b'destination-only\n'
        result['source_unchanged'] = (index == git(root, source, 'ls-files', '--stage', '-z')
                                      and (source / 'tracked').read_bytes() == b'staged-source\n')
    return result


if __name__ == '__main__':
    if len(sys.argv) != 2:
        raise SystemExit('usage: worktrunk_reference.py /absolute/path/to/pinned/wt')
    print(json.dumps(run(Path(sys.argv[1])), indent=2, sort_keys=True))
