---
name: grill-me
description: Clarify an idea through focused questions. Use for grilling, stress-testing a plan, or reaching shared understanding before a vision.
---

# Grill Me

Reach shared understanding of the proposed outcome before implementation begins. Help the human express their ideas, not merely answer questions.

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

## GitHub writing on request

If the human asks you to draft, create, or revise a GitHub issue or PR during the interview, first load and follow the [GitHub writing skill](../github-writing/SKILL.md).

Load this guidance when writing is requested, not merely because the interview references a GitHub issue or PR. Following the writing rules does not itself authorize publication, merging, or another workflow.

Carry out the explicit writing request within its scope. Distinguish confirmed decisions from tentative ideas; do not present unfinished discovery as settled requirements. An issue written to capture work for later need not resolve that work now.

## Completion Gate

When no material questions remain at the agreed scope and depth, summarize the shared understanding so it can stand alone for a fresh implementing agent. Include the intended outcome, settled decisions and terminology, constraints, assumptions, and what remains open to implementation.

Do not force deeper exploration or invent a ceremonial question. Ask the human to confirm or correct the summary.

Confirmation completes `grill-me`; confirmation is not publication and does not authorize further action. The confirmed summary alone is not ready for `implement-vision`: it must first become a repository vision and satisfy the target-branch durability gate.

Stop after confirmation. Do not implement, edit files, write a vision, or invoke another workflow unless the human explicitly requests that next action.
