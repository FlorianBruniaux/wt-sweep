"""Protocol regressions; real Git for successful samples, JSON mutations for refusal."""
import copy
import importlib.util
import json
import os
import signal
import sys
import tempfile
from types import SimpleNamespace
from unittest.mock import patch
from pathlib import Path
import unittest

WORKTREE = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('benchmark_review', WORKTREE / 'scripts/benchmark-review.py')
benchmark = importlib.util.module_from_spec(spec)
spec.loader.exec_module(benchmark)
from test_scale import ScaleFixture

BINARY = Path(os.environ.get('WT_SWEEP_BENCHMARK_BINARY', str(WORKTREE / 'target/debug/wt-sweep')))


class BenchmarkProtocolTests(unittest.TestCase):
    def test_partial_fact_prefix_does_not_count_and_complete_fact_does(self):
        head = b'a' * 40
        heads = [head]
        for text in (b'Comparing commit history...', b'Observed history:',
                     b'Observed history: ancestor (provisional, com',
                     b'Observed local: staged=', b'Observed local: staged=0 unstaged=0'):
            with self.subTest(text=text):
                self.assertFalse(benchmark.has_useful_fact(text, heads))
        for text in (b'Observed history: ancestor (provisional, complete); ',
                     b'Observed history: unknown (provisional, partial); ',
                     b'Observed local: staged=0 unstaged=0 conflicts=0 untracked=0 ignored=0 ',
                     b'Observed local: staged=>=1 unstaged=UNKNOWN conflicts=0 untracked=0 ignored=0 ',
                     b'"head": "' + head + b'"'):
            with self.subTest(text=text):
                self.assertTrue(benchmark.has_useful_fact(text, heads))
        self.assertFalse(benchmark.has_useful_fact(b'Observed local: staged=0 unstaged=0 conflicts=0 untracked=0 ignored=', heads))

    def test_row_oracle_refuses_empty_mismatched_and_contradictory_outputs(self):
        first, second = Path('/synthetic/main'), Path('/synthetic/linked')
        source, target = 'a' * 40, 'b' * 40
        fixture = SimpleNamespace(paths=[first, second], heads={first: source, second: target},
            target=target, target_chain=[target], ancestor=lambda head, oid: head == oid)
        items = []
        for path, head, result in ((first, source, 'not_ancestor'), (second, target, 'identical_commit')):
            items.append({'path': {'hex': os.fsencode(path).hex()}, 'head': head,
                'classification': 'keep' if path == first else 'cleanup_candidate',
                'decision': {'missing_conditions': []}, 'history_omitted_targets': 0,
                'history': [{'source_oid': head, 'target_oid': target,
                    'target_ref': {'hex': b'refs/heads/integration'.hex()}, 'result': result,
                    'observed_result': None, 'stop_reason': None,
                    'started_unix_ms': 1, 'finished_unix_ms': 2}]})
        report = {'schema': 'wt-sweep.review/2', 'items': items, 'shared_stashes': [],
            'coverage': {'traversal_complete': True, 'gaps': [], 'discovery_metrics': {'elapsed_ms': 1}},
            'collection_metrics': {'budget_profile': 'standard', 'applied_limits': {'private_timeout_ms': 60000, 'history_per_target_steps': 512}}}
        benchmark.validate_fixture_rows(report, fixture)
        for mutate in ('empty', 'duplicate', 'head', 'source', 'target', 'target_ref', 'result'):
            bad = copy.deepcopy(report)
            if mutate == 'empty':
                bad['items'] = []
            elif mutate == 'duplicate':
                bad['items'][1] = copy.deepcopy(bad['items'][0])
            elif mutate == 'head':
                bad['items'][0]['head'] = 'c' * 40
            elif mutate in ('source', 'target'):
                bad['items'][0]['history'][0][mutate + '_oid'] = 'c' * 40
            elif mutate == 'target_ref':
                bad['items'][0]['history'][0]['target_ref'] = {'hex': b'refs/heads/other'.hex()}
            else:
                bad['items'][1]['history'][0]['result'] = 'unknown'
            with self.subTest(refusal=mutate), self.assertRaises(AssertionError):
                benchmark.validate_fixture_rows(bad, fixture)
        with tempfile.TemporaryDirectory(prefix='wt-sweep-benchmark-proof-', dir='/private/tmp') as name:
            a, b = Path(name) / 'a.json', Path(name) / 'b.json'
            a.write_text(json.dumps(report))
            b.write_text(json.dumps(report))
            sample = {'worktrees':2, 'mode':'pipe', 'order':'forward', 'sample':0,
                'binary':'first', 'manifest_digest':'same', 'review_path':str(a)}
            other = {**sample, 'binary':'second', 'review_path':str(b)}
            comparison = benchmark.compare_samples(sample, other)
            self.assertEqual(comparison['changed_normalized_item_proofs'], 0)
            self.assertEqual(comparison['changed_classifications'], 0)
            self.assertTrue(comparison.get('comparable', True))
            different_proof = copy.deepcopy(report)
            different_proof['items'][1]['unknowns'] = ['private final allowance incomplete']
            b.write_text(json.dumps(different_proof))
            self.assertFalse(benchmark.compare_samples(sample, other).get('comparable', True),
                'equal verdicts cannot certify performance when collected facts differ')
            b.write_text(json.dumps(report))
            different_coverage = copy.deepcopy(report)
            different_coverage['coverage']['traversal_complete'] = False
            b.write_text(json.dumps(different_coverage))
            self.assertFalse(benchmark.compare_samples(sample, other).get('comparable', True))
            different_stashes = copy.deepcopy(report)
            different_stashes['shared_stashes'] = [{'enumeration_complete': False, 'gaps': ['finite proof gap']}]
            b.write_text(json.dumps(different_stashes))
            self.assertFalse(benchmark.compare_samples(sample, other).get('comparable', True))
            bad = copy.deepcopy(report)
            bad['items'][1]['classification'] = 'verify'
            b.write_text(json.dumps(bad))
            with self.assertRaises(AssertionError):
                benchmark.compare_samples(sample, other)

    def test_graph_oracle_uses_recorded_profile_cap(self):
        path = Path('/synthetic/main')
        source, target = 'a' * 40, 'b' * 40
        fixture = SimpleNamespace(paths=[path], heads={path: source}, target=target,
            target_chain=[target] * 600, ancestor=lambda head, oid: False)
        history = {'source_oid':source, 'target_oid':target,
            'target_ref':{'hex':b'refs/heads/integration'.hex()}, 'result':'not_ancestor', 'stop_reason':None}
        report = {'schema':'wt-sweep.review/2', 'items':[{'path':{'hex':os.fsencode(path).hex()},
            'head':source, 'history':[history], 'history_omitted_targets':0}],
            'collection_metrics':{'applied_limits':{'history_per_target_steps':1024}}}
        benchmark.validate_fixture_rows(report, fixture)
        report['collection_metrics']['applied_limits']['history_per_target_steps'] = 512
        with self.assertRaises(AssertionError):
            benchmark.validate_fixture_rows(report, fixture)
        history['result'] = 'unknown'
        history['stop_reason'] = 'per-target history traversal step budget reached'
        benchmark.validate_fixture_rows(report, fixture)

    def test_noncomparable_pair_keeps_both_valid_sample_timings_and_failure(self):
        with tempfile.TemporaryDirectory(prefix='wt-sweep-benchmark-persist-', dir='/private/tmp') as name:
            root = Path(name)
            binary = root / 'owned-inert-executable'
            binary.write_bytes(b'protocol-only inert fixture')
            binary.chmod(0o700)
            fixture = SimpleNamespace(root=root, manifest=lambda: {}, close=lambda: None)
            fixture.many_worktrees = lambda count: fixture
            out = root / 'samples.json'
            digest = benchmark.digest_file(binary)
            timings = [{'elapsed_ms': value, 'first_useful_ms': {'stdout': value, 'stderr': 2},
                'binary_sha256_before': digest, 'manifest_digest': 'same', 'review_path': str(root / f'{value}.json')}
                for value in (10, 12)]
            argv = ['benchmark-review.py', '--binary', f'baseline={binary}', '--binary', f'after={binary}',
                '--counts', '2', '--modes', 'pipe', '--budget', 'thorough', '--out', str(out)]
            with patch.object(sys, 'argv', argv), patch.object(benchmark, 'ScaleFixture', return_value=fixture), \
                 patch.object(benchmark.subprocess, 'run', return_value=SimpleNamespace(stdout=b'protocol fixture\n')), \
                 patch.object(benchmark.platform, 'platform', return_value='synthetic protocol test'), \
                 patch.object(benchmark, 'measure', side_effect=timings) as measurements, \
                 patch.object(benchmark, 'compare_samples', side_effect=AssertionError('explicit local deadline; C2/C3 gap')):
                benchmark.main()
            self.assertEqual([call.kwargs['budget'] for call in measurements.call_args_list], ['thorough', 'thorough'])
            saved = json.loads(out.read_text())
            self.assertEqual([sample['elapsed_ms'] for sample in saved['samples']], [10, 12])
            comparison = saved['comparisons'][0]
            self.assertFalse(comparison.get('comparable', True))
            self.assertIn('C2/C3', comparison['failure'])
            self.assertTrue(all(sample['measurement_protocol_sha256'] for sample in saved['samples']))
            self.assertEqual([sample['budget_profile'] for sample in saved['samples']], ['thorough', 'thorough'])

    def test_exited_leader_does_not_leave_pipe_holding_child_after_capture_refusal(self):
        with tempfile.TemporaryDirectory(prefix='wt-sweep-benchmark-child-', dir='/private/tmp') as name:
            root = Path(name)
            marker = root / 'owned-child.pid'
            helper = root / 'owned-helper'
            helper.write_text(f"""#!{sys.executable}
import os, time
from pathlib import Path
leader = os.getpid()
child = os.fork()
if child:
    Path({str(marker)!r}).write_text(str(leader) + ' ' + str(child))
    os._exit(0)
while os.getppid() == leader:
    time.sleep(0.005)
os.write(1, b'x' * 4096)
time.sleep(30)
""")
            helper.chmod(0o700)
            fixture = SimpleNamespace(root=root, repo=root, paths=[], heads={}, env={},
                command=lambda *args, **kwargs: b'{"families":[{"worktrees":[]}]}', manifest=lambda: {})
            child = None
            try:
                # Exercise the real kernel call; no host process inventory or zombie timing oracle.
                with patch.object(benchmark.os, 'killpg', wraps=os.killpg) as group_cleanup:
                    with patch.object(benchmark, 'MAX_OUTPUT', 1024), self.assertRaisesRegex(RuntimeError, 'output cap'):
                        benchmark.measure(helper, fixture, 'pipe', root / 'refused.json')
                leader, child = map(int, marker.read_text().split())
                group_cleanup.assert_called_once_with(leader, signal.SIGKILL)
            finally:
                if child is None and marker.exists():
                    _, child = map(int, marker.read_text().split())
                if child is not None:
                    try:
                        os.kill(child, signal.SIGKILL)
                    except ProcessLookupError:
                        pass

    def test_real_pipe_pty_samples_validate_rows_and_live_frames(self):
        fixture = ScaleFixture().many_worktrees(2, depth=3)
        self.addCleanup(fixture.close)
        original = fixture.manifest()
        reports = []
        for mode in ('pipe', 'pty'):
            with self.subTest(mode=mode):
                destination = fixture.root / f'{mode}-report.json'
                sample = benchmark.measure(BINARY, fixture, mode, destination)
                stderr = destination.with_suffix('.stderr').read_bytes()
                self.assertEqual(b'\x1b[2K' in stderr, mode == 'pty')
                self.assertEqual(b'\x1b[' in stderr, mode == 'pty')
                self.assertEqual(sample['terminal_protocol']['live_ansi_frames'], mode == 'pty')
                self.assertTrue(sample['fixture_rows_verified'])
                self.assertIsNotNone(sample['first_useful_ms']['stdout'])
                self.assertIsNotNone(sample['first_useful_ms']['stderr'])
                reports.append(json.loads(destination.read_text()))
        self.assertEqual(original, fixture.manifest())
        report = reports[0]
        benchmark.validate_fixture_rows(report, fixture)
        for mutate in ('empty', 'missing', 'duplicate', 'head', 'target', 'target_ref', 'source', 'history'):
            altered = copy.deepcopy(report)
            if mutate == 'empty':
                altered['items'] = []
            elif mutate == 'missing':
                altered['items'].pop()
            elif mutate == 'duplicate':
                altered['items'][1] = copy.deepcopy(altered['items'][0])
            elif mutate == 'head':
                altered['items'][0]['head'] = 'f' * 40
            elif mutate == 'target':
                altered['items'][0]['history'][0]['target_oid'] = 'f' * 40
            elif mutate == 'target_ref':
                altered['items'][0]['history'][0]['target_ref'] = {'hex': b'refs/heads/other'.hex()}
            elif mutate == 'source':
                altered['items'][0]['history'][0]['source_oid'] = 'f' * 40
            else:
                altered['items'][0]['history'] = []
            with self.subTest(refusal=mutate), self.assertRaises(AssertionError):
                benchmark.validate_fixture_rows(altered, fixture)


if __name__ == '__main__':
    unittest.main()
