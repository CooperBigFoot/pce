#!/usr/bin/env python3
"""Create a dated, durable vision path without overwriting content."""

from __future__ import annotations

import argparse
import datetime as dt
import os
from pathlib import Path
import re
import unicodedata


def slugify(name: str) -> str:
    """Return a lowercase ASCII slug for a human-readable name."""
    normalized = unicodedata.normalize("NFKD", name)
    ascii_name = normalized.encode("ascii", "ignore").decode("ascii").lower()
    slug = re.sub(r"[^a-z0-9]+", "-", ascii_name).strip("-")
    if not slug:
        raise ValueError("vision name must contain at least one letter or number")
    return slug


def create_vision(root: Path, name: str, date: dt.date | None = None) -> Path:
    """Create the vision file if absent and return its path relative to root."""
    day = date if date is not None else dt.date.today()
    relative = Path("planning") / "visions" / f"{day.isoformat()}-{slugify(name)}.md"
    destination = root / relative
    planning = root / "planning"
    visions = planning / "visions"
    for directory in (planning, visions):
        if os.path.lexists(directory):
            if directory.is_symlink() or not directory.is_dir():
                raise ValueError(f"vision path component is not a regular directory: {directory}")
        else:
            directory.mkdir()
    try:
        destination.open("x", encoding="utf-8").close()
    except FileExistsError:
        if destination.is_symlink() or not destination.is_file():
            raise ValueError(f"vision path exists but is not a regular file: {relative}") from None
    return relative


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("name", help="human-readable vision name")
    args = parser.parse_args()
    try:
        path = create_vision(Path.cwd(), args.name)
    except (OSError, ValueError) as error:
        parser.error(str(error))
    print(path.as_posix())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
