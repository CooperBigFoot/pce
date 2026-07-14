#!/usr/bin/env bash
# Installer for the pce tooling.
#
# Builds the release binary, links it onto $HOME/.local/bin, and symlinks the
# repo's skill directories into $HOME/.claude/skills/ so repo edits reflect
# live. Idempotent: safe to re-run; symlinks are replaced via ln -sfn.
set -euo pipefail

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

for skill in pce to-vision domain-modeling grill-with-docs; do
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

# --- Post-install verification ---------------------------------------------------
status=0

for link in "$BIN_DIR/pce" "$SKILLS_DIR/pce" "$SKILLS_DIR/to-vision" "$SKILLS_DIR/domain-modeling" "$SKILLS_DIR/grill-with-docs"; do
    if [ -L "$link" ] && [ -e "$link" ]; then
        echo "OK: $link resolves"
    else
        echo "ERROR: $link is missing or does not resolve." >&2
        status=1
    fi
done

for skill in domain-modeling grill-with-docs; do
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

if [ "$status" -ne 0 ]; then
    echo "Install verification FAILED." >&2
    exit 1
fi

echo "pce install complete."
