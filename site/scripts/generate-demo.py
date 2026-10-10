#!/usr/bin/env python3
"""Generate a native website report from an owned disposable Git fixture only."""
import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

if sys.version_info < (3, 11):
    raise SystemExit('Python 3.11+ is required for the independent fixture manifest.')
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tests'))
from inventory_support import HistoryFixture, filesystem_manifest

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--out', type=Path, default=ROOT / 'site/public/demo')
args = parser.parse_args()
binary = ROOT / 'target/debug/wt-sweep'
if not binary.is_file():
    raise SystemExit('Build the CLI first: cargo build --locked --offline')
fixture = HistoryFixture()
try:
    note = fixture.root / 'local-notes'
    feature = fixture.root / 'unmerged-feature'
    fixture.git('-C', str(fixture.principal), 'worktree', 'add', '-b', 'local-notes', str(note))
    fixture.git('-C', str(fixture.principal), 'worktree', 'add', '-b', 'unmerged-feature', str(feature))
    (note / 'notes.txt').write_text('Synthetic note to preserve.\n')
    fixture.write_commit(feature, 'feature.txt', b'Synthetic unmerged work.\n')
    before = filesystem_manifest(fixture.root)
    with tempfile.TemporaryDirectory(prefix='wt-sweep-site-report-', dir='/private/tmp') as reports:
        result = subprocess.run([str(binary), 'review', '--repo', str(fixture.principal),
                                 '--target', 'refs/heads/main', '--disk-usage', '--summary',
                                 '--html', '--run-dir', reports], env=fixture.env,
                                capture_output=True, timeout=45)
        if result.returncode != 0:
            raise SystemExit(result.stderr.decode(errors='replace'))
        if before != filesystem_manifest(fixture.root):
            raise SystemExit('Disposable source fixture changed during review')
        runs = list(Path(reports).glob('run-*'))
        if len(runs) != 1:
            raise SystemExit('Expected one complete run')
        run = runs[0]
        receipt = json.loads((run / 'run.json').read_bytes())
        for entry in receipt['artifacts']:
            content = (run / entry['name']).read_bytes()
            if len(content) != entry['bytes'] or hashlib.sha256(content).hexdigest() != entry['sha256']:
                raise SystemExit('Artifact does not match receipt')
        review = json.loads((run / 'review.json').read_bytes())
        verdicts = dict(Counter(row['classification'] for row in review['items']))
        if verdicts != {'keep': 3, 'cleanup_candidate': 1}:
            raise SystemExit(f'Unexpected native demo decisions: {verdicts}')
        args.out.mkdir(parents=True, exist_ok=True)
        for name in ['inventory.json', 'review.json', 'report.txt', 'report.html', 'run.json']:
            shutil.copy2(run / name, args.out / name)
        provenance = {
            'generated_at': datetime.now(timezone.utc).isoformat(),
            'source': 'native CLI report from a disposable synthetic Git family',
            'product_base_commit': 'c3e38f7810424ae57f58d6ea10c4db996e9122fb',
            'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
            'report_sha256': hashlib.sha256((args.out / 'report.html').read_bytes()).hexdigest(),
            'verdicts': verdicts, 'fixture_conserved': True, 'receipt_verified': True,
            'real_repository_scans': 0, 'worktrees_removed': 0,
            'usage_checked': False, 'recoverable_space_established': False,
            'paths': 'synthetic temporary paths only; the fixture no longer exists',
        }
        (args.out / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
        print('Native demo: 3 keep, 1 cleanup_candidate; fixture conserved and receipt verified.')
finally:
    fixture.close()
