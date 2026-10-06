#!/usr/bin/env bash
# Install PCE workflows and supporting guidance.
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
    "github-writing", "test-first-development",
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

guidance = home / ".prime/agent/AGENTS.md"
section_start = b"<!-- pce:guidance:start -->"
section_end = b"<!-- pce:guidance:end -->"
section = b"\n".join((
    section_start,
    b"## PCE guidance",
    b"",
    b"Before drafting or revising a GitHub issue or PR body, read and follow "
    b"[github-writing](~/.prime/agent/skills/github-writing/SKILL.md).",
    b"",
    b"Before implementing or reviewing code or tests, read and follow "
    b"[test-first-development](~/.prime/agent/skills/test-first-development/SKILL.md).",
    section_end,
))


def guidance_conflict(reason: str) -> None:
    print(f"ERROR: conflict: {guidance}: {reason}.", file=sys.stderr)
    print("Resolve the PCE guidance ownership conflict, then rerun install.sh.", file=sys.stderr)
    raise SystemExit(1)


# Prime Agent prefers AGENTS.md over these alternatives. Do not hide user rules.
context_names = {path.name for path in guidance.parent.iterdir()} if guidance.parent.is_dir() else set()
if "AGENTS.md" not in context_names and context_names & {"AGENTS.MD", "CLAUDE.md", "CLAUDE.MD"}:
    guidance_conflict("alternate global instructions exist; reconcile them into AGENTS.md first")

if exists(guidance) and (
    guidance.is_symlink() or not guidance.is_file() or guidance.stat().st_nlink != 1
):
    guidance_conflict("expected a regular, unshared file, not a link or directory")

original = guidance.read_bytes() if exists(guidance) else b""
markers = [line for line in original.splitlines() if b"<!-- pce:guidance:" in line]
if markers and (
    markers != [section_start, section_end]
    or original.count(b"<!-- pce:guidance:") != 2
):
    guidance_conflict("ambiguous PCE section markers; expected one start/end pair on separate lines")
if section_start in original:
    start = original.index(section_start)
    end = original.index(section_end) + len(section_end)
    updated = original[:start] + section + original[end:]
else:
    updated = original + (b"\n\n" if original else b"") + section + b"\n"

# Install only the exact supported matrix.
for destination, source in matrix.items():
    destination.parent.mkdir(parents=True, exist_ok=True)
    if exists(destination):
        destination.unlink()
    destination.symlink_to(source, target_is_directory=True)
    print(f"Linked {destination} -> {source}")

if updated != original:
    guidance.write_bytes(updated)
    print(f"Updated PCE guidance in {guidance}")

PY
