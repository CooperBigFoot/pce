#!/bin/sh
# criterion_protection : PreToolUse × ActiveRun × ProposedVision → Allow | Deny

python=${PCE_TEST_PYTHON:-/usr/bin/python3}
if [ ! -x "$python" ]; then
    printf '%s\n' 'REFUSED: criterion protection could not construct the complete proposed vision for verification.' >&2
    exit 2
fi

PYTHONDONTWRITEBYTECODE=1 "$python" -c '
import glob
import json
import os
import subprocess
import sys


CONSTRUCT_REFUSAL = "REFUSED: criterion protection could not construct the complete proposed vision for verification."
VERIFIER_REFUSAL = "REFUSED: criterion protection could not obtain an accepting decision from pce criteria check. Ratified criteria may not be removed, reordered, or changed; record additive criteria first with pce log --kind criterion-added."
BASH_REFUSAL = "REFUSED: Bash may not access vision.md during an active run; use Read for inspection, Edit or Write for a proposed change, and pce log --kind criterion-added for additive criteria."


def refuse_construct():
    sys.stderr.write(CONSTRUCT_REFUSAL + "\n")
    raise SystemExit(2)


def refuse_verifier():
    sys.stderr.write(VERIFIER_REFUSAL + "\n")
    raise SystemExit(2)


def project_root(payload):
    cwd = payload.get("cwd")
    if isinstance(cwd, str) and cwd:
        return os.path.abspath(cwd)
    fallback = os.environ.get("CLAUDE_PROJECT_DIR")
    if fallback:
        return os.path.abspath(fallback)
    return None


def active_vision(path):
    normalized = os.path.abspath(path)
    if os.path.basename(normalized) != "vision.md" or not os.path.isfile(normalized):
        return None
    vision_dir = os.path.dirname(normalized)
    if os.path.basename(os.path.dirname(vision_dir)) != "planning":
        return None
    log_path = os.path.join(vision_dir, "events.jsonl")
    if not os.path.isfile(log_path):
        return None
    return vision_dir, log_path


def proposed_write(tool_input):
    content = tool_input.get("content")
    if not isinstance(content, str):
        refuse_construct()
    return content.encode("utf-8")


def proposed_edit(tool_input, path):
    old = tool_input.get("old_string")
    new = tool_input.get("new_string")
    replace_all = tool_input.get("replace_all", False)
    if not isinstance(old, str) or not isinstance(new, str) or not isinstance(replace_all, bool):
        refuse_construct()
    try:
        with open(path, "rb") as source:
            current = source.read().decode("utf-8")
    except Exception:
        refuse_construct()
    occurrences = current.count(old)
    if occurrences < 1 or (not replace_all and occurrences != 1):
        refuse_construct()
    if replace_all:
        proposal = current.replace(old, new)
    else:
        proposal = current.replace(old, new, 1)
    return proposal.encode("utf-8")


def verify_proposal(document, cwd, log_path, vision_dir):
    executable = os.path.join(os.environ.get("HOME", ""), ".local", "bin", "pce")
    try:
        result = subprocess.run(
            [executable, "criteria", "check", "--file", log_path, "--vision-dir", vision_dir],
            input=document,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=cwd,
            check=False,
        )
    except Exception:
        refuse_verifier()
    if result.returncode != 0:
        refuse_verifier()


def handle_bash(payload, root):
    tool_input = payload.get("tool_input")
    if not isinstance(tool_input, dict):
        refuse_construct()
    command = tool_input.get("command")
    if not isinstance(command, str):
        refuse_construct()
    if "vision.md" not in command or root is None:
        return
    pattern = os.path.join(root, "planning", "*", "events.jsonl")
    if any(os.path.isfile(candidate) for candidate in glob.glob(pattern)):
        sys.stderr.write(BASH_REFUSAL + "\n")
        raise SystemExit(2)


def main():
    try:
        payload = json.load(sys.stdin)
    except Exception:
        refuse_construct()
    if not isinstance(payload, dict):
        refuse_construct()
    if payload.get("hook_event_name") != "PreToolUse":
        return
    tool_name = payload.get("tool_name")
    if tool_name not in ("Bash", "Edit", "Write"):
        return
    root = project_root(payload)
    if tool_name == "Bash":
        handle_bash(payload, root)
        return
    tool_input = payload.get("tool_input")
    if not isinstance(tool_input, dict):
        refuse_construct()
    file_path = tool_input.get("file_path")
    if not isinstance(file_path, str):
        refuse_construct()
    if os.path.isabs(file_path):
        path = os.path.abspath(file_path)
    elif root is not None:
        path = os.path.abspath(os.path.join(root, file_path))
    else:
        return
    active = active_vision(path)
    if active is None:
        return
    vision_dir, log_path = active
    try:
        document = proposed_write(tool_input) if tool_name == "Write" else proposed_edit(tool_input, path)
        if root is None:
            refuse_construct()
        verify_proposal(document, root, log_path, vision_dir)
    except SystemExit:
        raise
    except Exception:
        refuse_construct()


main()
'
