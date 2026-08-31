# Program and Effort-ticket workflow

## Purpose

The skill-only PCE workflow handles one coherent idea well, but it has no discovery layer for ideas too large to become one useful vision. Restore a lean Program layer without restoring the retired CLI, orchestration runtime, fixed planning schemas, or `grill-with-docs` workflow.

A Program is a large outcome tracked through a concise GitHub Map. The Map divides currently understood territory into ambitious, vision-sized Effort tickets, meaningful dependencies, and unresolved Fog. Each Effort receives its own discovery session and standalone vision before Prime Agent implements it.

The intended workflows are:

```text
standalone idea:
  grill-me → to-vision → implement-vision

large idea:
  chart-program → grill-ticket → implement-vision → land-ticket
```

## Product boundary

PCE should distribute six skills:

- Authoring environments, Claude Code and Codex: `grill-me`, `to-vision`, `chart-program`, `grill-ticket`, and `land-ticket`.
- Prime Agent: `implement-vision`.

The tracked PCE `grill-me` is the single source of truth for interview behavior. Program skills compose it instead of copying or modifying its question-loop rules. Retire `grill-with-docs` from this workflow. Program discovery must not edit `CONTEXT.md`, create ADRs, or restore the retired domain-modeling machinery.

PCE remains a skills distribution. Do not add an application, orchestration runtime, custom state store, scheduler, package manager, or CI workflow. GitHub Issues and repository vision documents are the durable coordination surfaces.

This vision supersedes the three-skill product boundary in `2026-08-31-skill-only-pce.md` while preserving that vision's removal of the retired runtime and its delegation of implementation planning to Prime Agent.

## Core model

### Program and Map

A Program is one large initiative. Its Map is a GitHub issue that communicates, at a glance:

- the destination and boundaries;
- open Effort tickets and the current Frontier;
- concise one-line outcomes from landed Efforts;
- unresolved Fog;
- explicit exclusions.

The Map is a compact index, not an implementation journal. Detailed vision, PR, validation, deviation, and recovery evidence belongs on the relevant Effort ticket and linked artifacts.

A repository may have multiple active Programs. Commands must use explicit issue identity rather than assuming a repository-wide singleton.

### Effort ticket

An Effort ticket is the durable feature record from initial charting through discovery, vision, implementation, and landing. It represents one ambitious, independently meaningful, contained outcome that can receive one coherent vision. It is not an implementation task, technical-layer slice, or promise to fit one PR or one agent session. Prime Agent may implement one Effort through several coherent vertical PR slices.

An Effort remains open until its complete vision is delivered and landed. GitHub assignment is its claim mechanism. Dependencies constrain delivery order; an open Effort may be grilled before its blockers land.

### Fog and Frontier

Fog is in-scope territory that is understood well enough to remember but not yet sharp enough to state as a contained Effort. Do not create speculative tickets for it.

The implementation Frontier consists of open Efforts whose recorded blockers have landed. Dependencies should express genuine outcome or delivery ordering, not an artificial total order.

## `chart-program`

`chart-program` accepts either a large idea for a new Program or an explicit Program issue for re-survey. It must inspect the repository, GitHub state, existing Maps, member tickets, comments, linked visions, delivery records, and other relevant evidence before asking questions that evidence can answer.

It composes `grill-me` at Program altitude. The interview surveys breadth-first across destination, boundaries, candidate Efforts, dependencies, success, exclusions, risks, and Fog. It must not deeply design one Effort or decompose implementation work.

Before GitHub mutation, present one concise intent-level proposal with this default shape:

- Destination: no more than three sentences.
- Effort tickets: one title and one-sentence question each.
- Dependencies: only meaningful blockers, one line each.
- Fog: short bullets.
- Exclusions: short bullets where useful.
- Re-survey changes: only substantive additions, removals, conflicts, or boundary changes.

Do not require the human to review generated issue bodies, templates, identifiers, placeholder substitution, API calls, unchanged-item inventories, or other mechanics. The user may request expanded detail for a particular Effort or dependency. Publish only after approval of the concise proposal, then generate and verify the GitHub representation deterministically.

A re-survey must reconstruct the complete current state with substantial effort, but keep questions and the approval report delta-focused. Do not reopen settled outcomes without contradictory evidence. Grill only new ideas, changed boundaries, conflicts, and Fog that may now be sharp.

If charting finds that the idea is one coherent Effort, create no one-ticket Program. Continue in the same session with `grill-me` at vision depth, confirm the shared understanding, and create a standalone vision through the `to-vision` mechanism. Stop with the vision path and its `implement-vision` handoff.

## `grill-ticket`

`grill-ticket` requires one explicit Effort issue number or URL. It must:

1. Load and validate the open Effort, its Program linkage, complete Map state, comments, relevant repository evidence, and any existing linked vision.
2. Claim an unassigned Effort by assigning the authenticated GitHub user. Treat assignment to that same user as a resumable claim. Stop without mutation when another user owns it, or when it is closed or malformed.
3. Compose PCE's canonical `grill-me` for deep discovery of this Effort only.
4. After confirmation, create one standalone vision under `planning/visions/` using the deterministic `to-vision` mechanism.
5. Link the vision and Effort in both directions and verify the links.
6. Stop before implementation and report the vision path and exact `implement-vision` handoff concisely.

An Effort-derived vision remains flexible but begins with minimal provenance:

```markdown
Program: <GitHub issue URL>
Effort: <GitHub issue URL>
```

A rerun on an open, same-user Effort resumes discovery. It loads existing durable state, grills only changed or unresolved territory, revises the same vision after confirmation, and records a concise revision note on the ticket. It must not create a competing second vision or silently discard settled content.

## `to-vision` naming behavior

When `to-vision` is invoked with a useful human-readable name, retain the current behavior. When invoked without a name, the agent must derive a concise, descriptive name from the confirmed shared understanding instead of asking the user to supply one. Ask only if the conversation does not contain enough information to choose a meaningful name.

Program skills may use the same deterministic creation mechanism directly after their composed grill completes. They should not require a redundant `/to-vision` invocation.

## `implement-vision`

Keep `implement-vision` responsible for native Prime Agent planning, delegation, implementation, independent PR review, and final vision-level verification.

For a standalone vision, behavior remains unchanged. When the vision contains an `Effort:` link, apply a conditional Effort delivery contract:

- reconstruct the Effort and Program context;
- respect recorded delivery blockers using ordinary tracker and engineering practice;
- ensure implementation PRs reference the Effort without allowing an individual PR to close it prematurely;
- after the complete vision is merged and verified, post a concise durable delivery record to the Effort containing the delivered outcome, merged PR links, validation evidence, material deviations, and unresolved follow-up risks;
- leave final Effort closure to `land-ticket`.

A fresh Prime Agent must be able to resume a valid partially implemented vision after another agent terminates. It should reconstruct merged, open, and remaining work rather than restarting or requiring a new grill solely because the earlier agent disappeared.

## `land-ticket`

`land-ticket` requires one explicit Effort issue number or URL. It is evidence-first and must not ask the human to explain code, PR contents, tests, technical decisions, implementation status, or other facts available from the repository and GitHub.

It reconstructs the ticket, vision, implementation PRs, target-branch state, validation, delivery record, Map, dependencies, and related Program evidence. It then follows this recovery hierarchy:

1. If delivery is complete but links or summaries are missing, repair the mechanical record from inspectable evidence and continue.
2. If implementation is incomplete and the vision remains valid, leave the Effort open and recommend resuming `implement-vision` with the same vision.
3. If implementation exposed a flaw, missing outcome, or obsolete assumption in the vision, leave the Effort open and recommend rerunning `grill-ticket` to revise that same vision.
4. Never require a regrill solely because an implementing agent terminated or omitted a comment.

After sufficient delivery evidence exists, close the Effort, add one linked outcome-level line to the Map, and recompute Program state. Inspect Fog against the landed outcome. Compose `grill-me` only when newly visible territory requires a genuine human intent, priority, scope, or outcome decision. If evidence and established practice settle the classification, complete landing without questions.

Newly sharp Fog becomes proposed Effort tickets. Retained uncertainty stays Fog. Any mutation proposal shown to the human should use the same concise, intent-level style as `chart-program`; mechanics remain the agent's responsibility.

When no open Efforts and no substantive Fog remain, show a short completion summary containing the destination, landed outcomes, and exclusions. Ask once before closing the Program Map. Do not close it without that confirmation.

## GitHub lifecycle and deterministic state

Use ordinary GitHub issue practices and the smallest reliable machine-readable contracts. At minimum, the system must make these relationships unambiguous and cold-session reconstructable:

- Effort to Program;
- Effort to its single vision;
- vision back to Program and Effort;
- Effort dependencies;
- implementation PRs to Effort;
- delivery evidence and landing state.

Prefer native GitHub relationships where reliable and useful in the UI, with deterministic textual linkage where needed for validation or fallback. Use labels and exact metadata consistently. Do not introduce local claim files or generated Program state.

Expected lifecycle:

1. Charted Effort: open and unassigned.
2. Grilled Effort: open, assigned, and linked to one vision.
3. Implemented Effort: open with linked merged PRs and delivery evidence.
4. Landed Effort: closed, with a compact outcome indexed by the Map.

Skill invocations authorize their ordinary expected mutations, subject to the concise proposal and confirmation gates described above. Partial GitHub failures must be reported precisely and left auditable; do not conceal or invent successful state.

## Interaction policy

Investigation depth must not become report length. Agents should spend substantial effort understanding current state and then communicate only decisions and evidence the human can evaluate.

Questions are reserved for intent, taste, priorities, outcome-level trade-offs, external authority, or contradictions that evidence cannot resolve. Routine workflow choices, tracker mechanics, code structure, test strategy, dependency APIs, and recovery bookkeeping belong to the agent. Follow the question cadence and completion gate defined by the canonical PCE `grill-me`.

## Observable completion

The redesign is complete when:

1. The tracked skill surface and installer match the six-skill environment matrix.
2. `chart-program` can create multiple independent Programs, re-survey one explicit Program, publish only an approved concise chart, and fall back to a standalone vision for a one-Effort idea.
3. `grill-ticket` deterministically validates and claims an explicit open Effort, composes `grill-me`, creates or revises one vision, establishes bidirectional linkage, and stops before implementation.
4. `to-vision` chooses a useful name from confirmed context when no name is supplied, while preserving explicit-name behavior and safe non-overwrite behavior.
5. `implement-vision` preserves standalone behavior and records verifiable delivery evidence for Effort-derived visions, including resumable partial implementations.
6. `land-ticket` reconstructs implementation without asking the human for inspectable facts, repairs safe bookkeeping gaps, distinguishes resume from regrill, evolves Fog, closes delivered Efforts, and asks once before closing a completed Program.
7. Program Maps and all reports remain concise at intent altitude while complete detail stays reachable through linked tickets, visions, PRs, and comments.
8. The workflow does not depend on `grill-with-docs`, domain-modeling documentation edits, the retired PCE CLI/runtime, or custom orchestration state.
9. Standard-library tests cover deterministic vision creation, no-name naming guidance where mechanically testable, installer behavior in isolated homes, linkage parsing and validation helpers if introduced, safe reruns, and failure behavior without mutating unrelated user state.
10. Repository documentation presents both standalone and Program workflows without reviving retired runtime concepts.

## Explicit non-goals

- Turning Effort tickets into implementation-task lists or enforcing one PR per Effort.
- Reintroducing the Rust application, graph runtime, work packages, overseer, hooks, event logs, or scheduling machinery.
- Restoring `grill-with-docs`, automatic glossary editing, or interview-time ADR creation.
- Requiring one active Program per repository.
- Creating one-ticket Programs by default.
- Asking the human to reproduce implementation evidence available to an agent.
- Making Program Maps carry full implementation history.
- Automatically starting implementation from `chart-program` or `grill-ticket`.
