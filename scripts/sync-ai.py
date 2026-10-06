"""Generate repository-only Claude/Codex instructions; no host activation."""

import argparse
import hashlib
import json
import posixpath
from pathlib import Path
import re
import sys


INVENTORY = ".harness-generated.json"


def safe_path(root, relative):
    parts = Path(relative).parts
    if not parts or Path(relative).is_absolute() or ".." in parts:
        raise ValueError(f"unsafe relative path: {relative}")
    current = root
    for part in parts:
        current /= part
        if current.is_symlink():
            raise ValueError(f"symlink refused: {relative}")
    if current.exists() and not current.is_file():
        raise ValueError(f"expected a file: {relative}")
    return current


def digest(data):
    return hashlib.sha256(data).hexdigest()


def skill_outputs(root):
    """Project skill bundles share bytes and the existing preimage guard."""
    manifest = json.loads(safe_path(root, "harness/skills.json").read_text())
    if not isinstance(manifest, list):
        raise ValueError("skill manifest must be a list")
    outputs, names = {}, set()
    for skill in manifest:
        if not isinstance(skill, dict) or set(skill) != {"name", "files"}:
            raise ValueError("invalid skill record")
        name, files = skill["name"], skill["files"]
        if not isinstance(name, str) or not re.fullmatch(r"[a-z][a-z0-9-]*", name) or name in names:
            raise ValueError("invalid or duplicate skill name")
        names.add(name)
        if (not isinstance(files, list) or not all(isinstance(f, str) for f in files)
                or len(set(files)) != len(files)
                or not {"SKILL.md", "evals/scenarios.json"}.issubset(files)):
            raise ValueError(f"invalid skill resources: {name}")
        bundle = {}
        for relative in files:
            # Validate the resource itself before prefixing its canonical directory.
            if (Path(relative).as_posix() != relative or Path(relative).is_absolute()
                    or ".." in Path(relative).parts or not Path(relative).parts):
                raise ValueError(f"unsafe skill resource: {relative}")
            bundle[relative] = safe_path(root, f"harness/skills/{name}/{relative}").read_bytes()
        document = bundle["SKILL.md"].decode()
        header = re.match(r"\A---\n(.*?)\n---\n", document, re.S)
        if not header:
            raise ValueError(f"missing skill frontmatter: {name}")
        fields = dict(line.split(": ", 1) for line in header[1].splitlines() if ": " in line)
        description = json.loads(fields.get("description", "null"))
        if fields.get("name") != name or not isinstance(description, str) or not description.strip():
            raise ValueError(f"invalid skill frontmatter: {name}")
        # Bundles deliberately use inline relative Markdown links, no imports.
        for relative, data in bundle.items():
            if not relative.endswith(".md"):
                continue
            for link in re.findall(r"\]\(([^)]+)\)", data.decode()):
                if re.match(r"https?://", link) or link.startswith("#"):
                    continue
                target = posixpath.normpath(posixpath.join(posixpath.dirname(relative), link.split("#", 1)[0]))
                if target not in bundle:
                    raise ValueError(f"missing or escaped skill link: {name}/{relative}: {link}")
        scenarios = json.loads(bundle["evals/scenarios.json"])
        if not isinstance(scenarios, dict) or scenarios.get("skill") != name:
            raise ValueError(f"invalid skill scenarios: {name}")
        for kind, minimum in (("positive", 8), ("negative", 2)):
            cases = scenarios.get(kind)
            if (not isinstance(cases, list) or not all(isinstance(c, str) and c.strip() for c in cases)
                    or len(set(cases)) != len(cases) or len(cases) < minimum):
                raise ValueError(f"insufficient or invalid {kind} skill scenarios: {name}")
        if set(scenarios["positive"]) & set(scenarios["negative"]):
            raise ValueError(f"contradictory skill scenarios: {name}")
        for host in (".claude", ".agents"):
            directory = f"{host}/skills/{name}"
            for relative, data in bundle.items():
                safe_path(root, f"{directory}/{relative}")
                outputs[f"{directory}/{relative}"] = data
            for path in (root / directory).rglob("*"):
                relative = str(path.relative_to(root))
                if path.is_symlink() or (path.is_file() and relative not in outputs):
                    raise ValueError(f"unmanaged skill path, reconcile explicitly: {relative}")
    return outputs


def expected_outputs(root):
    instructions = safe_path(root, "harness/instructions.md").read_bytes()
    agents = json.loads(safe_path(root, "harness/agents.json").read_text())
    if not isinstance(agents, list) or not agents:
        raise ValueError("agent manifest must be a non-empty list")
    outputs = {"AGENTS.md": instructions, "CLAUDE.md": instructions}
    names = set()
    for agent in agents:
        if not isinstance(agent, dict) or set(agent) != {"name", "description", "writable"}:
            raise ValueError("invalid agent record")
        name, description, writable = agent["name"], agent["description"], agent["writable"]
        if not isinstance(name, str) or not re.fullmatch(r"[a-z][a-z0-9-]*", name) or name in names:
            raise ValueError("invalid or duplicate agent name")
        if not isinstance(description, str) or not description or type(writable) is not bool:
            raise ValueError(f"invalid agent fields: {name}")
        names.add(name)
        body = safe_path(root, f"harness/agents/{name}.md").read_text()
        tools = "Read, Glob, Grep, Bash, Edit, Write" if writable else "Read, Glob, Grep"
        claude = (
            f"---\nname: {name}\ndescription: {json.dumps(description)}\n"
            f"tools: {tools}\nmodel: inherit\npermissionMode: default\n---\n"
            f"<!-- Generated from harness/agents/{name}.md. -->\n{body}"
        )
        codex = {
            "name": name, "description": description,
            "sandbox_mode": "workspace-write" if writable else "read-only",
            "developer_instructions": body,
        }
        outputs[f".claude/agents/{name}.md"] = claude.encode()
        outputs[f".codex/agents/{name}.toml"] = (
            f"# Generated from harness/agents/{name}.md.\n"
            + "".join(f"{key} = {json.dumps(value)}\n" for key, value in codex.items())
        ).encode()
    outputs[".codex/config.toml"] = (
        "# Generated by scripts/sync-ai.py. Models inherit the active session.\n"
        "[agents]\nenabled = true\nmax_concurrent_threads_per_session = 3\n"
    ).encode()
    outputs.update(skill_outputs(root))
    inventory = {"schema": "wt-sweep.harness/1", "files": {
        name: digest(data) for name, data in sorted(outputs.items())
    }}
    outputs[INVENTORY] = (json.dumps(inventory, indent=2, sort_keys=True) + "\n").encode()
    return outputs


def synchronize(root, write):
    outputs = expected_outputs(root)
    paths = {name: safe_path(root, name) for name in outputs}
    for directory in [".claude/agents", ".codex/agents"]:
        for path in (root / directory).rglob("*"):
            relative = str(path.relative_to(root))
            if path.is_symlink() or (path.is_file() and relative not in outputs):
                raise ValueError(f"unmanaged agent path, reconcile explicitly: {relative}")
    drift = [name for name, path in paths.items()
             if not path.exists() or path.read_bytes() != outputs[name]]
    if not write:
        if drift:
            raise ValueError("configuration drift: " + ", ".join(drift))
        print(f"AI configuration in sync ({len(outputs)} files); native activation UNKNOWN")
        return
    inventory_path = paths[INVENTORY]
    previous = json.loads(inventory_path.read_text()) if inventory_path.exists() else {
        "schema": "wt-sweep.harness/1", "files": {}
    }
    if not isinstance(previous, dict) or previous.get("schema") != "wt-sweep.harness/1":
        raise ValueError("invalid generated inventory")
    old_hashes = previous.get("files")
    if not isinstance(old_hashes, dict) or set(old_hashes) - (set(outputs) - {INVENTORY}):
        raise ValueError("obsolete or invalid generated inventory; reconcile explicitly")
    # Preflight the whole batch. The operator must exclude concurrent writers.
    for name in drift:
        path = paths[name]
        if name != INVENTORY and path.exists() and digest(path.read_bytes()) != old_hashes.get(name):
            raise ValueError(f"changed generated preimage, preserve and reconcile: {name}")
    # This is a cooperative development tool, not a durable backup transaction.
    for name in drift:
        path = paths[name]
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(outputs[name])
    print(f"Generated {len(drift)} repository files; native activation UNKNOWN")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true")
    mode.add_argument("--write", action="store_true")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    try:
        synchronize(args.root.resolve(strict=True), args.write)
    except (OSError, ValueError) as error:
        print(f"sync-ai: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
