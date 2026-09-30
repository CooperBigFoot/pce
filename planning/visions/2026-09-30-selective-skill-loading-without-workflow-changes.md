# Selective skill loading without workflow changes

## Outcome and baseline

Make PCE's six skills easier to navigate and load selectively without changing their successful workflow behavior. This is an instruction refactor, not a workflow redesign or a new audit of which safeguards to retain. The user reports that the workflow works well and approved a concrete evaluation before requesting this vision.

The research baseline is CooperBigFoot/pce immediately after PR #215, “Simplify PCE to its six-skill distribution,” merged into `main`: commit `826512a866392247c331dd594cc4dccfdb42b1e0`. At that baseline all 49 standard-library tests pass. Implement from the current intended target branch, preserving subsequent unrelated work; the baseline identifies the evaluated contracts, not permission to reset the repository.

The motivating article recommended short descriptions, progressive disclosure, and less prescriptive prompting. Its model-specific claims are not verified and do not justify removing checks. PCE remains model- and environment-independent. The defensible benefit is less irrelevant instruction loading and clearer composition, not a promised speedup or proven improvement in agent reliability.

## Evaluated problem

The baseline skill sizes are approximately 436 words for `grill-me`, 833 for `grill-ticket`, 1,071 for `chart-program`, 1,339 for `land-ticket`, 1,470 for `to-vision`, and 3,314 for `implement-vision`.

Length alone is not the problem. `grill-ticket` invokes `to-vision` publication, while publication refers back to `grill-ticket` for validation without assignment and to `implement-vision` for checkout preservation without implementation. These references are logically sound but can load unrelated workflows. Draft-only authoring does not need the roughly 936-word publication procedure. Standalone implementation does not need detailed Effort contracts.

The evaluation is complete. Implement the bounded changes below rather than conducting another open-ended optimization exercise.

## Selective disclosure and ownership

Introduce exactly four supporting Markdown references inside their owning skill directories. File names are an implementation detail; ownership and scope are settled:

1. **Effort implementation rules**, owned by `implement-vision`.
2. **Checkout and preservation policy**, owned by `implement-vision`.
3. **Vision publication procedure**, owned by `to-vision`.
4. **Read-only Effort validation**, owned by `grill-ticket`.

Every reference needs a direct link, an explicit loading condition, and a clear execution scope. Required rules must be loaded before their applicable actions. A reference is not optional background reading and does not invoke its owning workflow. Do not introduce recursive “load all related skills” instructions or a generic shared-document framework. Resolve links correctly through the installed skill directories as well as the repository checkout.

Keep six discoverable skills. The installer already links entire skill directories into its three supported locations; supporting files require no installer change. Do not split every repeated paragraph. Keep the short landed predicate visible in its independently invoked workflows and test its consistency.

### implement-vision

Keep input classification in `SKILL.md`, including safe repository/path resolution and promotion of an Effort-bearing vision path into the full Effort workflow. A path must never bypass the ticket or its authoritative Vision link. Load the Effort reference once an input is classified as Effort-derived, before applicable validation and before planning, delegation, branch creation, or substantive implementation.

Move detailed Effort validation, dependency rules, PR references, and delivery-record requirements into that reference. Retain explicit checkpoints in the main workflow for Effort validation, delivery blockers, and final recording. All three Effort entry forms—number, canonical URL, and provenance-bearing path—must converge on the same checks.

Order the main instructions by their existing execution sequence: resolve identity and target; validate published intent; reconstruct prior work; plan only the remaining outcome; execute and review; verify, reconcile records, and clean up. The baseline places “Plan the outcome” before “Reconstruct every run,” despite requiring reconstruction first. Correct presentation order without changing semantics.

Keep authority and stopping boundaries, evidence freshness and reuse, standalone durability checks, recovery classifications, complete no-op behavior, independent review, completion requirements, and product naming rules in the main document. Do not extract naming into another reference: it applies broadly to implementation and review. Preserve its compatibility, traceability, directly affected inherited architecture, and no-unrelated-cleanup distinctions.

Extract the complete checkout policy into its own reference. Link directly from implementation and publication, with loading and delegation requirements before checkout creation. Preserve placement below `.worktrees/`, the initial canonical clone exception, all checkout types and purposes, delegate coverage, source/history preservation, ownership limits, merged-branch rules, build-output exclusion, and final enumeration/reporting. Do not make the policy optional merely because the root agent does not expect to create a checkout; reviewers and delegates remain covered.

### to-vision

Keep naming, safe paths, authoring, fidelity checks, authority, and draft-only behavior in `SKILL.md`. Move the complete publication procedure into its reference and require it for publication and resumed publication. Draft-only authoring must not need to load that procedure.

`grill-ticket` and the single-Effort branch of `chart-program` must use that same publication procedure while retaining the existing authoring checks. Publication remains owned by `to-vision`; callers consume its verified, draft-only, or partial-failure result rather than repeating publication, linkage, or final verification.

Preserve publication order and all safeguards: staged-change isolation; ignored-path handling; pushed-content verification; independent review and required checks; fresh Effort state and ownership validation before issue mutation; commit-pinned linkage; inspection of closing references including negated phrases and the actual prospective merge/squash payload; permitted merge; exact target-copy verification; Effort open-state verification; and precise resumable partial-failure reporting. Direct publication to the read-only validation reference without executing assignment or the interview. Direct it to the checkout reference without invoking implementation.

### grill-ticket

Move read-only Effort state and ownership checks from “Validate and claim” into its reference. Keep assignment and assignment verification in `SKILL.md`. Both discovery and publication must explicitly load the applicable validation rules before acting.

Preserve draft-only detection before mutation, including assignment. Draft-only still validates state and ownership. Another assignee blocks both modes. Keep discovery-before-confirmed-authoring behavior and the distinction that blockers constrain delivery, not discovery.

Replace “Author a standalone vision” with “Author a self-contained Effort vision.” Here self-contained means understandable without the conversation; standalone elsewhere means no Effort provenance. This is terminology clarification only.

### chart-program

Make only small local edits. Shorten the single-Effort publication handoff using the direct shared procedure, retaining its authoring checks, approval, publication authority, draft-only handling, verified handoff, and stop before implementation. Make new-Program versus re-survey conditions clear within the existing document. Do not create separate workflow documents for these branches.

### land-ticket

Retain the structure and landing-specific checkout gate. Its inspection of related checkouts and closure decision are not identical to implementation cleanup and must not be replaced by that policy alone. Limit edits to clear local wording improvements that preserve meaning.

Keep explicit requirement-to-evidence checks, safe repair versus implementation resumption versus regrilling, dependency and target-copy gates, unique landing records, no scope expansion, and separate permission to close the Program.

### grill-me

Leave the body unchanged. Its compact interview method, question format, investigation policy, and confirmation boundary are the intended behavior, not excess scaffolding.

## Skill descriptions

Use these concise descriptions, preserving distinct entry points rather than advertising broader authority:

- `grill-me`: Clarify an idea through focused questions. Use for grilling, stress-testing a plan, or reaching shared understanding before a vision.
- `to-vision`: Turn confirmed understanding into a published repository vision, or a local draft when requested. Use when asked to write or revise a vision.
- `implement-vision`: Implement or resume a published vision. Accept a vision path or an Effort issue number or URL.
- `chart-program`: Chart a large outcome into a GitHub Program and Efforts, or re-survey an explicit Program. Use for /chart-program.
- `grill-ticket`: Claim and discover one Effort, then publish its vision. Use for /grill-ticket with an Effort issue number or URL.
- `land-ticket`: Verify and land one delivered Effort and update its Program Map. Use for /land-ticket with an Effort issue number or URL.

## Non-negotiable behavior and exclusions

Preserve every existing authority, safety, recovery, review, and completion guarantee. In particular, retain exact identity/provenance and target-content checks, dependency semantics, read-back verification, safe reruns, unique records, draft-only boundaries, protected-branch policy, independent review, checkout preservation, and the distinction between confirmation, publication, implementation delivery, and landing. Keep evidence reuse conditional on freshness. Continue reconstructing from Git/GitHub rather than prior conversation or runtime state.

Do not remove tests or review based on claims about newer models. Do not substitute “use judgment” for deterministic gates. Do not expand publication, merge, cleanup, assignment, or closure authority. Do not change Program/Effort formats or invent new records.

No installer changes, new scripts or validators, runtime or orchestration machinery, generated state, package manager, CI workflow, model-specific variant, benchmark platform, README expansion, or AGENTS.md change. Keep existing helpers unchanged. Do not add supporting files merely to shorten root files, extract additional topics, or impose an arbitrary percentage-reduction target. The implementation scope is skill instructions and their standard-library contract tests, not repository documentation redesign.

## Acceptance and evidence

Run `python3 -m unittest discover -s tests -v` from the repository root. Preserve existing contract coverage when rules move. Existing tests often inspect phrases in individual root skill files; update those tests to the new owned locations without deleting behavioral obligations or merely concatenating all documents into an apparent pass.

Add focused standard-library checks that references exist, installed-directory-relative links resolve, and callers explicitly require them at the correct stage. Extend relevant instruction scans to supporting Markdown, not just root SKILL.md files. Retain ordering checks for validation, publication, and mutation. Tests must catch a required rule becoming unreachable or optional, even if its text remains somewhere in the repository. Static checks are not evidence of actual model execution.

Review the complete resulting reading paths against these cases and include the findings in normal PR/review evidence, without adding a persistent evaluation framework:

- Standalone implementation skips detailed Effort rules while retaining common durability and execution gates.
- Effort implementation by number, canonical URL, and provenance-bearing path reaches identical validation and delivery requirements.
- Draft-only authoring avoids publication instructions; draft-only Effort discovery still validates ownership without assignment.
- Direct to-vision publication, grill-ticket publication, and single-Effort chart publication use one shared publication owner and preserve authoring checks.
- Interrupted publication and implementation resume from durable evidence without duplicate records or repeated completed work.
- Already-complete implementation remains a verified no-op.
- Ambiguous identities and duplicate records stop at the existing boundaries.
- Unlanded blockers retain their existing delivery constraints.
- Closing references and merge payloads retain full safeguards.
- Cleanup preserves unique work and uncertain ownership, including delegated checkouts.

Report before/after root and applicable-reference word counts for representative standalone implementation, Effort implementation, draft-only authoring, and publication reading paths. Do not claim a saving solely because words moved out of a root file; count every required reference and distinguish initial loading from later required loading. No speed or reliability claim is required.

A successful result has fewer irrelevant instructions on the applicable paths, direct scoped reuse instead of loading unrelated workflows, the four bounded references, readable main documents, and no lost contract. Independent review must examine both the full diff and the reading paths. Live GitHub mutations are not needed to validate the refactored instructions; ordinary PR publication and review remain part of implementation delivery.
