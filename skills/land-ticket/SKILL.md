---
name: land-ticket
description: Verify and land one delivered Program Effort, repair safe records, evolve Fog, and optionally complete its Program. Use for /land-ticket with an Effort issue URL or number.
---

# Land Ticket

`$ARGUMENTS` must identify exactly one Effort issue by number or URL. Ask only for missing identity. Never assume one active Program.

## Reconstruct evidence

From the repository root, inspect the explicit ticket, comments and timeline, its single linked vision, Program Map, dependencies, implementation PRs, target-branch commits and files, checks and validation, delivery record, related Efforts, and Fog. Validate the `pce:effort` / `<!-- pce:effort -->` and `pce:program` / `<!-- pce:program -->` contracts, the unique `Program:`, `Depends on:`, and `Vision:` lines, and the vision's matching `Program:` and `Effort:` provenance.

Do not ask the human to explain code, PRs, tests, technical decisions, status, or any fact available through the repository or GitHub. Questions are only for genuine intent, priority, scope, outcome decisions, contradictions, or external authority.

A sufficient implementation record consists of an Effort comment marked `<!-- pce:delivery -->` that concisely records the delivered outcome, every merged PR URL, validation evidence, material deviations, and unresolved follow-up risks. Verify its claims against the target branch and GitHub rather than trusting the comment alone. PRs must reference the Effort but must not have closed it before landing.

## Recovery hierarchy

Apply the first matching result:

1. When delivery is complete but deterministic links or summaries are missing, repair only facts proven by inspectable evidence, verify them, and continue.
2. When implementation is incomplete and the vision remains valid, leave the Effort open and recommend resuming `implement-vision` with the same vision.
3. When implementation reveals a flawed outcome, missing requirement, or obsolete assumption in the vision, leave the Effort open and recommend rerunning `grill-ticket` on this same ticket and vision.
4. Never require regrilling only because an agent stopped, a PR split changed, or a comment was omitted.

If evidence is ambiguous or delivery blockers remain open, do not close the Effort. Report the exact gap. Do not manufacture delivery evidence.

## Land and evolve the Map

Once the complete vision and delivery record are verified on the target branch, close the Effort with a concise outcome comment. Add or update exactly one linked outcome-level line for it in the Program Map, remove it from open Efforts and Frontier, and recompute Frontier from the remaining open Efforts' dependencies.

Inspect the Program's Fog against the landed result. Classify from evidence where established practice settles it. Load and follow canonical PCE `grill-me`, rather than copying its question loop, only when newly visible territory requires genuine human intent, priority, scope, or outcome decisions. After confirmation, show one concise mutation proposal in the same intent-level form as `chart-program`. Newly sharp Fog may become proposed `pce:effort` tickets with the deterministic Program, dependency, and pending-Vision contracts. Retained uncertainty stays Fog. Obtain approval before those issue or Map mutations, then execute and verify them. Do not create speculative tickets.

When no open Efforts and no substantive Fog remain, show a short completion summary with destination, linked landed outcomes, and exclusions. Ask once for authority to close the Program Map. Close it only after that explicit confirmation. Do not ask again if the answer is no; leave it open and report that state.

Report the Effort URL and closure, verified delivery evidence, Map change, recomputed Frontier and Fog, any new approved Efforts, and Program status. Keep the report at outcome altitude and link detailed evidence.
