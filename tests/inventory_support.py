"""Independent U1 fixture builders. No product implementation is imported."""

import os
import hashlib
from pathlib import Path
import shutil
import subprocess
import stat
import sys
import tempfile


REPO_ROOT = Path(__file__).resolve().parents[1]
BINARY = REPO_ROOT / "target/debug/wt-sweep"
GIT = "/usr/bin/git"


def clean_environment(root):
    return {
        "PATH": "/opt/homebrew/bin:/usr/bin:/bin",
        "HOME": str(root),
        "XDG_CONFIG_HOME": str(root / "no-user-config"),
        "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_AUTHOR_NAME": "Fixture",
        "GIT_AUTHOR_EMAIL": "fixture@example.invalid",
        "GIT_COMMITTER_NAME": "Fixture",
        "GIT_COMMITTER_EMAIL": "fixture@example.invalid",
        "LC_ALL": "C",
    }


class FamilyFixture:
    def __init__(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="wt-sweep-u1-oracle-", dir="/private/tmp")
        self.root = Path(self.temporary.name)
        self.env = clean_environment(self.root)
        self.principal = self.root / "principal"
        self.git("init", "-b", "main", str(self.principal))
        (self.principal / "tracked.txt").write_bytes(b"synthetic fixture\n")
        self.git("-C", str(self.principal), "add", "tracked.txt")
        self.git("-C", str(self.principal), "commit", "-m", "Fixture")
        self.head = self.git("-C", str(self.principal), "rev-parse", "HEAD").stdout.decode().strip()
        self.linked = self.root / "linked branch"
        self.detached = self.root / "detached"
        self.missing = self.root / "registered missing"
        self.git("-C", str(self.principal), "worktree", "add", "-b", "topic", str(self.linked))
        self.git("-C", str(self.principal), "worktree", "add", "--detach", str(self.detached))
        self.git("-C", str(self.principal), "worktree", "add", "-b", "missing", str(self.missing))
        self.heads = {os.fsencode(self.principal): self.head, os.fsencode(self.missing): self.head}
        for path, payload in ((self.linked, b"linked commit\n"), (self.detached, b"detached commit\n")):
            (path / "tracked.txt").write_bytes(payload)
            self.git("-C", str(path), "add", "tracked.txt")
            self.git("-C", str(path), "commit", "-m", "Independent fixture head")
            self.heads[os.fsencode(path)] = self.git("-C", str(path), "rev-parse", "HEAD").stdout.decode().strip()
        shutil.rmtree(self.missing)
        self.expected = {
            os.fsencode(self.principal): ("refs/heads/main", False, "present"),
            os.fsencode(self.linked): ("refs/heads/topic", False, "present"),
            os.fsencode(self.detached): (None, True, "present"),
            os.fsencode(self.missing): ("refs/heads/missing", False, "absent"),
        }
        self.manifest_roots = set(self.expected)

    def git(self, *args):
        # Fixture commands must not read the caller's terminal (pack-objects
        # reads revision input even with --all). Payload readers pass input explicitly.
        return subprocess.run([GIT, *args], env=self.env, stdin=subprocess.DEVNULL,
                              capture_output=True, check=True, timeout=15)

    def scan(self, *args, repo=None, env=None):
        return subprocess.run([str(BINARY), "scan", "--repo", str(repo or self.principal), *args],
                              env=env or self.env, capture_output=True, timeout=20)

    def add_worktree(self, raw_name, branch):
        path = self.root / os.fsdecode(raw_name)
        self.git("-C", str(self.principal), "worktree", "add", "-b", branch, str(path))
        self.expected[os.fsencode(path)] = ("refs/heads/" + branch, False, "present")
        self.heads[os.fsencode(path)] = self.head
        self.manifest_roots.add(os.fsencode(path))
        return path

    def install_witnesses(self):
        """Witnesses would leave a marker if executable Git configuration ran."""
        self.witness_marker = self.root / "EXECUTED"
        witness = self.root / "executable-witness"
        script = b"#!/bin/sh\nprintf 'executed\\n' >> " + os.fsencode(self.witness_marker) + b"\ncat\n"
        descriptor = os.open(witness, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o700)
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(script)
        for key, value in (
            ("core.fsmonitor", str(witness)),
            ("filter.witness.clean", str(witness)),
            ("filter.witness.smudge", str(witness)),
            ("filter.witness.required", "true"),
        ):
            self.git("-C", str(self.principal), "config", key, value)
        for path in (self.principal, self.linked, self.detached):
            (path / ".gitattributes").write_bytes(b"*.txt filter=witness\n")
            (path / ".gitignore").write_bytes(b"ignored/\n")
            (path / "ignored").mkdir()
            (path / "ignored" / "payload").write_bytes(b"preserve ignored bytes\x00\xff")
            (path / ".hidden").write_bytes(b"hidden bytes\n")
            os.symlink("tracked.txt", path / "link")
            weird = os.fsencode(path) + b"/untracked-utf8-\xc3\xa9\n"
            with open(weird, "wb") as stream:
                stream.write(b"raw byte filename\n")
        self.xattr_seeded = False
        if hasattr(os, "setxattr"):
            try:
                name = "com.wt-sweep.fixture" if sys.platform == "darwin" else "user.wt_sweep_fixture"
                os.setxattr(self.principal / ".hidden", name, b"synthetic attribute")
                self.xattr_seeded = True
            except OSError:
                # Host may prohibit xattrs. Existing readable xattrs are still compared.
                pass

    def target_manifest(self):
        return {raw: filesystem_manifest(raw) for raw in self.manifest_roots | set(self.expected)}

    def close(self):
        self.temporary.cleanup()


def decode_path(value):
    return bytes.fromhex(value["hex"])


class HistoryFixture(FamilyFixture):
    """Two real worktrees with independent commit clocks for U3 histories."""
    def __init__(self, files=None, object_format=None):
        self.temporary = tempfile.TemporaryDirectory(prefix="wt-sweep-u3-oracle-", dir="/private/tmp")
        self.root = Path(self.temporary.name)
        self.env = clean_environment(self.root)
        self.principal = self.root / "principal"
        self.linked = self.root / "source branch"
        self.clock = 1700000000
        arguments = ["init", "-b", "main"]
        if object_format:
            arguments.append("--object-format=" + object_format)
        self.git(*arguments, str(self.principal))
        for name, payload in (files or {"tracked.txt": b"base\n"}).items():
            (self.principal / name).write_bytes(payload)
        self.commit(self.principal, "Base", *(files or {"tracked.txt": b""}))
        self.base = self.oid(self.principal)
        self.git("-C", str(self.principal), "worktree", "add", "-b", "topic", str(self.linked))
        self.head = self.base
        self.heads = {os.fsencode(self.principal): self.base, os.fsencode(self.linked): self.base}
        self.expected = {
            os.fsencode(self.principal): ("refs/heads/main", False, "present"),
            os.fsencode(self.linked): ("refs/heads/topic", False, "present"),
        }
        self.manifest_roots = set(self.expected)

    def tick(self):
        self.clock += 1
        date = f"{self.clock} +0000"
        self.env.update(GIT_AUTHOR_DATE=date, GIT_COMMITTER_DATE=date)

    def commit(self, path, message, *filenames):
        self.tick()
        self.git("-C", str(path), "add", "--", *filenames)
        self.git("-C", str(path), "commit", "-m", message)
        return self.oid(path)

    def oid(self, path, revision="HEAD"):
        return self.git("-C", str(path), "rev-parse", revision).stdout.decode().strip()

    def write_commit(self, path, name, payload):
        (path / name).write_bytes(payload)
        return self.commit(path, "Synthetic history", name)


def filesystem_manifest(root):
    """Hash every entry without Git; exclude atime because ordinary reads change it."""
    entries = {}

    def visit(path):
        try:
            metadata = os.lstat(path)
        except OSError as error:
            entries[path] = ("absent",) if isinstance(error, FileNotFoundError) else ("read_error", error.errno)
            return
        mode = metadata.st_mode
        attributes = None
        if hasattr(os, "listxattr"):
            attributes = tuple(sorted((os.fsencode(name), os.getxattr(path, name, follow_symlinks=False))
                                      for name in os.listxattr(path, follow_symlinks=False)))
        if stat.S_ISREG(mode):
            with open(path, "rb") as stream:
                content = hashlib.file_digest(stream, "sha256").hexdigest()
        elif stat.S_ISLNK(mode):
            content = os.readlink(path)
        else:
            content = None
        entries[path] = (mode, metadata.st_uid, metadata.st_gid, metadata.st_size,
                         metadata.st_dev, metadata.st_ino, metadata.st_nlink,
                         metadata.st_mtime_ns, metadata.st_ctime_ns,
                         getattr(metadata, "st_flags", None), attributes, content)
        if stat.S_ISDIR(mode):
            with os.scandir(path) as children:
                names = sorted(child.name for child in children)
            for name in names:
                visit(os.path.join(path, name))

    visit(os.fsencode(root))
    return entries
