# ADR-0006: The repository contract has two authorities

## Status

Accepted

## Context

`SKILL.md:186` reads: "Do not create a tracked repository-contract file." A repository's facts are
instead inferred per run by a `repository-analyst` dispatch reading `AGENTS.md` or `CLAUDE.md` and
CI, and the orchestrator appends what it inferred as a twelve-field `repository-contract` record.

Inference produced two failures the corpus measured. A `build` command was invented at orientation
rather than derived from CI; it failed at base with no changes present, went unexercised for three
milestones, and cost an executor round when a fourth finally used it. In the same repository CI's
`mkdocs build --strict` was never captured at all, leaving the `Docs` workflow red on `main` for two
days across three merges while five local gates reported green. A third instance surfaced while
grilling this ticket: seven `SKILL.md` clauses instruct the executor to fold "the exact bump" into
its commit and the orchestrator to cut "the required tag", answering an `AGENTS.md` clause the
repository's owner has since deleted, against a `VersionPolicy` type whose only values are `None`
and `SerializeDispatches`.

Facts a run learns have nowhere to go. A lockfile regeneration trap and a gate-ordering constraint
each cost a failure, then survived as `state.json` keys named `lockfile_lesson` and
`stopwatch_provenance_rule` plus identical boilerplate in six fix prompts, and died with the run.

A tracked file is the Enforcement split's answer, but the corpus carries its own counter-evidence:
the one hazard that was documented (`< /dev/null`, `SKILL.md:47`) was hit anyway, so a home for a
fact is necessary and demonstrably not sufficient. A file mixing checked and unchecked claims under
one word gets trusted at the level of its strongest part.

## Decision

The contract is tracked per repository and honoured only as it stands on the default branch, and it
holds two halves under different authorities.

The stated half — gate commands, version policy, the workflow map — is a claim PCE falsifies by
executing it at base; a command that fails at base is not an acceptance gate. It is authored once,
when no contract exists, from CI where CI exists and otherwise from the widest candidate command
that passes at base, and the run continues without waiting on anyone. Thereafter no run may edit
it. Correcting it is an ordinary tracked-file edit that the next run reads.

The appendable half — environment hazards, gate orderings, lockfile rules — is inert text no
execution can check. A run appends to it when a fact has recurred across runs in that repository,
keeps using what it learned for the remainder of the run, and interrupts no one. Nothing verifies an
entry.

## Consequences

The immutability of the stated half is not distrust of any actor. Falsification at base is
asymmetric: it detects a command that fails and therefore never was a gate, and it cannot detect one
that passes while testing nothing. A run narrowing `cargo test --workspace` to `cargo test --lib`
would be reported as sound. That is vacuity, the defect reading cannot catch, applied to the file
that defines what catching means.

Admission to the appendable half is recurrence rather than judgement, because at discovery time
nothing distinguishes a durable repository fact from session noise — one report insisted a shell
quirk it hit was noise and refused to make it a rule, and any per-discovery gate would have stopped
on it.

The two halves decay differently and only one is bounded. A wrong hazard note costs prompt tokens;
this is why doc-truth defects are excluded from it, being true only until the document is corrected.

Reversing this means one undifferentiated contract, and it inherits the trust collapse: an
unverified hazard sitting beside a measured gate borrows the gate's credibility, and a run that
weakens a gate leaves no observable trace.
