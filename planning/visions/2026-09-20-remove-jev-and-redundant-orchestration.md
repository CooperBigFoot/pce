# Remove Jev and redundant orchestration

## Outcome

Keep PCE a lean, language-agnostic orchestration workflow by removing Jev from its active workflows and consolidating three identified sources of repeated work. Preserve the existing six skills, durable handoffs, independent review, recovery, and human-authority boundaries.

The user reports that PCE agents find no added value in Jev and has decided to remove it. A read-only review of rust10x and an external prompt about unnecessary intermediate work informed this exploration. Neither resource is a dependency or a commitment to adopt its implementation, terminology, or conventions. The orchestration audit identified instruction-level overlaps; it did not measure runtime savings or establish that every agent repeats the work.

## Product and scope boundaries

PCE is solely an orchestration workflow. It makes no language-specific assumptions and does not define how downstream projects should code. This change must not introduce engineering standards, an engineering handbook, architectural pattern libraries, language conventions, or a mandatory optimization audit on every invocation.

Downstream documentation is not an incidental delivery duty. Documentation work must be explicitly targeted in the agreed vision, not added by an agent as extra maintenance. This vision explicitly includes updating PCE's own active documentation to reflect the removal and workflow changes. It does not authorize documentation changes in downstream projects or unrelated cleanup of existing PCE policies.

Retain the standalone and Program workflows and their existing authorization boundaries. Do not automatically start implementation after discovery or publication, combine implementation with landing, remove the landing record, or change Program closure authority. Do not add a runtime, scheduler, persistent cache, generated state, replacement service, or new workflow record.

## Remove active Jev integration

Remove the Jev helper, shared service contract, active skill hooks, credential and sharing setup instructions, and dedicated Jev tests. Replace any still-needed checks with ordinary agent instructions rather than another hosted model, scoring system, helper protocol, or fallback mechanism.

The inspected active surface is:

- `skills/implement-vision/scripts/semantic_decisions.py`;
- `skills/implement-vision/semantic-decisions.md`;
- hooks in `to-vision`, `chart-program`, `grill-ticket`, `implement-vision`, and `land-ticket`;
- the automatic Jev support section in `README.md`;
- `tests/test_semantic_decisions.py`.

`grill-me` has no Jev hook. The installer has no Jev-specific integration; its existing six skill-directory symlinks expose the resources to all supported environments. No installer redesign or data migration is needed.

Preserve source investigation, fidelity to confirmed decisions, checks against unapproved scope, fresh independent full-diff review, and requirement-to-evidence reasoning. Tests, actual source and target effects, Git/GitHub state, exact content equality, provenance, dependency validation, and human authority remain the basis for verification. Removing advisory judgments must not remove these safeguards.

Historical visions, including `planning/visions/2026-09-17-automatic-jev-decisions-in-pce-skills.md`, remain historical records. This vision supersedes that vision's requirement for active Jev integration. Do not purge historical mentions, alter credentials or account state, read local secrets, or remove independently installed TypeSafe skills and unrelated user settings.

## Execute shared vision publication once

`grill-ticket` currently composes the complete `to-vision` publication contract, then restates link mutation and final verification instructions. Literal execution can repeat work already owned by the shared procedure.

Make ownership unambiguous: `grill-ticket` owns Effort discovery, claim, and confirmed vision preparation; the shared publication procedure owns publication, the single link mutation, independent review, merge, and final read-back verification. The caller consumes that verified result rather than executing a second publication tail. Apply the same single-execution principle to other callers of the shared procedure.

This consolidation must preserve the initial claim validation and fresh state validation before link mutation, particularly after an interview or review interval. Preserve commit-pinned linkage, closing-reference safeguards, exact target-branch content verification, matching provenance, and verification that the Effort remains open. Preserve draft-only behavior, partial-failure reporting, and resumption using the same path, branch, PR, and single link. Rechecks across actual state changes are not redundant merely because they concern the same facts.

## Reuse current evidence within an invocation

`implement-vision` currently requests substantially overlapping repository and GitHub evidence during input validation and recovery reconstruction. Allow one evidence-gathering pass to serve both purposes when that evidence remains current. Validate input and durability first; classify prior work and plan only after the validation gate succeeds. Do not repeat reads solely because execution reaches another instruction section.

This is in-session reuse, not a persistent cache or a replacement authority. Every fresh invocation must reconstruct from Git and GitHub without relying on a prior conversation. Refresh affected evidence after relevant state changes, external activity or waits that may make it stale, before consequential mutations, and for final verification. Do not treat an earlier valid snapshot as proof of current state.

Preserve complete dependency-graph validation, the existing merged/open/abandoned/incomplete/remaining work account, target-effect verification, no planning or delegation before valid input, and safe already-complete reruns. Uncertain or conflicting evidence must still be investigated.

## Approve complete Fog proposals once

`land-ticket` currently separates interview confirmation from approval of the resulting Fog-related issue and Map changes. Use the existing `chart-program` model: when the confirmation summary contains the complete intent-level mutation proposal and explicitly requests authorization for those changes, one approval is sufficient.

The proposal must make the proposed outcomes, Efforts, genuine dependencies, retained Fog, and relevant exclusions clear. Mere agreement with an interview summary does not authorize unspecified mutations. If the summary omits proposed changes, or the proposal materially changes afterward, obtain approval for the missing or changed scope before mutation.

Preserve the prohibition on speculative tickets and all mutation read-back checks. Approval to evolve Fog does not authorize Program closure; retain the separate explicit Program closure decision. Do not change standalone `grill-me` confirmation into publication or implementation authority.

## Evidence of completion

The implementing agent must demonstrate that:

1. Active PCE instructions, support files, and setup documentation no longer require or invoke Jev, its API, credentials, or fallback behavior. Historical records and unrelated user state remain intact.
2. Native fidelity, scope, independent review, and evidence checks remain explicit where needed, with deterministic gates and authority boundaries unchanged.
3. Shared publication has one execution owner, without a second caller-side linkage or verification tail. Fresh pre-mutation and post-merge checks, draft-only behavior, recovery, and the Effort open-state requirement remain intact.
4. Same-invocation validation and reconstruction can reuse current evidence without weakening fresh-invocation recovery, refresh requirements, input validation order, or final verification.
5. A complete, explicitly authorized Fog proposal needs one approval; incomplete or changed proposals still require approval, and Program closure remains separately authorized.
6. The standard-library test suite passes with coverage appropriate to these instruction contracts. Retain positive checks for the preserved safeguards as well as checks for removal of active Jev integration. For bug fixes, first demonstrate a failing regression test on the old behavior or instruction contract, then make that same test pass.

Run local validation from the repository root with `python3 -m unittest discover -s tests -v`. Any installer-related tests must continue using an isolated temporary `HOME`; do not mutate the real user installation to test removal. Do not claim runtime performance improvements from instruction changes alone.

Publication of this vision authorizes no implementation. A later explicit `implement-vision` invocation executes the agreed outcome.
