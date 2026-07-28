#!/bin/sh

python=${PCE_TEST_PYTHON:-/usr/bin/python3}
[ -x "$python" ] || exit 0

output=$(
    "$python" -c '
import glob
import json
import os
import re
import subprocess
import sys

TIMESTAMP = re.compile(
    r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{3}Z$"
)


def rehydrate():
    payload = json.load(sys.stdin)
    if not isinstance(payload, dict):
        return None
    event_name = payload.get("hook_event_name")
    if event_name != "SessionStart":
        return None
    if payload.get("source") not in ("resume", "compact"):
        return None

    if "cwd" in payload and not isinstance(payload["cwd"], str):
        return None
    root = payload.get("cwd")
    if not root:
        root = os.environ.get("CLAUDE_PROJECT_DIR")
    if not isinstance(root, str) or not root:
        return None

    candidates = sorted(glob.glob(os.path.join(root, "planning", "*", "events.jsonl")))
    if not candidates:
        return None

    latest = []
    for candidate in candidates:
        with open(candidate, "r", encoding="utf-8") as event_log:
            lines = event_log.read().splitlines()
        records = [line for line in lines if line != ""]
        if not records:
            return None
        record = json.loads(records[-1])
        if not isinstance(record, dict):
            return None
        timestamp = record.get("timestamp")
        if not isinstance(timestamp, str) or TIMESTAMP.fullmatch(timestamp) is None:
            return None
        latest.append((timestamp, candidate))

    greatest = max(timestamp for timestamp, _ in latest)
    winners = [candidate for timestamp, candidate in latest if timestamp == greatest]
    if len(winners) != 1:
        return None
    log_path = winners[0]
    vision_dir = os.path.dirname(log_path)

    status = subprocess.run(
        ["pce", "status", "--file", log_path, "--vision-dir", vision_dir],
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if status.returncode != 0:
        return None
    snapshot = json.loads(status.stdout)
    if not isinstance(snapshot, dict):
        return None
    digest = snapshot.get("recovery_digest")
    if not isinstance(digest, dict):
        return None

    context = json.dumps(digest, ensure_ascii=False, separators=(",", ":"))
    envelope = {
        "hookSpecificOutput": {
            "hookEventName": event_name,
            "additionalContext": context,
        }
    }
    return json.dumps(envelope, ensure_ascii=False, separators=(",", ":"))


try:
    result = rehydrate()
except Exception:
    result = None

if result is not None:
    sys.stdout.write(result)
' 2>/dev/null
) || exit 0

[ -n "$output" ] && printf '%s' "$output"
exit 0
