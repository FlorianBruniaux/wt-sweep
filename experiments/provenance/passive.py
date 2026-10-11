"""Bounded context-only reader for the inspected Aristote schema-v1 format.

Explicit paths only. The source has no current-worktree instance binding.
Pre/post samples detect some mutations; they are not an atomic snapshot.
"""

from datetime import datetime, timezone
import errno
import hashlib
import json
import os
from pathlib import Path
import re
import stat

CARD_BYTES = 65536
INDEX_BYTES = 1048576
ENTRY_LIMIT = 128
RELATION_LIMIT = 32
FRESHNESS_SECONDS = 86400
UUID = r"[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}"


class Refusal(Exception):
    def __init__(self, code):
        self.code = code


def require(condition, code="INVALID_FORMAT"):
    if not condition:
        raise Refusal(code)


def shape(value, required, optional=()):
    require(isinstance(value, dict) and set(required) <= set(value) <= set(required) | set(optional))


def text(value, maximum=4096):
    return (isinstance(value, str) and 0 < len(value) <= maximum
            and not re.search(r"[\x00-\x1f\ud800-\udfff]", value))


def instant(value):
    require(isinstance(value, str) and re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{3}Z", value))
    try:
        return datetime.strptime(value, "%Y-%m-%dT%H:%M:%S.%fZ").replace(tzinfo=timezone.utc)
    except ValueError as error:
        raise Refusal("INVALID_FORMAT") from error


def timestamp(value):
    if value is not None:
        instant(value)


def validate_card(value):
    """Accept the inspected format, with tighter finite list/date/UTF-8 limits."""
    shape(value, ["schemaVersion", "id", "repository", "path", "initial", "description", "creator",
                  "sessions", "prs", "timestamps", "dependencies", "revision", "updatedAt"])
    require(type(value["schemaVersion"]) is int and value["schemaVersion"] == 1)
    require(type(value["revision"]) is int and value["revision"] >= 1)
    for field, cap in [("id", 100), ("path", 4096), ("description", 200)]:
        require(text(value[field], cap))
    shape(value["repository"], ["commonDir"])
    require(text(value["repository"]["commonDir"]))
    shape(value["initial"], ["branch", "head"])
    require(text(value["initial"]["branch"], 300) and text(value["initial"]["head"], 100))
    shape(value["creator"], ["tool", "sessionId", "evidence"])
    creator = value["creator"]
    require(creator["tool"] in ["claude", "codex", "human", "UNKNOWN"])
    require(creator["sessionId"] is None or text(creator["sessionId"], 200))
    require(creator["evidence"] in ["UNKNOWN", "explicit", "entrypoint-declaration"])
    shape(value["timestamps"], ["createdAt", "creationSource", "registeredAt", "lastAgentInterventionAt",
                                "lastLocalChangeObservedAt", "lastCommitAt", "lastAuditAt"])
    for key, date in value["timestamps"].items():
        if key == "creationSource":
            require(date in ["UNKNOWN", "entrypoint-declaration"])
        else:
            timestamp(date)
    instant(value["updatedAt"])
    dependencies = value["dependencies"]
    shape(dependencies, ["entrypoint", "requested", "observation"])
    require(text(dependencies["entrypoint"], 200) and dependencies["requested"] in ["shared", "isolated", "UNKNOWN"])
    shape(dependencies["observation"], ["state", "target", "observedAt"])
    observation = dependencies["observation"]
    require(observation["state"] in ["symlink", "directory", "absent", "other"])
    require(observation["target"] is None or text(observation["target"]))
    timestamp(observation["observedAt"])
    for key in ["sessions", "prs"]:
        require(isinstance(value[key], list))
        require(len(value[key]) <= RELATION_LIMIT, "RELATION_LIMIT")
    for session in value["sessions"]:
        shape(session, ["tool", "sessionId", "evidence", "firstObservedAt", "lastObservedAt"], ["parent", "resumeLink"])
        require(session["tool"] in ["claude", "codex", "human"] and text(session["sessionId"], 200))
        require(session["evidence"] in ["explicit", "runtime"])
        require(instant(session["firstObservedAt"]) <= instant(session["lastObservedAt"]))
        if "parent" in session:
            parent = session["parent"]
            shape(parent, ["tool", "sessionId", "evidence"])
            require(parent["tool"] in ["claude", "codex", "human"] and text(parent["sessionId"], 200))
            require(parent["evidence"] == "explicit")
            require((parent["tool"], parent["sessionId"]) != (session["tool"], session["sessionId"]))
        if "resumeLink" in session:
            from urllib.parse import urlsplit
            link = session["resumeLink"]
            shape(link, ["url", "evidence"])
            require(text(link["url"], 2048) and link["evidence"] == "explicit")
            url = urlsplit(link["url"])
            require(url.scheme == "https" and bool(url.hostname) and not (url.username or url.password or url.query or url.fragment))
    for pr in value["prs"]:
        shape(pr, ["repository", "number", "role", "head", "evidence", "url", "attachedAt"])
        require(isinstance(pr["repository"], str) and re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", pr["repository"]))
        require(type(pr["number"]) is int and 0 < pr["number"] <= 9007199254740991)
        require(pr["role"] in ["primary", "historical", "replaced", "reference"])
        require(isinstance(pr["head"], str) and re.fullmatch(r"[a-f0-9]{40,64}", pr["head"]))
        require(pr["evidence"] in ["explicit", "github-exact-head", "creation-result"])
        require(pr["url"] == f'https://github.com/{pr["repository"]}/pull/{pr["number"]}')
        timestamp(pr["attachedAt"])
    return value


def signature(metadata):
    return (metadata.st_dev, metadata.st_ino, metadata.st_size, metadata.st_mtime_ns, metadata.st_ctime_ns)


def duplicates(pairs):
    value = {}
    for key, item in pairs:
        require(key not in value, "DUPLICATE_KEY")
        value[key] = item
    return value


def invalid_constant(_):
    raise Refusal("INVALID_FORMAT")


def read_json(path, limit, after_read=None):
    require(path.parent.resolve() == path.parent, "UNSAFE_SOURCE")
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise Refusal("UNSAFE_SOURCE") from error
        raise
    with os.fdopen(descriptor, "rb") as stream:
        before = os.fstat(stream.fileno())
        require(stat.S_ISREG(before.st_mode), "UNSAFE_SOURCE")
        require(before.st_size <= limit, "SOURCE_TOO_LARGE")
        raw = stream.read(limit + 1)
        require(len(raw) <= limit, "SOURCE_TOO_LARGE")
        if after_read:
            after_read()
        require(signature(before) == signature(os.fstat(stream.fileno()))
                == signature(path.lstat()), "SOURCE_CHANGED")
    try:
        value = json.loads(raw.decode("utf-8"), object_pairs_hook=duplicates, parse_constant=invalid_constant)
    except (ValueError, UnicodeError, RecursionError) as error:
        raise Refusal("INVALID_FORMAT") from error
    return value, hashlib.sha256(raw).hexdigest()


def fresh(date, now):
    age = (instant(now) - instant(date)).total_seconds()
    return {"state": "future" if age < 0 else "stale" if age > FRESHNESS_SECONDS else "recent", "ageSeconds": age}


def empty(source):
    return {"source": source, "state": "absent", "error": None, "origin": None,
            "identity": {"currentWorktree": "UNKNOWN", "reason": "NO_INSTANCE_BINDING"},
            "freshness": {"state": "UNKNOWN", "ageSeconds": None}, "contradictions": []}


def finish(observation, value, path, common_dir, now, date):
    validate_card(value)
    contradictions = observation["contradictions"]
    if value["path"] != path:
        contradictions.append("PATH_MISMATCH")
    if value["repository"]["commonDir"] != common_dir:
        contradictions.append("REPOSITORY_MISMATCH")
    observation.update(state="present", origin={key: value[key] for key in
                       ["id", "path", "repository", "creator", "sessions", "timestamps", "initial", "description", "revision"]},
                       freshness=fresh(date, now))
    observation["source"]["updatedAt"] = value["updatedAt"]
    if "PATH_MISMATCH" in contradictions or "REPOSITORY_MISMATCH" in contradictions:
        observation["identity"] = {"currentWorktree": "contradicted", "reason": "DECLARED_LOCATION_MISMATCH"}


def failed(observation, error):
    if isinstance(error, FileNotFoundError):
        observation["error"] = "SOURCE_ABSENT"
    else:
        code = (error.code if isinstance(error, Refusal) else "READ_UNAVAILABLE"
                if isinstance(error, OSError) else "INVALID_FORMAT")
        observation.update(state="changed" if code in ["SOURCE_CHANGED", "GENERATION_CHANGED"]
                           else "unavailable" if code == "READ_UNAVAILABLE" else "invalid",
                           error=code, origin=None)


def read_context(path, common_dir, *, main=None, now=None, after_read=None):
    """Read one card and optionally one explicit main registry, never discover.

    after_read is an experiment fault-injection seam, called after source bytes
    are read but before pre/post checks. No source writers run by default.
    """
    now = now or datetime.now(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")
    instant(now)
    result = {"observations": [], "contradictions": [], "authorizesRemoval": False,
              "decisionEvidence": [], "usage": "UNKNOWN", "freshnessThresholdSeconds": FRESHNESS_SECONDS}
    source = {"kind": "aristote-card-v1", "path": str(Path(path) / ".worktree.json"), "capturedAt": now}
    observation = empty(source)
    result["observations"].append(observation)
    try:
        require(all(text(item) and os.path.isabs(item) for item in [path, common_dir]), "UNSUPPORTED_PATH_BYTES")
        value, digest = read_json(Path(source["path"]), CARD_BYTES, after_read if main is None else None)
        source["sha256"] = digest
        finish(observation, value, path, common_dir, now, value["updatedAt"])
    except (Refusal, OSError, TypeError, KeyError, ValueError, RecursionError) as error:
        failed(observation, error)
    if main is not None:
        source = {"kind": "aristote-generation-v1", "path": str(Path(main) / ".worktrees/registry/current"), "capturedAt": now}
        observation = empty(source)
        result["observations"].append(observation)
        try:
            require(text(main) and os.path.isabs(main), "UNSUPPORTED_PATH_BYTES")
            root = Path(main) / ".worktrees/registry"
            require(root.resolve() == root, "UNSAFE_GENERATION")
            require(stat.S_ISDIR(root.stat().st_mode), "UNSAFE_GENERATION")
            current = root / "current"
            target = os.readlink(current)
            require(re.fullmatch("generations/" + UUID, target), "UNSAFE_GENERATION")
            pointer_before = signature(current.lstat())
            source["generation"] = target.split("/")[1]
            source["documentPath"] = str(root / target / "index.json")
            index, digest = read_json(Path(source["documentPath"]), INDEX_BYTES, after_read)
            source["sha256"] = digest
            try:
                stable = os.readlink(current) == target and signature(current.lstat()) == pointer_before
            except OSError as error:
                raise Refusal("GENERATION_CHANGED") from error
            require(stable, "GENERATION_CHANGED")
            shape(index, ["schemaVersion", "generation", "observedAt", "complete", "repository", "entries"])
            require(type(index["schemaVersion"]) is int and index["schemaVersion"] == 1)
            require(index["generation"] == source["generation"], "GENERATION_MISMATCH")
            require(type(index["complete"]) is bool and text(index["repository"]))
            require(isinstance(index["entries"], list))
            require(len(index["entries"]) <= ENTRY_LIMIT, "ENTRY_LIMIT")
            source["observedAt"] = index["observedAt"]
            source["complete"] = index["complete"]
            observation["publicationFreshness"] = fresh(index["observedAt"], now)
            require(all(isinstance(row, dict) and text(row.get("path")) for row in index["entries"]))
            rows = [row for row in index["entries"] if row["path"] == path]
            require(len(rows) <= 1, "AMBIGUOUS_PATH")
            if rows and rows[0].get("card") is not None:
                problems = rows[0].get("problems")
                require(isinstance(problems, list) and len(problems) <= RELATION_LIMIT and all(text(item, 200) for item in problems))
                observation["contradictions"] = list(problems)
                if index["repository"] != common_dir:
                    observation["contradictions"].append("INDEX_REPOSITORY_MISMATCH")
                value = rows[0]["card"]
                finish(observation, value, path, common_dir, now, value["updatedAt"])
            else:
                observation["error"] = "CARD_ABSENT" if rows else "PATH_NOT_RECORDED"
        except (Refusal, OSError, TypeError, KeyError, ValueError, RecursionError) as error:
            failed(observation, error)
    origins = [row["origin"] for row in result["observations"] if row["origin"] is not None]
    if len(origins) == 2 and origins[0] != origins[1]:
        result["contradictions"].append("SOURCE_DISAGREEMENT")
    return result
