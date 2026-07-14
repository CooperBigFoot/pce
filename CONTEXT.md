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
| Land | Close a delivered effort ticket, add its one-line decision to the map, graduate newly visible fog, and detect whether the program is complete. |
| Chart/work/land lifecycle | The program-level lifecycle in which charting establishes the visible map, working sharpens one effort ticket into a vision, and landing records the result and reveals the next frontier. |
| Decisions-so-far index | The concise section of the map that carries settled outcomes across effort tickets; an entry links an ADR when durable architectural rationale was recorded. It is an index, not a substitute for ADR detail. |
| Grill-with-docs | A one-question-at-a-time discovery process that combines deep interviewing with active maintenance of committed domain context and, only when warranted, ADRs. |
| Domain-modeling | The active discipline of maintaining canonical terms, aliases-to-avoid, relationships, and ambiguities in committed `CONTEXT.md`, plus recording consequential durable decisions sparingly in `docs/adr/`. |

## Aliases to avoid

| Avoid | Use instead | Why |
|---|---|---|
| Roadmap | Map | A roadmap implies a predetermined sequence; the map preserves fog-of-war and only charts what is currently knowable. |
| Task | Effort ticket | An effort ticket is a vision-sized question, not an implementation task or planner-authored step. |
| Backlog | Fog or frontier | Unspecified territory and actionable tickets have different states and must not be collapsed into one queue. |
| Build plan | Program or map | The program layer chooses vision-sized efforts; milestones and steps belong to the downstream delivery planner. |

## Relationships

| Concepts | Relationship |
|---|---|
| Program and map | A program has exactly one open map, and the map is the durable index for all program state. |
| Map and effort ticket | The map links every effort ticket; each effort ticket points back to its program map. |
| Fog and effort ticket | Fog remains only on the map until fog-graduation can state a contained, seeded effort ticket. |
| Effort ticket and frontier | An open effort ticket joins the frontier only when its blocking dependencies are satisfied. |
| Chart, work, and land | Chart creates the visible program surface, work turns one effort ticket into one vision, and land records delivery before exposing more work. |
| Decisions-so-far index and ADR | The index carries a one-line cross-ticket outcome; an ADR supplies durable rationale only when the decision crosses the ADR threshold. |
| Grill-with-docs and domain-modeling | Grill-with-docs uses domain-modeling throughout discovery so established vocabulary and durable decisions survive into cold sessions. |
| Program layer and PCE delivery | The program layer selects independent visions; each vision is delivered separately by PCE, whose milestone and step decomposition remains program-blind. |

## Ambiguities

| Topic | Current interpretation | Resolution condition |
|---|---|---|
| Ticket-to-vision linkage | An effort ticket will carry a machine-readable reference to the vision directory created from it. | Settle the exact line format when the work-ticket skill and land-ticket reader are specified together. |
| Land-time delivery evidence | Landing currently assumes the invoker has confirmed the ticket's vision was delivered. | Resolve when the land-ticket contract decides whether to trust the invoker or verify delivery independently. |
| Native GitHub blocking | The frontier prefers native GitHub blocking relationships; a `Depends on: #N` body line is the fallback. | Resolve after verifying which blocking operations the supported `gh` CLI surface can create and query. |
| Program completion | A program is complete when no effort tickets remain open and the map's fog is empty, after which map closure still requires confirmation. | Confirm the exact prompt and final-summary behavior when the land-ticket skill is authored. |
