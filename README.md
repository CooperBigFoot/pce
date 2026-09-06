# PCE

PCE is a lean distribution of six Agent Skills. It turns either one coherent idea or a large Program into durable visions that an implementing agent delivers. All six skills are available in every supported environment: Claude Code, Codex, and Prime Agent. Any environment that can spawn subagents, run shell commands, read and write files, and use Git and GitHub can run either workflow. GitHub Issues and repository vision documents are the only workflow records.

## Workflows

For a standalone idea:

```text
grill-me → to-vision → implement-vision
```

1. Use `grill-me` in Claude Code, Codex, or Prime Agent to clarify intent and material trade-offs.
2. Use `to-vision` in the same environment to write the confirmed understanding to `planning/visions/YYYY-MM-DD-<slug>.md`. This is a local draft until publication is verified.
3. Give the repository-relative path to a fresh implementing agent through `implement-vision planning/visions/<vision>.md`. On the first run it publishes a new local standalone draft through normal review and verifies the target branch before substantive implementation.

For an outcome too large for one useful vision:

```text
chart-program → grill-ticket → implement-vision → land-ticket
```

1. Use `chart-program` in Claude Code, Codex, or Prime Agent to survey the repository and publish an approved GitHub Program Map with contained Effort tickets, dependencies, Frontier, and Fog.
2. Use `grill-ticket <issue>` in Claude Code, Codex, or Prime Agent to claim and discover one explicit Effort, publish its single linked vision, and verify the merged target-branch copy.
3. Use `implement-vision <Effort number or canonical URL>` or pass the linked repository-relative vision path to a fresh implementing agent. A number resolves in the current repository. A canonical URL resolves its encoded repository. An Effort-derived path recovers that same identity from its canonical provenance, then runs the complete ticket workflow. The workflow records delivery evidence without closing the Effort.
4. Use `land-ticket <issue>` in Claude Code, Codex, or Prime Agent to verify delivery, close the Effort, update the Map, and evolve newly sharp Fog.

A repository may have multiple active Programs. `chart-program` accepts either a large idea for a new Program or an explicit Program issue for re-survey. `grill-ticket` and `land-ticket` require an explicit Effort identity. No command infers a repository-wide singleton. `grill-me` remains the canonical interview behavior composed by Program skills. Vision documents remain flexible standalone project records with no fixed schema beyond the two provenance lines on Effort-derived visions.


## Durable start and recovery

Every accepted vision has one canonical `planning/visions/<vision>.md` path and an exact copy on the intended target branch before implementation proceeds. A path with canonical Effort provenance is an Effort-derived path and only locates the ticket workflow; a path without it remains standalone. `implement-vision` validates input and durable linkage before any planning or implementation, then uses Git refs, GitHub issues and PRs, target-branch effects, and delivery records to start or resume work. It does not depend on the prior agent session or local checkout. An already completed rerun verifies and reports the result without creating replacement work.

Every checkout PCE creates, for any purpose, lives below `<repository>/.worktrees/`, including git worktrees, clones, and plain copies for implementation, review, audit, reproduction, and comparison. PCE-created checkouts for visions keep the `<repository>/.worktrees/visions/` layout. The initial canonical clone is exempt when no local checkout exists: it may live in a durable user-owned location and remain as the repository root. All additional checkouts use that root's managed hierarchy, never temporary directories or arbitrary repository siblings. At the end, enumerate every checkout the invocation created, including delegated checkouts and any retained canonical clone; remove clean disposable checkouts only when verified merged, and preserve and report uncommitted, unpushed, unmerged, conflicting, or uncertain source and history. Never remove checkouts created by another run or a human. Build output is never evidence; preserve logs, receipts, diffs, and patches, never build directories or other regenerable artifacts. `land-ticket` verifies the target-branch vision and this cleanup boundary for every Effort-related checkout before it closes an Effort.

## Install

Run:

```bash
./install.sh
```

The installer creates only this active skill matrix:

| Environment | Skills |
| --- | --- |
| Claude Code (`~/.claude/skills`) | `grill-me`, `to-vision`, `implement-vision`, `chart-program`, `grill-ticket`, `land-ticket` |
| Codex (`~/.codex/skills`) | `grill-me`, `to-vision`, `implement-vision`, `chart-program`, `grill-ticket`, `land-ticket` |
| Prime Agent (`~/.prime/agent/skills`) | `grill-me`, `to-vision`, `implement-vision`, `chart-program`, `grill-ticket`, `land-ticket` |

Each entry is a symlink to this checkout. The installer is safe to rerun. It replaces links owned by this checkout, refuses conflicting files, directories, and foreign links, and reports ambiguous legacy artifacts for manual cleanup.

During migration, proven links for retired PCE skills, including `grill-with-docs`, and the retired `~/.local/bin/pce`, `pce-rehydrate`, and `pce-protect-criteria` links are removed only when their targets prove that this checkout owns them. Known PCE-owned Claude `SessionStart` and `PreToolUse` hook entries are removed without changing unrelated settings. Copied files, directories, foreign links, and ambiguous settings are preserved.

PCE does not install an application, runtime, scheduler, package manager, hooks, or generated state.

## Local tests

No project environment or third-party package is required:

```bash
python3 -m unittest discover -s tests -v
```

The final pre-removal orchestrator remains available in Git history at the annotated tag `pre-skill-only-pce`.
