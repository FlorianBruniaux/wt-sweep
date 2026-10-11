#!/usr/bin/env python3
"""Replay only synthetic provenance; optional original-schema compatibility check."""

import argparse
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from fixtures import card, GENERATION, NEXT_GENERATION, publish, write_json
from passive import read_context


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-root", type=Path, help="Optional existing Aristote code checkout, no registry payloads read")
    args = parser.parse_args()
    reference = json.loads(Path(__file__).with_name("source-reference.json").read_text())
    with tempfile.TemporaryDirectory(prefix="sweep-provenance-replay-") as directory:
        root = Path(directory).resolve()
        topic = root / "topic"
        topic.mkdir()
        main_path = root / "main"
        common = main_path / ".git"
        value = card(topic, common)
        read = lambda **kwargs: read_context(str(topic), str(common), now="2026-10-11T12:10:00.000Z", **kwargs)
        results = {"absent": read()}
        write_json(topic / ".worktree.json", value)
        results["declared_context"] = read()
        historical = copy.deepcopy(value)
        historical["creator"]["tool"] = "human"
        publish(main_path, historical)
        results["contradictory_context"] = read(main=str(main_path))
        results["changed_generation"] = read(main=str(main_path), after_read=lambda: publish(main_path, historical, NEXT_GENERATION))
        results["schema_compatibility"] = {"state": "NOT_RUN", "reference": reference}
        if args.source_root:
            for path, expected in reference["sha256"].items():
                if hashlib.sha256((args.source_root / path).read_bytes()).hexdigest() != expected:
                    raise SystemExit("SOURCE_REVISION_CHANGED: " + path)
            module_url = (args.source_root.resolve() / "scripts/worktrees/registry.mjs").as_uri()
            script = """
import { readFileSync } from 'node:fs';
const { validateCard } = await import(process.argv[1]);
const value = JSON.parse(readFileSync(0, 'utf8'));
validateCard(value);
const historical = structuredClone(value);
historical.creator.tool = 'human';
validateCard(historical);
const invalid = structuredClone(value);
invalid.extra = 'synthetic-invalid-field';
let refused = false;
try { validateCard(invalid); } catch { refused = true; }
if (!refused) throw new Error('INVALID_FIELD_ACCEPTED');
console.log(JSON.stringify({state:'passed', validCards:2, invalidCardsRefused:1}));
"""
            process = subprocess.run(["node", "--input-type=module", "-e", script, module_url],
                                     input=json.dumps(value), text=True, capture_output=True, timeout=10)
            if process.returncode:
                raise SystemExit("SOURCE_VALIDATOR_FAILED: exit " + str(process.returncode))
            results["schema_compatibility"].update(json.loads(process.stdout))
            for path, expected in reference["sha256"].items():
                if hashlib.sha256((args.source_root / path).read_bytes()).hexdigest() != expected:
                    raise SystemExit("SOURCE_REVISION_CHANGED: " + path)
        results["selected_generation_before_mutation"] = GENERATION
        print(json.dumps(results, ensure_ascii=True, indent=2))


if __name__ == "__main__":
    main()
