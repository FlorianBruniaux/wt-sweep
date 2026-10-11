"""Fast macOS correctness entry point, separate from benchmark timing."""
import os
import argparse
import pathlib
import platform
import subprocess
import sys
import tempfile

SOURCE = pathlib.Path(__file__).resolve().parent


def build(out, sanitizers=False):
    flags = ["-std=c11", "-Wall", "-Wextra", "-Werror", "-O2"]
    if sanitizers:
        flags += ["-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
    subprocess.run(["/usr/bin/clang", *flags, str(SOURCE / "decoder.c"), str(SOURCE / "walk.c"),
                    "-o", str(out / "walk")], check=True, timeout=30)
    subprocess.run(["/usr/bin/clang", *flags, str(SOURCE / "decoder.c"), str(SOURCE / "test_decoder.c"),
                    "-o", str(out / "test-decoder")], check=True, timeout=30)
    subprocess.run(["/usr/bin/clang", *flags, str(SOURCE / "decoder.c"), str(SOURCE / "test_entry.c"),
                    "-o", str(out / "test-entry")], check=True, timeout=30)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--product", type=pathlib.Path)
    args = parser.parse_args()
    if platform.system() != "Darwin":
        raise SystemExit("Native disk experiment requires macOS and its SDK; unsupported here")
    with tempfile.TemporaryDirectory(prefix="wt-sweep-native-build-") as temp:
        out = pathlib.Path(temp)
        build(out, sanitizers=True)
        subprocess.run([str(out / "test-decoder")], check=True, timeout=10)
        fixture = out / "entry-fixture"
        fixture.mkdir()
        (fixture / "file").write_bytes(b"abc")
        (fixture / "d").mkdir()
        (fixture / "d" / "nested").write_bytes(b"four")
        subprocess.run([str(out / "test-entry"), str(fixture)], check=True, timeout=10)
        env = dict(os.environ, SWEEP_NATIVE_PROBE=str(out / "walk"))
        if args.product:
            env["SWEEP_NATIVE_PRODUCT"] = str(args.product.resolve(strict=True))
        subprocess.run([sys.executable, "-B", str(SOURCE / "test_walk.py"), "-v"], env=env, check=True, timeout=30)
        subprocess.run([sys.executable, "-B", str(SOURCE / "test_product.py"), "-v"], env=env, check=True, timeout=45)
