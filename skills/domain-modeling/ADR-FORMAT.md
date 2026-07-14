# Architecture Decision Record format

Store Architecture Decision Records in `docs/adr/` as `NNNN-short-title.md`, where `NNNN` is the next unused four-digit sequence. Use an ADR only for a consequential, durable choice with meaningful alternatives or tradeoffs.

## Required layout

```markdown
# ADR-NNNN: Title

## Status

Proposed | Accepted | Superseded by [ADR-NNNN](NNNN-short-title.md)

## Context

Describe the forces, constraints, and alternatives that make a decision necessary.

## Decision

State the chosen direction and its binding boundaries.

## Consequences

State the positive effects, costs, risks, and follow-up obligations created by the decision.
```

## Section rules

### Title

Use a short noun phrase that identifies the decision, not the work item that prompted it.

### Status

Use `Proposed` while the choice is under review, `Accepted` once it governs the project, or `Superseded by` with a relative link when a later ADR replaces it. Preserve accepted history instead of rewriting it.

### Context

Explain why the choice exists, what constraints bind it, and which credible alternatives were considered. Include only context needed to understand the decision later.

### Decision

State what was chosen in direct language. Include the scope and boundaries that future work must preserve.

### Consequences

Record benefits and liabilities, including operational or follow-up work. Consequences are expected tradeoffs, not an argument that the decision is cost-free.
