---
name: grill-with-docs
description: Relentlessly interview the user one question at a time about a program, vision, plan, or design while maintaining the project's domain glossary and recording only durable architectural decisions. Use when shared understanding must be sharpened and preserved in CONTEXT.md and sparing ADRs.
---

# Grill with Docs

Compose the existing `/grill-me` interviewer with the `domain-modeling` discipline. This skill does **not** use, invoke, wrap, adopt, or depend on Matt Pocock's `/grilling` interviewer.

## Interview loop

Interview the user relentlessly until the program, vision, plan, or design reaches shared understanding. Walk each branch of the decision tree and resolve dependencies between decisions one by one.

- Ask exactly one prose question at a time.
- Keep questions at program/vision altitude: goals, boundaries, domain concepts, durable constraints, alternatives, risks, and success conditions. Do not decompose the work into milestones, implementation steps, or code tasks.
- With each question, provide a recommended answer and the reason for it.
- If the codebase or committed documentation can answer a question, inspect those sources instead of asking the user. Surface contradictions between the sources and the user's stated model as a question.
- Challenge terms that conflict with `CONTEXT.md`, sharpen vague or overloaded language into a canonical term, and use concrete edge-case scenarios to test boundaries and relationships.
- After each answer, capture any crystallized terminology or qualifying decision before asking the next question.

## Documentation decision rule

Use the sibling `domain-modeling` skill's `CONTEXT-FORMAT.md` and `ADR-FORMAT.md` as the normative formats. Apply this rule during the interview, not as a batch at the end.

### Update CONTEXT.md for domain language

Update `CONTEXT.md` immediately when the conversation resolves a project-specific domain term, its tight one- or two-sentence definition, an alias to avoid, a relationship between domain concepts, or a previously flagged ambiguity. Choose one canonical term and list competing words under `_Avoid_`. Keep `CONTEXT.md` a glossary only: exclude implementation details, specifications, scratch notes, general programming concepts, and implementation decisions.

Use this shape for a resolved term:

```markdown
## Language

**Canonical term**:
A one- or two-sentence definition of what the concept is.
_Avoid_: Competing term, overloaded alias
```

Preserve existing entries and natural subgroup headings. If no `CONTEXT.md` exists, create the root file lazily when the first term is resolved. If `CONTEXT-MAP.md` exists, use it to select the relevant context; ask which context applies only when the answer cannot be inferred.

### Create an ADR for a durable decision

Offer an ADR only when all three conditions are true:

1. The decision is hard to reverse, so changing it later has meaningful cost.
2. The decision is surprising without context, so a future reader would reasonably ask why it was made.
3. The decision results from a real trade-off between genuine alternatives chosen for specific reasons.

If the user accepts the offer, create the ADR immediately in `docs/adr/`, scanning existing files for the highest sequence and using the next `NNNN-short-slug.md` filename. Create the directory lazily when the first ADR is needed. The default ADR is deliberately short:

```markdown
# Short title of the decision

One to three sentences stating the context, the decision, and why it was chosen.
```

Add status frontmatter, considered options, or consequences only when they add genuine value. Easy-to-reverse choices, unsurprising decisions, choices with no meaningful alternative, unresolved hypotheses, and implementation notes get no ADR. A resolved canonical term may update `CONTEXT.md` while the decision behind it separately receives an ADR only if all three ADR conditions also hold.

## Finish

Finish only after every relevant branch has been resolved or explicitly left open. Recap the resulting shared understanding, list the `CONTEXT.md` entries updated, list any ADRs created, and identify unresolved questions without inventing answers.
