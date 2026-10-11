"""Bounded synthetic creation comparison, never a product command.

Use stock Git for tracked state, then copy or clone only fixture node_modules.
No inferred physical-space savings, whole-source clone, automatic fallback,
permission change, hook install, network or user repository input.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import sys
import tempfile
import time


def clone(source, destination):
    libc = ctypes.CDLL('/usr/lib/libSystem.B.dylib', use_errno=True)
    libc.clonefile.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint32]
    libc.clonefile.restype = ctypes.c_int
    if libc.clonefile(os.fsencode(source), os.fsencode(destination), 0):
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code), os.fspath(destination))


def copy_bytes(source, destination):
    # Deliberate userspace writes, so the baseline cannot silently reflink.
    with Path(source).open('rb') as reader, Path(destination).open('xb') as writer:
        shutil.copyfileobj(reader, writer, length=1024 * 1024)
    return destination


def environment(root):
    return {'PATH': '/usr/bin:/bin', 'HOME': str(root / 'home'),
            'XDG_CONFIG_HOME': str(root / 'home'), 'TMPDIR': str(root / 'staging'),
            'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': os.devnull,
            'GIT_TERMINAL_PROMPT': '0', 'GIT_NO_REPLACE_OBJECTS': '1',
            'GIT_NO_LAZY_FETCH': '1', 'GIT_AUTHOR_NAME': 'APFS fixture',
            'GIT_AUTHOR_EMAIL': 'fixture@example.invalid',
            'GIT_COMMITTER_NAME': 'APFS fixture',
            'GIT_COMMITTER_EMAIL': 'fixture@example.invalid', 'LC_ALL': 'C',
            'GIT_CONFIG_COUNT': '4', 'GIT_CONFIG_KEY_0': 'core.hooksPath',
            'GIT_CONFIG_VALUE_0': str(root / 'empty'),
            'GIT_CONFIG_KEY_1': 'core.fsmonitor', 'GIT_CONFIG_VALUE_1': 'false',
            'GIT_CONFIG_KEY_2': 'commit.gpgSign', 'GIT_CONFIG_VALUE_2': 'false',
            'GIT_CONFIG_KEY_3': 'protocol.allow', 'GIT_CONFIG_VALUE_3': 'never'}


def git(root, cwd, *args):
    result = subprocess.run(['/usr/bin/git', '-C', str(cwd), *args],
                            env=environment(root), stdin=subprocess.DEVNULL,
                            capture_output=True, check=True, timeout=30)
    if len(result.stdout) + len(result.stderr) > 1024 * 1024:
        raise RuntimeError('fixture Git output limit')
    return result.stdout


def digest_tree(path):
    return {str(item.relative_to(path)): hashlib.sha256(item.read_bytes()).hexdigest()
            for item in sorted(path.rglob('*')) if item.is_file()}


class StatFS(ctypes.Structure):
    # Darwin SDK sys/mount.h __DARWIN_STRUCT_STATFS64 (arm64 and x86_64).
    _fields_ = [('bsize', ctypes.c_uint32), ('iosize', ctypes.c_int32),
                ('counts', ctypes.c_uint64 * 5), ('fsid', ctypes.c_int32 * 2),
                ('owner', ctypes.c_uint32), ('type', ctypes.c_uint32),
                ('flags', ctypes.c_uint32), ('subtype', ctypes.c_uint32),
                ('fstypename', ctypes.c_char * 16),
                ('mountpoint', ctypes.c_char * 1024),
                ('source', ctypes.c_char * 1024), ('flags_ext', ctypes.c_uint32),
                ('reserved', ctypes.c_uint32 * 7)]


def filesystem(path):
    libc = ctypes.CDLL('/usr/lib/libSystem.B.dylib', use_errno=True)
    libc.statfs.argtypes = [ctypes.c_char_p, ctypes.POINTER(StatFS)]
    libc.statfs.restype = ctypes.c_int
    state = StatFS()
    if libc.statfs(os.fsencode(path), ctypes.byref(state)):
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code))
    return {'type': bytes(state.fstypename).decode('ascii'),
            'device': path.stat().st_dev, 'fsid': list(state.fsid)}


def run(samples=3, files=32, bytes_per_file=262144, upstream=None):
    if sys.platform != 'darwin':
        raise RuntimeError('actual Darwin clonefile required')
    if not (1 <= samples <= 5 and 1 <= files <= 256 and 1 <= bytes_per_file <= 1048576):
        raise ValueError('bounded fixture parameters exceeded')
    result = {'platform': platform.platform(), 'samples': [],
              'files': files, 'bytes_per_file': bytes_per_file,
              'unique_physical_bytes': 'UNKNOWN', 'reclaimable_bytes': 'UNKNOWN',
              'cache_protocol': 'warm source; no cache purge; alternating copy/clone order; no discarded warmup',
              'timing_scope': 'Git add + dependency stage + publish; validation and fixture setup excluded'}
    with tempfile.TemporaryDirectory(prefix='wt-sweep-apfs-create-', dir='/private/tmp') as directory:
        root = Path(directory)
        for name in ('home', 'empty', 'staging', 'repo', 'destinations'):
            (root / name).mkdir()
        source = root / 'repo'
        git(root, source, 'init', '--template=' + str(root / 'empty'), '-b', 'main')
        filter_script = root / 'filter.py'
        filter_script.write_text("import sys\nb=sys.stdin.buffer.read()\n"
                                 "sys.stdout.buffer.write(b.replace(b'SMUDGED', b'CANONICAL') if sys.argv[1]=='clean' else b.replace(b'CANONICAL', b'SMUDGED'))\n")
        filter_command = shlex.quote(sys.executable) + ' -I -S ' + shlex.quote(str(filter_script))
        git(root, source, 'config', 'filter.fixture.clean', filter_command + ' clean')
        git(root, source, 'config', 'filter.fixture.smudge', filter_command + ' smudge')
        git(root, source, 'config', 'filter.fixture.required', 'true')
        (source / '.gitattributes').write_text('filtered.txt filter=fixture\n')
        (source / '.gitignore').write_text('node_modules/\n.env\n')
        (source / 'filtered.txt').write_bytes(b'SMUDGED\n')
        (source / 'tracked').write_bytes(b'committed\n')
        git(root, source, 'add', '--', '.gitattributes', '.gitignore', 'filtered.txt', 'tracked')
        git(root, source, 'commit', '-m', 'synthetic source')
        (source / 'tracked').write_bytes(b'staged source only\n')
        git(root, source, 'add', '--', 'tracked')
        dependencies = source / 'node_modules'
        dependencies.mkdir()
        # Non-sparse, deterministic nonuniform bytes. Setup outside measurement.
        for index in range(files):
            seed = hashlib.sha256(str(index).encode()).digest()
            (dependencies / f'package-{index:04d}.bin').write_bytes((seed * ((bytes_per_file + 31) // 32))[:bytes_per_file])
        (source / '.env').write_bytes(b'SYNTHETIC_SECRET=never-copy\n')
        expected = digest_tree(dependencies)
        source_index = git(root, source, 'ls-files', '--stage', '-z')
        source_status = git(root, source, 'status', '--porcelain=v1', '--untracked-files=all', '--ignored')
        probe = root / 'destinations' / 'stock'
        git(root, source, 'worktree', 'add', '--detach', str(probe), 'HEAD')
        result['need'] = {'stock_worktree_missing_dependencies': not (probe / 'node_modules').exists(),
                          'stock_git_filter_correct': (probe / 'filtered.txt').read_bytes() == b'SMUDGED\n',
                          'desired_content': 'selected ignored node_modules, with clean target Git state'}
        result['filesystem'] = filesystem(root)
        if result['filesystem']['type'] != 'apfs':
            raise RuntimeError('experiment requires confirmed APFS volume')
        for iteration in range(samples):
            methods = ('copy', 'clone') if iteration % 2 == 0 else ('clone', 'copy')
            for method in methods:
                destination = root / 'destinations' / f'{method}-{iteration}'
                stage = root / 'staging' / f'{method}-{iteration}'
                started = time.perf_counter_ns()
                git(root, source, 'worktree', 'add', '--detach', str(destination), 'HEAD')
                after_git = time.perf_counter_ns()
                if method == 'copy':
                    shutil.copytree(dependencies, stage, copy_function=copy_bytes)
                else:
                    clone(dependencies, stage)
                after_stage = time.perf_counter_ns()
                # Same-device publication. No shutil.move cross-device fallback.
                stage.rename(destination / 'node_modules')
                finished = time.perf_counter_ns()
                copied = destination / 'node_modules'
                equal = digest_tree(copied) == expected
                index_clean = not git(root, destination, 'status', '--porcelain=v1', '--untracked-files=all')
                clean = git(root, destination, 'diff', '--cached', '--exit-code') == b''
                first = copied / 'package-0000.bin'
                inode_distinct = first.stat().st_ino != (dependencies / first.name).stat().st_ino
                first.write_bytes(b'child-only')
                isolated = digest_tree(dependencies) == expected
                result['samples'].append({
                    'iteration': iteration, 'method': method,
                    'git_ms': (after_git-started)/1e6, 'stage_ms': (after_stage-after_git)/1e6,
                    'publish_ms': (finished-after_stage)/1e6, 'total_ms': (finished-started)/1e6,
                    'source_device': dependencies.stat().st_dev,
                    'staging_device': (root / 'staging').stat().st_dev,
                    'destination_device': destination.stat().st_dev,
                    'dependency_bytes_equal': equal, 'index_clean': index_clean and clean,
                    'source_staging_not_inherited': (destination / 'tracked').read_bytes() == b'committed\n',
                    'filter_correct': (destination / 'filtered.txt').read_bytes() == b'SMUDGED\n',
                    'unselected_ignored_absent': not (destination / '.env').exists(),
                    'isolated_write': isolated and inode_distinct})
        if upstream:
            destination = root / 'destinations' / 'upstream'
            result['upstream_sha256'] = hashlib.sha256(upstream.read_bytes()).hexdigest()
            started = time.perf_counter_ns()
            execution = subprocess.run([sys.executable, '-I', '-S', str(upstream), '--detach', str(destination), 'HEAD'],
                                       cwd=source, env=environment(root), stdin=subprocess.DEVNULL,
                                       capture_output=True, timeout=30)
            result['upstream'] = {'exit_code': execution.returncode,
                                  'total_ms': (time.perf_counter_ns()-started)/1e6,
                                  'dependencies_retained': (destination / 'node_modules').exists(),
                                  'stderr': execution.stderr.decode('utf-8', 'replace').replace(str(root), '<fixture>')}
            if execution.returncode:
                raise RuntimeError(result['upstream'])
        result['source_unchanged'] = (source_index == git(root, source, 'ls-files', '--stage', '-z')
                                      and source_status == git(root, source, 'status', '--porcelain=v1', '--untracked-files=all', '--ignored')
                                      and digest_tree(dependencies) == expected
                                      and (source / 'tracked').read_bytes() == b'staged source only\n')
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--files', type=int, default=32)
    parser.add_argument('--bytes-per-file', type=int, default=262144)
    parser.add_argument('--upstream', type=Path, help='explicit pinned apfs-worktree-add.py; executes only on owned fixture')
    args = parser.parse_args()
    print(json.dumps(run(args.samples, args.files, args.bytes_per_file, args.upstream), indent=2, sort_keys=True))
