#!/bin/sh
# criterion_protection : PreToolUse × ActiveRun × ProposedVision → Allow | Deny

python=${PCE_TEST_PYTHON:-/usr/bin/python3}
if [ ! -x "$python" ]; then
    printf '%s\n' 'REFUSED: criterion protection could not construct the complete proposed vision for verification.' >&2
    exit 2
fi

PYTHONDONTWRITEBYTECODE=1 "$python" -c '
import json
import os
import re
import subprocess
import sys


CONSTRUCT_REFUSAL = "REFUSED: criterion protection could not construct the complete proposed vision for verification."
VERIFIER_REFUSAL = "REFUSED: criterion protection could not obtain an accepting decision from pce criteria check. Ratified criteria may not be removed, reordered, or changed; record additive criteria first with pce log --kind criterion-added."
BASH_REFUSAL = "REFUSED: Bash may not access vision.md during an active run; use Read for inspection, Edit or Write for a proposed change, and pce log --kind criterion-added for additive criteria."


def refuse_construct():
    sys.stderr.write(CONSTRUCT_REFUSAL + "\n")
    raise SystemExit(2)


def refuse_verifier(detail=None):
    sys.stderr.write(VERIFIER_REFUSAL + "\n")
    if detail is not None:
        sys.stderr.write(detail + "\n")
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


def criterion_name(criteria, index):
    if index >= len(criteria):
        return "<missing>"
    criterion = criteria[index]
    if not isinstance(criterion, dict) or not isinstance(criterion.get("name"), str):
        return "<unnamed>"
    return json.dumps(criterion["name"])


def comparison_failure(output):
    try:
        decision = json.loads(output.decode("utf-8"))
        required = decision["required_criteria"]
        proposed = decision["proposed_criteria"]
        if not isinstance(required, list) or not isinstance(proposed, list):
            return None
    except Exception:
        return None
    for index in range(max(len(required), len(proposed))):
        required_criterion = required[index] if index < len(required) else None
        proposed_criterion = proposed[index] if index < len(proposed) else None
        if required_criterion != proposed_criterion:
            return "Criterion comparison failed at position {}: required {}; proposed {}.".format(
                index + 1,
                criterion_name(required, index),
                criterion_name(proposed, index),
            )
    return None


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
        refuse_verifier(comparison_failure(result.stdout))


def active_vision_paths(root):
    planning = os.path.join(root, "planning")
    try:
        entries = os.scandir(planning)
    except OSError:
        return []
    paths = []
    with entries:
        for entry in entries:
            if not entry.is_dir():
                continue
            vision = os.path.join(entry.path, "vision.md")
            log = os.path.join(entry.path, "events.jsonl")
            if os.path.isfile(vision) and os.path.isfile(log):
                paths.append(os.path.abspath(vision))
    return paths


def command_references_path(command, path, root):
    candidates = [path]
    try:
        relative = os.path.relpath(path, root)
        candidates.extend([relative, os.path.join(".", relative)])
    except ValueError:
        pass
    for candidate in candidates:
        pattern = r"(?<![A-Za-z0-9_./-])" + re.escape(candidate) + r"(?![A-Za-z0-9_./-])"
        if re.search(pattern, command):
            return True
    return False


def handle_bash(payload, root):
    tool_input = payload.get("tool_input")
    if not isinstance(tool_input, dict):
        refuse_construct()
    command = tool_input.get("command")
    if not isinstance(command, str):
        refuse_construct()
    if root is None:
        return
    if any(command_references_path(command, path, root) for path in active_vision_paths(root)):
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
