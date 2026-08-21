# Brief: the first live overseer run — a card the operator cannot read, and a door rule production can never reach

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine the
evidence, decide, implement, and record what you decided and why in your completion report and
`CONTEXT.md`. Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion),
stop and report. Written 2026-08-21 against `main` at `3b0b777`, from the first end-to-end run of the
overseer loop against the live fleet.

**What worked, so it is not disturbed.** Four runs registered themselves on relaunch. The overseer
woke on `store-changed`, spawned `gpt-5.6-sol` at high effort, completed its passes and exited 0 each
time, with no hot loop and no silent death. Holds were opened, sifted and routed. The mechanism is
sound; everything below is about what the operator receives.

---

## Defect 1 — the door rule is unreachable in production

`DOOR_QUESTION_KINDS` (`crates/core/src/hold_store.rs:260`) matches exact strings:
`criterion-revision`, `worker-environment-extension`, `base-currency-acceptance`, `park-overrule`,
`publication`, `spend`. A card may carry a consequence sentence only when the kind matches.

`skills/work-graph/SKILL.md:393` opens **every** terminal stop as:

```
--question-kind work-graph-terminal-stop:<STOP_ID>
```

where `STOP_ID` is a per-stop SHA-256. That string can never match a door kind. Every stop reaches
the store through this path, so **no hold a run opens can ever be a door**, whatever it is about.

Measured in a throwaway store with taqsim's own kind:

- a card carrying a consequence sentence is refused: `question kind
  work-graph-terminal-stop:2ff4fff17e4bec61 opens no door, so its card carries no consequence sentence`
- the same card without one is accepted

The live consequence: taqsim's TQ2 hold asks the operator to author a criterion revision. Its four
options are criterion-revision records with real, differing consequences. The overseer could not put
a consequence sentence on the card, so it wrote `"consequence": null` and moved the per-option
consequences into `note` fields. The operator now reads a door ruling with no statement of what the
world looks like afterwards — which is the one thing this Program decided a door card must carry.

**Door-ness is a property of the act being requested, not of the hold's identity.** The identity
hash exists so a restarted supervisor reopens the same hold; it must not also carry classification.
The sifter reads the options and knows which act they require.

Implement so that the declared requested act decides, and the store enforces the pairing: a
consequence sentence is present exactly when the declared act opens a door. Where the act is
declared — a field on the sifted card, a structured kind whose act segment is matched, or another
shape you can defend — is your decision. Two constraints:

- A run must not be able to make a question unclassifiable by accident. Today it does, by
  construction, and the failure is silent.
- The existing refusals must survive: a non-door card carrying a consequence is still refused, a door
  card missing one is still refused, and a consequence longer than one sentence is still refused.

## Defect 2 — sifting ran and the translation did not

This is the vision's whole purpose and it did not happen. The card the overseer produced for taqsim
TQ2, verbatim:

> **Title:** "Decide whether to further revise, remove, repartition, or retire TQ2's criterion after
> attempt 8 independently refused the plan-version-3 successor a second time."

> **Block:** "the v3 successor requires the retained residue to be strictly less than one declared
> unit at every timestep; at t0 it is exactly one declared unit; arrivals plus every named loss
> account equal the water that entered the delivery path, bit for bit, at both timesteps."

> **Block heading:** "Independent promotion constraint"

> **Option:** "Human authors a further criterion-revision record for TQ2 'The reported reproduction
> runs', predecessor = the v3 successor text, and freezes with `pce graph freeze --vision-dir <dir>
> --criterion-revisions <record> --repository incidence=<path> --repository taqsim=<path>`"

That is the orchestrator's register, reorganised. A 34-word title that is an instruction rather than
a question; block headings in workflow vocabulary; a shell command inside an option the operator is
meant to choose between; `overseer_facts` empty.

`skills/overseer/SKILL.md` §2 tells the overseer to build a card and to preserve modal force. It
does not require the result to be readable by someone who does not know the workflow. Preserving
hedges is a floor, not a translation.

Give the register change a standard the store can enforce and the skill can follow. The available
discipline is Simplified Technical English (ASD-STE100), already the basis for §2's modal rules: one
instruction per sentence, active voice, no phrasal verbs, no semicolons, sentence caps of roughly 20
words for instructions and 25 for descriptions, noun clusters of at most three words, and a
project-glossary allowance for domain terms that must stay. Its own boundary applies — it fixes the
form of a text and never its substance, and a hollow paragraph rewritten under it stays hollow.

Constraints, all of them already ratified in `CONTEXT.md`:

- The card carries the reporting run's own options and its own recommendation. The sifter never
  authors an option and never advocates.
- Modal force survives: `may`, `might`, `could`, `appears`, `likely`, `reported`, `according to`,
  and explicit negation. The existing refusal on promotion stays.
- No fact the source did not state.
- **The whole card is at most 200 words**, title, headings, blocks, options and notes together. The
  live card is **647**. Measured:

  | part | words |
  |---|---|
  | title | 22 |
  | block "What happened" | 91 |
  | block "Discriminating fact" | 54 |
  | block "What remains theory" | 137 |
  | block "Independent promotion constraint" | 51 |
  | block "Recommendation" | 20 |
  | four options with notes | 261 |

  The operator answers this from a phone, between other things. A page of reading is a card he defers,
  and a deferred card is the queue backing up, which is the failure this whole Program exists to
  remove. Everything cut stays reachable in the evidence and in the run's own report; nothing is lost,
  it stops being *shown*.

- **A title is one short question, at most twelve words.** It is what the operator answers, never an
  instruction to himself and never a summary of how the run got here. The rail already shows the
  repository and package, so the title does not repeat them. Worked example, from the live card:

  > Before, 22 words: "Decide whether to further revise, remove, repartition, or retire TQ2's
  > criterion after attempt 8 independently refused the plan-version-3 successor a second time."
  >
  > After, 8 words: "Change TQ2's test, drop it, or stop here?"

- **Block headings are a fixed set, and there are at most two blocks**: what happened, and why the run
  cannot settle it itself. Nothing else. The live card invented `Discriminating fact`,
  `What remains theory`, `Independent promotion constraint` and `Recommendation` — four headings in
  workflow vocabulary, one of them 137 words of labelled theory, and one announcing that it contains
  no recommendation. A theory the operator cannot evaluate is not decision material; it is evidence.

- **A block body is at most 60 words and contains no semicolon.** Three sentences is the working
  shape. The live blocks run to five clauses chained with semicolons, which STE bans outright.

- **An option is at most 15 words and its note at most 20.** The note says what that option costs or
  buys, in the run's own terms. The live options average 66 words each including notes.

- **The recommendation is a property of an option, never a block.** `recommended_by_run` already
  exists on `SiftedOption`. A card that spends 20 words saying it does not recommend anything has
  spent 20 words saying nothing.

- Where shorter wording would lose attribution, negation or modal force, quote the source unchanged
  and say so, rather than compressing.

### The voice, in the operator's own words

> "Explain things as if they were important and I am in a hurry."

That is the standard. Not clipped, not casual — **urgent**. Lead with what is at stake. Say the thing
that decides the answer first. Never make the reader assemble the point from four blocks of history.
The operator is triaging a queue between other work; every sentence he reads before reaching the
decision is a sentence that makes him defer the card.

### The live card, rewritten to the budget

Same facts, same options, nothing invented, hedges intact. 180 words against 647.

> **Change TQ2's test, drop it, or stop here?**
>
> **What happened.** TQ2's test has failed twice for the same reason, and you already revised it once.
> The test requires the leftover water to stay under one unit at every step. At the first step it is
> exactly one unit. As written, nothing can pass it.
>
> **Why the run cannot settle it.** Only you can change a frozen test. The run may not spend another
> attempt to reach that door. Separately, this plan names two repositories and promotion supports
> one, so this run cannot merge even if TQ2 passes.
>
> 1. **Revise the test again.** Plan version 4. TQ2 restarts; the four finished packages keep their work.
> 2. **Remove the test.** The reproduction check leaves the plan. Only TQ4's disclosure still touches sub-unit loss.
> 3. **Repackage without changing any test.** Already tried and retracted at version 2. Expected to fail the same way.
> 4. **Stop the run at version 3.** Nothing merges. Finished work stays on its branches.
>
> If you remove the test, nothing in the finished product proves the reproduction runs.

What left the card: attempt numbers, issuance numbers, plan-version-3 successor text, the
137-word theory about how CanalLosses partitions, the measured incidence ref, the byte-for-byte
arrivals claim, and every path. All of it stays in the evidence and in the run's own report. None of
it helps the operator choose.

What stayed: the reason the test cannot pass, the fact that only he can change it, one blocking fact
he would otherwise discover after answering, four options with their costs, and what he loses by
removing the check.

Decide what is mechanically checkable here and enforce that in the store; record what is left to the
skill and why. A rule only in prose is one this corpus has repeatedly measured being worked around.

## Defect 3 — an unsifted hold shows raw JSON in the queue rail

The rail's summary line prints the report payload verbatim:

```
{"driver_status_outcome":"blocked","establishing_ {"event":"package-parked","issuance":8,…
```

The open card handles the same state correctly — "Not yet sifted", a plain explanation, and the raw
report behind a disclosure. The rail should carry a neutral summary (repository, package, kind,
state, age) and no payload.

## Defect 4 — the registered-runs panel is a wall of absolute paths

Three runs render as nine lines of absolute paths, each repeating the same long prefix three times.
It answers none of the operator's questions: is this run alive, what is it working on, when did it
last move. Make it a fleet roster — repository, vision name, plan version, state — with paths
available but not dominant.

Per-run liveness is the subject of `briefs/orchestrator-liveness-brief.md` and belongs in this panel
when it lands; do not duplicate that work here.

## Defect 5 — the page does not refresh

The operator reloads by hand to see new holds. Refresh the page on an interval.

**It must not destroy typed work.** The answer box and the feedback box hold text the operator is
part-way through, and a ruling is exactly what he will be typing when the interval fires. Refresh
only when no field is focused and every field is empty, or preserve the contents across the refresh.
A naive meta-refresh is a defect, not a feature.

## Defect 6 — the server rejects valid requests, including answers, at random

Observed live, repeatedly, during the first session:

```
WARN pce: rejected overseer HTTP request peer=127.0.0.1:60608 error=failed to read HTTP request line
Caused by:
    Resource temporarily unavailable (os error 35)
```

`os error 35` is `EAGAIN`. `src/main.rs:1274` sets the listener non-blocking so the accept loop can
poll, and the stream accepted at `:1429` is passed straight to `serve_overseer_http` with no change
of mode. On macOS and the BSDs an accepted socket **inherits** `O_NONBLOCK` from its listener, so any
read that reaches the socket before the client's bytes arrive returns `EAGAIN`, and the request is
rejected rather than awaited. It is a race, so it fires intermittently and looks like a flaky page.

This is the highest-severity defect in this brief, because the same path serves the answer form. A
rejected POST is a lost ruling: the operator typed it, the page failed, and no record exists. The
operator has no way to tell a dropped answer from one that was never sent.

Put the accepted stream into a mode that can serve a request — blocking with a read and write
timeout is the obvious shape — and keep the listener non-blocking so the wake loop still polls.
A timeout is required: a blocking read with no deadline lets one stalled peer wedge the loop, and the
loop is also what wakes the overseer.

Do not paper over it by retrying on `EAGAIN` in the request parser. The parser is not the defect.

## Out of scope for this brief

The overseer reported: *"installed `pce` lacks overseer defect-brief and repair-dispatch
operations."* The fix pipeline — dispatching a brief for a binary defect, and installing when the
fleet is quiet — has counters (`Defect briefs dispatched: 0`, `Pending installs: 0`) and no
operations behind them. That is a separate brief; do not build it here.

## What this does not change

- The four attributed doors. Nothing here lets any session sign a record.
- The hold lifecycle, the wake-and-exit session model, routing refusals, or rule admission.
- The append-only store: every change is a new record, never a rewrite.
- Any frozen criterion. If satisfying this brief appears to require editing one, stop and report
  which criterion and why.

## Falsifiers worth pairing

Every defect here shipped behind passing criteria, so prefer checks that fail on `3b0b777` and pass
after:

- A hold opened with a `work-graph-terminal-stop:<hash>` kind whose declared act is a criterion
  revision accepts a card carrying one consequence sentence, and the page renders it.
- The same hold with a non-door act still refuses a consequence sentence.
- A card whose title exceeds twelve words, or is not a question, is refused or flagged — whichever
  you chose in Defect 2. The live 22-word title fails it.
- A card whose option text contains a shell command, a flag, or an absolute path is refused or
  flagged. The live option containing `pce graph freeze --vision-dir …` fails it.
- A block body over 60 words, or containing a semicolon, is refused or flagged.
- A card over 200 words in total is refused or flagged. The live card is 647.
- A card carrying more than two blocks, or a heading outside the fixed set, is refused or flagged.
  The live card has five blocks and four invented headings.
- A card that drops a hedge present in the report is still refused.
- An unsifted hold's rail entry contains no `{` from the report payload.
- The page refreshes on its interval, and a refresh with text in the answer box does not lose it.
- A request whose bytes arrive after the server accepts the connection is served, not rejected. A
  concurrent burst of requests, and a peer that connects and sends nothing, both leave the server
  answering and the wake loop running.
