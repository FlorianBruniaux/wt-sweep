"""Package a clean Apple Silicon alpha build with its licenses and file hashes."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile

source, destination = map(lambda x: Path(x).resolve(), sys.argv[1:])
def run(*args):
    return subprocess.check_output(args, cwd=source, text=True).strip()

assert not run('git', 'status', '--porcelain'), 'release checkout must be clean'
metadata = json.loads(run('cargo', 'metadata', '--format-version=1', '--locked', '--offline'))
package = next(p for p in metadata['packages'] if p['name'] == 'wt-sweep')
version = package['version']
assert version == '0.1.0-alpha.1'
binary = source / 'target/release/wt-sweep'
assert run(str(binary), '--version') == 'wt-sweep ' + version
assert run('uname', '-m') == 'arm64'
commit = run('git', 'rev-parse', 'HEAD')
name = f'wt-sweep-v{version}-aarch64-apple-darwin'
destination.mkdir(exist_ok=False)
stage = destination / name
stage.mkdir()
shutil.copy2(binary, stage / 'wt-sweep')
shutil.copy2(source / 'LICENSE.md', stage / 'LICENSE.md')
notices = stage / 'third-party-licenses'
notices.mkdir()
dependencies = []
for dep in metadata['packages']:
    if not dep['source']:
        continue
    root = Path(dep['manifest_path']).parent
    files = [p for p in root.rglob('*') if p.is_file() and any(p.name.upper().startswith(prefix) for prefix in ('LICENSE', 'LICENCE', 'COPYING', 'NOTICE'))]
    assert files, dep['name'] + ' has no bundled license'
    target = notices / (dep['name'] + '-' + dep['version'])
    for path in files:
        output = target / path.relative_to(root)
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, output)
    dependencies.append({'name': dep['name'], 'version': dep['version'], 'license': dep['license'], 'source': dep['source']})
rustdoc = Path(run('rustc', '--print', 'sysroot')) / 'share/doc/rust'
shutil.copytree(rustdoc / 'licenses', notices / 'rust/licenses')
shutil.copy2(rustdoc / 'COPYRIGHT-library.html', notices / 'rust/COPYRIGHT-library.html')
(notices / 'dependencies.json').write_text(json.dumps(dependencies, indent=2) + '\n')
(stage / 'README.txt').write_text(f'''Worktree Sweep {version}, macOS Apple Silicon alpha

Run ./wt-sweep --version and ./wt-sweep review --help.
Use the absolute path to this binary in the CLI tutorial:
https://github.com/FlorianBruniaux/wt-sweep/blob/v{version}/docs/getting-started.md

The Claude/Codex skills are included in the source repository.
No global installation or configuration is required.

This is a read-only audit. apply is unavailable.
Measured disk sizes do not establish recoverable space on APFS.
Incomplete collection remains incomplete, even when the command exits 0.

The binary is ad-hoc signed by the build tools, not Apple-notarized.
Downloaded-file Gatekeeper behavior has not been qualified.
No Intel, Linux or Windows binary is included.
''')
manifest = {'version': version, 'target': 'aarch64-apple-darwin', 'source_commit': commit, 'source_dirty': False, 'rustc': run('rustc', '--version'), 'files': {}}
for path in sorted(stage.rglob('*')):
    if path.is_file():
        manifest['files'][str(path.relative_to(stage))] = {'bytes': path.stat().st_size, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
(stage / 'MANIFEST.json').write_text(json.dumps(manifest, indent=2) + '\n')
archive = destination / (name + '.tar.gz')
with tarfile.open(archive, 'w:gz', format=tarfile.PAX_FORMAT) as tar:
    for path in [stage, *sorted(stage.rglob('*'))]:
        info = tar.gettarinfo(str(path), str(path.relative_to(destination)))
        info.uid = info.gid = 0
        info.uname = info.gname = ''
        info.mtime = 0
        if path.is_file():
            with path.open('rb') as reader:
                tar.addfile(info, reader)
        else:
            tar.addfile(info)
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
(destination / 'SHA256SUMS').write_text(f'{digest}  {archive.name}\n')
print(json.dumps({'archive': str(archive), 'sha256': digest, 'bytes': archive.stat().st_size, 'commit': commit, 'notice_packages': len(dependencies)}, indent=2))
