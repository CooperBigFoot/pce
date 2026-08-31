#!/usr/bin/env bash
# Install PCE's six skills and safely retire proven legacy entries.
set -eu

REPO_ROOT="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)"
PYTHON="${PCE_TEST_PYTHON:-python3}"
exec "$PYTHON" - "$REPO_ROOT" <<'PY'
from __future__ import annotations

import json
import os
from pathlib import Path
import stat
import sys
import tempfile

repo = Path(sys.argv[1]).resolve()
home_value = os.environ.get("HOME")
if not home_value:
    print("ERROR: HOME is not set.", file=sys.stderr)
    raise SystemExit(1)
home_spelling = Path(os.path.abspath(Path(home_value).expanduser()))
home = home_spelling.resolve(strict=False)

authoring_skills = (
    "grill-me", "to-vision", "chart-program", "grill-ticket", "land-ticket",
)
matrix = {
    **{
        home / environment / name: repo / "skills" / name
        for environment in (".claude/skills", ".codex/skills")
        for name in authoring_skills
    },
    home / ".prime/agent/skills/implement-vision": repo / "skills/implement-vision",
}
retired_names = (
    "pce", "to-graph", "work-graph", "overseer", "work-ticket",
    "grill-with-docs", "domain-modeling",
)
skill_roots = (
    home / ".claude/skills",
    home / ".codex/skills",
    home / ".prime/agent/skills",
)
# Current skills in environments outside the supported matrix are legacy entries too.
wrong_placements = (
    home / ".claude/skills/implement-vision",
    home / ".codex/skills/implement-vision",
    *(home / ".prime/agent/skills" / name for name in authoring_skills),
)


def exists(path: Path) -> bool:
    return os.path.lexists(path)


def link_target(path: Path) -> Path:
    raw = Path(os.readlink(path))
    if not raw.is_absolute():
        raw = path.parent / raw
    return raw.resolve(strict=False)


def inside_repo(path: Path) -> bool:
    try:
        path.relative_to(repo)
        return True
    except ValueError:
        return False


def owned_link(path: Path) -> bool:
    return path.is_symlink() and inside_repo(link_target(path))


def conflict(path: Path, source: Path) -> str | None:
    if not exists(path):
        return None
    if path.is_symlink():
        if owned_link(path):
            return None
        return f"{path} is a symlink to {link_target(path)}, which is not owned by {repo}"
    kind = "directory" if path.is_dir() else "file"
    return f"{path} is a {kind}, not a PCE-owned symlink"


for source in matrix.values():
    if not source.is_dir():
        print(f"ERROR: required skill directory is missing: {source}", file=sys.stderr)
        raise SystemExit(1)

conflicts = [(path, reason) for path, source in matrix.items() if (reason := conflict(path, source))]
if conflicts:
    for path, reason in conflicts:
        print(f"ERROR: conflict: {reason}.", file=sys.stderr)
        print(f"Move or remove {path}, then rerun install.sh.", file=sys.stderr)
    raise SystemExit(1)

# Capture binary ownership before cleanup so hook ownership can use it as evidence.
binary_paths = (
    home / ".local/bin/pce",
    home / ".local/bin/pce-rehydrate",
    home / ".local/bin/pce-protect-criteria",
)
binary_was_owned = {path: owned_link(path) for path in binary_paths}
managed_hook_commands = {
    "SessionStart": ("pce-rehydrate", "$HOME/.local/bin/pce-rehydrate"),
    "PreToolUse": ("pce-protect-criteria", "$HOME/.local/bin/pce-protect-criteria"),
}
owned_command_path_identities: dict[str, set[str]] = {}
for event_name, (binary_name, _literal) in managed_hook_commands.items():
    binary = home / ".local/bin" / binary_name
    identities: set[str] = set()
    if binary_was_owned[binary]:
        # Keep both the user's spelling and the resolved spelling before unlinking.
        # The former can contain symlinked HOME components that cannot be recovered
        # by resolving the command after the leaf symlink has been removed.
        for path in (
            binary,
            home_spelling / ".local/bin" / binary_name,
        ):
            identities.add(os.path.normcase(os.path.normpath(os.fspath(path))))
    owned_command_path_identities[event_name] = identities

# Install only the exact supported matrix.
for destination, source in matrix.items():
    destination.parent.mkdir(parents=True, exist_ok=True)
    if exists(destination):
        destination.unlink()
    destination.symlink_to(source, target_is_directory=True)
    print(f"Linked {destination} -> {source}")

cleanup_candidates = {root / name for root in skill_roots for name in retired_names}
cleanup_candidates.update(wrong_placements)
for path in sorted(cleanup_candidates, key=str):
    if not exists(path):
        continue
    if owned_link(path):
        target = link_target(path)
        path.unlink()
        print(f"Removed retired PCE link {path} -> {target}")
    else:
        detail = f"symlink to {link_target(path)}" if path.is_symlink() else (
            "directory" if path.is_dir() else "file"
        )
        print(
            f"WARNING: preserved ambiguous retired skill artifact {path} ({detail}); "
            "inspect and remove it manually if it belongs to PCE.",
            file=sys.stderr,
        )

for path in binary_paths:
    if not exists(path):
        continue
    if owned_link(path):
        target = link_target(path)
        path.unlink()
        print(f"Removed retired PCE link {path} -> {target}")
    else:
        detail = f"symlink to {link_target(path)}" if path.is_symlink() else (
            "directory" if path.is_dir() else "file"
        )
        print(
            f"WARNING: preserved ambiguous retired binary {path} ({detail}); "
            "inspect and remove it manually if it belongs to PCE.",
            file=sys.stderr,
        )

settings_path = home / ".claude/settings.json"


def command_is_owned(event_name: str, command: object) -> bool:
    if not isinstance(command, str):
        return False
    binary_name, literal = managed_hook_commands[event_name]
    if command == literal:
        binary = home / ".local/bin" / binary_name
        # The literal is evidence from the retired installer unless a foreign
        # artifact currently occupies the command path.
        return not exists(binary) or binary_was_owned[binary]
    command_path = Path(command).expanduser()
    if command_path.is_absolute():
        identity = os.path.normcase(os.path.normpath(os.fspath(command_path)))
        if identity in owned_command_path_identities[event_name]:
            return True
    try:
        return command_path.is_absolute() and inside_repo(command_path.resolve(strict=False))
    except (OSError, RuntimeError):
        return False


def clean_settings() -> None:
    if not settings_path.exists():
        return
    try:
        original = settings_path.read_bytes()
        settings = json.loads(original)
        hooks = settings.get("hooks") if isinstance(settings, dict) else None
        if hooks is None:
            return
        if not isinstance(hooks, dict):
            raise ValueError("hooks is not an object")
        changed = False
        ambiguous: list[tuple[str, str]] = []
        for event_name in managed_hook_commands:
            groups = hooks.get(event_name)
            if groups is None:
                continue
            if not isinstance(groups, list):
                raise ValueError(f"hooks.{event_name} is not an array")

            retained_groups = []
            for group in groups:
                if not isinstance(group, dict):
                    raise ValueError(f"a hooks.{event_name} entry is not an object")
                inner = group.get("hooks")
                if inner is None:
                    retained_groups.append(group)
                    continue
                if not isinstance(inner, list):
                    raise ValueError(f"a hooks.{event_name} hooks value is not an array")
                retained = []
                for hook in inner:
                    if not isinstance(hook, dict):
                        raise ValueError(f"a {event_name} hook is not an object")
                    command = hook.get("command")
                    if command_is_owned(event_name, command):
                        changed = True
                        print(f"Removed retired PCE {event_name} hook: {command}")
                    else:
                        retained.append(hook)
                        if isinstance(command, str) and "pce" in command.lower():
                            ambiguous.append((event_name, command))
                if retained or not inner:
                    if len(retained) != len(inner):
                        group = dict(group)
                        group["hooks"] = retained
                    retained_groups.append(group)
            hooks[event_name] = retained_groups
        if changed:
            encoded = (json.dumps(settings, ensure_ascii=False, indent=2) + "\n").encode()
            mode = stat.S_IMODE(settings_path.stat().st_mode)
            settings_path.parent.mkdir(parents=True, exist_ok=True)
            fd, temporary = tempfile.mkstemp(prefix=".settings.json.", dir=settings_path.parent)
            try:
                with os.fdopen(fd, "wb") as target:
                    target.write(encoded)
                    target.flush()
                    os.fsync(target.fileno())
                os.chmod(temporary, mode)
                os.replace(temporary, settings_path)
            finally:
                if os.path.exists(temporary):
                    os.unlink(temporary)
        for event_name, command in ambiguous:
            print(
                f"WARNING: preserved ambiguous {event_name} hook command {command!r}; "
                "inspect it manually if it belongs to retired PCE.",
                file=sys.stderr,
            )
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(
            f"WARNING: preserved ambiguous Claude settings {settings_path}: {error}; "
            "inspect retired PCE hooks manually.",
            file=sys.stderr,
        )


clean_settings()
PY
