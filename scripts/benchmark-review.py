#!/usr/bin/env python3
"""Bounded review measurements on owned constant Git families, never host scans.

Requires Python 3.11+. Example:
python3.14 -B scripts/benchmark-review.py --binary baseline=/path/to/frozen \
    --binary after=/path/to/frozen --counts 50 100 --modes pipe pty --out /private/tmp/result.json
OS cache is not cleared. First useful output requires an actual collected fact:
a known complete displayed history relation or numeric local proof with identity.
Unknown statuses and inventory HEADs never count. Receipt chunks are saved for offline audit.
"""
import argparse
import errno
import hashlib
import json
import os
from pathlib import Path
import platform
import pty
import re
import selectors
import signal
import struct
import subprocess
import sys
import termios
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tests"))
from test_scale import CliScaleTests, ScaleFixture, decode_path  # Independent real Git fixture/oracle.

MAX_OUTPUT = 32 * 1024 * 1024
MAX_CAPTURE_CHUNKS = 16384
MAX_CAPTURE_METADATA = 2 * 1024 * 1024


def digest_file(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def stable_digest(value):
    def plain(item):
        if isinstance(item, bytes):
            return {"bytes_hex": item.hex()}
        if isinstance(item, dict):
            return {(key.hex() if isinstance(key, bytes) else str(key)): plain(child)
                    for key, child in item.items()}
        if isinstance(item, (tuple, list)):
            return [plain(child) for child in item]
        return item
    return hashlib.sha256(json.dumps(plain(value), sort_keys=True,
                                    separators=(",", ":")).encode()).hexdigest()


HISTORY_STATUS = re.compile(
    rb"Observed history: (?:identical_commit|ancestor|not_ancestor|unknown) "
    rb"\(provisional, (?:complete|partial)\);")
HISTORY_FACT = re.compile(
    rb"Observed history: (?P<result>identical_commit|ancestor|not_ancestor) "
    rb"\(provisional, complete\);")
LOCAL_NUMBER = rb"(?:[0-9]+|>=[0-9]+|UNKNOWN)"
LOCAL_FACT = re.compile(
    rb"Observed local: staged=(?P<staged>" + LOCAL_NUMBER + rb") unstaged=(?P<unstaged>" + LOCAL_NUMBER
    + rb") conflicts=(?P<conflicts>" + LOCAL_NUMBER + rb") untracked=(?P<untracked>" + LOCAL_NUMBER
    + rb") ignored=(?P<ignored>" + LOCAL_NUMBER + rb")(?=[ \r\n;(\x1b])")
FACETS = ("staged", "unstaged", "conflicts", "untracked", "ignored")


def displayed_segments(output):
    """Never join text across a redraw/line boundary to invent a visible fact."""
    begin = 0
    for separator in re.finditer(rb"\r|\n|\x1b\[2K", output):
        yield begin, output[begin:separator.start()]
        begin = separator.end()
    yield begin, output[begin:]


def provisional_status_visible(output, stream):
    return stream == "stderr" and any(HISTORY_STATUS.search(segment) or LOCAL_FACT.search(segment)
                                      for _, segment in displayed_segments(output))


def complete_json_objects(output, pattern):
    decoder = json.JSONDecoder()
    for marker in re.finditer(pattern, output):
        begin = marker.start() if output[marker.start():marker.start() + 1] == b"{" else marker.end()
        while begin < len(output) and output[begin:begin + 1] in b" \t\r\n":
            begin += 1
        if output[begin:begin + 1] != b"{":
            continue
        try:
            text = output[begin:].decode("utf-8")
            value, end = decoder.raw_decode(text)
        except (UnicodeDecodeError, json.JSONDecodeError):
            continue
        if isinstance(value, dict):
            yield begin, begin + len(text[:end].encode("utf-8")), value


def visible_fact(output, heads, paths=(), stream="stderr"):
    """An actual visible relation/count, never an inventory HEAD or unknown status."""
    output = bytes(output)
    matches = []
    if stream == "stderr":
        for begin, segment in displayed_segments(output):
            for match in HISTORY_FACT.finditer(segment):
                matches.append((begin + match.end(), {"kind": "history_relation",
                    "result": match['result'].decode("ascii"), "complete": True,
                    "start_byte": begin + match.start(), "end_byte": begin + match.end()}))
            for match in LOCAL_FACT.finditer(segment):
                tokens = {facet: match[facet].decode("ascii") for facet in FACETS}
                numeric = any(value.isdecimal() or (value.startswith(">=") and int(value[2:]) > 0)
                              for value in tokens.values())
                tail = segment[match.end():]
                prefix = b" (provisional); "
                if not numeric or not tail.startswith(prefix):
                    continue
                identity = tail[len(prefix):]
                for path in paths:
                    if identity.startswith(path) and (len(identity) == len(path) or identity[len(path):len(path) + 1] in b" \t;"):
                        end = begin + match.end() + len(prefix) + len(path)
                        matches.append((end, {"kind": "local_counts", "path_hex": path.hex(),
                            "counts": tokens, "start_byte": begin + match.start(), "end_byte": end}))
    elif stream == "stdout":
        known_heads = {head.decode("ascii") for head in heads}
        for begin, end, value in complete_json_objects(output, rb'\{\s*"source_oid"\s*:'):
            if (value.get("source_oid") in known_heads and value.get("target_oid") in known_heads
                    and value.get("target_ref") == {"hex": b"refs/heads/integration".hex()}
                    and value.get("result") in ("identical_commit", "ancestor", "not_ancestor")
                    and "stop_reason" in value and value["stop_reason"] is None):
                matches.append((end, {"kind": "history_relation", "result": value["result"],
                    "source_oid": value["source_oid"], "target_oid": value["target_oid"],
                    "complete": True, "start_byte": begin, "end_byte": end}))
        for begin, end, value in complete_json_objects(output, rb'"local_summary"\s*:\s*'):
            identity = value.get("worktree_identity") or {}
            if (value.get("head") not in known_heads or not all(type(identity.get(key)) is int
                    and identity[key] >= 0 for key in ("device", "inode"))):
                continue
            observed = {}
            for facet in FACETS:
                count = value.get(facet) or {}
                number = count.get("observed")
                if type(number) is int and number >= 0 and (count.get("complete") is True or number > 0):
                    observed[facet] = {"observed": number, "complete": count.get("complete") is True}
            if observed:
                matches.append((end, {"kind": "local_counts", "head": value["head"],
                    "worktree_identity": identity, "counts": observed, "start_byte": begin, "end_byte": end}))
    else:
        raise ValueError("capture stream must be stdout or stderr")
    return min(matches, key=lambda pair: pair[0])[1] if matches else None


def has_useful_fact(output, heads, paths=(), stream="stderr"):
    return visible_fact(output, heads, paths, stream) is not None


def update_capture_timings(output, stream, elapsed_ns, heads, paths, timings):
    elapsed_ms = elapsed_ns / 1e6
    if timings["first_output_ms"][stream] is None:
        timings["first_output_ms"][stream] = elapsed_ms
    if (timings["first_provisional_status_ms"][stream] is None
            and provisional_status_visible(output[stream], stream)):
        timings["first_provisional_status_ms"][stream] = elapsed_ms
    if timings["first_useful_ms"][stream] is None:
        fact = visible_fact(output[stream], heads, paths, stream)
        if fact is not None:
            timings["first_useful_ms"][stream] = elapsed_ms
            timings["first_fact_details"][stream] = fact


def empty_capture_timings():
    return {key: {"stdout": None, "stderr": None} for key in (
        "first_output_ms", "first_provisional_status_ms", "first_useful_ms", "first_fact_details")}


def recompute_capture(capture_path, heads=None, paths=None):
    """Recompute exact receipt-chunk timing from bounded raw streams and offsets."""
    encoded = capture_path.read_bytes()
    if len(encoded) > MAX_CAPTURE_METADATA:
        raise AssertionError("capture metadata cap exceeded")
    capture = json.loads(encoded)
    if capture.get("schema") != "wt-sweep.benchmark-capture/1" or capture.get("clock") != "time.monotonic_ns":
        raise AssertionError("unexpected capture schema/clock")
    if heads is None:
        heads = [head.encode("ascii") for head in capture["fixture_identities"]["heads"]]
    if paths is None:
        paths = [bytes.fromhex(path) for path in capture["fixture_identities"]["paths_hex"]]
    streams = capture["streams"]
    if set(streams) != {"stdout", "stderr"}:
        raise AssertionError("capture stream set differs")
    raw = {}
    for stream, description in streams.items():
        source = Path(description["path"])
        if digest_file(source) != description["sha256"]:
            raise AssertionError("capture stream hash differs")
        raw[stream] = source.read_bytes()
        if len(raw[stream]) != description["bytes"]:
            raise AssertionError("capture stream byte length differs")
    if sum(map(len, raw.values())) > MAX_OUTPUT or len(capture["chunks"]) > MAX_CAPTURE_CHUNKS:
        raise AssertionError("capture output/chunk cap exceeded")
    output = {"stdout": bytearray(), "stderr": bytearray()}
    timings = empty_capture_timings()
    previous = -1
    for chunk in capture["chunks"]:
        stream, offset, length, elapsed = (chunk[key] for key in ("stream", "offset_bytes", "length_bytes", "elapsed_ns"))
        if stream not in output or type(offset) is not int or offset != len(output[stream]):
            raise AssertionError("capture offset discontinuity")
        if type(length) is not int or not 0 < length <= 65536 or offset + length > len(raw[stream]):
            raise AssertionError("capture chunk length differs")
        if type(elapsed) is not int or not max(0, previous) <= elapsed <= 720 * 1_000_000_000:
            raise AssertionError("capture monotonic timing differs")
        previous = elapsed
        output[stream].extend(raw[stream][offset:offset + length])
        update_capture_timings(output, stream, elapsed, heads, paths, timings)
    if any(bytes(output[stream]) != raw[stream] for stream in output):
        raise AssertionError("capture omitted raw bytes")
    return timings


def validate_fixture_rows(report, fixture):
    if report.get("schema") != "wt-sweep.review/2":
        raise AssertionError("unexpected review schema")
    items = report.get("items", [])
    actual = {decode_path(item["path"]): item for item in items}
    expected = {os.fsencode(path): fixture.heads[path] for path in fixture.paths}
    if len(actual) != len(items) or set(actual) != set(expected):
        raise AssertionError("review omitted, duplicated or invented fixture paths")
    graph_cap = report["collection_metrics"]["applied_limits"]["history_per_target_steps"]
    for path, head in expected.items():
        item = actual[path]
        if item["head"] != head:
            raise AssertionError("review HEAD differs from fixture DAG")
        records = item["history"]
        if len(records) != 1 or item["history_omitted_targets"]:
            raise AssertionError("review must contain exactly the selected history target")
        history = records[0]
        if (decode_path(history["target_ref"]) != b"refs/heads/integration"
                or history["target_oid"] != fixture.target or history["source_oid"] != head):
            raise AssertionError("history source/target identity differs from fixture DAG")
        if head == fixture.target:
            expected_result = "identical_commit"
        elif fixture.ancestor(head, fixture.target):
            expected_result = "ancestor"
        else:
            expected_result = "unknown" if len(fixture.target_chain) > graph_cap else "not_ancestor"
        if history["result"] != expected_result:
            raise AssertionError("history result differs from independent fixture DAG/cap oracle")
        if expected_result == "unknown" and not history["stop_reason"]:
            raise AssertionError("bounded unknown must retain its refusal cause")


def compare_samples(first, second):
    a = json.loads(Path(first["review_path"]).read_text())
    b = json.loads(Path(second["review_path"]).read_text())
    ia = {decode_path(item["path"]): item for item in a["items"]}
    ib = {decode_path(item["path"]): item for item in b["items"]}
    oracle = CliScaleTests()
    oracle.assertEqual(set(ia), set(ib))
    oracle.assertEqual(oracle.stable_results(ia), oracle.stable_results(ib))
    oracle.assert_order_verdicts_follow_collected_evidence(ia, ib)
    oracle.assertEqual(first["manifest_digest"], second["manifest_digest"])
    oracle.assertEqual(a["collection_metrics"]["applied_limits"], b["collection_metrics"]["applied_limits"])
    oracle.assertEqual(a["collection_metrics"]["budget_profile"], b["collection_metrics"]["budget_profile"])
    changed_proofs = sum(oracle.decision_evidence(ia[path]) != oracle.decision_evidence(ib[path]) for path in ia)
    same_stashes = oracle.decision_evidence({"stashes": a["shared_stashes"]}) == oracle.decision_evidence({"stashes": b["shared_stashes"]})
    # Discovery clocks/cost are metrics; traversal scope and gaps remain proof.
    coverage_a = {key: value for key, value in a["coverage"].items() if key != "discovery_metrics"}
    coverage_b = {key: value for key, value in b["coverage"].items() if key != "discovery_metrics"}
    same_coverage = coverage_a == coverage_b
    comparable = changed_proofs == 0 and same_stashes and same_coverage
    return {"comparable": comparable, "judgement_oracle_passed": True,
            "failure": None if comparable else "collected item proofs, stashes or discovery coverage differ; no performance comparability",
            "budget_profile": a["collection_metrics"]["budget_profile"], "same_relevant_coverage": same_coverage,"worktrees": first["worktrees"], "mode": first["mode"], "order": first["order"],
            "sample": first["sample"], "first_binary": first["binary"], "second_binary": second["binary"],
            "same_source_manifest": True, "same_applied_limits": True, "same_graph_results": True,
            "changed_normalized_item_proofs": changed_proofs,
            "changed_classifications": sum(ia[path]["classification"] != ib[path]["classification"] for path in ia),
            "same_normalized_shared_stashes": same_stashes,
            "oracle": "strict graph; identical normalized proofs require identical verdicts; final private gaps remain explicit"}


def evidence(report):
    """Keep evidence differences, including why a finite budget changed a verdict."""
    rows = []
    for item in report["items"]:
        local = item.get("local_summary")
        private = item.get("private_history")
        rows.append({
            "path": item["path"], "head": item["head"], "classification": item["classification"],
            "decision": item.get("decision"),
            "local": None if local is None else {key: local[key] for key in (
                "head_complete", "index_complete", "disk_complete", "staged", "unstaged",
                "conflicts", "untracked", "ignored", "gaps", "temporal_consistency")},
            "private": None if private is None else {key: private[key] for key in (
                "enumeration_complete", "anchors_complete", "roots", "gaps", "temporal_consistency")},
            "history": [{key: record[key] for key in (
                "source_oid", "target_ref", "target_oid", "result", "observed_result", "stop_reason")}
                for record in item["history"]],
            "controls": None if item.get("worktree_state") is None else {
                key: item["worktree_state"][key] for key in (
                    "checks", "protection_complete", "operations_complete", "gaps")},
            "deep": [{key: value for key, value in record.items()
                      if key not in ("started_unix_ms", "finished_unix_ms")}
                     for record in item["observations"]], "unknowns": item["unknowns"],
        })
    return rows


def measure(binary, fixture, mode, output_path, order="forward", budget="standard"):
    binary_before = digest_file(binary)
    # Saved inventory is produced once per sample from the unchanged family.
    scan = fixture.command([str(binary), "scan", "--repo", str(fixture.repo), "--format", "json"], timeout=120)
    inventory = json.loads(scan)
    if order == "reverse":
        inventory["families"][0]["worktrees"].reverse()
    inventory_path = fixture.root / "benchmark-inventory.json"
    inventory_path.write_text(json.dumps(inventory))
    before = fixture.manifest()
    argv = [str(binary), "plan", "--inventory", str(inventory_path), "--target",
            "refs/heads/integration", "--no-context", "--format", "json", "--color", "auto", "--budget", budget]
    master = slave = None
    if mode == "pty":
        master, slave = pty.openpty()
        # Consistent PTY width; the live formatter separately caps each status at 68 characters.
        import fcntl
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 200, 0, 0))
    selector = selectors.DefaultSelector()
    output = {"stdout": bytearray(), "stderr": bytearray()}
    timings = empty_capture_timings()
    chunks = []
    heads = [head.encode() for head in fixture.heads.values()]
    paths = [os.fsencode(path) for path in fixture.paths]
    started = time.monotonic_ns()
    process = subprocess.Popen(argv, env={**fixture.env, "TERM": "xterm-256color"},
                               stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=slave if slave is not None else subprocess.PIPE,
                               start_new_session=True)
    if slave is not None:
        os.close(slave)
        slave = None
    selector.register(process.stdout, selectors.EVENT_READ, "stdout")
    selector.register(master if master is not None else process.stderr, selectors.EVENT_READ, "stderr")
    try:
        while selector.get_map():
            if (time.monotonic_ns() - started) / 1e9 > 720:
                raise TimeoutError("owned review exceeded 720 second deadline")
            for key, _ in selector.select(timeout=0.1):
                try:
                    chunk = os.read(key.fd, 65536)
                except OSError as error:
                    if master is not None and key.fd == master and error.errno == errno.EIO:
                        chunk = b""
                    else:
                        raise
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                elapsed_ns = time.monotonic_ns() - started
                stream = key.data
                offset = len(output[stream])
                output[stream].extend(chunk)
                if sum(map(len, output.values())) > MAX_OUTPUT:
                    raise RuntimeError("benchmark output cap reached")
                if len(chunks) >= MAX_CAPTURE_CHUNKS:
                    raise RuntimeError("benchmark capture chunk cap reached")
                chunks.append({"stream": stream, "offset_bytes": offset,
                               "length_bytes": len(chunk), "elapsed_ns": elapsed_ns})
                update_capture_timings(output, stream, elapsed_ns, heads, paths, timings)
        returncode = process.wait(timeout=5)
        elapsed = (time.monotonic_ns() - started) / 1e6
        if returncode:
            raise RuntimeError((returncode, bytes(output["stderr"]).decode(errors="replace")))
        binary_after = digest_file(binary)
        if binary_before != binary_after:
            raise AssertionError("frozen binary changed during sample")
        report = json.loads(output["stdout"])
        validate_fixture_rows(report, fixture)
        if report["collection_metrics"]["budget_profile"] != budget:
            raise AssertionError("reported profile differs from requested CLI profile")
        ansi = b"\x1b[" in output["stderr"]
        live_frames = b"\r\x1b[2K" in output["stderr"]
        if b"\x1b[" in output["stdout"] or (mode == "pipe" and ansi) or (mode == "pty" and not live_frames):
            raise AssertionError("pipe/PTY output did not exercise the declared terminal protocol")
        after = fixture.manifest()
        if before != after:
            raise AssertionError("review changed owned fixture bytes/metadata; atime excluded")
        output_path.write_bytes(output["stdout"])
        output_path.with_suffix(".stderr").write_bytes(output["stderr"])
        capture_path = output_path.with_suffix(".capture.json")
        capture = {"schema": "wt-sweep.benchmark-capture/1", "clock": "time.monotonic_ns",
            "fixture_identities": {"heads": [head.decode("ascii") for head in heads],
                                   "paths_hex": [path.hex() for path in paths]},
            "limits": {"output_bytes": MAX_OUTPUT, "chunks": MAX_CAPTURE_CHUNKS,
                       "metadata_bytes": MAX_CAPTURE_METADATA, "timeout_seconds": 720},
            "streams": {stream: {"path": str(path), "sha256": digest_file(path), "bytes": len(output[stream])}
                        for stream, path in (("stdout", output_path), ("stderr", output_path.with_suffix(".stderr")))},
            "chunks": chunks}
        encoded = (json.dumps(capture, separators=(",", ":")) + "\n").encode("utf-8")
        if len(encoded) > MAX_CAPTURE_METADATA:
            raise RuntimeError("benchmark capture metadata cap reached")
        capture_path.write_bytes(encoded)
        if recompute_capture(capture_path, heads, paths) != timings:
            raise AssertionError("offline capture replay differs from live receipt timings")
        phases = (report.get("collection_metrics") or {}).get("phases", [])
        return {"mode": mode, "order": order, "budget_profile": budget, "elapsed_ms": elapsed,
                **timings,
                "first_useful_criterion": "known complete displayed history relation or numeric local proof with identity; unknown statuses and inventory HEADs excluded",
                "capture_path": str(capture_path), "capture_sha256": digest_file(capture_path),
                "capture_chunks": len(chunks), "capture_replay_verified": True,
                "fixture_rows_verified": True,
                "binary_sha256_before": binary_before, "binary_sha256_after": binary_after,
                "terminal_protocol": {"color": "auto", "stderr_is_pty": mode == "pty",
                                      "ansi_present": ansi, "live_ansi_frames": live_frames,
                                      "stdout_plain_json": True},
                "coverage": report["coverage"],
                "history_metrics": report.get("history_metrics"),
                "collection_metrics": report.get("collection_metrics"),
                "measured_git_processes": sum(value for phase in phases
                    for name, value in phase.get("counters", {}).items() if name.endswith("git_processes")),
                "git_process_scope": "collection phase counters; unmeasured discovery/control calls excluded",
                "source_conserved": True, "manifest_digest": stable_digest(before),
                "review_sha256": digest_file(output_path), "review_path": str(output_path),
                "evidence": evidence(report)}
    finally:
        # The exited leader may have left descendants holding capture pipes.
        # This group belongs to this sample's newly created session only.
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=5)
        selector.close()
        process.stdout.close()
        if process.stderr is not None:
            process.stderr.close()
        if master is not None:
            os.close(master)
        if slave is not None:
            os.close(slave)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", action="append", required=True, help="label=absolute frozen executable")
    parser.add_argument("--counts", type=int, nargs="+", default=[50, 100], choices=range(2, 101), metavar="N")
    parser.add_argument("--modes", nargs="+", choices=["pipe", "pty"], default=["pipe", "pty"])
    parser.add_argument("--orders", nargs="+", choices=["forward", "reverse"], default=["forward"])
    parser.add_argument("--samples", type=int, default=1, choices=range(1, 4))
    parser.add_argument("--budget", choices=["standard", "thorough"], default="standard",
                        help="existing product profile, identical for every binary in this campaign")
    parser.add_argument("--ignored-files", type=int, default=0, choices=range(0, 33), metavar="N")
    parser.add_argument("--ignored-bytes", type=int, default=65536)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if not 0 <= args.ignored_bytes <= 1048576:
        parser.error("ignored bytes must be between 0 and 1048576")
    binaries = []
    for entry in args.binary:
        label, name = entry.split("=", 1)
        binary = Path(name).resolve(strict=True)
        if not label or not binary.is_file() or not os.access(binary, os.X_OK):
            parser.error("binary must be a labeled executable file")
        binaries.append((label, binary))
    if len({label for label, _ in binaries}) != len(binaries):
        parser.error("binary labels must be distinct")
    args.out.parent.mkdir(parents=True, exist_ok=True)
    git = subprocess.run(["/usr/bin/git", "--version"], capture_output=True, check=True).stdout.decode().strip()
    result = {"schema": "wt-sweep.benchmark-review/1", "environment": {
        "system": platform.platform(), "machine": platform.machine(), "python": platform.python_version(),
        "git": git, "os_cache": "not cleared or controlled; samples sequential on same unchanged corpus",
        "clock": "time.monotonic_ns", "source_head": subprocess.run(
            ["/usr/bin/git", "rev-parse", "HEAD"], cwd=Path(__file__).resolve().parents[1],
            capture_output=True, check=True).stdout.decode().strip()},
        "corpus": {"history_commits": 600, "distinct_heads": True, "ignored_files_per_worktree": args.ignored_files,
                   "ignored_bytes_per_file": args.ignored_bytes, "synthetic_only": True},
        "binaries": {label: {"path": str(binary), "sha256": digest_file(binary)} for label, binary in binaries},
        "measurement_protocol": {"path": str(Path(__file__).resolve()),
                                 "sha256": digest_file(Path(__file__))},
        "samples": [], "comparisons": []}
    for count in args.counts:
        fixture = ScaleFixture().many_worktrees(count)
        try:
            if args.ignored_files:
                (fixture.repo / ".git/info/exclude").write_bytes(b"bench-ignored-*\n")
                for worktree in fixture.paths:
                    for number in range(args.ignored_files):
                        (worktree / f"bench-ignored-{number:02}").write_bytes(b"x" * args.ignored_bytes)
            corpus_manifest = fixture.manifest()
            for sample in range(args.samples):
                for mode in args.modes:
                    for order in args.orders:
                        reference = None
                        for label, binary in binaries:
                            destination = args.out.parent / f"{args.out.stem}-{label}-{count}-{mode}-{order}-{sample}.json"
                            print(f"collecting {label} count={count} mode={mode} order={order}", flush=True)
                            measured = measure(binary, fixture, mode, destination, order, budget=args.budget)
                            if measured["binary_sha256_before"] != result["binaries"][label]["sha256"]:
                                raise AssertionError("frozen binary differs from initial audit hash")
                            if fixture.manifest() != corpus_manifest:
                                raise AssertionError("source conservation changed between comparable samples")
                            record = {"binary": label, "worktrees": count, "sample": sample, "budget_profile": args.budget,
                                      "measurement_protocol_sha256": result["measurement_protocol"]["sha256"], **measured}
                            # A pair refusal must not erase an independently validated sample's timing.
                            result["samples"].append(record)
                            args.out.write_text(json.dumps(result, indent=2) + "\n")
                            if reference is None:
                                reference = record
                            else:
                                try:
                                    comparison = {"comparable": True, **compare_samples(reference, record)}
                                except AssertionError as failure:
                                    comparison = {"comparable": False, "failure": str(failure),
                                        "worktrees": count, "mode": mode, "order": order, "sample": sample,
                                        "first_binary": reference["binary"], "second_binary": label,
                                        "first_review_path": reference["review_path"], "second_review_path": record["review_path"]}
                                if not comparison["comparable"]:
                                    print("pair comparison refused; exact failure retained in evidence", flush=True)
                                result["comparisons"].append(comparison)
                                args.out.write_text(json.dumps(result, indent=2) + "\n")
                            print(f"finished {label} count={count} elapsed_ms={measured['elapsed_ms']:.1f} "
                                  f"useful={measured['first_useful_ms']}", flush=True)
        finally:
            fixture.close()
    print(args.out)


if __name__ == "__main__":
    main()
