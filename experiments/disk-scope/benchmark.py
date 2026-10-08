"""Compare two disk collectors on owned, fixed Git fixtures with an independent stat oracle."""
import argparse, hashlib, json, os, pathlib, statistics, subprocess, tempfile, time
from stat import S_ISDIR, S_ISREG, S_ISLNK
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--baseline', type=pathlib.Path, required=True)
parser.add_argument('--candidate', type=pathlib.Path, required=True)
parser.add_argument('--out', type=pathlib.Path, required=True)
args = parser.parse_args()
if not __debug__:
    parser.error('run without Python optimization so oracle assertions stay enabled')
before = args.baseline.resolve()
after = args.candidate.resolve()
if args.out.exists():
    parser.error('--out must be a new file')
results = {'before_sha256': hashlib.sha256(before.read_bytes()).hexdigest(), 'after_sha256': hashlib.sha256(after.read_bytes()).hexdigest(), 'samples': [], 'protocol': 'macOS; release; 4 linked worktrees plus main/shared Git; one warm-up per binary, 3 alternating measured samples; caches not cleared; fixture creation excluded'}
for count in [20, 200, 2000]:
    with tempfile.TemporaryDirectory(prefix='wt-sweep-disk-corpus-', dir='/private/tmp') as name:
        root = pathlib.Path(name)
        repo = root / 'repo'
        env = {'PATH': '/usr/bin:/bin', 'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null', 'GIT_AUTHOR_NAME': 'Fixture', 'GIT_AUTHOR_EMAIL': 'fixture@example.invalid', 'GIT_COMMITTER_NAME': 'Fixture', 'GIT_COMMITTER_EMAIL': 'fixture@example.invalid', 'LC_ALL': 'C', 'HOME': name}

        def git(*args):
            subprocess.run(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null', *map(str, args)], env=env, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=15)
        git('init', '-b', 'main', repo)
        git('-C', repo, 'commit', '--allow-empty', '-m', 'fixture')
        roots = [repo, repo / '.git']
        for n in range(4):
            path = root / f'wt-{n}'
            git('-C', repo, 'worktree', 'add', '-b', f'b{n}', path)
            roots.append(path)
            for i in range(count):
                (path / f'file-{i}').write_bytes(b'synthetic payload\n')
        roots_set = set(roots)
        global_seen = set()
        unique = [0, 0]
        expected = {}
        for path in roots:
            seen = set()
            total = [0, 0]
            pending = [path]
            while pending:
                p = pending.pop()
                st = p.lstat()
                key = (st.st_dev, st.st_ino)
                if key in seen:
                    continue
                seen.add(key)
                logical = st.st_size if S_ISREG(st.st_mode) or S_ISLNK(st.st_mode) else 0
                values = [logical, st.st_blocks * 512]
                for (i, value) in enumerate(values):
                    total[i] += value
                if key not in global_seen:
                    global_seen.add(key)
                    for (i, value) in enumerate(values):
                        unique[i] += value
                if S_ISDIR(st.st_mode):
                    pending.extend((c for c in p.iterdir() if c not in roots_set))
            expected[os.fsencode(path).hex()] = total

        def signature():
            result = {}
            for (parent, dirs, files) in os.walk(root, followlinks=False):
                for name in ['.'] + dirs + files:
                    p = pathlib.Path(parent) / name
                    st = p.lstat()
                    result[os.fsencode(p).hex()] = [st.st_mode, st.st_size, st.st_mtime_ns, st.st_ctime_ns, hashlib.sha256(p.read_bytes()).hexdigest() if S_ISREG(st.st_mode) else None]
            return result
        manifest = signature()

        def collect(binary):
            started = time.monotonic()
            p = subprocess.run([str(binary), 'scan', '--repo', str(repo), '--disk-usage', '--format', 'json'], env=env, check=True, capture_output=True, timeout=60)
            wall = time.monotonic() - started
            usage = json.loads(p.stdout)['disk_usage']
            assert usage['complete'] is True, usage
            assert len(usage['measurements']) == len(expected)
            assert {r['path']['hex']: [r['bytes']['logical'], r['bytes']['allocated']] for r in usage['measurements']} == expected
            assert [usage['unique_bytes']['logical'], usage['unique_bytes']['allocated']] == unique
            return {'wall_seconds': wall, 'disk_ms': usage['elapsed_ms'], 'entries': sum((r['entries_checked'] for r in usage['measurements']))}
        collect(before)
        collect(after)
        samples = {'files_per_linked_worktree': count, 'measured_roots': len(roots), 'before': [], 'after': [], 'oracle_sizes_match': True}
        for trial in range(3):
            for (label, binary) in [('before', before), ('after', after)] if trial % 2 == 0 else [('after', after), ('before', before)]:
                samples[label].append(collect(binary))
        for label in ['before', 'after']:
            samples[label + '_median_disk_ms'] = statistics.median((x['disk_ms'] for x in samples[label]))
        samples['median_disk_ratio'] = samples['after_median_disk_ms'] / max(1, samples['before_median_disk_ms'])
        assert signature() == manifest, 'inspected fixture changed'
        samples['fixture_unchanged'] = True
        results['samples'].append(samples)
        print(json.dumps(samples), flush=True)
results['script_sha256'] = hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest()
with args.out.open('x') as output:
    output.write(json.dumps(results, indent=2) + '\n')
print(args.out)
