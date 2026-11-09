#!/usr/bin/env bash
# Install PCE workflows and supporting guidance.
set -eu

REPO_ROOT="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)"
exec uv run --no-project --no-cache "$REPO_ROOT/scripts/install.py" "$REPO_ROOT"
