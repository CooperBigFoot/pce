#!/usr/bin/env python3
"""Deterministic fixture seam for the overseer report sifter."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile


def operator_card(report: str) -> str:
    marker = "## Operator card"
    if marker not in report:
        raise ValueError("report has no Operator card section")
    return report.split(marker, 1)[1].strip()


def classify(report: str) -> tuple[str, str]:
    absent = re.search(
        r"^Observed stop: requested verb .+ is absent from the installed binary surface\.$",
        report,
        flags=re.MULTILINE,
    )
    if absent:
        fact = next(
            line.removeprefix("Discriminating fact: ").strip()
            for line in report.splitlines()
            if line.startswith("Discriminating fact: ")
        )
        return "binary-defect", fact
    return "candidate-ruling", "report does not prove an absent installed capability"


def record_brief(root: Path, source: Path, report: str, fact: str) -> Path:
    digest = hashlib.sha256(report.encode()).hexdigest()
    directory = root / "overseer" / "briefs"
    directory.mkdir(parents=True, exist_ok=True)
    destination = directory / f"{digest}.json"
    record = {
        "kind": "binary-defect-brief",
        "id": digest,
        "source_report": str(source),
        "discriminating_fact": fact,
        "report": report,
    }
    encoded = (json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n").encode()
    if destination.exists():
        if destination.read_bytes() != encoded:
            raise ValueError(f"brief identity collision at {destination}")
        return destination
    descriptor, temporary = tempfile.mkstemp(prefix=f".{digest}.", dir=directory)
    try:
        with os.fdopen(descriptor, "wb") as stream:
            stream.write(encoded)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, destination)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
    return destination


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fixture", required=True, type=Path)
    parser.add_argument("--root", required=True, type=Path)
    args = parser.parse_args()
    report = args.fixture.read_text()
    classification, fact = classify(report)
    result = {
        "classification": classification,
        "card": operator_card(report),
        "brief": None,
    }
    if classification == "binary-defect":
        result["brief"] = str(record_brief(args.root, args.fixture, report, fact))
    human_routes = 0
    for hold_path in args.root.glob("*.jsonl"):
        for line in hold_path.read_text().splitlines():
            record = json.loads(line)
            if record.get("kind") == "routed" and record.get("route") == "human":
                human_routes += 1
    result["human_holds_opened"] = human_routes
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
