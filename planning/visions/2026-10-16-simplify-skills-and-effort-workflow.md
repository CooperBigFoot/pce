# Simplify skills and Effort workflow

PCE should help people develop ideas and deliver them, without making them maintain a second project-management system. Charting already works as a breadth-first conversation; grilling develops depth. Preserve that distinction while making GitHub records shorter and removing formal Effort bookkeeping.

GitHub issues and PRs serve several purposes: a to-do list, context for later grilling, coordination across agent sessions, review, and audit. They are not always requests for human attention. Write the smallest useful context for the next action, rather than an account of everything the agent investigated.

## The skills that remain

Keep five workflow skills: `chart-program`, `grill-ticket`, `grill-me`, `to-vision`, and `implement-vision`. Keep `github-writing` and `test-first-development` as supporting guidance. Remove `land-ticket`; do not relocate its ceremony into another skill.

The workflows become:

- `grill-me` → `to-vision` → `implement-vision`
- `chart-program` → `grill-ticket` → `implement-vision`

A ticket-linked vision is a normal vision. An issue provides context and a way to find the vision, not a separate implementation protocol.

## Short GitHub records

Use precise titles and proportionate bodies. A one-line issue is valid when it gives the next session enough to begin. Do not require headings, action-item sections, evidence appendices, or a human-facing narrative on every agent working record.

A bug report usually needs the problem, how to reproduce it, and what should happen. The fixing agent must still verify the report. Include a version, revision, or other condition when it matters for reproduction. A short interpretation may help, but distinguish a hypothesis from an observed fact. Suggested fixes are optional, brief ideas, not proposed implementation code. Do not invent reproduction evidence for a finding based only on source inspection or an advisory.

Preserve extra evidence when it carries facts that cannot readily be recreated, such as an intermittent failure or a disappearing response. Link existing context instead of repeating it. Substantial supporting details can be linked or collapsed when useful; neither is mandatory. Keep blockers, material limitations, and necessary human decisions visible.

When human attention is needed, state the actual request. For example: “Please try selecting stations and copying a Python request. Does the flow make sense without explanation? I’m looking for usability feedback, not a code review.” Do not manufacture a request when none exists. No new audience-label scheme is part of this change.

PRs should explain the change and preserve useful verification context without becoming investigation diaries. Existing visions, issues, and PR evidence can carry detail once. Concision must not conceal an unverified result, a safety constraint, or unfinished work.

## Chart breadth; grill depth

A Program issue communicates the destination and links its Efforts. Include useful shared understanding where needed, but do not create a mandatory large Program document or duplicate an existing guide, research record, or vision.

Effort issues identify meaningful pieces of work to explore. They may contain different amounts of detail. Some can be one-line reminders; others can preserve decisions already reached in charting. Do not invent requirements or detailed solutions before the user has explored them. A provider-port Program can use the steps in an existing provider guide as a simple list of Efforts without copying that guide into each ticket.

`grill-ticket` uses the ticket and relevant Program context as input to the existing `grill-me` conversation. Agent-written issue content is not proof that the user endorsed its assumptions. Preserve settled decisions while investigating new evidence and discussing genuine user choices. Keep the existing discovery approach rather than inventing another interview mechanism.

Record material updates to understanding in concise issue comments, often bullets. Useful content includes a concrete finding, a decision and its reason, a specific unresolved question, or a consequence for the work. For example:

- The archive contains daily discharge only; stage needs a different source.
- We agreed to deliver discharge first.

Do not add comments merely to announce that plans can change or tickets are placeholders. Do not require a separate charting-summary artifact. Preserve useful understanding once, with links where it already exists. Earlier context informs later discovery but is not automatically current authority; reconcile it with newer decisions and evidence. The user should not have to keep a parallel context document continuously synchronized.

There is no prescribed backlog-reconsideration step after every Effort. The user asks for that conversation when needed. Completing an Effort does not authorize reorganizing remaining tickets or closing the Program.

## Finish work without a landing protocol

Remove Frontier computation, the formal landed predicate, special delivery and landing markers, exact Map-membership and dependency-declaration contracts, and their synchronization and validation machinery. Do not reproduce these requirements in ordinary prose or move them into supporting files.

Use ordinary issue links, open/closed state, linked PRs, and brief notes about real blockers. Real dependencies and repository permissions still matter; they do not require a custom dependency graph or a second completion ledger. Existing Program/Effort labels may remain useful for navigation, but no new label system is needed.

`implement-vision` retains meaningful testing, independent review, repairs, authorized merging, and final verification of the actual target-branch outcome. It also retains the ability to resume from repository and GitHub evidence without the original conversation. Remove the special Effort implementation track while preserving safe repository resolution, relevant context, user-state preservation, and ordinary publication and implementation authority boundaries.

When the vision’s outcome is delivered and verified, close its associated Effort and leave a short comment linking the implementation PRs and explaining the delivered result where useful. Publishing a vision is not delivery and must not close the Effort. Do not close an issue simply because one partial implementation PR merged. A standalone vision follows the same implementation process without an issue to close. Do not automatically close the Program.

Keep the authoring-to-implementation handoff: a vision must be published and verified on the intended target branch before implementation begins. Simplify how issue links carry that handoff; do not replace publication with an unpublished local draft or retain ticket-specific provenance machinery as a parallel workflow.

## Write guidance for capable agents

Apply the user-supplied article, “Rethinking skills and prompts for GPT-6 Astra,” as a design direction, not as a requirement to depend on a particular model:

- Keep descriptions short and precise about when the skill applies.
- Use progressive disclosure where it avoids loading unrelated instructions. Do not split a short skill into files merely to satisfy a pattern.
- Prefer intended outcomes, useful context, decision boundaries, and completion criteria over elaborate itineraries.
- Remove redundant handholding and repeated instructions. Keep guidance that expresses real user preferences, repository constraints, or important safety boundaries.
- Make any retained `AGENTS.md` pointers relevant to the task rather than requiring a stack of unrelated reading.
- Define completion clearly enough that an implementing agent continues through review and verification instead of stopping at a first pass.

This is a focused simplification, not removal of all safeguards or a model-specific rewrite. Keep the interview behavior, useful test-first guidance, independent review, private-data protections, and user authority over exceptional external actions. Leave reversible implementation details to the implementing agent.

## Repository work and evidence of success

The current machinery is spread across `chart-program`, `grill-ticket`, `implement-vision`, `land-ticket`, the Effort validation and implementation references, and `to-vision/publication.md`. `github-writing` currently requires preserving those machine-checkable contracts. Update these together so no remaining instruction sends an agent back into the removed protocol.

Update the README, installer, relevant repository guidance, and standard-library tests to match the seven-skill distribution. Remove obsolete supporting files and assertions rather than preserving the old protocol for compatibility. Historical visions and existing GitHub records are history, not a bulk-migration target. Do not bulk-edit Programs or rewrite old audit evidence.

Installer changes must preserve unrelated user state, including foreign skill paths and instruction content. Handle checkout-owned installation remnants of the removed skill without deleting user-owned material. Test installer behavior with an isolated temporary HOME. Keep PCE a lean skills distribution; add no orchestration runtime, package manager, CI workflow, or generated workflow state.

Success means a fresh agent can chart a short Program, grill a minimal Effort, publish a normal vision, and implement and close the Effort without Frontier, delivery records, landing records, or a separate landing invocation. Standalone visions still work. A vision publication alone leaves its issue open, partial implementation does not imply completion, and final closure follows verified delivery. Relevant tests should protect these outcomes and preserved safety behavior, not mandate replacement prose templates. Run the repository’s local suite with `python3 -m unittest discover -s tests -v`.

No implementation is authorized by publishing this vision. The implementing agent chooses the specific wording, supporting-file layout, and ordinary GitHub linkage details within these constraints.
