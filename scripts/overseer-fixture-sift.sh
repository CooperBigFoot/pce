#!/bin/sh
set -eu

usage() {
    echo "usage: $0 --fixture <report.md> --expect <brief-not-ruling|hedge-preserved>" >&2
    exit 2
}

fixture=
expectation=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --fixture) [ "$#" -ge 2 ] || usage; fixture=$2; shift 2 ;;
        --expect) [ "$#" -ge 2 ] || usage; expectation=$2; shift 2 ;;
        *) usage ;;
    esac
done
[ -n "$fixture" ] && [ -f "$fixture" ] && [ -n "$expectation" ] || usage
root=$(mktemp -d "${TMPDIR:-/tmp}/overseer-sift.XXXXXX")
trap 'rm -rf "$root"' EXIT HUP INT TERM
result=$(python3 "$(dirname "$0")/overseer-sift.py" --fixture "$fixture" --root "$root")
python3 - "$expectation" "$root" "$result" <<'PY'
import json
from pathlib import Path
import sys

expectation, root, encoded = sys.argv[1:]
result = json.loads(encoded)
if expectation == "brief-not-ruling":
    assert result["classification"] == "binary-defect", result
    assert result["human_holds_opened"] == 0, result
    brief = Path(result["brief"])
    assert brief.is_file() and brief.is_relative_to(Path(root)), result
    stored = json.loads(brief.read_text())
    assert stored["kind"] == "binary-defect-brief", stored
    assert "no `hold brief` verb" in stored["discriminating_fact"], stored
elif expectation == "hedge-preserved":
    card = result["card"]
    assert "environment may have failed" in card, card
    assert "The environment failed" not in card, card
else:
    raise SystemExit(f"unknown expectation: {expectation}")
print(encoded)
PY
