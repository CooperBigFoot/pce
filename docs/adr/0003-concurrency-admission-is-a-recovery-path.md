# ADR-0003: Concurrency admission is a recovery path, not a prediction

## Status

Accepted

## Context

`SKILL.md:178` admits concurrent dispatch only for ready steps with disjoint `files_touched`.
The rule is safe by construction and, measured across the in-flight run for ticket #38, it never
fired: three step graphs, thirteen steps, twelve `depends_on` edges, zero parallel edges, merges
32 minutes apart. That repository's contract had explicitly retired the version-bump policy and
stated that steps "may parallelize on genuinely disjoint `files_touched`", so nothing forced the
chain.

The cause is that write-sets are not disjoint in real code. `crates/core/src/lib.rs` appears in
nine of the thirteen steps because it is the module-declaration hub every new module must touch;
all four steps of one milestone touched a single test file. A predicate that must predict safety
before dispatch has to be conservative, and a conservative predicate over hub files is inert.

The firstmate comparison had already recorded the alternative and declined it: firstmate treats
file overlap as a risk signal rather than a reason to wait and serializes only for a true
semantic dependency, and the comparison judged PCE's disjointness rule "cheap and safe… not
worth changing." The measurement above falsifies the second half of that judgment while leaving
the first intact.

## Decision

Concurrency admission is a recovery path. Overlapping ready nodes dispatch concurrently and the
merge absorbs the resulting rebase. Serialization requires a semantic dependency returned by an
actor that read the code, not an overlapping write-set.

The rule applies at both altitudes. Milestones with no ordering edge between them proceed
concurrently, which is where the wall-clock actually is: a milestone is a whole
plan-critique-execute-review-merge cycle, and `SKILL.md:237` walked them strictly in dependency
order regardless of whether any edge existed.

This rests on two properties the workflow already has: nodes run in fully isolated worktrees, and
an unexpected merge conflict is already routed to a Codex rebase dispatch from named base and head
refs. Extending admission to milestones does stretch the second one, because a milestone PR merges
into `main` while a step PR merges into `milestone-<m>`, so concurrent milestones can conflict at
a wider blast radius than concurrent steps.

## Consequences

Scheduling stops depending on a prediction the planning levels are not equipped to make, which
is what makes the planning descent worth doing: a write-set authored by a level that read no
code could not have supported the old rule either.

The cost moves from wall-clock spent waiting to occasional rebase work at merge time, and it
becomes visible where it is paid rather than invisible in a chain that looks intentional. A
merge conflict is now an ordinary outcome rather than a signal that something went wrong, so the
conflict path becomes load-bearing rather than incidental; if it degrades, the whole admission
rule degrades with it, and it degrades first at milestone altitude where the target is `main`.

Reversing this means returning to a predicate over write-sets, and any such predicate inherits
the same problem — the hub file is a property of how code is organized, not of how PCE plans.
