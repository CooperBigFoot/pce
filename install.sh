#!/usr/bin/env bash
# Install PCE's six skills.
set -eu

REPO_ROOT="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)"
PYTHON="${PCE_TEST_PYTHON:-python3}"
exec "$PYTHON" - "$REPO_ROOT" <<'PY'
from __future__ import annotations

import os
from pathlib import Path
import sys

repo = Path(sys.argv[1]).resolve()
home_value = os.environ.get("HOME")
if not home_value:
    print("ERROR: HOME is not set.", file=sys.stderr)
    raise SystemExit(1)
home = Path(home_value).expanduser().resolve(strict=False)

skills = (
    "grill-me", "to-vision", "implement-vision",
    "chart-program", "grill-ticket", "land-ticket",
)
skill_roots = (
    home / ".claude/skills",
    home / ".codex/skills",
    home / ".prime/agent/skills",
)
matrix = {
    root / name: repo / "skills" / name
    for root in skill_roots
    for name in skills
}


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


def conflict(path: Path) -> str | None:
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

conflicts = [(path, reason) for path in matrix if (reason := conflict(path))]
if conflicts:
    for path, reason in conflicts:
        print(f"ERROR: conflict: {reason}.", file=sys.stderr)
        print(f"Move or remove {path}, then rerun install.sh.", file=sys.stderr)
    raise SystemExit(1)

# Install only the exact supported matrix.
for destination, source in matrix.items():
    destination.parent.mkdir(parents=True, exist_ok=True)
    if exists(destination):
        destination.unlink()
    destination.symlink_to(source, target_is_directory=True)
    print(f"Linked {destination} -> {source}")

PY
