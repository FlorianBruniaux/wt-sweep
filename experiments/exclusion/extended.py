"""Real counterexamples, not an exclusion authority or backup implementation.

Only owned temporary fixtures and children. Messages order operations; timeouts
bound waits. SIGTERM is sent solely to the guard child created by this script.
"""
import fcntl
import json
import mmap
import multiprocessing
import os
from pathlib import Path
import platform
import tempfile

from experiment import receive


def _writer(channel, directory, action):
    root = Path(directory)
    os.chdir(root / 'outside')
    try:
        if action == 'mapping':
            with (root / 'target' / 'data').open('r+b') as handle:
                with mmap.mmap(handle.fileno(), 0, access=mmap.ACCESS_WRITE) as mapping:
                    channel.send(('READY', str(Path.cwd())))
                    assert receive(channel) == 'GO'
                    mapping[:] = b'after!'
                    mapping.flush()
        else:
            channel.send(('READY', str(Path.cwd())))
            assert receive(channel) == 'GO'
            paths = {'new_open': root / 'renamed' / 'data',
                     'hard_alias': root / 'outside' / 'hard',
                     'symlink_alias': root / 'outside' / 'symbolic'}
            with paths[action].open('r+b') as handle:
                handle.write(b'after!')
                handle.flush()
                os.fsync(handle.fileno())
        channel.send(('DONE', None))
    finally:
        channel.close()


def _guard(channel, directory):
    root = Path(directory)
    with (root / 'guard').open('r+b') as handle:
        fcntl.flock(handle, fcntl.LOCK_EX)
        # Labels identify interruption points only. The 'capture' is diagnostic,
        # and the receipt below is not a durable backup publication.
        channel.send(('READY', None))
        for phase in ('acquired', 'captured', 'published', 'before_retirement'):
            assert receive(channel) == phase
            if phase == 'captured':
                (root / 'diagnostic-copy').write_bytes((root / 'source').read_bytes())
            if phase == 'published':
                (root / 'diagnostic-receipt').write_text('not a backup\n')
            channel.send(('CHECKPOINT', phase))
        receive(channel)  # Always interrupted before any retirement implementation.


def _finish(process, channel, interrupt=False):
    if interrupt and process.is_alive():
        process.terminate()
    process.join(5)
    if process.is_alive():
        process.kill()
        process.join(5)
    channel.close()
    assert not process.is_alive()
    process.close()


def run():
    if not __debug__:
        raise RuntimeError('optimized Python is unsupported: witness assertions are required')
    result = {'platform': platform.platform(), 'writers': {}, 'guard_loss': [],
              'independent_authority': 'NOT_ESTABLISHED', 'apply': 'BLOCKED'}
    context = multiprocessing.get_context('spawn')
    with tempfile.TemporaryDirectory(prefix='wt-sweep-e1-extended-', dir='/private/tmp') as directory:
        base = Path(directory)
        for action in ('mapping', 'new_open', 'hard_alias', 'symlink_alias'):
            root = base / action
            (root / 'target').mkdir(parents=True)
            (root / 'outside').mkdir()
            data = root / 'target' / 'data'
            data.write_bytes(b'before')
            os.link(data, root / 'outside' / 'hard')
            (root / 'outside' / 'symbolic').symlink_to('../renamed/data')
            identity = (data.stat().st_dev, data.stat().st_ino)
            parent, remote = context.Pipe()
            process = context.Process(target=_writer, args=(remote, str(root), action))
            with data.open('r+b') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX)
                process.start()
                remote.close()
                try:
                    ready, cwd = receive(parent)
                    assert ready == 'READY'
                    (root / 'target').rename(root / 'renamed')
                    parent.send('GO')
                    assert receive(parent)[0] == 'DONE'
                    process.join(5)
                    assert process.exitcode == 0
                    observed = root / 'renamed' / 'data'
                    result['writers'][action] = {
                        'write_observed': observed.read_bytes() == b'after!',
                        'same_inode': identity == (observed.stat().st_dev, observed.stat().st_ino),
                        'outside_cwd': Path(cwd) == root / 'outside'}
                finally:
                    _finish(process, parent, interrupt=True)
        phases = ('acquired', 'captured', 'published', 'before_retirement')
        for stop in phases:
            root = base / stop
            root.mkdir()
            (root / 'source').write_bytes(b'before')
            (root / 'guard').write_bytes(b'guard')
            parent, remote = context.Pipe()
            process = context.Process(target=_guard, args=(remote, str(root)))
            process.start()
            remote.close()
            try:
                assert receive(parent)[0] == 'READY'
                for phase in phases:
                    parent.send(phase)
                    assert receive(parent) == ('CHECKPOINT', phase)
                    if phase == stop:
                        break
                with (root / 'guard').open('r+b') as contender:
                    refused = False
                    try:
                        fcntl.flock(contender, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    except BlockingIOError:
                        refused = True
                    process.terminate()
                    process.join(5)
                    assert process.exitcode is not None and process.exitcode != 0
                    fcntl.flock(contender, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    (root / 'source').write_bytes(b'after!')
                    result['guard_loss'].append({
                        'checkpoint': stop, 'locked_before_interrupt': refused,
                        'acquired_after_interrupt': True,
                        'write_observed': (root / 'source').read_bytes() == b'after!',
                        'source_retained': (root / 'source').exists()})
            finally:
                _finish(process, parent, interrupt=True)
    return result


if __name__ == '__main__':
    print(json.dumps(run(), indent=2, sort_keys=True))
