"""E1 negative witnesses. Only mutate a uniquely owned /private/tmp fixture.

No scan, cleanup of real worktrees, exclusion guarantee or backup implementation.
Children exchange READY/GO/DONE messages; deadlines bound waits, never order them.
"""

import contextlib
import fcntl
import hashlib
import json
import multiprocessing
import os
from pathlib import Path
import platform
import shutil
import stat
import subprocess
import tempfile


def git(root, *args, raw=False):
    executable = shutil.which("git")
    if executable is None:
        raise RuntimeError("Git unavailable")
    env = {
        "PATH": "/usr/bin:/bin",
        "HOME": str(root / "home"),
        "XDG_CONFIG_HOME": str(root / "home"),
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_NO_REPLACE_OBJECTS": "1",
        "GIT_NO_LAZY_FETCH": "1",
        "GIT_AUTHOR_NAME": "E1 fixture",
        "GIT_AUTHOR_EMAIL": "e1@example.invalid",
        "GIT_COMMITTER_NAME": "E1 fixture",
        "GIT_COMMITTER_EMAIL": "e1@example.invalid",
        "LC_ALL": "C",
    }
    result = subprocess.run(
        [executable, "-c", "core.hooksPath=" + str(root / "empty"),
         "-c", "core.fsmonitor=false", "-c", "commit.gpgSign=false",
         "-c", "protocol.allow=never", "-C", str(root / "repo"), *args],
        env=env, stdin=subprocess.DEVNULL, capture_output=True, timeout=10,
        check=True,
    )
    if len(result.stdout) + len(result.stderr) > 65536:
        raise RuntimeError("Unexpected fixture Git output size")
    return result.stdout if raw else result.stdout.strip().decode("ascii")


def receive(channel):
    if not channel.poll(15):
        raise RuntimeError("Child handshake timeout")
    return channel.recv()


def writer(channel, root_string, action):
    root = Path(root_string)
    os.chdir(root / "outside")
    try:
        if action == "fd":
            with (root / "target" / "data").open("r+b", buffering=0) as handle:
                channel.send(("READY", str(Path.cwd())))
                assert receive(channel) == "GO"
                refused = False
                try:
                    fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    refused = True
                handle.write(b"after!")
                os.fsync(handle.fileno())
                channel.send(("DONE", refused))
        elif action == "stat":
            path = root / "same-stat"
            before = path.stat()
            channel.send(("READY", None))
            assert receive(channel) == "GO"
            path.write_bytes(b"second")
            os.utime(path, ns=(before.st_atime_ns, before.st_mtime_ns))
            channel.send(("DONE", None))
        elif action == "commit":
            channel.send(("READY", None))
            assert receive(channel) == "GO"
            (root / "repo" / "late").write_bytes(b"late detached data\n")
            git(root, "add", "--", "late")
            git(root, "commit", "-m", "late detached commit")
            channel.send(("DONE", git(root, "rev-parse", "HEAD")))
        else:
            raise ValueError(action)
    except BaseException as error:
        channel.send(("ERROR", repr(error)))
        raise
    finally:
        channel.close()


@contextlib.contextmanager
def child(root, action):
    context = multiprocessing.get_context("spawn")
    parent, remote = context.Pipe()
    process = context.Process(target=writer, args=(remote, str(root), action))
    process.start()
    remote.close()
    try:
        ready = receive(parent)
        assert ready[0] == "READY", ready
        yield parent, ready[1]
        process.join(15)
        assert process.exitcode == 0, process.exitcode
    finally:
        parent.close()
        if process.is_alive():
            process.terminate()
        process.join(5)
        if process.is_alive():
            process.kill()
            process.join(5)
        assert not process.is_alive(), "Child remains alive"
        process.close()


def metadata(root):
    regular = root / "metadata-file"
    regular.write_bytes(b"metadata specimen")
    symbolic = root / "symbolic"
    symbolic.symlink_to(regular.name)
    hard = root / "hard"
    os.link(regular, hard)
    fifo = root / "fifo"
    os.mkfifo(fifo)
    assert os.readlink(symbolic) == regular.name
    assert os.path.samefile(regular, hard)
    assert stat.S_ISFIFO(fifo.lstat().st_mode)
    result = {
        "modes": {"observed": oct(stat.S_IMODE(regular.stat().st_mode))},
        "symlinks": {"observed": os.readlink(symbolic)},
        "hardlinks": {"observed_same_inode": True, "nlink": regular.stat().st_nlink},
        "acl": {"inspection": "UNKNOWN", "reason": "No ACL capture/restore implemented"},
        "special_types": {"fifo_observed": True, "others": "UNKNOWN"},
    }
    try:
        result["xattrs"] = {"observed_names": os.listxattr(regular),
                            "nonempty_roundtrip": "UNKNOWN"}
    except (OSError, AttributeError) as error:
        result["xattrs"] = {"inspection": "UNKNOWN", "reason": type(error).__name__}
    for item in result.values():
        item["restoration"] = "UNKNOWN"
    return result


def run():
    with tempfile.TemporaryDirectory(prefix="wt-sweep-e1-", dir="/private/tmp") as directory:
        root = Path(directory)
        for name in ("outside", "home", "empty", "target", "repo"):
            (root / name).mkdir()
        result = {"decision": "NON_DEMONTREE", "platform": platform.platform(),
                  "process_visibility": "UNKNOWN", "durability": "UNKNOWN"}
        data = root / "target" / "data"
        data.write_bytes(b"before")
        with data.open("r+b") as mutex:
            fcntl.flock(mutex, fcntl.LOCK_EX)
            with child(root, "fd") as (channel, cwd):
                (root / "target").rename(root / "renamed")
                channel.send("GO")
                done, refused = receive(channel)
                assert done == "DONE", (done, refused)
                result["fd_after_rename"] = {
                    "outside_cwd": Path(cwd) == root / "outside",
                    "cooperative_lock_refused": refused,
                    "write_succeeded": (root / "renamed" / "data").read_bytes() == b"after!",
                }
                # An injected restricted observer sees no processes, while the
                # owned writer is independently proven by its write and handshake.
                result["partial_process_view"] = {
                    "injected_visible_pids": [], "writer_observed_by_handshake": True,
                    "empty_view_despite_write": result["fd_after_rename"]["write_succeeded"],
                }
        same = root / "same-stat"
        same.write_bytes(b"before")
        with child(root, "stat") as (channel, _):
            before = same.stat()
            digest = hashlib.sha256(same.read_bytes()).hexdigest()
            channel.send("GO")
            assert receive(channel)[0] == "DONE"
            after = same.stat()
            result["same_stat"] = {
                "same_size_and_mtime": (before.st_size, before.st_mtime_ns)
                == (after.st_size, after.st_mtime_ns),
                "different_bytes": digest != hashlib.sha256(same.read_bytes()).hexdigest(),
            }
        git(root, "init", "--template=" + str(root / "empty"), "-b", "sentinel")
        git(root, "commit", "--allow-empty", "-m", "initial")
        git(root, "checkout", "--detach")
        with child(root, "commit") as (channel, _):
            captured = git(root, "rev-parse", "HEAD")
            channel.send("GO")
            done, late = receive(channel)
            assert done == "DONE", (done, late)
            result["late_detached_commit"] = {
                "captured_oid": captured, "late_oid": late,
                "head_changed": captured != late and git(root, "rev-parse", "HEAD") == late,
                "branch_unchanged": git(root, "rev-parse", "refs/heads/sentinel") == captured,
                "object_readable": git(root, "cat-file", "-t", late) == "commit"
                and git(root, "show", late + ":late") == "late detached data",
            }
        result["metadata"] = metadata(root)
        return result


if __name__ == "__main__":
    print(json.dumps(run(), indent=2, sort_keys=True))
