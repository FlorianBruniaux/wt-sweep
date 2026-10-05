#!/usr/bin/env python3
"""Measure two native E1 candidates on exclusively owned temporary files.

No permissions/flags, mounts, global settings or foreign processes are changed.
This is a capability probe, not an exclusion or restoration implementation.
Darwin constants are from the installed SDK's sys/fcntl.h.
"""
import ctypes
import errno
import fcntl
import json
import os
from pathlib import Path
import platform
import tempfile


def observed(operation):
    try:
        return {"result": operation(), "errno": None}
    except OSError as error:
        return {"result": None, "errno": error.errno,
                "error_name": errno.errorcode.get(error.errno, "UNKNOWN")}


def run():
    if platform.system() != "Darwin":
        return {"decision": "UNSUPPORTED", "reason": "Darwin-only capability probe"}
    report = {"decision": "NON_DEMONTREE", "platform": platform.platform(),
              "effective_uid": os.geteuid(), "exclusion_proven": False}
    with tempfile.TemporaryDirectory(prefix="wt-sweep-e1-native-", dir="/private/tmp") as root:
        path = Path(root) / "regular-file"
        fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
        try:
            os.write(fd, b"before")
            # F_SETLEASE=106, F_GETLEASE=107, F_WRLCK=3, F_UNLCK=2.
            report["write_lease"] = observed(lambda: fcntl.fcntl(fd, 106, 3))
            report["lease_readback"] = observed(lambda: fcntl.fcntl(fd, 107))
            if report["write_lease"]["errno"] is None:
                report["lease_release"] = observed(lambda: fcntl.fcntl(fd, 106, 2))
                if report["lease_release"]["errno"] is not None:
                    raise RuntimeError("fixture lease release failed; descriptor will close")
            libc = ctypes.CDLL(None, use_errno=True)
            libc.revoke.argtypes = [ctypes.c_char_p]
            libc.revoke.restype = ctypes.c_int

            def revoke_regular():
                ctypes.set_errno(0)
                result = libc.revoke(os.fsencode(path))
                if result != 0:
                    number = ctypes.get_errno()
                    raise OSError(number, os.strerror(number))
                return result

            report["revoke_regular"] = observed(revoke_regular)
            report["existing_fd_write_after_revoke"] = observed(
                lambda: os.pwrite(fd, b"after!", 0))
            report["existing_fd_remains_writable"] = (
                report["existing_fd_write_after_revoke"].get("result") == 6
                and os.pread(fd, 6, 0) == b"after!")
        finally:
            os.close(fd)
    return report


if __name__ == "__main__":
    print(json.dumps(run(), sort_keys=True, indent=2))
