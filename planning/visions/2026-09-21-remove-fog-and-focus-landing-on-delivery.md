# Remove Fog and Focus Landing on Delivery

## Outcome

Remove Fog as a concept throughout the active PCE workflow. Program Maps describe agreed work and delivered outcomes, not a speculative uncertainty inventory. Landing verifies delivery; it does not search for future work.

Repeated use has shown that agents populate Fog because the instructions require the section, even when no such uncertainty was discussed. Making it optional is not the intended change. Do not replace it with a renamed backlog, mandatory risk inventory, or another section that invites invented work.

## Settled behavior

- `chart-program` no longer requests, generates, preserves as a workflow obligation, or reports Fog. Its proposal and Map retain the destination, meaningful Efforts, genuine dependencies, Frontier, landed outcomes, and relevant exclusions.
- Discovery still resolves material questions about the proposed outcome. Future work can emerge during a grilling session, but agents must not invent work to populate a template. Concrete risks belong with the relevant issue or vision when useful.
- Concrete problems found during implementation can be recorded in ordinary GitHub issues and addressed as needed. No separate Program uncertainty inventory is required. This change does not introduce a new issue-management workflow or grant blanket authority to expand scope.
- `land-ticket` verifies the agreed delivery, lands the Effort, updates the Map, and recomputes Frontier. Remove its future-work discovery, Fog review, Fog-to-Effort conversion, and associated expansion-approval process. A concrete delivery problem must still be investigated and reported; it is not an invitation to manufacture a next-work agenda.
- Program completion no longer depends on clearing Fog. Retain the no-open-Efforts condition, delivery verification, and explicit human approval before closing the Program. An explicitly requested `chart-program` re-survey remains the route for changing Program scope.

## Repository context and scope

The current obligations appear in `skills/chart-program/SKILL.md`, including the interview scope, approval proposal, Map contract, re-survey behavior, and final report. `skills/land-ticket/SKILL.md` includes Fog in reconstruction, post-landing discovery and mutation, the completion condition, and reporting. `README.md` advertises this behavior. `tests/test_skill_contracts.py` currently enforces the Fog mutation approval contract.

Update active skill instructions, repository documentation, and affected tests consistently. Inspect the full active product surface for remaining obligations rather than treating this as removal of one heading. Historical visions are context, not active workflow requirements; do not rewrite historical records merely to erase the word.

Existing GitHub Programs are not subject to bulk editing under this vision. Legacy Fog text must not remain a prerequisite for using the revised workflow or for proposing completion. This does not authorize deleting unrelated issue content or silently converting old Fog into tickets.

Keep PCE a lean, language-agnostic collection of skills. Do not add a runtime, package manager, compiled application, CI workflow, replacement tracking system, or generated state. Preserve unrelated user state.

## Evidence of success

- A new chart can be approved and published without any Fog section, speculative placeholder, or replacement uncertainty inventory.
- Re-survey operates on agreed scope changes without requiring a Fog inventory.
- Landing a complete Effort performs delivery verification and Map updates without future-work discovery, interviews about future work, or proposed new Efforts.
- A Program with no open Efforts can be presented for explicit closure approval without a Fog-clearance condition, including when historical Fog text exists.
- Concrete blockers, incomplete requirements, and evidence of a flawed vision continue to trigger the existing delivery and recovery safeguards. Removing Fog does not weaken dependency, provenance, publication, landing-evidence, or human-approval gates.
- Contract tests demonstrate the removed obligations and retained safeguards. For behavioral bug fixes, first establish a failing regression test against the current instructions, then make the same test pass. Run `python3 -m unittest discover -s tests -v` from the repository root.

This is a standalone vision. Publication authorizes no implementation or live Program migration by itself.
