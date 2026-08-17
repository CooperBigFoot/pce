#!/usr/bin/env bash
# Installer for the pce tooling.
#
# Builds the release binary, links it onto $HOME/.local/bin, and symlinks the
# repo's skill directories into $HOME/.claude/skills/ so repo edits reflect
# live. Idempotent: safe to re-run; symlinks are replaced via ln -sfn.
set -euo pipefail

python=${PCE_TEST_PYTHON:-/usr/bin/python3}
if [ -x "$python" ]; then
    python_executable=1
else
    python_executable=0
fi

merge_hook_settings() {
    settings_path=$1
    PYTHONDONTWRITEBYTECODE=1 "$python" -c '
import json
import os
import stat
import sys
import tempfile


def merge(settings_path, rehydrate_command, protection_command):
    if os.path.exists(settings_path):
        with open(settings_path, "r", encoding="utf-8") as source:
            settings = json.load(source)
        existing_mode = stat.S_IMODE(os.stat(settings_path).st_mode)
    else:
        settings = {}
        existing_mode = None

    if not isinstance(settings, dict):
        raise ValueError("settings document must be a JSON object")
    hooks = settings.get("hooks")
    if hooks is None:
        hooks = {}
        settings["hooks"] = hooks
    if not isinstance(hooks, dict):
        raise ValueError("hooks must be a JSON object")

    for event_name, groups in list(hooks.items()):
        if not isinstance(groups, list):
            raise ValueError(f"hooks.{event_name} must be an array")
        retained_groups = []
        for group in groups:
            if not isinstance(group, dict):
                raise ValueError(f"hooks.{event_name} entries must be objects")
            inner_hooks = group.get("hooks")
            if inner_hooks is None:
                retained_groups.append(group)
                continue
            if not isinstance(inner_hooks, list):
                raise ValueError(f"hooks.{event_name} group hooks must be an array")
            for hook in inner_hooks:
                if not isinstance(hook, dict):
                    raise ValueError(f"hooks.{event_name} inner hooks must be objects")
            filtered = [
                hook
                for hook in inner_hooks
                if hook.get("command") not in (rehydrate_command, protection_command)
            ]
            removed_managed = len(filtered) != len(inner_hooks)
            if removed_managed and not filtered:
                continue
            if removed_managed:
                group["hooks"] = filtered
            retained_groups.append(group)
        hooks[event_name] = retained_groups

    session_start = hooks.get("SessionStart")
    if session_start is None:
        session_start = []
        hooks["SessionStart"] = session_start
    if not isinstance(session_start, list):
        raise ValueError("hooks.SessionStart must be an array")
    session_start.append(
        {
            "matcher": "resume|compact",
            "hooks": [{"type": "command", "command": rehydrate_command}],
        }
    )

    pre_tool_use = hooks.get("PreToolUse")
    if pre_tool_use is None:
        pre_tool_use = []
        hooks["PreToolUse"] = pre_tool_use
    if not isinstance(pre_tool_use, list):
        raise ValueError("hooks.PreToolUse must be an array")
    pre_tool_use.append(
        {
            "matcher": "Bash|Edit|Write",
            "hooks": [{"type": "command", "command": protection_command}],
        }
    )

    encoded = (json.dumps(settings, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    parent = os.path.dirname(os.path.abspath(settings_path))
    os.makedirs(parent, exist_ok=True)
    descriptor = None
    temporary_path = None
    try:
        descriptor, temporary_path = tempfile.mkstemp(
            dir=parent, prefix=".settings.json."
        )
        with os.fdopen(descriptor, "wb") as destination:
            descriptor = None
            destination.write(encoded)
            destination.flush()
            os.fsync(destination.fileno())
        if existing_mode is not None:
            os.chmod(temporary_path, existing_mode)
        os.replace(temporary_path, settings_path)
        temporary_path = None
    finally:
        if descriptor is not None:
            os.close(descriptor)
        if temporary_path is not None:
            try:
                os.unlink(temporary_path)
            except FileNotFoundError:
                pass


try:
    merge(sys.argv[1], sys.argv[2], sys.argv[3])
except Exception as error:
    sys.stderr.write(f"ERROR: failed to merge hook settings: {error}\n")
    sys.exit(1)
' "$settings_path" '$HOME/.local/bin/pce-rehydrate' '$HOME/.local/bin/pce-protect-criteria'
}

verify_hook_settings() {
    settings_path=$1
    PYTHONDONTWRITEBYTECODE=1 "$python" -c '
import json
import sys


def verify(settings_path, rehydrate_command, protection_command):
    with open(settings_path, "r", encoding="utf-8") as source:
        settings = json.load(source)
    if not isinstance(settings, dict):
        raise ValueError("settings document must be a JSON object")
    hooks = settings.get("hooks")
    if not isinstance(hooks, dict):
        raise ValueError("hooks must be a JSON object")
    session_start = hooks.get("SessionStart")
    if not isinstance(session_start, list):
        raise ValueError("hooks.SessionStart must be an array")

    counts = {rehydrate_command: 0, protection_command: 0}
    exact_rehydrate = 0
    exact_protection = 0
    for event_name, groups in hooks.items():
        if not isinstance(groups, list):
            raise ValueError(f"hooks.{event_name} must be an array")
        for group in groups:
            if not isinstance(group, dict):
                raise ValueError(f"hooks.{event_name} entries must be objects")
            inner_hooks = group.get("hooks")
            if inner_hooks is not None and not isinstance(inner_hooks, list):
                raise ValueError(f"hooks.{event_name} group hooks must be an array")
            if isinstance(inner_hooks, list):
                for hook in inner_hooks:
                    if not isinstance(hook, dict):
                        raise ValueError(f"hooks.{event_name} inner hooks must be objects")
                    command = hook.get("command")
                    if command in counts:
                        counts[command] += 1
                        if command == rehydrate_command and event_name != "SessionStart":
                            raise ValueError("managed rehydration command is under a noncanonical event")
                        if command == protection_command and event_name != "PreToolUse":
                            raise ValueError("managed protection command is under a noncanonical event")
            if event_name == "SessionStart" and group == {
                "matcher": "resume|compact",
                "hooks": [{"type": "command", "command": rehydrate_command}],
            }:
                exact_rehydrate += 1
            if event_name == "PreToolUse" and group == {
                "matcher": "Bash|Edit|Write",
                "hooks": [{"type": "command", "command": protection_command}],
            }:
                exact_protection += 1

    if counts[rehydrate_command] != 1:
        raise ValueError(f"managed rehydration command count is {counts[rehydrate_command]}, expected 1")
    if exact_rehydrate != 1:
        raise ValueError(f"canonical SessionStart group count is {exact_rehydrate}, expected 1")
    if counts[protection_command] != 1:
        raise ValueError(f"managed protection command count is {counts[protection_command]}, expected 1")
    if exact_protection != 1:
        raise ValueError(f"canonical PreToolUse group count is {exact_protection}, expected 1")


try:
    verify(sys.argv[1], sys.argv[2], sys.argv[3])
except Exception as error:
    sys.stderr.write(f"ERROR: hook settings verification failed: {error}\n")
    sys.exit(1)
' "$settings_path" '$HOME/.local/bin/pce-rehydrate' '$HOME/.local/bin/pce-protect-criteria'
}

if [ "${1:-}" = "--merge-hook-settings" ]; then
    if [ "$#" -ne 2 ] || [ -z "${2:-}" ]; then
        echo "ERROR: --merge-hook-settings requires exactly one nonempty settings path." >&2
        exit 1
    fi
    if [ "$python_executable" -ne 1 ]; then
        echo "ERROR: Python 3 interpreter is missing or not executable: $python" >&2
        exit 1
    fi
    merge_hook_settings "$2"
    exit 0
fi

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"

# --- Build the release binary -------------------------------------------------
echo "Building pce (release) in $REPO_ROOT ..."
(cd "$REPO_ROOT" && cargo build --release)

# --- Link the binary onto PATH ------------------------------------------------
BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR"
ln -sfn "$REPO_ROOT/target/release/pce" "$BIN_DIR/pce"
echo "Linked $BIN_DIR/pce -> $REPO_ROOT/target/release/pce"

case ":$PATH:" in
    *":$BIN_DIR:"*)
        ;;
    *)
        echo "WARNING: $BIN_DIR is not on your PATH." >&2
        echo "Add it to your shell profile: export PATH=\"\$HOME/.local/bin:\$PATH\"" >&2
        ;;
esac

# --- Symlink the skill directories (never copy) ---------------------------------
SKILLS_DIR="$HOME/.claude/skills"
mkdir -p "$SKILLS_DIR"

for skill in pce to-vision domain-modeling grill-with-docs chart-program work-ticket land-ticket work-graph; do
    src="$REPO_ROOT/skills/$skill"
    dst="$SKILLS_DIR/$skill"
    if [ -e "$dst" ] && [ ! -L "$dst" ]; then
        echo "ERROR: $dst already exists and is not a symlink." >&2
        echo "Refusing to overwrite it. Move it aside, then re-run install.sh." >&2
        exit 1
    fi
    ln -sfn "$src" "$dst"
    echo "Linked $dst -> $src"
done

# --- Install the rehydration hook -----------------------------------------------
status=0

HOOK_LINK="$BIN_DIR/pce-rehydrate"
HOOK_SOURCE="$REPO_ROOT/hooks/pce-rehydrate.sh"
HOOK_PROTECTION_LINK="$BIN_DIR/pce-protect-criteria"
HOOK_PROTECTION_SOURCE="$REPO_ROOT/hooks/pce-protect-criteria.sh"
for hook_link in "$HOOK_LINK" "$HOOK_PROTECTION_LINK"; do
    if [ -e "$hook_link" ] && [ ! -L "$hook_link" ]; then
        echo "ERROR: $hook_link already exists and is not a symlink." >&2
        echo "Refusing to overwrite it. Move it aside, then re-run install.sh." >&2
        exit 1
    fi
done
ln -sfn "$HOOK_SOURCE" "$HOOK_LINK"
echo "Linked $HOOK_LINK -> $HOOK_SOURCE"
ln -sfn "$HOOK_PROTECTION_SOURCE" "$HOOK_PROTECTION_LINK"
echo "Linked $HOOK_PROTECTION_LINK -> $HOOK_PROTECTION_SOURCE"

SETTINGS_PATH="$HOME/.claude/settings.json"
if [ "$python_executable" -eq 1 ]; then
    if ! merge_hook_settings "$SETTINGS_PATH"; then
        status=1
    fi
else
    echo "ERROR: Python 3 interpreter is missing or not executable: $python" >&2
    status=1
fi

# --- Post-install verification ---------------------------------------------------
for link in "$BIN_DIR/pce" "$HOOK_LINK" "$HOOK_PROTECTION_LINK" "$SKILLS_DIR/pce" "$SKILLS_DIR/to-vision" "$SKILLS_DIR/domain-modeling" "$SKILLS_DIR/grill-with-docs" "$SKILLS_DIR/chart-program" "$SKILLS_DIR/work-ticket" "$SKILLS_DIR/land-ticket" "$SKILLS_DIR/work-graph"; do
    if [ -L "$link" ] && [ -e "$link" ]; then
        echo "OK: $link resolves"
    else
        echo "ERROR: $link is missing or does not resolve." >&2
        status=1
    fi
done

if [ ! -x "$HOOK_LINK" ]; then
    echo "ERROR: $HOOK_LINK is not executable." >&2
    status=1
fi
if [ ! -x "$HOOK_PROTECTION_LINK" ]; then
    echo "ERROR: $HOOK_PROTECTION_LINK is not executable." >&2
    status=1
fi

if [ "$python_executable" -eq 1 ]; then
    if ! verify_hook_settings "$SETTINGS_PATH"; then
        status=1
    fi
fi

for skill in domain-modeling grill-with-docs chart-program work-ticket land-ticket work-graph; do
    skill_path="$SKILLS_DIR/$skill/SKILL.md"
    if [ -f "$skill_path" ]; then
        echo "OK: skill definition present at $skill_path"
    else
        echo "ERROR: skill definition missing at $skill_path" >&2
        status=1
    fi
done

SCHEMA_PATH="$SKILLS_DIR/pce/schemas/verdict.schema.json"
if [ -f "$SCHEMA_PATH" ]; then
    echo "OK: verdict schema present at $SCHEMA_PATH"
else
    echo "ERROR: verdict schema missing at $SCHEMA_PATH" >&2
    status=1
fi

GRAPH_SCHEMA_PATH="$SKILLS_DIR/pce/schemas/graph.schema.json"
if [ -f "$GRAPH_SCHEMA_PATH" ]; then
    echo "OK: graph schema present at $GRAPH_SCHEMA_PATH"
else
    echo "ERROR: graph schema missing at $GRAPH_SCHEMA_PATH" >&2
    status=1
fi

RUN_SNAPSHOT_SCHEMA_PATH="$SKILLS_DIR/pce/schemas/run-snapshot.schema.json"
if [ -f "$RUN_SNAPSHOT_SCHEMA_PATH" ]; then
    echo "OK: run snapshot schema present at $RUN_SNAPSHOT_SCHEMA_PATH"
else
    echo "ERROR: run snapshot schema missing at $RUN_SNAPSHOT_SCHEMA_PATH" >&2
    status=1
fi

if [ "$status" -ne 0 ]; then
    echo "Install verification FAILED." >&2
    exit 1
fi

echo "pce install complete."
