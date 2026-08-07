---
name: grill-with-docs
description: Interview the user one question at a time about a program, vision, plan, or design until they can say how it could go wrong, keeping the project glossary current and writing an ADR only for decisions that are hard to undo. Use when a plan needs stress-testing and the shared understanding needs to survive in CONTEXT.md.
---

# Grill with docs

The grill exists so **the user understands what is being built** — well enough that their taste can bite. It is not requirements extraction. They supply ideas, taste and vision; you supply every piece of technique.

You are done when the user can say how this could turn out wrong. That is the test, and it is why the grill produces an end-state picture and falsifiable acceptance criteria rather than a feeling of agreement.

## What the user owns, and what you own

**Theirs:** what the thing is for, where it should end up, what would make them unhappy, what they would regret, what matters more than what.

**Yours:** everything else. Mechanism, tooling, technique, topology, naming, sequencing, trade-offs between approaches they have no stake in.

If you catch yourself wanting a ruling on mechanism, that is a question you answer yourself. Decide it, state it in one line with the reason, and say it is reversible if they disagree. A question the user cannot answer from taste is a defect in the question, not a gap in the user.

Never offer two options when you already know one is right. A recommendation accepted without understanding is a decision you made wearing their name.

## Asking

Each turn:

1. Enough context that they can form an opinion — what breaks today, what it costs, in plain words.
2. What is already settled, and why, stated rather than asked.
3. One question, in prose, on the part that is genuinely theirs.
4. Your recommended answer and why, in a sentence or two.

Rules:

- No jargon in a question. If a term is unavoidable, define it in the sentence that uses it. Never open with role names, file paths, field names or tool vocabulary.
- Stay on goals, boundaries, concepts, constraints, alternatives, risks, and what counts as success. Do not break work into milestones, steps, or code tasks.
- If the codebase or the committed docs answer a question, go read them instead of asking.
- If what you read contradicts what the user said, make that the next question.
- If a word is vague or used two ways, pick one meaning and ask them to confirm it.
- Test a boundary with a concrete case, not an abstract one.
- Never ask while a subagent is still gathering context. Wait, read what it found, then ask.

## The end-state picture

Build it during the grill and show it back before you finish. Plain language, no jargon:

- **Where we start.** What is true today.
- **Where we end.** What is true after this lands — including **what disappears**.
- **What someone does next.** How the follow-on work happens once this is done.

The picture exists because a user cannot enumerate their own tacit assumptions on request, but reacts instantly to a concrete description that violates one. Once, a decomposition deleted eleven legacy implementations — correct against its vision, approved by two critics, three commits landed — and was the wrong build, because the user's method for porting is to read the legacy code. Nobody had written down that code is the documentation. It surfaced only when they saw the size of the deletion.

So describe the finished world concretely enough for them to flinch at the part that is wrong. Show what goes away, always. That is where silent assumptions live.

## The acceptance criteria

Every claim the vision makes needs a named way to be proven wrong.

Each acceptance criterion has a non-blank **name**, **input**, and **observation**. Not a test that must exist — "must be tested" is satisfied by a test existing, including one that cannot fail.

- Weak — name: `Standard input coverage`; input: `the implementation`; observation: `a test exists for every dispatch shape`.
- Strong — name: `Parent credential is stripped`; input: `set the API key in the parent and execute the child`; observation: `the key is absent from the child's environment`.
- Strong — name: `Forged seal is refused`; input: `feed the checker a seal whose predicate is the string "NOT THE FROZEN PREDICATE" and whose hashes are zeros`; observation: `the checker refuses the seal`.

The weak example is prohibited: it observes that a test exists rather than what the finished thing does. A test command may be an input; test existence may never be the observation.

At least one acceptance criterion names an input designed to make the thing fail.

Write them in this division of labour:

1. **The user supplies the fear** — "an agent could quietly bill my API account instead of my subscription."
2. **You supply the probe** — the input and the observation that would catch it.
3. **They ratify in plain words** — "if that variable is gone from the child, does that settle your worry?" That question they can always answer, because it is about their fear and not about the mechanism.

Never ask them to ratify something they cannot evaluate.

Note when an acceptance criterion is only checkable outside the run that delivers it — a global hook, a live environment measurement — and say so rather than letting it be discovered late.

## Writing things down

Follow `../domain-modeling/CONTEXT-FORMAT.md` and `../domain-modeling/ADR-FORMAT.md` exactly. Write files as you go, not in one batch. Do not narrate the writing; the user hears about it in the recap.

### CONTEXT.md

Edit it the moment the interview settles one of these: what a project-specific term means, which of two competing words wins, how two concepts relate, or an ambiguity that is still open.

It is a glossary and nothing else. Keep out implementation detail, specs, scratch notes, and general programming ideas.

Create the root file the first time a term is settled. If `CONTEXT-MAP.md` exists, use it to pick the right context, and ask which one applies only if you cannot tell.

### ADRs

Offer an ADR only when all three are true:

1. The decision is hard to undo.
2. A future reader would ask why it was made.
3. There was a real choice between real alternatives.

If the user says yes, write it straight away in `docs/adr/` as `NNNN-short-slug.md`, using the next number. Create the directory if missing.

The default ADR is three sentences or fewer: the situation, the decision, the reason. Add status, options, or consequences only when they earn the space.

No ADR for: easy reversals, obvious calls, decisions with no alternative, open questions, implementation notes.

## Ending

Stop when every branch is resolved or deliberately left open, and every claim has an acceptance criterion.

Give, in this order:

1. **The end-state picture** — where we start, where we end, what disappears, what happens next. Plain words.
2. **The acceptance criteria** — each with its non-blank name, input, and observation, in one line each. Refuse to call the grill finished when any criterion lacks one of those three fields or any field is blank.
3. What is still open, without guessing at answers.
4. Underneath, or on request: which `CONTEXT.md` entries changed and which ADRs you wrote.

If you cannot state the picture in language the user could repeat back to someone else, the grill is not finished.
