# Lean implementation and capable model delegation

Make `implement-vision` less wasteful without weakening delivery quality. The user reports expensive agents spending too many tokens on implementation work. This discussion identified instructions that can encourage excessive work, but did not measure run traces or establish a savings estimate.

The intended result is shorter guidance centered on outcomes, boundaries, and responsibilities. Agents should exercise judgment about routine execution rather than follow an expanding checklist.

## Preserve the workflow and safeguards

Keep `grill-me → to-vision → implement-vision`: discover and confirm intent, publish a repository vision, then implement it. Publishing this vision does not authorize its implementation.

Preserve dedicated worktrees for isolated implementation work, protection of unrelated user changes, published-intent checks, meaningful testing, permissions and approvals, and verification of the delivered outcome on the actual target branch. Keep the existing checkout preservation and ownership protections; do not trade isolation for fewer instructions.

Every implementation PR must receive independent review by an agent who did not implement it. Smaller models can review independently; independence does not require a more expensive model. Preserve justified resolution of findings and honest reporting of incomplete validation or review.

## Delegate substantive work to capable smaller models

Teach a harness-independent philosophy in the skill:

- Prefer smaller, capable models for implementation, investigation, testing, and review when the task suits their capabilities. They are not limited to extraction, summaries, or administrative chores.
- Give delegates clear responsibilities and sufficient context to work independently.
- Reserve stronger models and deeper reasoning for genuinely demanding work, such as ambiguous architectural decisions, difficult unresolved debugging, and complex cross-cutting changes.
- Match reasoning effort to the difficulty instead of maximizing it by default. Escalate when the work proves harder than expected.

Keep the shared task-to-model philosophy in `SKILL.md`, without hardcoded model names. Use progressive disclosure for the harness-specific preferences: link two small Markdown reference files and instruct agents to load only the one matching their current harness.

- **Prime Agent reference:** use the GPT family.
- **Antigravity CLI reference:** use Sonnet or Gemini Flash, depending on availability.

These references contain model-family preferences, not execution recipes. Do not add model discovery procedures, routing APIs, runtime configuration, lifecycle instructions, cost-accounting procedures, token budgets, classifier infrastructure, or a model-selection framework. Leave reference filenames and placement to the implementing agent; do not turn this into a shared harness framework.

## Remove unnecessary work

Revise `skills/implement-vision/SKILL.md` and directly related guidance where necessary:

- **Bound reconstruction.** Inspect relevant source, history, issues, branches, and validation only as needed to establish the remaining outcome and proceed safely. Avoid an exhaustive history audit merely because those sources exist. Reuse evidence while it remains valid.
- **Separate delegation from PR topology.** One accountable implementation owner may integrate bounded contributions into one coherent PR. Delegating a task must not automatically require another branch, worktree, or PR. Preserve dedicated worktree isolation for implementation owners and avoid conflicting concurrent edits. Choose additional delivery slices when the work warrants them, not to mirror agent count.
- **Keep review proportional.** After changes, examine their effects and verify prior findings rather than automatically restarting the whole review. Broaden review when changes warrant it. Preserve the initial independent review and do not treat partial examination as whole-PR approval.
- **Prevent delivery noise in new code instead of mandating legacy cleanup.** Use established repository and domain vocabulary; name production components for their stable responsibility, not tickets, visions, or implementation phases. Remove mandatory repair of inherited naming problems in components touched or depended upon solely because this workflow encountered them. Necessary changes remain governed by the vision and compatibility requirements. Delivery identifiers may remain in appropriate traceability metadata.
- **Replace scoring rituals with judgment.** In `skills/test-first-development/SKILL.md`, remove mandatory numerical likelihood-and-impact scoring for potential failures. Keep practical prioritization by realistic risk, meaningful test-first development, and resistance to speculative fixes and unnecessary coverage.

Trim repetitive or overly prescriptive prose where the same safeguard can be stated clearly once. Do not replace deleted instructions with new planning artifacts, mandatory reports, or another detailed procedure. Keep useful evidence, actual blockers, and material limitations visible.

## Repository facts and boundaries

At authoring, the installed and repository copies of `implement-vision` matched. The skill already asks agents to reuse evidence, makes experimentation optional, and requires repeat review only after material changes; it does not explicitly mandate a full review restart. Preserve those useful intentions rather than claiming every inefficient execution was required by the text.

The current checkout policy allows the current checkout when safe and constrains agent-created checkouts to `.worktrees/`. Preserve its protection of source, ownership, and uncertain work. Keep dedicated worktrees as the isolation approach for implementation; do not create separate checkouts for every read-only or supporting task.

This is a focused revision of workflow guidance, not a redesign of all PCE skills or a new orchestration runtime. Related documentation and existing tests may be updated to keep the changed guidance consistent. Leave exact wording and reversible implementation details to the implementing agent. Do not install the revisions into user environments unless separately authorized.

## Evidence of success

A fresh agent reading the revised guidance should understand that smaller, capable models can own real implementation and review work, with escalation based on task difficulty rather than a default to the strongest model. The main skill should remain model-agnostic and load only the relevant harness reference: GPT-family guidance for Prime Agent, or Sonnet/Gemini Flash guidance for Antigravity CLI depending on availability. Neither reference should prescribe delegation APIs or expand into a harness manual.

The revised workflow should allow a small change to proceed with focused investigation, isolated implementation, one coherent PR, independent review, and actual-target verification—without mandatory historical archaeology, unrelated naming repairs, numerical risk scoring, or a separate PR per delegate. Larger work may still use multiple owners and PRs where justified.

The instructions should be materially shorter and clearer without losing the safeguards above; no arbitrary word-count or token-savings target is required. Validate consistency with linked guidance and run the repository's standard tests: `python3 -m unittest discover -s tests -v`. Do not add tests that merely freeze prose or invent an evaluation platform to demonstrate this change.

## Research informing the direction

- [NVIDIA: Small Language Models are the Future of Agentic AI](https://research.nvidia.com/labs/lpr/slm-agents/) advocates smaller models for specialized work and heterogeneous systems for more demanding reasoning. This is a position paper, not proof that every small model can implement every feature.
- [Anthropic: How we built our multi-agent research system](https://www.anthropic.com/research/multi-agent-research-system) describes a stronger lead with capable, less expensive workers and clear delegation boundaries. Its findings concern research; it cautions that coding often offers less independent parallel work.
- [Google: Cost per task](https://developers.googleblog.com/cost-per-task-using-cheap-classifiers-to-route-across-gemini-models-and-thinking-levels/) supports matching model and reasoning effort to task difficulty. Its small, short-task experiment is not an end-to-end coding benchmark. Borrow the principle, not its routing machinery.

This is a standalone vision with no associated issue to close.
