import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tarfile

source, package = map(lambda p: Path(p).resolve(), sys.argv[1:])
out = package.parent / 'archive-acceptance'
out.mkdir()
checksum, filename = (package / 'SHA256SUMS').read_text().split()
archive = package / filename
assert hashlib.sha256(archive.read_bytes()).hexdigest() == checksum
with tarfile.open(archive) as tar:
    assert all(m.isfile() or m.isdir() for m in tar.getmembers()), 'unexpected archive member type'
    tar.extractall(out, filter='data')
roots = [p for p in out.iterdir() if p.is_dir()]
assert len(roots) == 1
root = roots[0]
manifest = json.loads((root / 'MANIFEST.json').read_text())
for name, expected in manifest['files'].items():
    data = (root / name).read_bytes()
    assert len(data) == expected['bytes']
    assert hashlib.sha256(data).hexdigest() == expected['sha256']
binary = root / 'wt-sweep'
for args in (['--version'], ['--help'], ['review', '--help']):
    result = subprocess.run([str(binary), *args], capture_output=True, text=True, timeout=10)
    assert result.returncode == 0, result.stderr
    if args == ['--version']:
        assert result.stdout.strip() == 'wt-sweep 0.1.0-alpha.2'
section = (source / 'docs/getting-started.md').read_text().split('## Disposable demo\n',1)[1].split('## Review your repository',1)[0]
script = re.search(r'```sh\n(.*?)```', section, re.S).group(1)
assert script.count('./target/release/wt-sweep') == 1
script = script.replace('./target/release/wt-sweep', shlex.quote(str(binary)))
(out / 'demo.sh').write_text(script)
result = subprocess.run(['bash',str(out/'demo.sh')], cwd=source, capture_output=True, text=True, timeout=240)
(out / 'demo.log').write_text(result.stdout + result.stderr)
assert result.returncode == 0, str(out/'demo.log')
for label, count in [('Keep',2),('Cleanup candidates',1),('Needs verification',0)]:
    assert re.search(rf'{label}:\s*{count}\b', result.stdout), (label, str(out/'demo.log'))
demo = Path(re.search(r'Demo and reports: (.+)',result.stdout).group(1).strip())
runs = list((demo / 'reports').glob('run-*'))
assert len(runs) == 1
receipt = json.loads((runs[0] / 'run.json').read_text())
for artifact in receipt['artifacts']:
    data = (runs[0] / artifact['name']).read_bytes()
    assert len(data) == artifact['bytes']
    assert hashlib.sha256(data).hexdigest() == artifact['sha256']
record = {'archive':str(archive),'archive_sha256':checksum,'source_commit':manifest['source_commit'],'archive_files_verified':len(manifest['files']), 'demo_exit_code':0, 'verdicts':{'keep':2,'cleanup_candidate':1,'verify':0},'receipt_artifacts':len(receipt['artifacts']), 'demo_run':str(runs[0]), 'not_verified':['Fresh-machine toolchain installation','Downloaded-file Gatekeeper behavior','Apple notarization','Intel/Linux runtime','Browser rendering','Native skill discovery']}
(out / 'result.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record,indent=2))
