# Brief: an answered hold reaches nobody, so answering a card changes nothing

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine the
evidence, decide, implement, and record what you decided and why in your completion report and
`CONTEXT.md`. Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion),
stop and report. Written 2026-08-21 from the second live overseer run, against `main` at `3b0b777`
plus the uncommitted first-run fixes.

**This is the missing half of the loop, and it makes the built half inert.** Four conforming cards are
waiting for the operator right now. If he answered all four, nothing would happen.

## The defect

The channel is one-way. Runs can speak to the overseer; the overseer cannot reach a run.

`skills/work-graph/SKILL.md:380` — a stopped run *"opens a durable hold and waits"*.

`skills/work-graph/SKILL.md:406` — *"On later invocations, read that key from the store and its full
record thread before acting."*

**"On later invocations" means the next time a human types `/work-graph`.** The supervising session
opens its hold, ends its turn, and idles. Nothing wakes it when an answer, a route, or a fact lands
in the store. Measured tonight: the overseer answered pourpoint's hold and routed it to
`reporting-run`; pourpoint's session was idle and stayed idle; no driver ran; the operator observed
that every orchestrator "looks stuck". They are.

Compare what exists for the overseer: `pce overseer serve` watches the store, computes a fingerprint,
and wakes a session on change (`src/main.rs`, the wake loop). Runs have no equivalent. The asymmetry
was never decided — it is an omission, not a design.

## What must become true

A run whose hold is answered resumes without the human touching it. That is the whole promise of the
vision: the operator answers a card in one sentence and the fleet moves. Today he answers a card and
must then remember which repository it was, open a session there, and re-invoke the skill — which is
the pane-to-pane relaying this Program exists to delete.

Shape is your decision. These constraints hold:

- **The human performs no mechanics.** He answers in the queue. Nothing else.
- **A run must not be woken into unsafe work.** Only a recorded answer or route that supplies a
  goal-preserving action allowed by section 6 may resume a driver. An answer that settles nothing
  leaves the run waiting, exactly as today.
- **Never two drivers against one journal.** `SKILL.md:168` is explicit, and a wake that races a live
  driver is the way to break it. Whatever wakes a run must first establish that no driver is running
  for that journal.
- **Liveness is observed, never asserted** (ADR 0012). A run that died is not a run that is waiting,
  and waking a dead run's session is not the same as resuming its work.
- **The run's identity is already recorded.** `pce hold runs` returns repository, vision directory,
  frozen graph, journal and Herdr session for every registered run. Everything needed to reach a run
  is already in the store.
- **Bounded and observable.** A wake that fails must be a recorded fact, not a silence. This corpus
  has spent a day on indicators wired to claims rather than to things; do not add another.

Candidate mechanisms, none mandated: a supervisor process per run analogous to `overseer serve`; a
run-side watch inside the existing driver; the overseer spawning a supervising session for the target
run when it routes a hold to `reporting-run`. Weigh them against the constraints above, choose, and
record the reasoning. The third is attractive because the overseer already runs and already knows the
run's Herdr session, and hazardous because it makes the overseer a spawner of repository-mutating
work; if you choose it, say what stops it from starting a run that should stay stopped.

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

**Your files.** `skills/work-graph/SKILL.md` and the wake mechanism in `src/main.rs`. The parallel
agent owns `crates/core/src/overseer_view.rs`, `crates/core/src/hold_store.rs` and
`skills/overseer/SKILL.md` — leave those alone.

**Read the overseer serve wake loop in `src/main.rs` before designing yours.** It already solves the
problems you are about to meet: never two passes at once, a failure budget that a store change must
not reset, and a slow periodic wake as the only recovery path for an exhausted budget. Diverging from
it needs a reason you can state.

**This brief is what makes the others matter.** Until it lands, answering a card changes nothing.
Treat "the operator answers in the queue and the run resumes, with no other human action" as the
thing you are proving.

## What this does not change

- The four attributed doors. Nothing here lets any session sign a record.
- The hold lifecycle, the card discipline, or the overseer's wake-and-exit model.
- The rule that only the overseer closes a hold.
- Any frozen criterion. If satisfying this brief appears to require editing one, stop and report which
  criterion and why.

## Falsifiers worth pairing

- A registered run whose hold is answered with a goal-preserving action resumes, with no human action
  beyond answering in the queue. On `3b0b777` nothing happens; this is the pairing.
- A hold answered with something that settles nothing leaves the run waiting and records why.
- A wake attempted while a driver is already running for that journal is refused and recorded, and no
  second driver starts.
- A wake attempted against a run whose session and driver are both gone is recorded as a failure the
  operator can see, not as a silent no-op.
