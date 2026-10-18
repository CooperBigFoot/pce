# Continuous design discussions and experiment handoffs

## Outcome

PCE should support a continuous discussion that develops an idea, explores its design, branches into experiments when evidence is missing, and resumes with the results. The human controls depth and pace. Preserve useful understanding across sessions without requiring a finished specification or making the human carry context between agents.

Keep PCE skill-centered. This is a change to skills, their supporting guidance, installation, tests, and documentation, not a new orchestration system. The user's projects primarily concern data science and machine learning; do not assume browser applications or UI prototypes.

## One conversational entry point

`grill-me` accepts an idea, existing implementation, bug report, or an issue number or URL. An issue number resolves in the verified current repository; a canonical URL identifies its repository. Resume from the issue, comments, linked evidence, relevant Program context, and current repository facts. Reconstruct settled decisions, unresolved questions, and changed assumptions rather than restarting the interview or treating the original issue body as permanent authority.

A session can clarify desired behavior and then smoothly continue into API, representation, or architectural design when the human wants. Shared understanding is not an automatic stopping point when the human asks to explore further. The agent contributes concrete examples, alternatives, explanations, and recommendations, rather than only extracting requirements. Load focused supporting guidance when relevant; the human should not need to navigate a sequence of workflow commands.

There is no mandatory clarification-to-design-to-experiment pipeline. The human may delegate design for a quick prototype, try the result, and later return to understand and redesign it. Close participation in design and the quality standard for a deliverable are separate choices. Existing behavior is evidence, not automatically the correct scientific contract to preserve. Keep the existing distinction between facts agents investigate and meaningful choices for the human. Preserve the interview's research-before-questions discipline and user control of depth.

`chart-program` remains the breadth-oriented conversation for a larger destination and meaningful pieces of work. `grill-me` explores one area in depth; its existing breadth-first batching of independent questions within a round need not be replaced by serial questioning.

Retire `grill-ticket`. Its discovery role is handled by `grill-me <issue>`, and it must not survive as a parallel protocol or an automatic publication path. When satisfied, the human explicitly invokes `to-vision`. Confirmation of understanding alone does not publish or implement anything. Explicit requests to proceed on small work without a vision remain possible; this does not make unpublished drafts valid inputs to `implement-vision`.

## Save and resume one kind of discussion issue

Provide a small save capability, naturally usable when the human says “let's save this.” A dedicated skill is appropriate; its exact name and file organization remain implementation choices. It creates a normal GitHub issue, or appends useful new context to the issue already associated with the discussion. Saving does not require completing discovery, inventing implementation requirements, or publishing a vision.

Use the `discussion` label to identify saved discovery/design context. Use ordinary open/closed state, not additional in-progress/done labels or a new status system. These are GitHub issues, not GitHub Discussions. The label replaces the ticket-specific classification for newly charted work rather than introducing another category alongside it. Historical issues and labels are not a bulk-migration target.

A saved standalone grill and a ticket created by charting follow the same mechanism. Program membership adds shared context, not another implementation protocol. A Program coordinates the broader objective and related discussion issues; it does not require a standalone discussion to acquire a parent Program. Preserve the useful Program/Effort vocabulary where needed without retaining a separate ticket-specific track.

Write saved context primarily for agents continuing the work: purpose, useful shared understanding, settled decisions and reasons, uncertainties, supporting evidence, and the next useful continuation. Use only what matters for the particular discussion. This is neither a transcript dump nor a mandatory heading template. Preserve incomplete but valuable understanding without making a fresh agent believe it must immediately finish a specification.

Comments append context. Do not delete or silently rewrite earlier understanding. When a decision changes, append what changed, why, and what earlier decision it supersedes. A concise synthesis comment can help a long discussion resume, but does not replace its history. Program-level findings belong on the Program; issue-specific findings belong on that issue. Link shared evidence rather than duplicating it. Ordinary link maintenance must preserve existing content and must not become routine rewriting of a parallel current-state summary.

Once saving, chart publication, or an experiment handoff is authorized, the skills own the associated context recording and links. The human should not have to repeat “post the findings back to this issue.” Avoid per-turn logging and comments without substantive new context. Respect repository permissions and privacy, especially when linking across repositories.

## Branch experiments from the evidence need

The originating discussion identifies the decision requiring evidence and preserves the agreed requirements, relevant findings, uncertainties, and why the question matters. For example: choose an HFX physical layout under agreed access patterns, memory constraints, and data-preservation requirements. Do not prematurely prescribe a full experiment methodology merely to open the issue.

Create a linked experiment issue in the appropriate repository when the human authorizes the branch. The issue carries sufficient context that its agent need not ask the human to reconstruct the originating agent's reasoning. It identifies the originating discussion and the evidence to return. Use ordinary links, not a new machine-readable ledger or fixed metadata schema.

The human grills the experiment issue to understand and agree on how to answer that question. The agent inherits settled requirements, investigates available facts, explains alternatives, and asks only genuinely missing choices. The experiment grill determines suitable workloads, comparisons, observations, and limits. It must not blindly replay the original discovery, assume a preferred winner, or burden the human with routine measurement mechanics.

When satisfied, the human invokes `to-vision` for the experiment. Its vision authorizes bounded experimental work, not adoption into production. Use existing implementation, independent review, and target verification processes. Experiments should be proportionate and use project-native tools; no mandatory experiment platform, benchmark framework, or exhaustive alternative tournament is required. Where relevant, distinguish correctness from performance, charge preparation and deferred work, compare equivalent outputs, and identify data, settings, uncertainty, and measurement limits. Concrete evidence should inform decisions without pretending a noisy measurement settles every trade-off.

The experimental vision carries the original evidence question and the return destination. Completion includes a concise comment on the originating discussion explaining what was established, what was not, relevant implications, and links to detailed results. Preserve useful negative and inconclusive findings. If posting the required return fails, report unfinished reconciliation rather than claiming the handoff is complete.

An experiment vision may complete its own experiment issue while adding evidence to and leaving open the originating design issue. Never confuse an evidence-return link with a closing relationship. Experimental completion neither silently selects the production design nor restarts the parent discussion from scratch. The human can resume it with `grill-me <issue>` and continue from the new evidence. Several such branches can inform the same discussion without changing its identity.

## Visions and closure

`to-vision` remains the explicit publication handoff for agreed work. A vision gives a fresh implementing agent the context it needs; neither a separate spec document nor a vision is mandatory merely to save an unfinished discussion. Reuse and link existing evidence without duplicating its whole history.

Carry forward PCE's existing target-branch publication gate, independent review, preservation rules, and verified-delivery requirements. A vision explicitly designated to complete a discussion issue closes that issue only after the whole intended outcome is delivered and verified. That outcome can itself be research or design; issue closure does not inherently claim production software was built. Publication alone, one partial PR, or an experiment returning evidence does not close the parent issue. Do not automatically close Programs or reorganize their other issues.

## Clear GitHub writing without formal bodies

Improve `github-writing` with positive guidance for scannability, not another prescribed template. Use short plain-language prose to explain the purpose and bullets for distinct changes, decisions, or actual next actions. A PR with several changes should make those changes easy to find rather than burying them in a paragraph. Discussion records distinguish settled understanding from remaining work. State a real action request when one exists without manufacturing one for every record.

Headings and collapsed detail are optional aids. A short issue or PR can remain short. Keep limitations, blockers, incomplete work, and necessary human decisions visible; link detailed evidence rather than narrating the investigation. “No mandatory structure” must not be interpreted as “avoid useful structure.” Saved discussions remain primarily agent-to-agent context while still being readable by the human.

## Small skills that respect the harness

Follow the direction of [Rethinking skills and prompts for GPT-6 Astra](https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra): precise concise triggers, progressive disclosure where useful, intended outcomes, decision boundaries, and clear completion instead of elaborate itineraries. This is not a model-specific dependency.

Prime Agent owns execution mechanics. Do not prescribe worker counts, model routing, polling, waiting, heartbeat management, or an alternative session runtime. Let agents use the harness's native tools and judgment. Supporting skills or references should earn their place by guiding a distinct reasoning activity and load only when useful. Exact decomposition remains an implementation choice, not a requirement to create one skill per conversational activity.

No separate refactoring workflow is needed: redesign can happen in a grill. No dedicated codebase-learning workflow is required. When explaining existing architecture, distinguish observed mechanics from historically evidenced rationale rather than inventing intent from code.

Do not import pstack's full debugging playbook. Existing test-first guidance already covers meaningful reproduction and independent expectations. Targeted guidance may help when repeated fixes fail: test their shared premise before another patch, and remove changes motivated by disproven hypotheses instead of accumulating speculative fixes. Avoid mandatory hypothesis counts, design panels, or specialized machinery for routine bugs.

PCE evolution stays manually controlled. Do not add agent self-improvement, automatic skill revision, or an agent-behavior evaluation workflow. No plugins, hooks, application runtime, new package manager, CI workflow, or generated orchestration state belongs in this change.

## Repository changes and evidence of success

Update the workflow skills, shared publication/implementation linkage guidance, README, installer, and relevant standard-library tests coherently. Remove obsolete `grill-ticket` routing and checkout-owned installed links while preserving foreign paths and unrelated user state. Follow existing isolated-HOME installer testing. Historical visions and GitHub records remain history, not material to rewrite en masse. Do not retain the current seven-skill count as a constraint if the small save capability or useful supporting guidance changes it.

A fresh agent should be able to:

- Save an unfinished grill, resume it by issue, and move from clarification into collaborative design without restarting or publishing automatically.
- Resume a chart-created issue through exactly the same mechanism as a standalone saved discussion.
- Recognize later decisions that supersede earlier comments without deleting the old evidence.
- Carry a specific evidence need into an experiment issue, grill its methodology with the human, publish and implement its vision, and return the relevant findings to the correct parent discussion.
- Distinguish the experiment's own completion from the parent's unresolved design, and close only the issue whose designated outcome is fully verified.
- Produce concise GitHub text with visible changes and relevant actions without mandatory formal bodies.

Use the repository's normal local validation, `python3 -m unittest discover -s tests -v`, with proportionate tests protecting behavior and preserved boundaries rather than exact prose templates. These checks are not a new model-behavior evaluation system. Publication of this vision authorizes none of its implementation.
