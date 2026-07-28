# Project domain context

## Canonical terms

| Term | Meaning |
|---|---|
| Program | A durable GitHub-tracked container for one big idea that requires multiple independent visions. A program is program-level discovery, not a build decomposition into milestones or steps. |
| Map | The single GitHub issue that holds a program's destination, notes, Decisions so far, fog, and links to its effort tickets. It is the program's sole durable state. |
| Effort ticket | A GitHub issue representing one ambitious, contained, vision-sized question within a program. Each effort ticket gets its own grill, vision, and PCE delivery cycle. |
| Fog-of-war / fog | Program territory that is acknowledged but not yet specified sharply enough to become an effort ticket. `Fog` is the accepted short form. |
| Frontier | The currently actionable set of open effort tickets whose blocking dependencies are satisfied. The frontier excludes fog and blocked tickets. |
| Fog-graduation | The land-time process that turns newly understood fog into seeded effort tickets and wires their blocking relationships without decomposing implementation work. |
| Chart | Survey a program breadth-first, create or refresh its map, mint only the effort tickets that are currently sharp, and leave unresolved territory in fog. Charting resolves no ticket. |
| Work | Claim one effort ticket, reconstruct its map and glossary context, grill it deeply toward one vision, and hand the result to vision creation. Working does not automatically run delivery. |
| Land | Trust the human assertion that the linked vision was delivered and merged, close the effort ticket, add its one-line decision to the map, graduate newly visible fog, and detect completion when the active map has no open linked effort tickets and no substantive fog. Map closure still requires separate confirmation. |
| Chart/work/land lifecycle | The program-level lifecycle in which charting establishes the visible map, working sharpens one effort ticket into a vision, and landing records the result and reveals the next frontier. |
| Decisions-so-far index | The concise section of the map that carries settled outcomes across effort tickets; an entry links an ADR when durable architectural rationale was recorded. It is an index, not a substitute for ADR detail. |
| Grill-with-docs | A one-question-at-a-time discovery process that combines deep interviewing with active maintenance of committed domain context and, only when warranted, ADRs. |
| Role frame | The versioned agent-facing scaffold the `pce` binary composes for one dispatch role, carrying ground-truth ref, read commands, environment hazards, output path, boundaries, and the role's fixed review obligations. The orchestrator supplies only the task-specific text. |
| Event log | The single ordered append-only file recording what only the orchestrator knows: one record per dispatch, deltas, escalations, key findings, measured repo contracts, and the identity of each approved planning artifact. Every record shares a fixed envelope — sequence, timestamp, kind, node, payload — over an open kind set whose payloads are validated per kind. Only the orchestrator appends; a record is written once, when the event occurs, and never revised. |
| Derived state | Run status computed on read rather than stored: whether a step merged, whether a branch or worktree exists, where a tag points, which node the run resumes at, how many rounds an artifact has spent, and which ref a dispatch was issued against. It is a fold over the three authorities named by the placement rule. Merge status is three-valued — merged, not merged, or inconclusive when the authorities disagree or one is unreachable — and inconclusive is never collapsed into either side. |
| Placement rule | The rule assigning every fact to its authority: git, `gh`, and the event log's own record sequence. A fact any authority implies is never also stored, so no running tally exists anywhere; a fact no authority implies is appended once, when it happens. |
| Run snapshot | The named, versioned JSON document the status verb emits: derived state, each repository's observation ref and fetch time, and a bounded recovery digest that names whatever it elided rather than truncating silently. It is the contract its consumers read; a human rendering is a separate view of the same computation. |
| Evidence command | The exact command or invocation that produced a claim, required on every event-log record asserting a fact about code, environment, or repository state. A fact-asserting record without one is rejected; a record that only reports an event carries none. |
| Labelled assertion | A factual claim the orchestrator inlines into a dispatch, admissible only when the receiving agent could not derive it at the ref, and only carrying the exact command that produced it. Every gate treats labelled assertions as its first verification target. |
| Decision hold | A keyed, idempotent escalation that blocks its dependent work and closes only when the human's answer is recorded and every dependent node is named and unblocked. |
| Convergence signal | The computed continuation test for a gate loop: blocker count strictly decreasing with no recurring finding. It replaces the round count as the escalation trigger; the round ceiling remains only as a cost backstop. |
| Enforcement split | The rule assigning each workflow invariant to a layer by its kind: prohibitions are command shapes enforced by hooks, verifications are computations owned by the `pce` binary, durable per-repo facts live in a tracked file read only from the default branch, and context the orchestrator must hold but cannot be relied on to fetch is injected at the harness boundary by a hook that fails open. |
| Delivery-trust boundary | The human's assertion that a vision was delivered and merged is authoritative and never independently verified. The boundary covers only that assertion; reading the vision, event log, merge commits, and pull requests to ground a landed decision or Fog graduation is required rather than forbidden. |
| Planning descent | The move from intent to mechanism, performed by the actor that reads the code at the ref. The orchestrator hands down what must become true; the descending actor returns the structure — write-set and genuine ordering — that only reading the source can establish. A level that has not read the code authors neither. |
| Concurrency admission | The rule deciding whether two ready nodes dispatch together. It is a recovery path, not a prediction: overlap dispatches and the merge absorbs the rebase, because a rule that must predict safety has to be conservative, and a conservative rule never fires on real code. |
| Domain-modeling | The active discipline of maintaining canonical terms, aliases-to-avoid, relationships, and ambiguities in committed `CONTEXT.md`, plus recording consequential durable decisions sparingly in `docs/adr/`. |

## Aliases to avoid

| Avoid | Use instead | Why |
|---|---|---|
| Roadmap | Map | A roadmap implies a predetermined sequence; the map preserves fog-of-war and only charts what is currently knowable. |
| Task | Effort ticket | An effort ticket is a vision-sized question, not an implementation task or planner-authored step. |
| Backlog | Fog or frontier | Unspecified territory and actionable tickets have different states and must not be collapsed into one queue. |
| Build plan | Program or map | The program layer chooses vision-sized efforts; milestones and steps belong to the downstream delivery planner. |
| State file | Event log | A run's durable file records events that already happened; current status is derived from git and `gh`, so calling the file "state" invites storing a claim that can contradict ground truth. |
| Resume instructions | Run snapshot | A written next-action narrative is a maintained claim sitting beside the facts that already imply it; the snapshot computes the resume point from the authorities instead. |
| Round counter | Dispatch records | A maintained tally drifts from the artifacts it summarizes, which is the observed failure; a round count is the number of dispatch records for that artifact. |
| Prompt template | Role frame | A template is copied by hand and drifts per dispatch; a frame is composed by the binary and cannot be omitted. |
| Disjoint write-set rule | Concurrency admission | Naming the rule after its current predicate freezes the predicate; the question is what admits concurrent dispatch, and the disjointness test is one answer that never fired. |

## Relationships

| Concepts | Relationship |
|---|---|
| Program and map | A program has exactly one open map, and the map is the durable index for all program state. |
| Map and effort ticket | The map links every effort ticket; each effort ticket points back to its program map with exactly one root-level `Program: #N` line and, after Work, carries exactly one root-level `Vision: planning/<YYYY-MM-DD>-<slug>` line required by Land. |
| Fog and effort ticket | Fog remains only on the map until fog-graduation can state a contained, seeded effort ticket. |
| Effort ticket and frontier | An open effort ticket joins the frontier only when its blocking dependencies are satisfied. Blocking is represented natively: the `addBlockedBy` GraphQL mutation creates the edge and `Issue.blockedBy` / `Issue.blocking` query it, verified on `gh` 2.89.0. The `Depends on: #M` body line remains the contingency for a failed native write. |
| Chart, work, and land | Chart creates the visible program surface, work turns one effort ticket into one vision, and land records delivery before exposing more work. |
| Decisions-so-far index and ADR | The index carries a one-line cross-ticket outcome; an ADR supplies durable rationale only when the decision crosses the ADR threshold. |
| Grill-with-docs and domain-modeling | Grill-with-docs uses domain-modeling throughout discovery so established vocabulary and durable decisions survive into cold sessions. |
| Program layer and PCE delivery | The program layer selects independent visions; each vision is delivered separately by PCE, whose milestone and step decomposition remains program-blind. |
| Event log and derived state | Together they replace a single rewritten state file: the log carries what no authority implies, derivation answers everything git, `gh`, or the log's own sequence can, and no fact is written in two places. |
| Run snapshot and the rehydration hook | The hook injects the snapshot's recovery digest after compaction and on a resumed or compacted session start, never on a cold start; it selects the run by most recent log record and injects nothing when that choice is ambiguous. |
| Event log and the per-repo contract | A measured contract is appended as a record because no authority implies it; the tracked per-repo file later changes the contract's source, not its home. |
| Evidence command and labelled assertion | Every labelled assertion is a fact-asserting record and so already carries its evidence command; what distinguishes it is additionally passing the admission test for entering a dispatch. |
| Role frame and the skill | The binary owns the frame, the skill owns the task; agent-facing obligations live in the frame so they cannot be dropped, and the skill keeps only orchestration logic. |
| Labelled assertion and the ref | The ref is the only authority and no actor's assertion is transitive, so an assertion is admissible only when it cannot be re-derived at the ref. |
| Convergence signal and decision hold | The signal decides when a loop stops; the hold defines what an unresolved stop owes the human before it can close. |
| Delivery-trust boundary and grill grounding | Trusting the delivery assertion does not license knowing nothing else; a landing grill recommends from the delivered artifacts and reserves trust for the merged-and-delivered claim alone. |
| Planning descent and concurrency admission | The descent produces the structure admission consumes; a write-set authored by a level that read no code cannot support any scheduling decision, whichever rule reads it. |

## Ambiguities

| Topic | Current interpretation | Resolution condition |
|---|---|---|
| Artifact class | One workflow serves code and prose deliverables alike; a prose diff is handled by a frame obligation on the reviewer, not by a separate path. Ceremony that only prose runs defend is treated as under-measured rather than as domain-specific. | Resolve once per-dispatch cost measurement shows whether those defenses hold on numbers. |
