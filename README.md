# PCE

PCE is a lean distribution of six Agent Skills. It turns either one coherent idea or a large Program into durable visions for Prime Agent. GitHub Issues and repository vision documents are the only workflow records.

## Workflows

For a standalone idea:

```text
grill-me → to-vision → implement-vision
```

1. Use `grill-me` in Claude Code or Codex to clarify intent and material trade-offs.
2. Use `to-vision` there to write the confirmed understanding to `planning/visions/YYYY-MM-DD-<slug>.md`.
3. Give that file to a root Prime Agent through `implement-vision` for implementation and independent PR review.

For an outcome too large for one useful vision:

```text
chart-program → grill-ticket → implement-vision → land-ticket
```

1. Use `chart-program` in Claude Code or Codex to survey the repository and publish an approved GitHub Program Map with contained Effort tickets, dependencies, Frontier, and Fog.
2. Use `grill-ticket <issue>` to claim and discover one explicit Effort and create or revise its single linked vision.
3. Give that vision to a root Prime Agent through `implement-vision`. It records delivery evidence without closing the Effort.
4. Use `land-ticket <issue>` to verify delivery, close the Effort, update the Map, and evolve newly sharp Fog.

A repository may have multiple active Programs. `chart-program` accepts either a large idea for a new Program or an explicit Program issue for re-survey. `grill-ticket` and `land-ticket` require an explicit Effort identity. No command infers a repository-wide singleton. `grill-me` remains the canonical interview behavior composed by Program skills. Vision documents remain flexible standalone project records with no fixed schema beyond the two provenance lines on Effort-derived visions.

## Install

Run:

```bash
./install.sh
```

The installer creates only this active skill matrix:

| Environment | Skills |
| --- | --- |
| Claude Code (`~/.claude/skills`) | `grill-me`, `to-vision`, `chart-program`, `grill-ticket`, `land-ticket` |
| Codex (`~/.codex/skills`) | `grill-me`, `to-vision`, `chart-program`, `grill-ticket`, `land-ticket` |
| Prime Agent (`~/.prime/agent/skills`) | `implement-vision` |

Each entry is a symlink to this checkout. The installer is safe to rerun. It replaces links owned by this checkout, refuses conflicting files, directories, and foreign links, and reports ambiguous legacy artifacts for manual cleanup.

During migration, proven links for retired PCE skills, including `grill-with-docs`, and the retired `~/.local/bin/pce`, `pce-rehydrate`, and `pce-protect-criteria` links are removed only when their targets prove that this checkout owns them. Known PCE-owned Claude `SessionStart` and `PreToolUse` hook entries are removed without changing unrelated settings. Copied files, directories, foreign links, and ambiguous settings are preserved.

PCE does not install an application, runtime, scheduler, package manager, hooks, or generated state.

## Local tests

No project environment or third-party package is required:

```bash
python3 -m unittest discover -s tests -v
```

The final pre-removal orchestrator remains available in Git history at the annotated tag `pre-skill-only-pce`.
