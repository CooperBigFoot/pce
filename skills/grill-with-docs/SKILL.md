---
name: grill-with-docs
description: Interview the user one question at a time about a program, vision, plan, or design, keeping the project glossary current and writing an ADR only for decisions that are hard to undo. Use when a plan needs stress-testing and the shared understanding needs to survive in CONTEXT.md.
---

# Grill with docs

Interview the user one question at a time until both sides understand the plan the same way. Keep the project glossary current as you go. Write an ADR only for decisions that are hard to undo.

## The interview

Work down the decision tree one branch at a time. Settle the decisions that other decisions depend on first.

Each turn has exactly three parts:

1. One question, in prose.
2. Your recommended answer.
3. Why you recommend it, in a sentence or two. Be concise, clear, and to the point.

Rules:

- Stay on goals, boundaries, concepts, constraints, alternatives, risks, and what counts as success. Do not break the work into milestones, steps, or code tasks.
- If the codebase or the committed docs answer a question, go read them instead of asking.
- If what you read contradicts what the user said, make that the next question.
- If a word is vague or used two ways, pick one meaning and ask the user to confirm it.
- Test a boundary with a concrete case, not an abstract one.
- Never ask a question while a subagent is still gathering context. Wait for it to come back, read what it found, then ask. Otherwise you spend the user's turn on something the subagent was already answering.

## Writing things down

Follow `../domain-modeling/CONTEXT-FORMAT.md` and `../domain-modeling/ADR-FORMAT.md` exactly. Write files as you go, not in one batch at the end. Do not narrate the writing: no running commentary on what might be glossary-worthy. The user hears about it in the recap.

### CONTEXT.md

Edit `CONTEXT.md` the moment the interview settles one of these: what a project-specific term means, which of two competing words wins, how two concepts relate, or an ambiguity that is still open.

It is a glossary and nothing else. Keep out implementation detail, specs, scratch notes, and general programming ideas.

Create the root file the first time a term is settled. If `CONTEXT-MAP.md` exists, use it to pick the right context, and ask which one applies only if you cannot tell.

### ADRs

Offer an ADR only when all three are true:

1. The decision is hard to undo.
2. A future reader would ask why it was made.
3. There was a real choice between real alternatives.

If the user says yes, write it straight away in `docs/adr/`. Find the highest existing number and use the next one: `NNNN-short-slug.md`. Create the directory if it is missing.

The default ADR is three sentences or fewer: the situation, the decision, the reason. Add status, options, or consequences only when they earn the space.

No ADR for: easy reversals, obvious calls, decisions with no alternative, open questions, implementation notes.

A settled term goes in `CONTEXT.md` on its own. It needs an ADR only if the three tests above also pass.

## Ending

Stop when every branch is resolved or deliberately left open. Then give:

- What you both now understand.
- Which `CONTEXT.md` entries changed.
- Which ADRs you wrote.
- What is still open, without guessing at answers.
