"""Synthetic schema-v1 cards, never copied from personal registry payloads."""

import json
from pathlib import Path

DATE = "2026-10-11T12:00:00.000Z"
GENERATION = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
NEXT_GENERATION = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"


def card(path, common_dir):
    return {
        "schemaVersion": 1,
        "id": "11111111-1111-4111-8111-111111111111",
        "repository": {"commonDir": str(common_dir)},
        "path": str(path),
        "initial": {"branch": "synthetic/topic", "head": "a" * 40},
        "description": "Synthetic origin lookup fixture",
        "creator": {"tool": "codex", "sessionId": "synthetic-creator", "evidence": "explicit"},
        "sessions": [{"tool": "claude", "sessionId": "synthetic-later-session", "evidence": "runtime",
                      "firstObservedAt": DATE, "lastObservedAt": DATE}],
        "prs": [],
        "timestamps": {"createdAt": None, "creationSource": "UNKNOWN", "registeredAt": DATE,
                       "lastAgentInterventionAt": DATE, "lastLocalChangeObservedAt": None,
                       "lastCommitAt": None, "lastAuditAt": None},
        "dependencies": {"entrypoint": "UNKNOWN", "requested": "UNKNOWN",
                         "observation": {"state": "absent", "target": None, "observedAt": DATE}},
        "revision": 1,
        "updatedAt": DATE,
    }


def write_json(path, value):
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_text(json.dumps(value, ensure_ascii=True), encoding="utf-8")


def publish(main, value, generation=GENERATION, problems=()):
    """Publish the exact relative symlink shape of registry-index.mjs."""
    root = Path(main) / ".worktrees" / "registry"
    row = {"path": value["path"], "branch": "synthetic/topic", "head": "a" * 40,
           "present": True, "card": value, "problems": list(problems), "observedAt": DATE,
           "prObservations": [], "cleanupVerdict": "UNKNOWN",
           "dependencies": value["dependencies"]["observation"],
           "weights": {"state": "UNKNOWN", "method": None, "measuredAt": None},
           "localState": {"state": "UNKNOWN", "error": "LOCAL_STATE_TOO_LARGE", "dirty": "UNKNOWN",
                          "fingerprint": None, "lastCommitAt": None, "lastLocalChangeObservedAt": None}}
    index = {"schemaVersion": 1, "generation": generation, "observedAt": DATE,
             "complete": True, "repository": value["repository"]["commonDir"], "entries": [row]}
    write_json(root / "generations" / generation / "index.json", index)
    current = root / "current"
    current.unlink(missing_ok=True)
    current.symlink_to("generations/" + generation)
    return root, index
