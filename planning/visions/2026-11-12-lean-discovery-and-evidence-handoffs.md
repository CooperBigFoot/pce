# Lean discovery and evidence handoffs

Reduce repeated investigation and modest instruction duplication in PCE discovery and vision authoring. This follows the [lean implementation vision](2026-11-12-lean-implementation-and-capable-model-delegation.md), delivered in [PR #233](https://github.com/CooperBigFoot/pce/pull/233). That earlier work is context, not work to repeat.

A static audit found the remaining skills already fairly lean. The useful opportunities are evidence reuse, bounded research delegation, and a small consolidation in `grill-me`, not a broad rewrite. No invocation traces were measured and no token-savings estimate is claimed.

## Intended changes

### Reuse evidence from discovery during authoring

Clarify `skills/to-vision/SKILL.md` so its pre-authoring investigation uses established findings from the completed discussion. Investigate gaps, changed facts, uncertainty, or conflicting evidence rather than automatically repeating the repository survey performed during `grill-me`.

Keep the separate post-draft check against confirmed decisions for omissions, weakened constraints, contradictions, and unapproved scope. Keep independent publication review. Reusing evidence is not permission to trust stale claims or omit the checks required to publish safely. The skill already prohibits repeating completed checks for an unchanged draft; preserve that intent.

### Delegate independent research questions when useful

In the discovery guidance used by `grill-me` and `chart-program`, allow capable smaller models to own substantive, bounded investigations such as reconciling a narrow history question, comparing sources, or investigating repository evidence.

When there are genuinely independent evidence questions, give each worker a clear, non-overlapping responsibility and request findings with relevant uncertainty. Reconcile the results before advancing the interview. Keep dependent discussion and design judgments with the lead. Do not parallelize every workflow step or ask several workers to repeat the same investigation.

This is optional judgment-based delegation, not a mandatory research stage. Do not add worker quotas, token budgets, routing machinery, fixed model tiers, model names, or new harness-specific instructions. `chart-program` already loads the canonical `grill-me` interview; reuse that relationship rather than duplicate shared guidance. Leave exact placement and wording to the implementing agent.

### Consolidate repeated grill instructions

Trim the repeated warning that a supplied issue or proposal is not settled user intent. Consolidate the overlapping depth-control explanations where clarity survives.

Preserve plain-language grounding, concrete conceptual examples, technical precision, user-controlled depth, frontier question batching, and recommendations with questions. Keep the ability to continue into design without authorizing implementation. Examples can be worth their token cost; do not remove useful explanation merely to shorten the file.

## Boundaries

Keep `grill-me → to-vision → implement-vision`, its confirmation and authorization boundaries, independent PR review, worktree isolation and preservation, published-intent checks, and actual-target verification. Do not weaken publication safeguards or replace them with a vague instruction to publish safely.

Keep progressive disclosure: GitHub-writing guidance loads when writing, publication guidance loads when publishing, and draft-only work stops before publication. Do not inline these references into every skill for uniformity.

The main scope is `skills/grill-me/SKILL.md`, `skills/to-vision/SKILL.md`, and a minimal `skills/chart-program/SKILL.md` adjustment only if needed. Related documentation or existing tests may change for consistency, but no changes are sought to the substance of `github-writing`, `save-discussion`, or the publication procedure. Leave the just-delivered implementation and testing guidance alone unless a concrete consistency issue requires a focused correction. Do not create a general delegation framework or redesign all skills.

This vision authorizes a focused guidance revision when handed to `implement-vision`, not installation into user environments. Publishing it alone does not start implementation. There is no associated issue to close.

## Evidence of success

A fresh agent should understand that authoring builds on valid discovery evidence, while investigating gaps and retaining the draft-consistency and publication checks. Research delegation should be optional, substantive, and non-overlapping, with results reconciled before the next interview batch. The shortened grill guidance must retain the same interaction quality and user control.

Review the changed guidance against these outcomes and its linked skills. Run `python3 -m unittest discover -s tests -v`. Reuse meaningful coverage; do not add tests that merely freeze prose or a new evaluation platform. No arbitrary word-count target or measured savings claim is required. Prefer a small change over replacing repetition with another checklist.

## Research context

[Anthropic's effective-agent guidance](https://www.anthropic.com/research/building-effective-agents) favors the simplest sufficient workflow. Its [multi-agent research article](https://www.anthropic.com/research/multi-agent-research-system) supports clear worker objectives and boundaries while warning about duplicated work and limited useful parallelism in coding. [NVIDIA's small-model position paper](https://research.nvidia.com/labs/lpr/slm-agents/) supports capable smaller models doing specialized substantive work, not universal equivalence to larger models. These principles inform the changes; they do not justify weaker safeguards or mandatory delegation.
