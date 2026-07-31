# ADR-0007: A dispatch is an object the binary spawns and measures

## Status

Accepted

## Context

The orchestrator composes every dispatch by hand in its own shell. `SKILL.md` fixes the executor's
core prompt text and the flags each phase must pass, and nothing checks that a given invocation
carries them. The 2026-07-31 RivRetrieve run built each executor prompt by copying the previous
step's and running `sed` over it; one derivation inverted a sentence about which sibling adds which
re-export, and it was caught by the orchestrator reading its own output rather than by any rule. The
one Codex trap the corpus documented — a `codex exec` without `< /dev/null` hangs — was hit anyway,
which is the founding hypothesis of this Program on its smallest instance: an invariant the
orchestrator must apply to itself does not survive in prose.

Two runs and 63 plus 129 dispatches have produced no cost record of any kind. `codex exec` prints
token counts to a transcript, which is not durable, and the Program's third done-condition — a
recorded per-dispatch cost baseline — has stayed unmet through every ticket landed so far. Two Fog
items wait on it specifically.

Gate dispatches are roughly half of every run: 61 of 129 in the RivRetrieve run. They execute as
harness subagents, and a subagent's token usage and cost are not exposed to the parent in any
machine-readable form — not documented, and absent from the subagent result schema. Anything the
orchestrator recorded about a gate's cost would be a number transcribed out of a completion message.
The same run showed the message channel failing repeatedly: several critics went idle without their
verdict arriving and two were killed outright by a weekly rate limit, with the run surviving only
because the orchestrator read the verdict from disk.

## Decision

`pce dispatch` spawns every dispatch of both halves — `codex exec` children and gates run headless as
`claude -p` — and no dispatch of any kind, including conflict rebases and commit completions, occurs
by another path. The binary composes the envelope and spawns the child directly with no shell, so
standard input is bound on the child handle rather than redirected and `ANTHROPIC_API_KEY` is
stripped from every gate spawn. Each dispatch appends two records: the dispatch when issued, and a
completion when the child exits, carrying wall-clock, token usage, exit status, and the verdict
artifact the binary validated. A missing or non-conforming verdict artifact means the dispatch
produced nothing; the child's transcript is never read as a fallback.

## Consequences

The alternative for the gate half was to keep subagents and have the binary register the expected
verdict path before the spawn and validate it afterwards. That kills the result-message read just as
well, but leaves half of every run with no cost channel and leaves the registration machinery
existing only because the spawn happens where the binary cannot see it. It is the fallback if
headless gates cannot draw on the subscription: the docs state credential precedence puts subscription
OAuth ahead of everything when `ANTHROPIC_API_KEY` is unset, but they do not state that a child
spawned from inside a running session inherits the parent's OAuth, and that is unverified.

Two records rather than one follows from the placement rule: cost and duration are unknowable when a
dispatch is issued, and a record is never revised. It also disposes of a finding the same report
raised separately. That report proposed an optional `class` field on the dispatch payload to stop
round counts conflating defect rounds with attempts that produced nothing, while admitting its
vocabulary was a guess. Under two records, an attempt that produced nothing is a dispatch with no
completion beside it, derived from the log rather than declared by whoever appended it. The four
defensive `key-finding` notes that run wrote purely to keep cap accounting honest have no successor.

The dollar figure `claude -p` reports is computed locally at list rates and is explicitly not a bill
on a subscription plan. The baseline this buys is therefore tokens, seconds, and a list-price
estimate — enough to compare rounds against each other, not enough to reconcile against an invoice.
