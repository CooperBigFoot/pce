# Brief: /work-graph promotes a finished assembly — the PR is a record, not a review

Status: DRAFT — to be grilled before dispatch. Written 2026-08-17, near the end of the
second real-world driver run, when the human stated the ground truth: "there's no way I as a
human can verify all the work."

## The problem

Doctrine says nothing merges and promotion is a human act. In practice the run ends at a
proven assembly and the human's "promotion" would be rubber-stamping thousands of lines they
cannot read — eight packages and four plan versions in today's run. A human gate that cannot
distinguish good from bad work protects nothing; it only adds latency and implies a review
that never happened.

## The re-founded safety argument

The human act that matters happens at **freeze**, not at merge. The frozen criteria are the
definition of done; the binary now refuses any plan-version advance that changes or removes a
criterion (criteria invariance, landed 2026-08-17), so the promise cannot be weakened after
the ruling. The driver proves that promise per package (criteria + adversarial gate with
witness/repair replay) and again against the composed assembly (every criterion re-executed).
Promotion then adds no information — it carries an already-proven promise to main.

Therefore: when, and only when, a run reaches `assembly-completed`, `/work-graph` opens a PR
and merges it. The PR is the durable, human-legible record of what was promised, what was
proven, and what the human ruled along the way.

## What the skill does at assembly-completed

1. Push the assembly ref (and the per-package attempt branches it composes) to the
   repository's remote.
2. Open a PR from the assembly ref to the repository's default branch whose body is generated
   from the journal: vision name and plan versions traversed; per-package table (criteria and
   their final executions, gate findings accepted with witness/repair refs); every
   `package-park-overruled` rationale verbatim; every plan-version advance and what it
   revised; the assembly criteria results against the composed whole.
3. Merge it (no waiting for approval), with the merge commit referencing the journal path and
   the frozen graph sha256.
4. Report the merged PR URL to the human as the run's final line.

## Hard boundaries

- **Only `assembly-completed` promotes.** A blocked, parked, or partially complete run never
  pushes, opens, or merges anything — those end, as today, with the evidence and the human's
  options.
- **The skill still never freezes, overrules, or edits the journal.** Promotion is downstream
  of proof; the rulings that create proof remain human.
- **One repository at a time as authored.** Today's graphs are single-repository; the
  multi-repository question (graph schema has one authored_at_ref — pending ticket on #37)
  is explicitly out of scope.

## What a grill must settle

- **Merge mechanics:** merge commit vs squash; who is the committer; what happens when main
  moved since fa57982-style authored base (assembly is based on the authored ref — rebase?
  refuse and notify? merge and let git decide?). The authored-ref staleness case is real:
  visions run for days.
- **CI interplay:** if the repo has required checks on main, does the skill wait for them,
  and is a red required check a notify-the-human terminal state?
- **Push scope:** assembly ref only, or also the attempt branches for archaeology? (The
  journal references their oids; unpushed refs die with the local clone.)
- **Failure honesty:** if push or merge fails (auth, protection rules), the run's proof is
  intact — the skill must report exactly what succeeded and stop, never retry into a
  half-promoted state.
- **Doctrine text:** README/CONTEXT still say promotion is a human act. This work must
  update the doctrine to "the ruling is human, at freeze; promotion of a proven assembly is
  mechanical," or the docs will contradict the binary's behavior.

## Environment facts a dispatched agent will need

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`; skill at `skills/work-graph/SKILL.md`
  (landed 2026-08-17, commit 74ed34a on integration). This work extends that skill's SKILL.md
  and possibly adds a `pce` verb if journal-to-PR-body generation belongs in the binary
  rather than in skill prose — grill it; the binary already renders journals (run_render.rs).
- Merge workflow for pce itself: only in `/Users/nicolaslazaro/Desktop/work/pce-integration`,
  full suite, fast-forward main; `./install.sh` from the MAIN checkout only.
- Worked example for the PR body generator: today's journal at
  `/Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-10-incidence-core/driver-journal.jsonl`
  — 160+ events, 4 plan versions, 2 overrules with rationales, 1 join conflict, gate
  amendments; if the generator renders that legibly it renders anything.
- `gh` CLI is available and authenticated for github.com in this environment.
