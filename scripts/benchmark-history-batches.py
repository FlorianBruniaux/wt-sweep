#!/usr/bin/env python3
"""Compare frozen CLI binaries on one owned Git DAG and private-history corpus.

Python 3.11+. No host discovery, network, global configuration or cache clearing.
Example: python3 -B scripts/benchmark-history-batches.py --baseline /tmp/before \
    --after /tmp/after --out /private/tmp/history-batches-result.json
Raw outputs and failed comparisons are retained beside the requested output.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import selectors
import signal
import stat
import statistics
import subprocess
import tempfile
import time

REPOSITORY = Path(__file__).resolve().parents[1]
MAX_OUTPUT = 32 * 1024 * 1024


def sha256(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def execute(argv, env, data=b'', timeout=180):
    """Bound output and stop only the owned process group, including on failure."""
    process = subprocess.Popen(argv, env=env, stdin=subprocess.PIPE if data else subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    selector = selectors.DefaultSelector()
    output = {'stdout': bytearray(), 'stderr': bytearray()}
    started = time.monotonic_ns()
    try:
        if data:
            # Fixture input is small; product measurements never receive stdin.
            process.stdin.write(data)
            process.stdin.close()
        selector.register(process.stdout, selectors.EVENT_READ, 'stdout')
        selector.register(process.stderr, selectors.EVENT_READ, 'stderr')
        while selector.get_map():
            if (time.monotonic_ns() - started) / 1e9 >= timeout:
                raise TimeoutError('owned command exceeded deadline')
            for key, _ in selector.select(0.1):
                chunk = os.read(key.fd, 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                output[key.data].extend(chunk)
                if sum(map(len, output.values())) > MAX_OUTPUT:
                    raise RuntimeError('owned command output cap reached')
        status = process.wait(timeout=5)
        return {**{key: bytes(value) for key, value in output.items()}, 'exit_status': status,
                'elapsed_ms': (time.monotonic_ns() - started) / 1e6}
    except (TimeoutError, RuntimeError) as failure:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        status = process.wait(timeout=5)
        return {**{key: bytes(value) for key, value in output.items()}, 'exit_status': status,
                'elapsed_ms': (time.monotonic_ns() - started) / 1e6, 'failure': str(failure)}
    finally:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=5)
        selector.close()
        process.stdout.close()
        process.stderr.close()
        if process.stdin is not None and not process.stdin.closed:
            process.stdin.close()


class Corpus:
    """Deterministic commits; private ORIG_HEAD and reflog roots have a named anchor."""
    def __init__(self, chain=128, side=16):
        if not 4 <= chain <= 256 or not 1 <= side <= 32:
            raise ValueError('fixture size exceeds finite benchmark caps')
        self.temporary = tempfile.TemporaryDirectory(prefix='wt-sweep-history-corpus-', dir='/private/tmp')
        self.root = Path(self.temporary.name)
        self.repo = self.root / 'principal'
        self.env = {'PATH': '/usr/bin:/bin', 'HOME': str(self.root),
                    'XDG_CONFIG_HOME': str(self.root / 'no-user-config'), 'LC_ALL': 'C',
                    'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_SYSTEM': os.devnull,
                    'GIT_CONFIG_GLOBAL': os.devnull, 'GIT_ATTR_NOSYSTEM': '1',
                    'GIT_TERMINAL_PROMPT': '0', 'GIT_OPTIONAL_LOCKS': '0',
                    'GIT_NO_LAZY_FETCH': '1', 'GIT_ALLOW_PROTOCOL': '',
                    'GIT_NO_REPLACE_OBJECTS': '1',
                    'GIT_AUTHOR_NAME': 'Benchmark', 'GIT_AUTHOR_EMAIL': 'fixture@example.invalid',
                    'GIT_COMMITTER_NAME': 'Benchmark', 'GIT_COMMITTER_EMAIL': 'fixture@example.invalid',
                    'GIT_AUTHOR_DATE': '1700000000 +0000', 'GIT_COMMITTER_DATE': '1700000000 +0000',
                    'NO_COLOR': '1', 'TERM': 'dumb'}
        try:
            self.repo.mkdir()
            self.git('init', '-q', '--template=', '-b', 'integration', str(self.repo))
            tree = self.git('mktree').decode().strip()
            self.chain_oids = []
            for number in range(chain):
                parents = self.chain_oids[-1:]
                self.chain_oids.append(self.commit(tree, parents, f'chain {number:03}'))
            tip = self.chain_oids[chain // 2]
            self.side_oids = []
            for number in range(side):
                tip = self.commit(tree, [tip], f'side {number:03}')
                self.side_oids.append(tip)
            self.target = self.commit(tree, [self.chain_oids[-1], tip], 'second parent merge')
            self.git('update-ref', 'refs/heads/integration', self.target)
            self.chain_worktree = self.root / 'chain-worktree'
            self.side_worktree = self.root / 'second-parent-worktree'
            self.heads = {self.repo: self.target, self.chain_worktree: self.chain_oids[0],
                          self.side_worktree: tip}
            for path in (self.chain_worktree, self.side_worktree):
                self.git('worktree', 'add', '-q', '--detach', str(path), self.heads[path])
            self.private_roots = {}
            zero = '0' * 40
            for path, head in self.heads.items():
                admin = Path(self.git('-C', str(path), 'rev-parse', '--absolute-git-dir').decode().strip())
                (admin / 'ORIG_HEAD').write_text(self.chain_oids[0] + '\n')
                (admin / 'logs').mkdir(exist_ok=True)
                roots = list(dict.fromkeys([self.chain_oids[0], self.chain_oids[chain // 2], head]))
                previous = zero
                records = []
                for oid in roots:
                    records.append(f'{previous} {oid} Benchmark <fixture@example.invalid> 1700000000 +0000\tbenchmark root\n')
                    previous = oid
                (admin / 'logs/HEAD').write_text(''.join(records))
                self.private_roots[path] = roots
        except BaseException:
            self.temporary.cleanup()
            raise

    def git(self, *args, data=b''):
        result = execute(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null',
                          '-c', 'core.fsmonitor=false', '-c', 'gc.auto=0',
                          '-c', 'maintenance.auto=false', '-C', str(self.repo), *args], self.env, data, 30)
        if result['exit_status']:
            raise RuntimeError((args, result['exit_status'], result['stderr'].decode(errors='replace')))
        return result['stdout']

    def commit(self, tree, parents, subject):
        args = ['commit-tree', tree]
        for parent in parents:
            args.extend(['-p', parent])
        return self.git(*args, data=(subject + '\n').encode()).decode().strip()

    def manifest(self):
        records = []
        for path in [self.root, *sorted(self.root.rglob('*'))]:
            info = path.lstat()
            payload = os.readlink(path).encode() if path.is_symlink() else path.read_bytes() if path.is_file() else b''
            records.append([os.fsencode(path.relative_to(self.root)).hex(), stat.S_IFMT(info.st_mode),
                            stat.S_IMODE(info.st_mode), info.st_size, info.st_mtime_ns, info.st_ctime_ns,
                            hashlib.sha256(payload).hexdigest()])
        return records

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.temporary.cleanup()


def normalized_evidence(report):
    """Only clocks, their container digests and diagnostic counters are excluded.

    Source byte hashes, identities, OIDs, root provenance, stops, gaps, completeness,
    verdicts, requested profile and actual caps remain comparable evidence.
    """
    def normalize(value):
        if isinstance(value, list):
            return [normalize(child) for child in value]
        if isinstance(value, dict):
            return {key: normalize(child) for key, child in value.items()
                    if key not in {'started_unix_ms', 'finished_unix_ms', 'digest_sha256',
                                   'history_metrics', 'discovery_metrics', 'phases'}}
        return value
    return normalize(report)


def sample_order(pairs, warmups):
    return [(stage, number, label) for stage, count in [('warmup', warmups), ('measurement', pairs)]
            for number in range(count) for label in (('baseline', 'after') if number % 2 == 0 else ('after', 'baseline'))]


def validate_output(path):
    path = Path(path)
    if not path.is_absolute() or path.resolve().is_relative_to(REPOSITORY):
        raise ValueError('output must be absolute and outside the repository')
    if path.exists():
        raise ValueError('output already exists; choose a new artifact path')


def validate_report(report, corpus):
    if report['schema'] != 'wt-sweep.review/2':
        raise AssertionError('unexpected review schema')
    items = {Path(os.fsdecode(bytes.fromhex(row['path']['hex']))): row for row in report['items']}
    if len(items) != len(report['items']) or set(items) != set(corpus.heads):
        raise AssertionError('missing, duplicated or invented fixture rows')
    for path, head in corpus.heads.items():
        item = items[path]
        expected = 'identical_commit' if path == corpus.repo else 'ancestor'
        if item['head'] != head or item['history_omitted_targets'] or len(item['history']) != 1:
            raise AssertionError('fixture HEAD or target count differs')
        relation = item['history'][0]
        if (relation['result'] != expected or relation['source_oid'] != head
                or relation['target_oid'] != corpus.target or relation['stop_reason'] is not None
                or bytes.fromhex(relation['target_ref']['hex']) != b'refs/heads/integration'):
            raise AssertionError('positive full-DAG history relation missing')
        history = item['private_history']
        if not history or not history['enumeration_complete'] or not history['anchors_complete'] or history['gaps']:
            raise AssertionError('private source/anchor coverage incomplete')
        roots = history['roots']
        if len(roots) != len(corpus.private_roots[path]) or {r['oid'] for r in roots} != set(corpus.private_roots[path]):
            raise AssertionError('private roots omitted, duplicated or invented')
        for root in roots:
            if (root['result'] != 'covered' or root['anchor'] is None or root['reason'] is not None
                    or root['anchor']['oid'] != corpus.target
                    or bytes.fromhex(root['anchor']['reference']['hex']) != b'refs/heads/integration'):
                raise AssertionError('private root lacks the retained named anchor')
        base = next(root for root in roots if root['oid'] == corpus.chain_oids[0])
        provenances = [(p['kind'], bytes.fromhex(p['source']['hex'])) for p in base['provenance']]
        if (not any(kind == 'root_ref' and source.endswith(b'/ORIG_HEAD') for kind, source in provenances)
                or not any(kind in {'reflog_old', 'reflog_new'} and source.endswith(b'/logs/HEAD')
                           for kind, source in provenances)):
            raise AssertionError('ORIG_HEAD and reflog provenance missing')
    phases = {phase['name']: phase for phase in report['collection_metrics']['phases']}
    for name in ('history', 'private_history'):
        if name not in phases or not {'object_reads', 'object_bytes', 'git_processes'} <= phases[name]['counters'].keys():
            raise AssertionError('actual CLI history/private phase counters missing')
        if phases[name]['counters']['object_reads'] <= 0 or phases[name]['counters']['object_bytes'] <= 0:
            raise AssertionError('positive object collection counters missing')
    scalar_roots = {oid for roots in corpus.private_roots.values() for oid in roots} | {corpus.target}
    if phases['private_history']['counters']['object_reads'] <= len(scalar_roots):
        raise AssertionError('private traversal never read beyond scalar roots and anchor')


def measure(binary, expected_hash, corpus, inventory, destination, budget):
    before = corpus.manifest()
    if sha256(binary) != expected_hash:
        raise AssertionError('frozen binary changed before sample')
    argv = [str(binary), 'plan', '--inventory', str(inventory), '--target', 'refs/heads/integration',
            '--no-context', '--format', 'json', '--color', 'never', '--budget', budget]
    capture = execute(argv, corpus.env)
    # Persist failed output before evaluating its claims.
    destination.write_bytes(capture['stdout'])
    destination.with_suffix('.stderr').write_bytes(capture['stderr'])
    sample = {'argv': argv, 'elapsed_ms': capture['elapsed_ms'], 'exit_status': capture['exit_status'],
              'review_path': str(destination), 'review_sha256': sha256(destination),
              'stderr_path': str(destination.with_suffix('.stderr')),
              'load_average_before_next_sample': list(os.getloadavg()), 'valid': False}
    try:
        if capture['exit_status'] or capture.get('failure'):
            raise AssertionError(capture.get('failure', 'CLI returned a failure exit status'))
        report = json.loads(capture['stdout'])
        sample['collection_metrics'] = report['collection_metrics']
        sample['history_metrics'] = report['history_metrics']
        sample['normalized_evidence_sha256'] = digest(normalized_evidence(report))
        validate_report(report, corpus)
        if report['collection_metrics']['budget_profile'] != budget:
            raise AssertionError('CLI profile differs from request')
        if corpus.manifest() != before:
            raise AssertionError('sample changed corpus bytes/metadata; atime excluded')
        if sha256(binary) != expected_hash:
            raise AssertionError('frozen binary changed during sample')
        sample.update(valid=True, source_manifest_sha256=digest(before), source_conserved=True)
    except (AssertionError, ValueError, KeyError, TypeError) as failure:
        sample['failure'] = str(failure)
    return sample


def summarize(result):
    measurements = [sample for sample in result['samples'] if sample['stage'] == 'measurement']
    complete_pairs = [pair for pair in result['comparisons'] if pair['stage'] == 'measurement']
    usable = (bool(complete_pairs) and len(measurements) == 2 * len(complete_pairs)
              and all(pair['comparable'] for pair in result['comparisons']))
    result['performance_comparable'] = usable
    if not usable:
        result.pop('summary', None)
        return
    totals = {}
    for label in ('baseline', 'after'):
        rows = [row for row in measurements if row['binary'] == label]
        values = {'wall_elapsed_ms': [row['elapsed_ms'] for row in rows]}
        for phase in ('history', 'private_history'):
            phases = [next(p for p in row['collection_metrics']['phases'] if p['name'] == phase) for row in rows]
            values[f'{phase}.elapsed_ms'] = [p['elapsed_ms'] for p in phases]
            for counter in ('git_processes', 'object_reads', 'object_bytes'):
                values[f'{phase}.{counter}'] = [p['counters'][counter] for p in phases]
        totals[label] = {key: {'samples': value, 'median': statistics.median(value),
                              'min': min(value), 'max': max(value)} for key, value in values.items()}
    result['summary'] = totals


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--after', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--chain', type=int, default=128, choices=range(4, 257), metavar='N')
    parser.add_argument('--side', type=int, default=16, choices=range(1, 33), metavar='N')
    parser.add_argument('--pairs', type=int, default=3, choices=range(1, 6))
    parser.add_argument('--warmups', type=int, default=1, choices=range(0, 3))
    parser.add_argument('--budget', choices=('standard', 'thorough'), default='standard')
    args = parser.parse_args()
    try:
        validate_output(args.out)
        binaries = {label: path.resolve(strict=True) for label, path in [('baseline', args.baseline), ('after', args.after)]}
        if any(not path.is_file() or not os.access(path, os.X_OK) for path in binaries.values()):
            raise ValueError('each binary must be an executable file')
        artifacts = args.out.with_suffix('.artifacts')
        artifacts.mkdir(parents=True, exist_ok=False)
    except (ValueError, OSError) as failure:
        parser.error(str(failure))
    result = {'schema': 'wt-sweep.history-batch-benchmark/1', 'environment': {
        'system': platform.platform(), 'machine': platform.machine(), 'python': platform.python_version(),
        'logical_cpus': os.cpu_count(), 'load_average': list(os.getloadavg()),
        'clock': 'time.monotonic_ns', 'os_cache': 'not cleared; same corpus, sequential alternating pairs'},
        'protocol': {'path': str(Path(__file__).resolve()), 'sha256': sha256(__file__),
                     'warmups': args.warmups, 'pairs': args.pairs, 'budget': args.budget,
                     'phase_process_scope': 'actual collection_metrics history/private counters; discovery and controls excluded',
                     'normalization': normalized_evidence.__doc__},
        'binaries': {label: {'path': str(path), 'sha256': sha256(path)} for label, path in binaries.items()},
        'samples': [], 'comparisons': []}
    with Corpus(args.chain, args.side) as corpus:
        result['environment']['git'] = corpus.git('--version').decode().strip()
        result['corpus'] = {'temporary_root': str(corpus.root), 'chain': args.chain, 'side': args.side,
                            'target': corpus.target, 'heads': {str(k): v for k, v in corpus.heads.items()},
                            'private_roots': {str(k): v for k, v in corpus.private_roots.items()},
                            'private_root_layout': 'sparse: chain root, chain midpoint, worktree HEAD; unenumerated commits require graph reads',
                            'manifest': corpus.manifest(), 'synthetic_only': True}
        args.out.write_text(json.dumps(result, indent=2) + '\n')
        inventory = artifacts / 'inventory.json'
        scan = execute([str(binaries['baseline']), 'scan', '--repo', str(corpus.repo), '--format', 'json'], corpus.env)
        inventory.write_bytes(scan['stdout'])
        inventory.with_suffix('.stderr').write_bytes(scan['stderr'])
        if scan['exit_status'] or corpus.manifest() != result['corpus']['manifest']:
            raise AssertionError('fixed inventory failed or changed corpus')
        json.loads(scan['stdout'])
        result['inventory_sha256'] = sha256(inventory)
        for stage, number, label in sample_order(args.pairs, args.warmups):
            print(f'collecting {stage} pair={number} binary={label}', flush=True)
            destination = artifacts / f'{stage}-{number}-{label}.json'
            measured = measure(binaries[label], result['binaries'][label]['sha256'], corpus, inventory, destination, args.budget)
            result['samples'].append({'stage': stage, 'pair': number, 'binary': label, **measured})
            rows = [row for row in result['samples'] if row['stage'] == stage and row['pair'] == number]
            if len(rows) == 2:
                comparable = all(row['valid'] for row in rows) and len({row.get('normalized_evidence_sha256') for row in rows}) == 1
                comparable = comparable and len({row.get('source_manifest_sha256') for row in rows}) == 1
                result['comparisons'].append({'stage': stage, 'pair': number, 'order': [row['binary'] for row in rows],
                    'comparable': comparable, 'same_normalized_evidence': len({row.get('normalized_evidence_sha256') for row in rows}) == 1,
                    'failure': None if comparable else 'invalid sample or changed meaningful evidence/caps/source manifest'})
            summarize(result)
            args.out.write_text(json.dumps(result, indent=2) + '\n')
        result['corpus']['conserved_after_campaign'] = corpus.manifest() == result['corpus']['manifest']
        if not result['corpus']['conserved_after_campaign']:
            result['performance_comparable'] = False
            result.pop('summary', None)
        args.out.write_text(json.dumps(result, indent=2) + '\n')
    print(args.out)
    return 0 if result['performance_comparable'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
