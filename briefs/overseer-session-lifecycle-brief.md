# Brief: the overseer never starts, and the queue reports it alive anyway

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine the
evidence, decide, implement, and record what you decided and why in your completion report and
`CONTEXT.md`. Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion),
stop and report. Written 2026-08-21 against `main` at `740d169`, immediately after
`2026-08-21-what-reaches-the-human` promoted its assembly (PR #187).

Plan version 1 of that vision is landed and merged. This brief is new work against the merged code,
not a re-freeze. No criterion of `graph.v1.json` is edited by it — see the note under Defect 2,
which establishes that the model criterion is parameterised rather than literal.

Nothing in this brief may let any session sign an attributed record. Criterion revision,
worker-environment extension, base-currency acceptance and park overrule keep requiring the human's
name.

## The decision this brief records

**The overseer runs as `prime-agent` on provider `openai-codex`, model `gpt-5.6-sol`, thinking
`high`.**

The seam was never decided when the assembly was authored, and the four halves disagree:

| Fact | Where it points |
|---|---|
| `skills/overseer/SKILL.md`, installed under `$HOME/.claude/skills` (`install.sh:241`) | Claude Code |
| `OVERSEER_MODEL = "claude-fable-5"` (`crates/core/src/overseer_view.rs:18`) | a Claude model |
| `--session-program` default `prime-agent` (`src/main.rs:1143`) | prime-agent |
| `--thinking high` (`src/main.rs:1264`) | prime-agent's flag vocabulary |

The reasoning, recorded so it is not relitigated. Measured live, `prime-agent model list` offers
exactly two providers, `openai-codex` and `prime-inference`. There is no Anthropic-subscription
route, so any Claude model under prime-agent is separately billed. Meanwhile every Claude Code
session on this machine shares one subscription window, and this corpus has already recorded that
window being exhausted mid-gate by three concurrent critics with no dispatch involved. An overseer
that wakes all day would compete with the orchestrators for it. Running the overseer on
`openai-codex` puts it on a different meter and leaves the Claude window to the runs.

prime-agent remains what it already is for workers and gates. Keep `--session-program`, provider and
model configurable; only the defaults change.

## Defect 1 — the skill is unreachable after install

`install.sh:244` iterates a hardcoded list:

```
for skill in pce to-vision domain-modeling grill-with-docs chart-program work-ticket land-ticket work-graph; do
```

`overseer` is absent, so `$HOME/.claude/skills/overseer` is never created. The verification list at
`install.sh:286` omits it identically, so the install reports success while the skill cannot be
found. It shipped, passed its criteria, and has no way to be invoked.

Two things follow, and both are needed:

- **The spawn should not depend on that list at all.** prime-agent accepts `--skill <path>`,
  repeatable and additive, and its documentation covers loading Claude Code skill directories. Point
  the spawn at the overseer skill explicitly so the overseer's own startup cannot be broken by an
  installer omission.
- **Fix the list anyway, and prefer the class fix.** A hardcoded list means every future skill ships
  invisible by default; this is the first time it has cost anything. Deriving the list from
  `skills/*/SKILL.md` removes the class, extending it widens an exemption by one, and this Program's
  standing rule prefers the former. Whichever you choose, the link loop and the verification loop
  must cover exactly the same set — two lists that can disagree is the same defect twice.

## Defect 2 — the spawned model name is wrong, and the criterion cannot detect that

`spawn_overseer_session` (`src/main.rs:1264`) spawns:

```
<program> --model claude-fable-5 --thinking high /overseer
```

`--thinking high` is correct for prime-agent (`off, minimal, low, medium, high, xhigh, max`). The
model name is not. `prime-agent model list` names Anthropic models as `anthropic/claude-fable-5`,
with a provider column, so the bare `claude-fable-5` matches nothing. The spawn fails.

Implement the decision above: provider `openai-codex`, model `gpt-5.6-sol`, thinking `high`. The
spawn must be able to express a provider, which it cannot today.

**The criterion covering this is self-referential and must not be trusted.**
`tests/overseer_server.rs:328` asserts the invocation contains `--model {OVERSEER_MODEL}` — the
constant, not a literal. It therefore proves the spawn matches whatever is declared, which it does by
construction, and can never detect a model the spawned program rejects. Changing the constant keeps
it green. This is the corpus's recorded pattern of clauses the producer writes about itself.

The check that would have caught this is that **the spawned session actually starts**. Add it.

**And make failure loud.** `stdout` and `stderr` are both `Stdio::null()` (`src/main.rs:1279-1281`),
so a child that exits non-zero on an unknown model or flag produces no error anywhere. That silence
is why a wrong model name could ship behind a passing criterion. A spawned session that exits
non-zero must become a recorded, visible fact.

## Defect 3 — the overseer runs continuously and the heartbeat proves the wrong process

**The heartbeat is written by the server loop, not by the session** (`src/main.rs:1227`). It appends
unconditionally on its interval whether or not a session is alive. So the queue's liveness claim
means *the server process is running*, never *the overseer is working*. With Defect 2 present, every
spawned session dies instantly and the page still reports the overseer alive.

This is the *silence is not calm* criterion passing while the exact failure it was written to catch
goes undetected: the criterion kills the session and observes a stale heartbeat, but nothing can make
that heartbeat stale while `serve` runs. Proven against its own criterion, unproven against its
blast radius. Do not treat the passing criterion as evidence the behaviour is right.

**The respawn loop is unbounded** (`src/main.rs:1245-1246`): on child exit it sleeps 100ms and
spawns again, forever. A headless session performs one pass and exits by construction, so once
Defect 2 is fixed this becomes a fresh high-effort session roughly ten times a second.

**Decision to implement: the overseer wakes, performs one pass, and exits.** It is never
long-lived. The reasoning is already recorded in `CONTEXT.md` under this Program — the overseer holds
no durable state in context, reconstructs from the store every turn, and does not compact. A one-pass
session cannot drift because it remembers nothing; the store is the memory. Idle costs nothing.

Constraints:

- **Wake on work, not on a clock.** A hold opening, changing state, or being answered through the
  page is a wake. Add a slow periodic wake for work no store change triggers — an install waiting for
  a quiet fleet is the known case. Choose the interval and record why.
- **Never run two passes at once.** A wake arriving mid-pass must not spawn a second session against
  the same store.
- **The session writes its own liveness.** Record a wake and a completed pass as separate facts. A
  wake with no completion is a session that could not start or died mid-pass, and the view must say
  so. This is what makes Defect 2 visible instead of silent.
- **Distinguish idle from dead.** `OverseerLiveness` (`crates/core/src/overseer_view.rs:165`) has
  two variants, `Running` and `NotRunning`. Nothing to do is not the same as cannot start, and the
  operator must tell them apart at a glance.
- **Spawn in the pce repository, not the store root.** `current_dir(store_root)`
  (`src/main.rs:1277`) leaves the session without repository context while its own duties include
  dispatching briefs and running the fix pipeline, which are work in this repository.
  `PCE_HOLD_STORE_ROOT` continues to name the store.

The intent behind *Respawn keeps the thread — restart half* survives: a session that exits without
completing its pass is replaced. The unbounded hot loop does not.

## What this does not change

- The four attributed doors, the hold lifecycle, routing refusals, or rule admission.
- The queue view's contents, the card shape, or the sifting discipline.
- Any frozen criterion. If satisfying this brief appears to require editing one, stop and report
  which criterion and why, rather than re-freezing it.
