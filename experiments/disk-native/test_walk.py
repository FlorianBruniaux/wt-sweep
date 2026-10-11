"""Behavior tests for the complete experimental walks. No real repository is read."""
import json
import os
import pathlib
import stat
import socket
import selectors
import sys
import time
import subprocess
import tempfile
import unittest

BIN = os.environ.get("SWEEP_NATIVE_PROBE", "/private/tmp/wt-sweep-evaluations-20261011/disk-native/walk")


def read_pause_marker(process, timeout=5):
    deadline = time.monotonic() + timeout
    data = bytearray()
    with selectors.DefaultSelector() as selector:
        selector.register(process.stderr, selectors.EVENT_READ)
        while True:
            left = deadline - time.monotonic()
            if left <= 0 or not selector.select(left):
                raise TimeoutError("probe did not pause before deadline")
            chunk = os.read(process.stderr.fileno(), 256)
            if not chunk:
                raise RuntimeError("probe exited without pause marker")
            data.extend(chunk)
            if len(data) > 256:
                raise RuntimeError("probe pause output exceeds bound")
            if b"\n" in data:
                if bytes(data) != b"paused\n":
                    raise RuntimeError("unexpected probe pause output")
                return bytes(data)


def stop_probe(process):
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=2)
    for pipe in (process.stdin, process.stdout, process.stderr):
        if pipe and not pipe.closed:
            pipe.close()


class HarnessDeadlineTests(unittest.TestCase):
    def test_silent_probe_times_out_and_is_reaped(self):
        p = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(10)"],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            with self.assertRaises(TimeoutError):
                read_pause_marker(p, timeout=0.05)
        finally:
            stop_probe(p)
        self.assertIsNotNone(p.returncode)

    def test_probe_exiting_without_marker_is_rejected_and_reaped(self):
        p = subprocess.Popen([sys.executable, "-c", "raise SystemExit(4)"],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            with self.assertRaises(RuntimeError):
                read_pause_marker(p)
        finally:
            stop_probe(p)
        self.assertEqual(p.returncode, 4)

    def test_partial_marker_cannot_block_past_deadline(self):
        p = subprocess.Popen([sys.executable, "-c",
                              "import sys,time; sys.stderr.write('pa'); sys.stderr.flush(); time.sleep(10)"],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            with self.assertRaises(TimeoutError):
                read_pause_marker(p, timeout=0.2)
        finally:
            stop_probe(p)
        self.assertIsNotNone(p.returncode)


def oracle(roots):
    registered = {os.fsencode(p) for p in roots}
    unique = set()
    total = [0, 0]
    rows = []
    for root in roots:
        seen = set()
        entries = []
        pending = [os.fsencode(root)]
        counts = [0, 0]
        device = os.lstat(root).st_dev
        while pending:
            path = pending.pop()
            st = os.lstat(path)
            assert st.st_dev == device
            key = (st.st_dev, st.st_ino)
            if key in seen:
                continue
            seen.add(key)
            sizes = [st.st_size if stat.S_ISREG(st.st_mode) or stat.S_ISLNK(st.st_mode) else 0, st.st_blocks * 512]
            entries.append([path.hex(), st.st_dev, st.st_ino, *sizes])
            counts = [a + b for a, b in zip(counts, sizes)]
            if key not in unique:
                unique.add(key)
                total = [a + b for a, b in zip(total, sizes)]
            if stat.S_ISDIR(st.st_mode):
                pending.extend(os.path.join(path, p) for p in os.listdir(path)
                               if os.path.join(path, p) not in registered)
        # Different directory orders can choose a different hard-link spelling.
        rows.append({"bytes": counts, "objects": sorted(e[1:] for e in entries),
                     "paths": sorted(e[0] for e in entries)})
    return rows, total


def collect(mode, roots, *options):
    p = subprocess.run([BIN, "--mode", mode, "--manifest", *map(str, options), "--", *map(str, roots)],
                       check=True, capture_output=True, timeout=20)
    return json.loads(p.stdout)


class WalkTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="wt-sweep-native-unit-", dir="/private/tmp")
        self.root = pathlib.Path(self.tmp.name) / "root"
        self.root.mkdir()
        (self.root / "d").mkdir()
        (self.root / "d" / "ignored").write_bytes(b"keep these bytes")
        (self.root / ".hidden").write_bytes(b"hidden")
        (self.root / "file").write_bytes(b"abc")
        os.link(self.root / "file", self.root / "alias")
        os.symlink("d", self.root / "link")
        os.symlink("/outside/missing", self.root / "broken")
        os.mkfifo(self.root / "fifo")
        self.sock = socket.socket(socket.AF_UNIX)
        try:
            self.sock.bind(str(self.root / "socket"))
        except PermissionError as error:
            self.socket_error = str(error)
            self.sock.close()
            self.sock = None
        with (self.root / "sparse").open("wb") as f:
            f.truncate(1024 * 1024)
        with open(str(self.root / "file") + "/..namedfork/rsrc", "wb") as fork:
            fork.write(b"resource-fork" * 500)
        raw = os.fsencode(self.root) + b"/name-\xff\n"
        try:
            fd = os.open(raw, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        except OSError as error:
            if error.errno != 92:
                raise
            self.raw_error = str(error)
            self.raw_name = False
        else:
            os.write(fd, b"raw"); os.close(fd); self.raw_name = True

    def tearDown(self):
        if self.sock:
            self.sock.close()
        self.tmp.cleanup()

    def test_complete_collectors_match_independent_oracle(self):
        expected, total = oracle([self.root])
        calls = {}
        for mode in ("scalar", "native"):
            result = collect(mode, [self.root])
            calls[mode] = result["calls"]
            self.assertTrue(result["complete"], result)
            self.assertEqual(result["unique_bytes"], total)
            row = result["roots"][0]
            self.assertEqual(row["bytes"], expected[0]["bytes"])
            self.assertEqual(sorted(e[1:] for e in row["objects"]), expected[0]["objects"])
            names = {bytes.fromhex(e[0]) for e in row["objects"]}
            self.assertIn(os.fsencode(self.root / "fifo"), names)
            self.assertNotIn(os.fsencode(self.root / "link" / "ignored"), names)
            if self.raw_name:
                self.assertIn(os.fsencode(self.root) + b"/name-\xff\n", names)
        self.assertGreater(calls["native"]["bulk"], 0)
        self.assertLess(calls["native"]["stat"], calls["scalar"]["stat"])

    def test_registered_nested_roots_and_global_hardlinks_are_deduplicated(self):
        roots = [self.root, self.root / "d"]
        os.link(self.root / "file", self.root / "d" / "cross-root")
        expected, total = oracle(roots)
        for mode in ("scalar", "native"):
            result = collect(mode, roots)
            self.assertTrue(result["complete"], result)
            self.assertEqual(result["unique_bytes"], total)
            self.assertEqual([r["bytes"] for r in result["roots"]], [r["bytes"] for r in expected])

    def test_limits_refuse_complete_claim(self):
        for mode in ("scalar", "native"):
            for options, reason in ((["--max-entries", 2], "entry budget"),
                                    (["--max-depth", 0], "depth budget"),
                                    (["--timeout-ms", 0], "time budget")):
                result = collect(mode, [self.root], *options)
                self.assertFalse(result["complete"], result)
                self.assertIn(reason, " ".join(result["roots"][0]["gaps"]))
            result = collect(mode, [self.root], "--max-entries", 2)
            self.assertLessEqual(result["entries_checked"], 2)

    def test_root_symlink_is_refused(self):
        root = pathlib.Path(self.tmp.name) / "root-link"
        root.symlink_to(self.root)
        for mode in ("scalar", "native"):
            result = collect(mode, [root])
            self.assertFalse(result["complete"])
            self.assertIsNone(result["roots"][0]["bytes"])

    def test_socket_metadata_is_accounted_without_opening_content(self):
        if not self.sock:
            self.skipTest("host forbids owned Unix socket fixture: " + self.socket_error)
        for mode in ("scalar", "native"):
            result = collect(mode, [self.root])
            self.assertTrue(result["complete"], result)
            self.assertIn(os.fsencode(self.root / "socket").hex(),
                          {e[0] for e in result["roots"][0]["objects"]})

    def test_nonutf8_runtime_path_bytes_are_preserved_when_volume_supports_them(self):
        if not self.raw_name:
            self.skipTest("owned APFS fixture refuses non-UTF8 name: " + self.raw_error)
        for mode in ("scalar", "native"):
            result = collect(mode, [self.root])
            self.assertIn((os.fsencode(self.root) + b"/name-\xff\n").hex(),
                          {e[0] for e in result["roots"][0]["objects"]})

    def test_external_directory_replacement_is_detected_before_descent(self):
        self._mutation("--pause-child", "d", lambda: (os.rename(self.root / "d", self.root / "saved"),
                                                       os.symlink("saved", self.root / "d")), "directory")

    def test_external_root_replacement_preserves_fd_anchor_and_refuses_complete(self):
        self._mutation("--pause-root", None, lambda: (os.rename(self.root, self.root.with_name("saved")),
                                                      self.root.mkdir()), "root changed")

    def test_external_late_entry_is_not_reported_as_complete(self):
        self._mutation("--pause-end", None, lambda: (self.root / "late").write_bytes(b"late"), "directory changed")

    def _mutation(self, flag, name, mutate, reason):
        # Each mode needs a new fixture so one mutation cannot contaminate the other.
        for mode in ("scalar", "native"):
            if mode == "native":
                self.tearDown(); self.setUp()
            argv = [BIN, "--mode", mode, flag]
            if name is not None:
                argv.append(name)
            p = subprocess.Popen([*argv, "--", str(self.root)], stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                self.assertEqual(read_pause_marker(p), b"paused\n")
                mutate()
                out, err = p.communicate(b"continue\n", timeout=10)
                self.assertEqual(p.returncode, 0, err)
                result = json.loads(out)
                self.assertFalse(result["complete"], result)
                self.assertIn(reason, " ".join(result["roots"][0]["gaps"]))
            finally:
                stop_probe(p)


if __name__ == "__main__":
    unittest.main()
