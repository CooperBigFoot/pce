# Gemini skill and global guidance installation

PCE currently links its workflow and supporting guidance skills into Claude Code (`~/.claude/skills`), Codex (`~/.codex/skills`), and Prime Agent (`~/.prime/agent/skills`), and maintains task-triggered guidance pointers only in `~/.prime/agent/AGENTS.md`. Extend `./install.sh` so the same single installation also links all seven PCE skills into the shared Gemini skill directory (`~/.gemini/skills`) and maintains the PCE guidance section in `~/.gemini/AGENTS.md` for Gemini CLI and Antigravity CLI (`agy`).

This makes PCE workflows and supporting guidance available across Gemini and Antigravity sessions without per-project configuration, manual symlinks, or duplicate copies of skill files.

## Shared skill links under `~/.gemini/skills`

Antigravity CLI discovers global skills from both `~/.gemini/antigravity-cli/skills` (CLI-specific) and `~/.gemini/skills` (shared across Gemini tools). Install PCE skills into `~/.gemini/skills/<skill_name>` only, not `~/.gemini/antigravity-cli/skills` or workspace-local `.agents/skills`.

Include `~/.gemini/skills` in the installer's skill root matrix alongside `~/.claude/skills`, `~/.codex/skills`, and `~/.prime/agent/skills` for all seven distributed skills:

- `grill-me`
- `save-discussion`
- `to-vision`
- `implement-vision`
- `chart-program`
- `github-writing`
- `test-first-development`

Each installed entry remains a directory symlink pointing back to `skills/<name>` in this checkout, so relative references between sibling skills resolve identically through `~/.gemini/skills`. Apply the existing pre-flight conflict checks and retired checkout-owned link cleanup (`land-ticket` and `grill-ticket`) to `~/.gemini/skills` under the same rules as the existing roots.

## Global guidance in `~/.gemini/AGENTS.md`

Antigravity and Gemini CLI load global rules from `~/.gemini/AGENTS.md`. Extend `./install.sh` to create or update the delimited PCE guidance section (`<!-- pce:guidance:start -->` to `<!-- pce:guidance:end -->`) in both `~/.prime/agent/AGENTS.md` and `~/.gemini/AGENTS.md`.

Each file's PCE section must reference the supporting skills at its own installed skill root:

- `~/.prime/agent/AGENTS.md` points to `~/.prime/agent/skills/github-writing/SKILL.md` and `~/.prime/agent/skills/test-first-development/SKILL.md`.
- `~/.gemini/AGENTS.md` points to `~/.gemini/skills/github-writing/SKILL.md` and `~/.gemini/skills/test-first-development/SKILL.md`.

Keep both files as independent regular files rather than symlinking `~/.gemini/AGENTS.md` to `~/.prime/agent/AGENTS.md`. Do not modify global instruction files for Claude Code or Codex.

Preserve all existing ownership and pre-mutation safety checks across both global instruction files before any skill link, retired link, or instruction file is mutated:

- Refuse symlinked, dangling, directory, or hardlinked (`st_nlink != 1`) instruction files.
- Refuse ambiguous, unpaired, duplicate, or inline `<!-- pce:guidance:... -->` markers.
- Preserve unrelated user content, exact surrounding bytes, and file permissions when updating or appending the PCE section, and make reruns idempotent.
- Keep the Prime Agent check against shadowing alternate global instruction filenames (`AGENTS.MD`, `CLAUDE.md`, `CLAUDE.MD`) in `~/.prime/agent/`. In `~/.gemini/`, Antigravity loads `GEMINI.md` and `AGENTS.md` cumulatively rather than letting `AGENTS.md` hide `GEMINI.md`; only a case-variant `AGENTS.MD` in `~/.gemini/` when `AGENTS.md` is absent is an ambiguous spelling conflict.

## Documentation and evidence of completion

Update `README.md` to document that `./install.sh` links skills into `~/.gemini/skills` and maintains the `<!-- pce:guidance:start -->` / `<!-- pce:guidance:end -->` section in both `~/.prime/agent/AGENTS.md` and `~/.gemini/AGENTS.md`.

Extend `tests/test_install.py` to verify:

- All seven skills are linked into `.gemini/skills` and relative markdown links within installed skills resolve through `.gemini/skills`.
- Independently installed skills in `.gemini/skills` are preserved, checkout-owned retired links (`land-ticket`, `grill-ticket`) in `.gemini/skills` are removed, and foreign or non-symlink conflicts in `.gemini/skills` abort installation before any mutation.
- Both `.prime/agent/AGENTS.md` and `.gemini/AGENTS.md` are created or updated idempotently with environment-specific skill paths, preserving unrelated bytes and file permissions, while ownership or marker conflicts in either file abort installation before any link or file is modified.

Validate all installer behavior using an isolated temporary `HOME` and run the full test suite from the repository root with:

```bash
python3 -m unittest discover -s tests -v
```

This is a standalone vision with no associated GitHub issue. Publishing this vision does not authorize implementation; implementation begins only when explicitly handed to `implement-vision`.
