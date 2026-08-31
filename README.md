# PCE

PCE is a small distribution of three Agent Skills. It turns an idea into a durable implementation vision, then hands that vision to Prime Agent.

## Workflow

1. Use `grill-me` in Claude Code or Codex to clarify intent, outcomes, and material trade-offs.
2. Use `to-vision` there to write the shared understanding to `planning/visions/YYYY-MM-DD-<slug>.md`.
3. Give that file to a root Prime Agent through `implement-vision`. Prime Agent plans the work, delegates implementation, and obtains independent PR review.

Vision documents are standalone project records. They have no fixed template or schema.

## Install

Run:

```bash
./install.sh
```

The installer creates only this active skill matrix:

| Environment | Skills |
| --- | --- |
| Claude Code (`~/.claude/skills`) | `grill-me`, `to-vision` |
| Codex (`~/.codex/skills`) | `grill-me`, `to-vision` |
| Prime Agent (`~/.prime/agent/skills`) | `implement-vision` |

Each entry is a symlink to this checkout. The installer is safe to rerun. It replaces links owned by this checkout, refuses conflicting files, directories, and foreign links, and reports ambiguous legacy artifacts for manual cleanup.

During migration, obsolete PCE skill links and the retired `~/.local/bin/pce` and `pce-rehydrate` links are removed only when their targets prove that this checkout owns them. Known PCE-owned Claude `SessionStart` hook entries are removed without changing unrelated settings. Copied files, directories, foreign links, and ambiguous settings are preserved.

## Local tests

No project environment or third-party package is required:

```bash
python3 -m unittest discover -s tests -v
```

The final pre-removal orchestrator remains available in Git history at the annotated tag `pre-skill-only-pce`.
