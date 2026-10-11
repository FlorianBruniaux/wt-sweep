"""Replay complete disk walks on owned Git corpora; conservation precedes timing."""
import argparse
import hashlib
import json
import os
import pathlib
import platform
import stat
import statistics
import subprocess
import tempfile
import time

from test_walk import oracle

SOURCE = pathlib.Path(__file__).resolve().parent


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def signature(root):
    """Independent content/identity manifest; sockets/FIFOs are never opened."""
    result = {}
    pending = [os.fsencode(root)]
    while pending:
        path = pending.pop()
        st = os.lstat(path)
        content = None
        if stat.S_ISREG(st.st_mode):
            with open(path, "rb") as f:
                content = hashlib.file_digest(f, "sha256").hexdigest()
            try:
                with open(path + b"/..namedfork/rsrc", "rb") as f:
                    fork = hashlib.file_digest(f, "sha256").hexdigest()
            except FileNotFoundError:
                fork = None
            content = [content, fork]
        elif stat.S_ISLNK(st.st_mode):
            content = os.readlink(path).hex()
        result[path.hex()] = [st.st_dev, st.st_ino, st.st_mode, st.st_nlink,
                              st.st_size, st.st_blocks, st.st_flags,
                              st.st_mtime_ns, st.st_ctime_ns, content]
        if stat.S_ISDIR(st.st_mode):
            pending.extend(os.path.join(path, name) for name in os.listdir(path))
    return result


def run(args):
    if platform.system() != "Darwin":
        raise SystemExit("Native disk experiment requires macOS and its SDK")
    if not __debug__:
        raise SystemExit("Run without Python optimization: conservation assertions are required")
    if args.out.exists():
        raise SystemExit("--out must be a new directory")
    product = args.product.resolve(strict=True)
    args.out.mkdir()
    probe = args.out / "walk"
    command = ["/usr/bin/clang", "-std=c11", "-Wall", "-Wextra", "-Werror", "-O2",
               str(SOURCE / "decoder.c"), str(SOURCE / "walk.c"), "-o", str(probe)]
    subprocess.run(command, check=True, timeout=30)
    result = {"schema": "wt-sweep.disk-native-experiment/1", "decision": "evaluation",
              "product_sha256": digest(product), "probe_sha256": digest(probe),
              "sources_sha256": {p.name: digest(p) for p in SOURCE.iterdir() if p.suffix in (".py", ".h", ".c")},
              "environment": {"system": platform.system(), "release": platform.release(),
                              "macos": platform.mac_ver()[0], "machine": platform.machine(),
                              "python": platform.python_version(),
                              "clang": subprocess.check_output(["/usr/bin/clang", "--version"], text=True).splitlines()[0],
                              "sdk": subprocess.check_output(["/usr/bin/xcrun", "--show-sdk-version"], text=True).strip()},
              "protocol": "Owned fixed Git families; discovery/product scalar correctness checked separately; full C scalar/native fd walks timed internally; one warmup, alternating order; caches not cleared; fixture creation, oracle and JSON serialization excluded; API call counters are not kernel syscall tracing",
              "samples": []}
    for count in args.sizes:
        with tempfile.TemporaryDirectory(prefix="wt-sweep-native-corpus-", dir="/private/tmp") as name:
            fixture = pathlib.Path(name)
            repo = fixture / "repo"
            env = {"PATH": "/usr/bin:/bin", "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
                   "GIT_AUTHOR_NAME": "Fixture", "GIT_AUTHOR_EMAIL": "fixture@example.invalid",
                   "GIT_COMMITTER_NAME": "Fixture", "GIT_COMMITTER_EMAIL": "fixture@example.invalid",
                   "LC_ALL": "C", "HOME": name}

            def git(*argv):
                subprocess.run(["/usr/bin/git", "-c", "core.hooksPath=/dev/null", *map(str, argv)],
                               env=env, check=True, capture_output=True, timeout=20)
            git("init", "-b", "main", repo)
            (repo / ".gitignore").write_text("ignored/\n")
            (repo / "tracked").write_bytes(b"tracked synthetic payload\n")
            git("-C", repo, "add", ".gitignore", "tracked")
            git("-C", repo, "commit", "-m", "fixture")
            roots = [repo, repo / ".git"]
            nonutf8_cases = []
            for n in range(4):
                path = fixture / f"wt-{n}"
                git("-C", repo, "worktree", "add", "-b", f"b{n}", path)
                roots.append(path)
                for part in ("ignored", ".hidden", "nested", "nested/deep"):
                    (path / part).mkdir()
                for i in range(count):
                    parent = path / ("ignored" if i % 3 == 0 else "nested/deep" if i % 3 == 1 else ".hidden")
                    (parent / f"file-{i}").write_bytes(f"synthetic payload {i}\n".encode())
                os.link(path / "tracked", path / "ignored" / "hardlink")
                os.symlink("ignored", path / "directory-link")
                os.symlink("/outside/missing", path / "broken-link")
                os.mkfifo(path / "fifo")
                with (path / "sparse").open("wb") as f:
                    f.truncate(1024 * 1024)
                with open(str(path / "tracked") + "/..namedfork/rsrc", "wb") as fork:
                    fork.write(b"synthetic resource fork" * 200)
                raw = os.fsencode(path) + b"/nonutf8-\xff\n"
                try:
                    fd = os.open(raw, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                except OSError as error:
                    if error.errno != 92:  # EILSEQ on Darwin: unsupported by this volume.
                        raise
                    nonutf8_cases.append({"state": "UNKNOWN", "errno": error.errno,
                                          "reason": "owned APFS fixture rejects non-UTF8 name"})
                    (path / "unicode-é\n").write_bytes(b"unicode and newline name payload")
                else:
                    os.write(fd, b"raw name payload"); os.close(fd)
                    nonutf8_cases.append({"state": "VERIFIED"})
            # A hardlink across requested roots exercises global and per-root totals.
            os.link(roots[2] / "tracked", roots[3] / "cross-root-link")
            expected, unique = oracle(roots)
            before = signature(fixture)
            sample_dir = args.out / f"corpus-{count}"
            sample_dir.mkdir()
            (sample_dir / "oracle.json").write_text(json.dumps({"roots": expected, "unique_bytes": unique, "manifest": before}, indent=2) + "\n")
            started = time.monotonic_ns()
            completed = subprocess.run([str(product), "scan", "--repo", str(repo), "--disk-usage", "--format", "json"],
                                       env=env, check=True, capture_output=True, timeout=60)
            product_wall = time.monotonic_ns() - started
            product_output = json.loads(completed.stdout)
            (sample_dir / "product.json").write_bytes(completed.stdout)
            (sample_dir / "product.stderr.txt").write_bytes(completed.stderr)
            usage = product_output["disk_usage"]
            assert usage["complete"], usage
            actual = {r["path"]["hex"]: [r["bytes"]["logical"], r["bytes"]["allocated"]] for r in usage["measurements"]}
            assert actual == {os.fsencode(p).hex(): row["bytes"] for p, row in zip(roots, expected)}, actual
            assert [usage["unique_bytes"]["logical"], usage["unique_bytes"]["allocated"]] == unique

            def collect(mode, manifest=False):
                argv = [str(probe), "--mode", mode, *(["--manifest"] if manifest else []), "--", *map(str, roots)]
                p = subprocess.run(argv, check=True, capture_output=True, timeout=60)
                record = json.loads(p.stdout)
                assert record["complete"], record
                assert record["unique_bytes"] == unique
                assert [r["bytes"] for r in record["roots"]] == [r["bytes"] for r in expected]
                if manifest:
                    for r, want in zip(record["roots"], expected):
                        assert sorted(e[1:] for e in r["objects"]) == want["objects"]
                    (sample_dir / f"{mode}-complete.json").write_bytes(p.stdout)
                return record
            correctness = {mode: collect(mode, manifest=True) for mode in ("scalar", "native")}
            assert correctness["native"]["calls"]["bulk"] > 0, "candidate did not exercise native API"
            assert correctness["native"]["calls"]["stat"] < correctness["scalar"]["calls"]["stat"], "candidate falls back for all metadata; timing refused"
            assert signature(fixture) == before, "collector changed owned fixture"
            samples = {"files_per_linked_worktree": count, "roots": len(roots),
                       "nonutf8_creation": nonutf8_cases,
                       "oracle_and_product_match": True, "conservation_before_timing": True,
                       "native_metadata_elimination_verified": True,
                       "product_scalar_elapsed_ms": usage["elapsed_ms"], "product_cli_wall_ns": product_wall,
                       "scalar": [], "native": []}
            for mode in ("scalar", "native"):
                collect(mode)
            for trial in range(args.trials):
                order = ("scalar", "native") if trial % 2 == 0 else ("native", "scalar")
                for mode in order:
                    value = collect(mode)
                    samples[mode].append({"elapsed_ns": value["elapsed_ns"], "calls": value["calls"],
                                          "entries_checked": value["entries_checked"]})
            assert signature(fixture) == before, "collector changed owned fixture during timing"
            samples["conservation_after_timing"] = True
            for mode in ("scalar", "native"):
                samples[mode + "_median_ns"] = statistics.median(v["elapsed_ns"] for v in samples[mode])
            samples["native_scalar_ratio"] = samples["native_median_ns"] / samples["scalar_median_ns"]
            result["samples"].append(samples)
            (args.out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
            print(json.dumps({k: v for k, v in samples.items() if k not in ("scalar", "native")}), flush=True)
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product", type=pathlib.Path, required=True)
    parser.add_argument("--out", type=pathlib.Path, required=True)
    parser.add_argument("--sizes", type=int, nargs="+", default=[32, 512, 4096])
    parser.add_argument("--trials", type=int, default=5)
    args = parser.parse_args()
    if not 1 <= args.trials <= 20 or not args.sizes or any(not 1 <= n <= 10000 for n in args.sizes):
        parser.error("finite corpus sizes 1..10000 and trials 1..20 required")
    run(args)
