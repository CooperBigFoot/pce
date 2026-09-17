# PCE

PCE is a lean distribution of six Agent Skills. It turns either one coherent idea or a large Program into durable visions that an implementing agent delivers. All six skills are available in every supported environment: Claude Code, Codex, and Prime Agent. Any environment that can spawn subagents, run shell commands, read and write files, and use Git and GitHub can run either workflow. GitHub Issues and repository vision documents are the only workflow records.

## Workflows

For a standalone idea:

```text
grill-me → to-vision → implement-vision
```

1. Use `grill-me` in Claude Code, Codex, or Prime Agent to clarify intent and material trade-offs.
2. Use `to-vision` in the same environment to author the confirmed understanding at `planning/visions/YYYY-MM-DD-<slug>.md`, publish it through an independently reviewed documentation PR, merge under repository policy, and verify exact target-branch content. This invocation authorizes ordinary vision publication, not implementation. Explicit draft-only output performs no publication and is not implementation-ready; a later `to-vision` invocation can publish the same path.
3. After verified publication, give the repository-relative path to a fresh implementing agent through `implement-vision planning/visions/<vision>.md`. It verifies existing publication before planning or implementation. A missing publication stops the workflow and must be resolved through `to-vision`, not by the implementing agent.

For an outcome too large for one useful vision:

```text
chart-program → grill-ticket → implement-vision → land-ticket
```

1. Use `chart-program` in Claude Code, Codex, or Prime Agent to survey the repository and publish an approved GitHub Program Map with contained Effort tickets, dependencies, Frontier, and Fog.
2. Use `grill-ticket <issue>` in Claude Code, Codex, or Prime Agent to claim and discover one explicit Effort, publish its single linked vision, and verify the merged target-branch copy.
3. Use `implement-vision <Effort number or canonical URL>` or pass the linked repository-relative vision path to a fresh implementing agent. A number resolves in the current repository. A canonical URL resolves its encoded repository. An Effort-derived path recovers that same identity from its canonical provenance, then runs the complete ticket workflow. The workflow records delivery evidence without closing the Effort.
4. Use `land-ticket <issue>` in Claude Code, Codex, or Prime Agent to verify delivery, close the Effort, update the Map, and evolve newly sharp Fog.

A repository may have multiple active Programs. `chart-program` accepts either a large idea for a new Program or an explicit Program issue for re-survey. `grill-ticket` and `land-ticket` require an explicit Effort identity. No command infers a repository-wide singleton. `grill-me` remains the canonical interview behavior composed by Program skills. Vision documents remain flexible standalone project records with no fixed schema beyond the two provenance lines on Effort-derived visions. `grill-me` confirmation alone authorizes neither publication nor implementation.

Both authoring workflows report success only after reviewed documentation publication and exact target-branch verification. Effort authoring also verifies one commit-pinned link, matching provenance, and an open Effort after merge. Vision PR descriptions and commit messages must avoid auto-closing references, including negated phrases; inspect existing text and explicit closing relationships before merge. Publication is never delivery or landing. Blocked publication reports precise partial file, branch, commit, PR, and linkage state without claiming readiness. A successful handoff reports the vision path, publication PR, verified Git refs, and exact `implement-vision` invocation; it does not start implementation automatically.


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

## Automatic Jev support

Normal `to-vision`, `implement-vision`, and Program handoffs automatically use
TypeSafe's hosted Jev for focused context, draft fidelity/scope, review focus, and
requirement-to-evidence judgments. The existing commands and approval boundaries
stay unchanged. `grill-me` and its interview process do not change. There is no
seventh skill or separate Jev workflow to invoke.

The standard-library helper and full agent contract live in
[`skills/implement-vision/semantic-decisions.md`](skills/implement-vision/semantic-decisions.md).
Existing installer symlinks share it across all three supported environments;
Python 3 is the only local requirement. No SDK or package installation is needed.

### One-time credential and sharing setup

Supply `TYPESAFE_API_KEY` to the **agent command process environment** through your
trusted OS/session secret facility or environment's secret injection setting.
For a POSIX interactive shell, a hidden prompt avoids putting a literal key in
shell history (disable shell tracing first):

```bash
set +x
read -r -s -p 'TypeSafe API key: ' TYPESAFE_API_KEY; printf '\n'
export TYPESAFE_API_KEY
# Start the agent from this shell so its command processes inherit the variable.
```

Never paste the value into chat, tracked files, arguments, diagnostic output, or
PRs. Do not run `env` or otherwise dump credentials to verify setup. The helper
never reads `.env`, including the locally excluded file in the canonical PCE
checkout. That file is not portable setup and is not shared with delegates or
downstream projects. Provision each intended environment securely. No paid
connectivity test is required; missing access simply uses normal reasoning.

Credentials are **not consent to transmit private content**. Initially use only
actually public or explicitly permitted, inspected excerpts, including subjects,
requirements, confirmed interview decisions, and unpublished drafts. Private
projects need existing project-level permission covering TypeSafe and the relevant
content, recorded in their normal policy/instructions by an authorized person.
Absent that permission, continue locally without Jev, without per-call prompts.
Exclude secrets and sensitive material even from otherwise permitted projects.
Standard hosted use is not zero retention; see the shared guide for the reviewed
storage, US-hosting, no-training, telemetry, backup, and enterprise distinctions.

Fallback is disclosed briefly once per invocation, not a retry loop. Normal
review, tests, source/target verification, and publication/landing gates remain
mandatory. Compact questions, evidence references, answers/uncertainty, model
identity, and actual resulting actions are inspectable on request during the
session. Payloads are not retained by PCE by default, and no decision database is
created. Actual speed and reliability gains remain unproven until everyday use;
judge fewer lost decisions, unsupported claims, irrelevant reading, and rework,
not confidence scores alone. Removing the helper hooks restores ordinary PCE
behavior without migrating Git, GitHub, or vision records.

## Local tests

No project environment or third-party package is required:

```bash
python3 -m unittest discover -s tests -v
```

The final pre-removal orchestrator remains available in Git history at the annotated tag `pre-skill-only-pce`.
