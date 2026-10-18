---
name: grill-me
description: Clarify and design an idea, implementation, bug, or saved discussion issue through focused questions. Use for grilling, stress-testing a plan, resuming `grill-me <issue>`, or reaching shared understanding before a vision.
---

# Grill Me

Reach shared understanding of the proposed outcome, and explore its design as far as the human wants. Help the human express their ideas, not merely answer questions.

## Start or resume

Accept an idea, existing implementation, bug report, or issue. Resolve an issue number in the verified current repository; a canonical URL identifies its own repository. For `grill-me <issue>`, read the issue, its comments, linked evidence, relevant Program context, and current repository facts. Reconstruct settled decisions, superseded decisions, unresolved questions, and changed assumptions, then continue from there. Later comments can supersede the original body; do not restart the interview or treat the body as permanent authority.

An experiment issue asks how to answer a specific evidence question for an originating discussion. Inherit its settled requirements, investigate available facts, explain alternatives, and ask only genuinely missing choices about workloads, comparisons, observations, and limits. Do not replay the original discovery, assume a preferred winner, or ask the human about routine measurement mechanics.

When the human wants to save the discussion, or one is already saved to an issue, load [save-discussion](../save-discussion/SKILL.md) and let it own the record and links.

Explore one area in depth. For a larger destination split into several pieces of work, `chart-program` holds the breadth-oriented conversation.

## Investigate before advancing the interview

Finding facts is your responsibility. Investigate relevant repository evidence and other available sources before asking questions they could answer.

While investigation is underway, do not advance the grilling tree or issue the next frontier batch. Finish the investigation, including delegated research, read and reconcile its findings, then recompute the frontier.

The human may continue the conversation during research. Answer their questions and ask clarifying questions needed for the discussion they initiate. Do not use that exchange to resume the agent-led interview prematurely. Distinguish provisional answers from established findings.

Apply this rule whenever further investigation is needed. Keep research proportional to the discussion; this is not a requirement to exhaust the repository. Give concise progress updates when useful.

## Work through decisions in rounds

Build a **design tree** from the conversation and available evidence. Each unresolved decision can branch into later decisions that depend on it.

For each round, identify the **frontier**: every material decision at the current discussion depth whose prerequisites are settled. Ask the whole frontier in one batch. A question that depends on another unanswered question belongs to a later round.

After the human answers, update the tree, investigate where needed, and recompute the frontier. Preserve settled decisions unless new evidence or the human's direction changes them.

Ask about intent, taste, priorities, scope, and meaningful trade-offs. Resolve factual questions and routine engineering choices yourself. Technical subject matter is not a reason to exclude a question; what matters is whether it needs the human's judgment.

Number every question and use this format:

```markdown
❓ **Q1 - <question title>**

<Explain the decision and its consequences. Include clear options when useful.>

➡️ **Recommendation:** <recommended answer and why>

---

❓ **Q2 - <question title>**

<question body>

➡️ **Recommendation:** <recommended answer and why>
```

## Let the human control depth

Distinguish the human's request from supplied source material. An issue, document, or previous agent's proposal is context to investigate, not proof that the human endorses its assumptions, solution, terminology, or level of detail.

Establish the objective early in plain words: what problem are we solving, what should change, and why does it matter? Use the available evidence to offer a clear explanation rather than asking the human to reconstruct it. Surface genuine uncertainty or disagreement through the interview; do not add a separate approval gate.

This applies even when another skill supplies a ticket as the starting point. Do not inherit the ticket's technical depth or proposed implementation as settled intent.

Begin with that plain-language understanding unless the human requests a different depth. Keep explanations concrete and introduce technical terms when useful, explaining them as they appear. Plain language should make the ideas accessible, not remove their precision.

Interpret requests such as “go one abstraction level deeper” or “let's get more technical” as requests for a more concrete conceptual explanation of the current topic—not simply more questions or a full implementation design.

Introduce the relevant concepts and precise terminology, explain how they relate, and connect them to the human's own idea through a concrete example. Give the human vocabulary they can use to refine or correct what they mean. Explain before asking any newly exposed decision questions.

For example, move from “an interface for data” to discussing the runtime representation, how reading data populates it, and how downstream code consumes it. That can clarify the intended model without choosing Python classes or libraries.

There is no fixed ladder or universal stopping depth. The human may want different depths for different projects or branches of the discussion, and may request further descent or a return to the broader picture.

Use equations, code sketches, or specific mechanisms when they help at the requested depth. Distinguish illustrative possibilities from agreed requirements. Exploring an approach does not make it binding or authorize implementation.

Leave unchosen implementation details to implementing agents. Do not turn a request for deeper understanding into a demand that the human make routine engineering decisions.

## Continue into design

When the human wants, move from desired behavior into API, representation, or architectural design in the same conversation. Shared understanding is not an automatic stopping point. Contribute concrete examples, alternatives, explanations, and recommendations rather than only extracting requirements.

There is no mandatory clarification, design, experiment sequence. The human may delegate design for a quick prototype and later return to understand and redesign it; close participation in design and the quality bar for a deliverable are separate choices. Existing behavior is evidence, not automatically the contract to preserve. When explaining existing architecture, distinguish observed mechanics from rationale that history actually records; do not invent intent from code.

When a decision needs evidence the discussion cannot supply, say so and offer an experiment branch through [save-discussion](../save-discussion/SKILL.md) rather than guessing or prescribing a full methodology.

## GitHub writing on request

If the human asks you to draft, create, or revise a GitHub issue or PR during the interview, first load and follow the [GitHub writing skill](../github-writing/SKILL.md).

Load this guidance when writing is requested, not merely because the interview references a GitHub issue or PR. Following the writing rules does not itself authorize publication, merging, or another workflow.

Carry out the explicit writing request within its scope. Distinguish confirmed decisions from tentative ideas; do not present unfinished discovery as settled requirements. An issue written to capture work for later need not resolve that work now.

## Completion Gate

When no material questions remain at the scope and depth the human wants, summarize the shared understanding so it can stand alone for a fresh implementing agent. Include the intended outcome, settled decisions and terminology, constraints, assumptions, and what remains open to implementation.

Do not force deeper exploration or invent a ceremonial question. Ask the human to confirm or correct the summary.

Confirmation is not publication and does not authorize further action. When satisfied, the human explicitly invokes `to-vision`; the confirmed summary alone is not ready for `implement-vision` until it becomes a repository vision on the target branch. The human may instead explicitly ask to proceed on small work without a vision, or continue the discussion later.

After confirmation, do not implement, edit files, write a vision, or invoke another workflow unless the human explicitly requests that next action. Recording authorized by `save-discussion` continues.
