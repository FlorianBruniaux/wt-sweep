"""Fixed-corpus benchmark protocol tests, without running product benchmarks."""
import copy
import importlib.util
from pathlib import Path
import tempfile
import sys
from types import SimpleNamespace
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts/benchmark-history-batches.py'


def load_benchmark(test):
    test.assertTrue(SCRIPT.is_file(), 'fixed-corpus benchmark runner is missing')
    spec = importlib.util.spec_from_file_location('history_benchmark', SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class HistoryBenchmarkTests(unittest.TestCase):
    def test_fixture_has_real_second_parent_ancestry_and_preserved_private_sources(self):
        bench = load_benchmark(self)
        with bench.Corpus(chain=12, side=3) as corpus:
            before = corpus.manifest()
            self.assertEqual(corpus.git('rev-list', '--count', corpus.target).strip(), b'16')
            parents = corpus.git('rev-list', '--parents', '-n', '1', corpus.target).split()
            self.assertEqual(parents[2].decode(), corpus.heads[corpus.side_worktree])
            source = corpus.heads[corpus.side_worktree]
            self.assertEqual(corpus.git('merge-base', '--is-ancestor', source, corpus.target), b'')
            self.assertNotIn(source.encode(), corpus.git('rev-list', '--first-parent', corpus.target).split())
            for path, roots in corpus.private_roots.items():
                admin = Path(corpus.git('-C', str(path), 'rev-parse', '--absolute-git-dir').decode().strip())
                self.assertEqual((admin / 'ORIG_HEAD').read_text().strip(), corpus.chain_oids[0])
                self.assertEqual({corpus.chain_oids[0], corpus.chain_oids[6], corpus.heads[path]}, set(roots))
                self.assertLess(len(roots), len(corpus.chain_oids))
                reflog = (admin / 'logs/HEAD').read_text()
                self.assertTrue(all(oid in reflog for oid in roots))
            self.assertEqual(corpus.manifest(), before)

    def test_normalization_keeps_reasons_oids_caps_and_private_source_proofs(self):
        bench = load_benchmark(self)
        report = {'started_unix_ms': 1, 'finished_unix_ms': 2,
                  'collection_metrics': {'phases': [{'elapsed_ms': 10}],
                                         'budget_profile': 'standard', 'applied_limits': {'reads': 20}},
                  'history_metrics': {'elapsed_ms': 3},
                  'items': [{'head': 'a' * 40, 'private_history': {
                      'sources': [{'sha256': 'b' * 64, 'complete': True}],
                      'roots': [{'oid': 'c' * 40, 'result': 'reachable_from_retained_ref', 'reason': None}]}}]}
        changed = copy.deepcopy(report)
        changed['started_unix_ms'] = 90
        changed['collection_metrics']['phases'][0]['elapsed_ms'] = 99
        changed['history_metrics']['elapsed_ms'] = 99
        self.assertEqual(bench.normalized_evidence(report), bench.normalized_evidence(changed))
        for mutation in ('oid', 'reason', 'source', 'cap'):
            changed = copy.deepcopy(report)
            if mutation == 'cap':
                changed['collection_metrics']['applied_limits']['reads'] = 21
            elif mutation == 'source':
                changed['items'][0]['private_history']['sources'][0]['complete'] = False
            else:
                changed['items'][0]['private_history']['roots'][0][mutation] = 'changed'
            self.assertNotEqual(bench.normalized_evidence(report), bench.normalized_evidence(changed))

    def test_campaign_alternates_pair_order(self):
        bench = load_benchmark(self)
        self.assertEqual(bench.sample_order(3, 1), [
            ('warmup', 0, 'baseline'), ('warmup', 0, 'after'),
            ('measurement', 0, 'baseline'), ('measurement', 0, 'after'),
            ('measurement', 1, 'after'), ('measurement', 1, 'baseline'),
            ('measurement', 2, 'baseline'), ('measurement', 2, 'after')])

    def test_positive_report_validator_rejects_lost_roots_relations_and_provenance(self):
        bench = load_benchmark(self)
        path = Path('/synthetic/linked')
        source, target = 'a' * 40, 'b' * 40
        bytepath = lambda text: {'hex': text.encode().hex()}
        corpus = SimpleNamespace(repo=Path('/synthetic/main'), heads={path: source},
                                 target=target, private_roots={path: [source]}, chain_oids=[source])
        report = {'schema': 'wt-sweep.review/2', 'items': [{
            'path': bytepath(str(path)), 'head': source, 'history_omitted_targets': 0,
            'history': [{'result': 'ancestor', 'source_oid': source, 'target_oid': target,
                         'target_ref': bytepath('refs/heads/integration'), 'stop_reason': None}],
            'private_history': {'enumeration_complete': True, 'anchors_complete': True, 'gaps': [],
                'roots': [{'oid': source, 'result': 'covered', 'reason': None,
                           'anchor': {'oid': target, 'reference': bytepath('refs/heads/integration')},
                           'provenance': [{'kind': 'root_ref', 'source': bytepath('/synthetic/admin/ORIG_HEAD')},
                                          {'kind': 'reflog_new', 'source': bytepath('/synthetic/admin/logs/HEAD')}]}]}}],
            'collection_metrics': {'phases': [
                {'name': 'history', 'counters': {'object_reads': 2, 'object_bytes': 200, 'git_processes': 1}},
                {'name': 'private_history', 'counters': {'object_reads': 3, 'object_bytes': 300, 'git_processes': 1}}]}}
        bench.validate_report(report, corpus)
        for mutation in ('roots', 'relation', 'anchor', 'provenance', 'counter', 'only_cached_roots'):
            changed = copy.deepcopy(report)
            item = changed['items'][0]
            if mutation == 'roots':
                item['private_history']['roots'] = []
            elif mutation == 'relation':
                item['history'][0]['result'] = 'unknown'
            elif mutation == 'anchor':
                item['private_history']['roots'][0]['anchor']['oid'] = source
            elif mutation == 'provenance':
                item['private_history']['roots'][0]['provenance'].pop()
            elif mutation == 'only_cached_roots':
                changed['collection_metrics']['phases'][1]['counters']['object_reads'] = 2
            else:
                changed['collection_metrics']['phases'][0]['counters']['object_bytes'] = 0
            with self.subTest(mutation=mutation), self.assertRaises(AssertionError):
                bench.validate_report(changed, corpus)

    def test_timeout_retains_partial_output_and_failure_status(self):
        bench = load_benchmark(self)
        result = bench.execute([sys.executable, '-c', "import time; print('partial', flush=True); time.sleep(30)"],
                               {'PATH': '/usr/bin:/bin'}, timeout=0.2)
        self.assertEqual(result['stdout'], b'partial\n')
        self.assertNotEqual(result['exit_status'], 0)
        self.assertIn('deadline', result['failure'])

    def test_summary_never_uses_warmups_or_an_incomplete_measurement_pair(self):
        bench = load_benchmark(self)
        result = {'comparisons': [{'stage': 'warmup', 'pair': 0, 'comparable': True}],
                  'samples': [], 'summary': {'obsolete': True}}
        bench.summarize(result)
        self.assertFalse(result['performance_comparable'])
        self.assertNotIn('summary', result)
        result['samples'].append({'stage': 'measurement', 'pair': 0, 'binary': 'baseline'})
        bench.summarize(result)
        self.assertFalse(result['performance_comparable'])
        self.assertNotIn('summary', result)

    def test_output_refuses_repository_or_existing_artifact(self):
        bench = load_benchmark(self)
        with self.assertRaisesRegex(ValueError, 'outside'):
            bench.validate_output(SCRIPT.parent / 'private-results.json')
        with tempfile.TemporaryDirectory(prefix='wt-sweep-history-protocol-', dir='/private/tmp') as name:
            target = Path(name) / 'result.json'
            bench.validate_output(target)
            target.write_text('retain')
            with self.assertRaisesRegex(ValueError, 'exists'):
                bench.validate_output(target)
            self.assertEqual(target.read_text(), 'retain')


if __name__ == '__main__':
    unittest.main()
