# Skill-only PCE

## Purpose

PCE currently duplicates orchestration capabilities that now belong to the Prime Agent harness. It contains a large Rust CLI, frozen work-package graphs, package scheduling, custom dispatch, recovery accounting, event-log state, fleet holds, an overseer, assembly logic, hooks, and several generations of workflow skills. This machinery is costly to understand and maintain, and it requires the human to judge technical planning artifacts after a grill.

Reduce PCE to its core purpose: help a human clarify an idea, turn that shared understanding into a durable vision, and hand the vision to Prime Agent for implementation. Prime Agent should apply its own technical judgement, inspect the codebase, decompose the work, delegate it, and use independent PR review.

The finished repository is a lean Agent Skills distribution, not an orchestration product and not a Rust application.

## Intended workflow

1. The human uses `grill-me` in Claude Code or Codex.
2. The grill asks one question at a time, recommends an answer, and investigates questions that the codebase can answer.
3. The human supplies intent, taste, desired outcomes, priorities, and meaningful trade-offs. The agent translates these into technical language. It does not push code-specific decisions back to the human merely because they are difficult.
4. The human uses `to-vision` in Claude Code or Codex.
5. `to-vision` writes a standalone vision under `planning/visions/` for a fresh implementing agent that has no access to the prior conversation.
6. The human gives that vision to Prime Agent through `implement-vision`.
7. The root Prime Agent inspects the repository, applies its best technical judgement, chooses the number and boundaries of PRs, delegates implementation, and tracks the vision-level outcome.
8. An implementation agent owns each branch, its tests, and its PR. A fresh reviewer examines the vision, repository rules, full diff, and validation evidence. Findings return for repair and re-review. The reviewer merges only after the change satisfies the vision and required checks pass.
9. Handing a vision to Prime Agent grants standing authority for ordinary branches, PRs, repairs, and merges. The agent must still request authority for deployments, destructive data operations, spending, credentials, external publication, infrastructure changes, or other irreversible external acts not clearly authorized by the vision.

## Product boundary

PCE owns exactly three skills:

- `grill-me`: reach shared understanding through a one-question-at-a-time interview.
- `to-vision`: materialize that understanding as a flexible, durable implementation handoff.
- `implement-vision`: tell a root Prime Agent how to execute a vision using the harness's native planning, delegation, review, and progress capabilities.

PCE does not own implementation scheduling, graph authoring, runtime state, agent liveness, recovery budgets, fleet triage, custom gates, worktree orchestration, assembly, or merge state machines.

## Vision document contract

A vision is a standalone handoff for a fresh Prime Agent. The author chooses the structure that communicates the specific work best. PCE imposes no fixed headings, JSON schema, command-level acceptance format, or universal template.

The authoring skill must ensure the document communicates, where relevant:

- the desired outcome and why it matters;
- what externally observable state would demonstrate success;
- scope boundaries and explicit exclusions;
- constraints and settled decisions;
- important context learned from the repository;
- material risks or genuine uncertainty that remain.

The document should contain enough technical translation to guide implementation, but it should not pretend the human must design mechanisms that the implementing agent can determine from code and engineering practice. When repeated implementation failures reveal a missing class of information, improve the skill with focused guidance rather than expanding a universal template.

Vision documents are durable project records at:

```text
planning/visions/YYYY-MM-DD-<slug>.md
```

A helper packaged inside `to-vision` creates the path from the local ISO date and a normalized human-readable name. Creation is idempotent for the same date and name. It returns the existing path without changing existing content. It never silently overwrites a document. It creates only the path and file; it does not author or validate the document structure.

## Repository end state

The tracked repository should be reduced to this structure, apart from normal Git metadata:

```text
pce/
├── skills/
│   ├── grill-me/
│   │   └── SKILL.md
│   ├── to-vision/
│   │   ├── SKILL.md
│   │   └── scripts/
│   │       └── create_vision.py
│   └── implement-vision/
│       └── SKILL.md
├── planning/
│   └── visions/
│       └── 2026-08-31-skill-only-pce.md
├── tests/
│   ├── test_create_vision.py
│   └── test_install.py
├── .gitignore
├── AGENTS.md
├── CLAUDE.md
├── README.md
└── install.sh
```

There is no CI pipeline. Local tests use the Python standard library and require no project environment or third-party dependency.

`AGENTS.md` and `CLAUDE.md` describe only the lean skill repository. They must not retain Rust architecture or retired orchestration instructions. `README.md` explains the three-step workflow, installation matrix, vision location, local test command, and migration behavior without documenting the removed product as an active option.

## Skill installation

`install.sh` creates global symlinks with this exact product boundary:

```text
~/.claude/skills/grill-me    -> <pce-repo>/skills/grill-me
~/.claude/skills/to-vision   -> <pce-repo>/skills/to-vision

~/.codex/skills/grill-me     -> <pce-repo>/skills/grill-me
~/.codex/skills/to-vision    -> <pce-repo>/skills/to-vision

~/.prime/agent/skills/implement-vision
                              -> <pce-repo>/skills/implement-vision
```

`implement-vision` is Prime-Agent-only. Claude Code and the direct Codex harness are authoring environments, not implementation orchestrators. Prime Agent may use Codex-family models internally without requiring a direct Codex implementation skill.

The installer is lean and performs no compilation. It creates parent directories when needed and is safe to rerun. It replaces links already owned by this PCE repository. It refuses to overwrite a directory, copied file, or link owned by another source, and reports the exact conflict and corrective action.

## Migration and removal

Before the removal lands, preserve the final pre-removal orchestrator commit with one annotated Git tag. The tag and ordinary Git history are the archive. Do not retain an `archive/` directory on `main`.

Delete the Rust application and all retired product surfaces from `main`, including:

- `Cargo.toml`, `Cargo.lock`, `clippy.toml`, `src/`, and `crates/`;
- graph, package, dispatch, gate, contract, event-log, status, readiness, hold, overseer, repair, and assembly code;
- `pce`, `to-graph`, `work-graph`, `overseer`, `chart-program`, `work-ticket`, `land-ticket`, `grill-with-docs`, `domain-modeling`, and other skills outside the three-skill boundary;
- hooks, schemas, examples, historical briefs, orchestrator feedback, Program-layer documentation and issue templates, and historical runtime planning artifacts;
- generated or local runtime residue from the tracked product surface.

Preserve this vision under `planning/visions/`. Do not carry other historical planning output into the lean tree merely as an archive.

On installation, remove obsolete global PCE skill entries only when the existing entry is a symlink that resolves into this PCE repository. This includes the retired orchestrator, graph, overseer, and Program-layer skill names. Never delete unrelated directories, copied files, or links owned by another source.

The migration also removes the retired `~/.local/bin/pce`, `~/.local/bin/pce-rehydrate`, and PCE-owned Claude `SessionStart` hook entries when ownership can be proven. Preserve unrelated binaries and settings exactly. Report ambiguous artifacts for manual action instead of guessing.

Tests must exercise installation and cleanup against an isolated temporary home directory. They must not mutate the developer's real global skill directories, binaries, or Claude settings.

## Implementation authority and question policy

The implementing Prime Agent investigates before asking. It may delegate specialist analysis and should decide reversible technical details using the strongest available evidence and established software-engineering practice.

Technical uncertainty alone is not a reason to ask the human a code-specific question. Ask only when the missing information concerns intent, priorities, an outcome-level trade-off, credentials, legal or organizational authority, or permission for an exceptional irreversible act. Translate any necessary question into consequences the human can evaluate and include a recommended answer. If no meaningful human preference separates the options, choose.

The root decides whether this vision requires one PR or several coherent vertical slices. Avoid splitting only by technical layer. Each PR receives independent review. After all PRs merge, verify the resulting target branch against this complete vision and run the repository's local tests.

## Observable completion

The work is complete when all of the following are true:

1. The tracked tree matches the lean repository boundary above and contains no active Rust or orchestration product.
2. The three skills satisfy the responsibilities and authority boundaries in this vision and conform to the Agent Skills directory and metadata conventions.
3. `create_vision.py` deterministically creates `planning/visions/YYYY-MM-DD-<slug>.md`, safely handles repeated creation, and never changes existing content.
4. Local tests prove vision-path creation, slug behavior, idempotency, non-overwrite behavior, symlink installation, safe reruns, owned obsolete-link cleanup, conflict refusal, and isolated cleanup of proven retired artifacts.
5. No CI configuration exists.
6. The installer establishes the Claude Code, Codex, and Prime Agent symlink matrix exactly as specified without compiling software or mutating unrelated user state.
7. The final pre-removal implementation is reachable through an annotated Git tag, while `main` contains no archive copy.
8. The README presents only the simplified workflow as the current product.
9. A fresh Prime Agent can read this vision, implement the redesign, obtain independent review for every PR, merge approved work, and report completion without relying on the removed PCE machinery.

## Explicit non-goals

- Preserving compatibility with active legacy PCE runs.
- Translating existing graphs, journals, holds, or overseer state into a new runtime.
- Providing a fixed vision template or machine validator.
- Supporting direct implementation through Claude Code or the Codex harness.
- Recreating Prime Agent planning, delegation, recovery, review, or progress features inside PCE.
- Adding CI, a package manager, a Rust binary, a service, telemetry, a database, or a new workflow state format.
- Keeping retired source or historical runtime artifacts in the cleaned tree for convenience.

## Bootstrap dispatch instructions

This is the first vision and `implement-vision` does not exist yet. For this dispatch, the root Prime Agent should treat the intended workflow and authority rules in this document as the temporary implementation skill. It should inspect the current checkout, choose a safe PR decomposition, delegate substantive work, and use a fresh independent reviewer for each PR. The reviewer may merge after findings are repaired and required checks pass. The root must keep the human informed at meaningful milestones and verify the final merged state against this vision.
