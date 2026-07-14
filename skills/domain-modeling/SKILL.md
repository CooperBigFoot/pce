---
name: domain-modeling
description: Maintain the project's committed domain glossary and record consequential architecture decisions sparingly. Use when work introduces, changes, or exposes ambiguity in domain terms, relationships, or durable design choices.
---

# Domain modeling

Keep the language and durable reasoning of the project available to every future session. Treat domain documentation as active committed source, not as a retrospective summary.

## Start with the shared context

Before discussing or changing domain behavior, read the repository-root `CONTEXT.md`. Use its canonical terms consistently. If the work reveals a missing term, a misleading alias, an important relationship, or an unresolved ambiguity, update `CONTEXT.md` as part of the same change.

Follow [`CONTEXT-FORMAT.md`](CONTEXT-FORMAT.md) exactly. Keep entries concise, domain-facing, and useful without access to the conversation that produced them. Record established meaning rather than speculative implementation detail. When meaning changes, edit the existing entry and reconcile affected aliases, relationships, and ambiguities instead of appending a contradictory definition.

## Maintain the glossary actively

Update `CONTEXT.md` when any of these occurs:

- a domain term gains a precise meaning;
- two names are being used for the same concept and one should be canonical;
- the relationship between concepts affects design or workflow;
- a term remains ambiguous and the ambiguity could change later work.

Do not turn the glossary into a changelog, task list, or general architecture manual. Remove an ambiguity when it is resolved and incorporate the resolution into the relevant canonical term or relationship.

## Record decisions sparingly

Create an Architecture Decision Record under `docs/adr/` only when a decision is consequential, durable, and has meaningful alternatives or tradeoffs that future work must understand. Routine implementation choices, easily reversible details, glossary wording, and status updates do not merit an ADR.

Name ADRs `NNNN-short-title.md`, using the next unused four-digit sequence, and follow [`ADR-FORMAT.md`](ADR-FORMAT.md) exactly. Never rewrite an accepted ADR to make history look current. If a later decision replaces it, add a new ADR and mark the old record `Superseded` with a link to its replacement.

When an ADR establishes or changes domain language, update `CONTEXT.md` in the same change. Keep the glossary as the fast orientation surface and the ADR as the durable rationale.

## Finish with a consistency pass

Before handing off work:

1. Confirm new prose uses canonical terms from `CONTEXT.md`.
2. Confirm aliases-to-avoid have not leaked into new artifacts except where they are explicitly identified as aliases.
3. Reconcile relationships and ambiguities affected by the work.
4. Confirm every new ADR meets the sparing-decision threshold and uses the required format.
5. Commit `CONTEXT.md` and any ADR with the work that made them necessary.
