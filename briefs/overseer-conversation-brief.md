# Brief: the card is translated and the conversation is not

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine the
evidence, decide, implement, and record what you decided and why in your completion report and
`CONTEXT.md`. Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion),
stop and report. Written 2026-08-21 from the second live overseer run, against `main` at `3b0b777`
plus the uncommitted first-run fixes.

**What the last brief achieved, so it is not disturbed.** Four cards were written under the new
limits: 178–186 words against a 200-word budget, titles of 7–9 words, two blocks each, correct
`requested_act`, and a consequence sentence on every door card. taqsim went from 647 words to 178
with no hedge promoted. The card discipline works. Everything below is what that discipline does not
yet cover.

---

## Defect 1 — a reply is never sifted, so half the conversation is untranslated

pourpoint's record thread:

```
opened -> sifted -> routed(human) -> answered(human, 21 words)
       -> routed(overseer) -> answered(overseer, 94 words) -> routed(reporting-run)
```

The card obeyed every limit. The reply obeyed none, because the limits live on `SiftedCard` and a
reply is an `answered` record carrying free text. What reached the operator:

> "In `src/main.rs`, `amendment_counterfactual` treats any successful `git revert --no-commit
> <repair_ref>` as `AmendmentProof::Reverted`. It does not verify that the repair still affects the
> composed tree or that the revert changes the tree. `execute_effective_criterion` then requires the
> counterfactual execution to fail. `RepairCreditStale` is emitted when the revert is unconstructable,
> but not when a superseded repair produces a successful no-op."

The operator's response, verbatim: *"I cannot understand a word, idk how to act on it now."*

A conversation is not a card plus exhaust. Every message crossing to the human must be readable, and
translating only the first one makes the rest worse by contrast. **Apply the register discipline to
every human-directed record**, not to the opening card alone.

The same content, translated, is short and loses nothing:

> pce has a bug. To check whether a repair mattered, it reverts the repair and requires the test to
> then fail. It only checks that the revert command succeeded, never that the revert changed
> anything. A repair that later work already superseded reverts to nothing, succeeds, and is accepted
> as proof. That is why GD10 passed while proving nothing.

Decide where the limits belong so they cannot be bypassed by choosing a different record kind, and
record the decision. The existing refusals — hedge promotion, invented facts — must apply to replies
too.

## Defect 2 — routing away from the human silences him mid-conversation

After answering, the overseer routed the hold to `reporting-run`. The operator's reply box
disappeared while the exchange was live: he had asked a question, received an answer he could not
read, and had no way to say so.

**Routing decides who owes the next action. It must never decide who is allowed to speak.** The human
can write into any open hold, and writing into one pulls it back to him. Only the overseer closes a
hold — already ratified — and a closed hold is the only one that stops accepting his words.

This matters beyond convenience: the operator's only channel for "this is unreadable", "you have
misunderstood", or "stop" is the hold he is looking at.

## Defect 3 — the consequence sentence satisfies its shape and misses its purpose

taqsim's card ends:

> "The ruling changes or removes TQ2's frozen criterion, restructures the graph, or ends the run."

That restates the four options. It is not what is true in the world afterwards. The sentence the rule
exists to produce is closer to: *"If you remove the criterion, nothing checks the reproduction any
more."*

The mechanical rule — a door card carries exactly one sentence — passes. Its purpose, giving the
operator something to flinch at, does not. This corpus has recorded this shape repeatedly: a check
whose form is satisfied while its substance is absent. A word count will never catch it.

State the requirement so it can be followed: the consequence names **what stops being true, or what
becomes possible, once the chosen act lands** — not which acts are available. Where the options
differ in what they cost, the sentence names the loss the operator is most likely to overlook.

## Defect 4 — the sifter has no glossary, so it cannot reach the operator's vocabulary

The sifter is handed the report and nothing else. The result is a faithful compression that is still
not plain language: *"Measurement shows the timestep-zero residue is exactly one declared unit, but
the criterion requires strictly less than one."*

Nothing in the report defines *residue*, *declared unit*, or what the criterion protects, so no
compression of it can produce the operator's register. Translation needs a glossary and the run has
one.

**Read `vision.md`.** It is the only artifact in a run written by the operator, for the operator: what
the run is for, what done means, and what the domain words mean, in his own words. It is about a
page, stable for the life of the plan, and `pce hold runs` already returns its directory — the
overseer has the pointer and does not follow it.

Two boundaries, or this breaks ratified rules:

- **Anything learned by reading that the report did not state is a labelled overseer fact**, never
  blended into the translated blocks. The sifter still may not add facts; the overseer may supply them
  under its own name, and the card already has separate fields for the two acts.
- **Bounded reads only.** `vision.md` and the named criterion text, not the journal — a driver journal
  reached 32MB in the runaway killed today. A one-pass session stops being cheap the moment it reads
  whatever `evidence_paths` offers.

## Defect 5 — every card came back with no overseer facts

All four cards in the first conforming batch carry `overseer_facts: []`. Supplying facts a run cannot
see by construction is half the reason the overseer exists.

Either there genuinely were none — plausible for four unrelated runs — or nothing is looking.
Instrument it rather than guess: a pass that supplies no fact across a whole fleet should record that
as a fact about the pass, so the difference between "nothing to say" and "never looked" is visible.

## Defect 6 — the rail prints a SHA-256 and it overflows the box

Every rail entry renders the full question kind:

```
pourpoint   WORK-GRAPH-TERMINAL-STOP:D3376E0…
bluesmith   WORK-GRAPH-TERMINAL-STOP:4670E12…
```

It runs off the right edge on every row, and it is the widest, most dominant element in each entry
while carrying nothing the operator can use — it exists so a restarted supervisor reopens the same
hold. Four rows read as four near-identical hashes. The genuinely useful line, `Package work-graph ·
plan 21`, is styled as secondary.

Put the **requested act** where the hash is — `criterion revision`, `spend`, `publication`, or nothing
for a non-door — since that is what tells the operator whether this is his to answer. Identity belongs
in the card detail. Nothing in the rail may exceed its width.

## Defect 7 — liveness still reports the claim, not the process

The pill read `overseer is idle` while a prime-agent session was three minutes into a pass and
actively sifting. Confirmed against the process table: the server had a live child throughout, and
the journal showed `session-spawned: 3` against `session-exited: 2`.

Cause: the session writes `pass-completed` and then keeps running. Liveness derives from that record,
so the page calls the overseer idle from the moment it announces completion until the process exits —
minutes, in practice.

This is the second time today the liveness indicator has been wired to a claim rather than to the
thing it describes; the first was the heartbeat written by the server loop. Treat it as one pattern.
The server reaps the child and therefore knows the truth.

Wakes are correctly deferred during that window, so no work is lost. But a session that announces
completion and then lingers indefinitely stalls the fleet while the page reports calm.

## Defect 8 — render markdown in cards and replies

Identifiers, paths and commands appear as bare prose: `src/main.rs`, `amendment_counterfactual`,
`git revert --no-commit <repair_ref>`. Nothing separates a symbol from a sentence, so a dense passage
reads as one undifferentiated wall.

Render inline code, code blocks, bold and lists in card blocks and in replies. This is a reading aid,
not a licence to put commands into options: commands, flags and paths still belong in evidence, never
in an option the operator chooses between.

## Defect 9 — the warning cries wolf every sixteen seconds

The accepted-socket fix works. Verified live: two requests to the running server returned `200`,
24196 bytes, 8ms. But this still prints once per refresh cycle:

```
WARN pce: rejected overseer HTTP request peer=127.0.0.1:63533 error=failed to read HTTP request line
Caused by: Resource temporarily unavailable (os error 35)
```

A Rust read that hits `set_read_timeout` returns `WouldBlock` — errno 35, the same text as the old
defect. The cause is now the opposite: a peer that connected and sent nothing for two seconds, which
is the browser holding a speculative connection open.

Zero bytes read before the deadline is an idle peer and belongs at debug or nowhere. Some bytes then a
deadline is a truncated request and deserves the warning. Today both emit the same alarming line,
which reads as *your ruling was rejected* and never is. This Program's own finding applies: a check
whose true positives are outnumbered by structural false positives will be skimmed.

## Out of scope

Dispatching defect briefs and running the fix pipeline are `briefs/overseer-fix-pipeline-brief.md`.
Do not build them here.

## Operating constraints for this dispatch

Read this before touching anything.

**Work in your own git worktree with your own `CARGO_TARGET_DIR`.** Never build in the main checkout
at `/Users/nicolaslazaro/Desktop/work/pce`. On this machine `~/.local/bin/pce` is a symlink to
`target/release/pce`, so a release build there replaces the operator's live binary and every parallel
agent's test binary while they are running. This has already caused one recorded incident.

**You are not alone.** Another agent is working from a different brief in a parallel worktree.
Per-brief file ownership is stated below. Keep edits to shared files narrow and in one region so the
merge stays mechanical, and never reformat or reorganise code you are not changing.

**The live hold store is not yours.** The overseer is stopped. `~/.pce/holds` holds four real sifted
cards which are the operator's actual pending decisions. They are excellent fixtures — copy the store
and work against the copy. Never write to `~/.pce/holds`.

**Report what you decided and why**, in your completion report and in `CONTEXT.md`. Prefer falsifiers
that fail against the code as it stands and pass after; the defects in this brief all shipped behind
passing criteria. If satisfying this brief appears to require editing a frozen criterion, stop and
report which one and why.

**Your files.** `crates/core/src/overseer_view.rs`, `crates/core/src/hold_store.rs` and
`skills/overseer/SKILL.md`, plus one log line in `src/main.rs`. The parallel agent owns
`skills/work-graph/SKILL.md` — leave it alone.

**A third brief lands after yours** and adds an update indicator to the queue page you are
redesigning. Leave a place for it rather than a layout that has to be undone.

**Two of your defects are one defect twice.** The liveness pill reads a claim rather than the process,
and the log line reports a browser's idle connection as a rejected request. Both are indicators wired
to something other than the thing they describe. Fix them as one pattern and say so.

## What this does not change

- The four attributed doors. Nothing here lets any session sign a record.
- The card limits, the door/act pairing, the hold lifecycle, or the wake-and-exit session model.
- The append-only store: every change is a new record, never a rewrite.
- Any frozen criterion. If satisfying this brief appears to require editing one, stop and report which
  criterion and why.

## Falsifiers worth pairing

- A reply to the human exceeding the register limits is refused or flagged. The 94-word pourpoint
  reply fails it.
- A reply that promotes a hedge present in the source is refused, exactly as a card is.
- A human can write into an open hold routed to the overseer or to the reporting run, and doing so
  routes it back to the human.
- A closed hold refuses a human message.
- A card whose consequence sentence only enumerates the options is refused or flagged. taqsim's
  current sentence fails it.
- A pass that reads `vision.md` puts nothing from it into a block without also recording it as a
  labelled overseer fact.
- A rail entry never contains the question-kind hash, and no rail entry exceeds the rail's width.
- Liveness reports `working` for the whole interval a child process is alive, including after
  `pass-completed` is written.
- A peer that connects and sends nothing produces no warning; a peer that sends a partial request and
  stops produces one.
