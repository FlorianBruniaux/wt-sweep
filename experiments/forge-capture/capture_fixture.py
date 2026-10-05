"""Run a local fixture producer and persist its actual stdout plus executor receipt.

No network/forge authority: this proves local execution and byte binding only.
Usage: capture_fixture.py INPUT RAW_OUTPUT RECEIPT_OUTPUT
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time


def main():
    source, raw_path, receipt_path = map(Path, sys.argv[1:])
    argv = [sys.executable, "-B", "-c",
            "import sys; from pathlib import Path; sys.stdout.buffer.write(Path(sys.argv[1]).read_bytes())",
            str(source)]
    started = time.time_ns() // 1_000_000
    child = subprocess.run(argv, capture_output=True, timeout=10, check=False)
    finished = time.time_ns() // 1_000_000
    with raw_path.open("xb") as stream:
        stream.write(child.stdout)
    # Re-read the bytes actually persisted, instead of trusting a model transcript.
    persisted = raw_path.read_bytes()
    decoder = json.JSONDecoder()
    remaining = persisted.decode()
    pages = []
    while remaining.strip():
        page, end = decoder.raw_decode(remaining.lstrip())
        remaining = remaining.lstrip()[end:]
        pages.append({"number": len(pages) + 1, "items": len(page),
                      "cursor": None, "next_cursor": None})
    receipt = {"schema": "wt-sweep.forge-receipt/1", "argv": argv,
               "provider": "gitlab", "host": "forge.example.invalid", "project": "17",
               "started_unix_ms": started, "finished_unix_ms": finished,
               "exit_code": child.returncode, "raw_sha256": hashlib.sha256(persisted).hexdigest(),
               "raw_bytes": len(persisted), "truncated": False,
               "pages": pages, "pagination_complete": True}
    with receipt_path.open("x") as stream:
        json.dump(receipt, stream)


if __name__ == "__main__":
    main()
