# Feedback review round 1 — 2026-07-27

Five orchestrator reports reviewed in one session. **No Program was charted and no GitHub
issue was created.** The review was deliberately paused so more runs can produce more
evidence before any Effort ticket is minted. This file exists so the next review round
resumes from these decisions instead of re-deriving them.

## Reviewed

| Handle | Report | Run | Outcome |
|---|---|---|---|
| R1 | `2026-07-26-iteration-benchmark-harness.md` | bluesmith↔stopwatch, cross-repo, Fable 5 | completed + unmodelled operational phase |
| R2 | `2026-07-26-tile-count-independent-planetary-cog-reads.md` | pourpoint, Opus 5 | completed, 0.2.1 to PyPI |
| R3 | `2026-07-27-no-d8-raster-lies-silently.md` | pourpoint, Opus 5 | paused (M1–M2 merged) |
| R4 | `2026-07-27-no-d8-raster-lies-silently-2.md` | same vision, resumed | completed (M3–M5) |
| R5 | `2026-07-27-post-2019-deep-learning-directions.md` | grant-proposal-nik, LaTeX prose | completed |

Plus `ANALYSIS-feedback-synthesis.md` (cross-report synthesis, conflicts, structural themes)
and `ANALYSIS-firstmate-comparison.md` (mechanism comparison against
`thirdparty/firstmate`).

Every run delivered. No product-quality defect escaped into merged code that later needed
reverting. The findings are about burned rounds and structural blindness, not about the
workflow failing.

## Structural themes

Ranked, from the cross-report synthesis:

1. **The orchestrator is the workflow's unspecified region.** `SKILL.md` specifies Codex and
   the Claude gates exhaustively and the orchestrator in one line of prose. Two faces: what
   it authors is ungated and is the most privileged text in the system, and what it does was
   improvised in every run (operational phase, filesystem verdict channel, red phase,
   executor escalation terminal state, retry policy, withdrawal disposition). Every
   improvisation was competent, which is the problem: rigor became a per-instance judgment
   call rather than a chosen policy.
2. **Falsifiability is not modelled anywhere.** A check is validated by having produced
   output, never by being able to fail. R2 counts 30+ instances in one run and notes that
   none was found by reading.
3. **Gate scope is drawn around the artifact of record.** Defects live in composition (every
   step green, the milestone broken) and in artifact kinds the ladder has no theory of
   (prose, CI workflows, real environments).
4. **Artifacts freeze at authoring time and nothing re-grounds at consumption time.**
5. **Caps proxy for a signal the runs already produce and the workflow never reads.**
6. **Runtime-learned facts are homeless, so knowledge does not travel.**
7. **The cost model is implicit, so every proposed fix is priced in a currency nobody
   tracks.**
8. **The loop these reports exist to close is open.** Zero of ~30 recommendations had
   reached `skills/pce/SKILL.md` as of this review; verified line by line.

## Destination

The orchestrator stops being the unspecified region. Invariants move out of prose into
mechanism, and firstmate's transferable mechanisms are adopted. Point fixes fall out as
consequences rather than as thirty independent prose clauses.

**The Program stands on one falsifiable hypothesis:** prose fails for invariants the
orchestrator must apply to itself under context pressure, and works for contracts handed to
a fresh agent that reads them once. If a later report shows a frame clause being dropped or
a hook being worked around, the premise needs revisiting rather than more mechanism.

## Resolved decisions

1. **Destination is mechanism, not a patch pass.** Falsifiability is the ordering criterion
   for what gets mechanized first. Whatever genuinely remains prose stays prose.
2. **Scope boundary is the corpus.** Nothing gets a ticket without a finding behind it.
3. **Enforcement split by finding class.** Prohibitions are command shapes and belong at the
   tool boundary as hooks. Verifications are computations and belong in the `pce` binary.
   Durable per-repo facts belong in a tracked file read only from the default branch.
4. **Hooks install globally**, with each hook's condition narrow enough that it almost never
   fires outside a PCE run. Scoping by run marker was rejected because it reintroduces a
   thing that can be missing, which is the failure firstmate documented in
   `docs/subagent-guard.md`.
5. **`state.json` is replaced by an append-only event log plus derived state.** Rule: if git
   or `gh` knows it, never store it; if only the orchestrator knows it, append it once when
   it happens and never revise it. Append-only prevents retroactive revision and gives an
   auditable partial-failure trail; derivation makes a hallucinated success impossible rather
   than merely detectable. Both halves are needed; they solve different problems.
6. **The binary owns dispatch.** `codex exec` is a subprocess so the binary builds the whole
   invocation, which kills the backtick-substitution, stdin-hang and schema-path traps by
   construction. Claude subagents cannot be spawned from a CLI, so the binary registers the
   dispatch and its expected verdict path beforehand and validates the verdict file
   afterwards, with a hook denying the Agent tool when no registration exists. Keying the
   hook on tool *shape* rather than on metadata is required, because the failure mode is the
   metadata being absent. A `pce log` verb makes appends schema-valid, timestamped, atomic,
   and increments the round counter in the same call.
7. **Dispatch does prompt assembly, not just invocation shape.** The verb owns the frame,
   the skill owns the task. The frame injects ground-truth ref and read command, repo and
   gate commands, environment hazards, output path, role-fixed review obligations, the
   delegation contract's five elements, and the mutation prohibition for gate roles. This is
   the unbuilt feature that hand-copied boilerplate was paying for: ~10 retyped ref clauses
   in R3, 14 retyped verdict-path clauses in R3/R4, 6 identical lockfile blocks in R1.
   Requires a `--dry-run` that prints the composed prompt, to offset the legibility loss.
8. **Orchestrator assertions go through a labelled channel.** A claim enters a prompt only
   via an assertion flag carrying the exact command that produced it. The verb labels it
   orchestrator-authored and unverified, appends it to the event log, and injects into every
   gate frame that labelled assertions are its first verification target and a false one is
   CRITICAL. Discriminator: **reproducibility at the ref.** If the agent could derive it at
   the named ref, the orchestrator must not inline it and must say what to derive instead.
   If it could not, because the fact came from a live or network-gated run, it is inlined
   with its command attached. Requiring the command also catches R5's failure shape for
   free, where the claim was "no double-hyphen" and the evidence was a scan for `' -- '`.
9. **Loop termination is repetition-triggered, not count-triggered.** Compute the
   convergence signal (blocker count strictly decreasing, no recurring finding) from the
   verdict JSON instead of counting rounds. Demote the ceiling to a cost backstop; PCE keeps
   one because a round is 40–58k tokens, unlike firstmate. Put the stop trigger in the
   dispatched agent's own contract, firstmate-style ("same obstacle twice, stop"). Escalations
   become keyed decision holds that block dependents and close only when the human's answer
   is recorded and every dependent is named and unblocked. Add deterministic superseded
   detection. Split `paused` (bounded external wait) from `blocked` (supervisor must act).
   Add WITHDRAW, presented to the human, never decided by the orchestrator.
   Note: monotonic decrease is mechanical; "no finding recurs" beyond exact match on
   `location` plus `problem` stays judgment.
10. **Agent-facing text moves into versioned role frames owned by the binary.** `SKILL.md`
    keeps only orchestration logic: graph structure, sequencing, parallelism, escalation
    routing, git and merge ownership. The seven inlined prompt templates go away. This is how
    the ~20 judgment-class findings land: the end-state question, the prose glossary check,
    invariant-not-cause scoping, confirmation critics continuing past the first verified fix
    become fixed clauses in the relevant role's frame rather than rules to remember.
    Accepted costs: no single-file readability, frames need versioning, frames become the new
    drift location with fewer copies.
11. **Measure cost, do not budget it.** Every dispatch routes through the dispatch verb, so
    the binary records tokens and wall-clock per dispatch into the event log. This makes the
    corpus's unresolvable cost arguments settleable on data. No ceiling is set now.
12. **No artifact-class fork.** One workflow for code and prose. The prose gap is a frame
    obligation on the reviewer triggered by the diff touching user-facing prose, not a second
    path. R5's ceremony defenses are treated as under-measured rather than domain-specific;
    recorded as an ambiguity in `CONTEXT.md` with the meter as its resolution condition.
13. **Reality-contact moves earlier, with a bounded fallback.** Cost-bounded, pre-authorized
    contract probes before mocked milestones pin external contracts in fixtures; R1 estimates
    this converts 5 of its 8 operational defects into ordinary build-phase test inputs. The
    compressed post-merge fix loop is permitted but **keeps the PR reviewer**. Principle: a
    hard external oracle answers the plan critic's question, so dropping the plan critic is
    defensible; nothing replaces the reviewer, which answers whether the diff claims
    something false.
14. **The vision is checked at Phase 1**, not at `/to-vision`, because the planning ref does
    not exist at authoring time. Two checks: do the vision's claims hold at the ref
    (classify each acceptance criterion as holds / already satisfied / not reproducible, with
    the third class escalating before decomposition), and does the vision contradict itself.
15. **Knowledge leaves a run through three chosen channels.** Repo-portable facts to the
    tracked per-repo file (gate commands with measured exit status, lockfile regen, gate
    ordering, environment hazards, doc-truth defects). Workflow rules to frames, hooks and
    verbs, with tests. Report recommendations to Effort tickets or Fog on the Program Map.
    `orchestrator-feedback/README.md` currently tells an orchestrator how to write a report
    and nothing about where it goes, which makes a report a dead end by construction.
16. **`/land-ticket` isolation relaxes for evidence-carrying entries.** The trust-boundary
    argument weakens once the log is append-only with labelled assertions carrying their
    evidence commands.
17. **`planning/` stays gitignored; planning artifacts become append-by-round.** A revision
    writes a new round file and moves a pointer, never overwriting. The binary emits the
    inter-round diff and halts for confirmation on a net deletion beyond a threshold. Keeping
    it gitignored preserves the self-containment discipline two reports credit; append-by-round
    catches R2's 72% truncation mechanically. Both R2 and R5 already improvised this by hand.
18. **Phase 0 must execute the gates it records.** A command that fails at base is not an
    acceptance gate. The contract additionally enumerates every workflow in
    `.github/workflows/` with its local stand-in or an explicit note that none exists.
19. **`freshness_check` must be the consumer's test command**, or state why weaker suffices.
    Check-only defaults are forbidden. R1's `cargo check` false-greened 21 latent failures
    and forced an unplanned milestone.
20. **Landed-work proof before any destructive step.** firstmate's three-way proof reduced to
    PCE's shape: PR reports merged and the squash commit's content is reachable from the
    milestone branch, otherwise refuse and escalate. Inconclusive means refuse. Addresses the
    `gh pr merge --delete-branch` recurrence that three reports raised and nothing fixed.

## `/land-ticket` defect found during this review (not from a report)

The skill asks bare questions instead of grilling with recommendations, and has to be told
each time that the invoking session orchestrated the ticket. One cause, three symptoms:

- `SKILL.md:58` bans inspecting the planning directory, `vision.md`, PCE state, commits,
  history, PRs, implementation output, acceptance gates and delivery artifacts. Its stated
  purpose is narrow ("to independently verify delivery") but its scope is total, so §6's
  grill has no evidence from which to recommend anything.
- `SKILL.md:62` asks the human for the one-line decision "when it cannot be reconstructed
  unambiguously." Reconstruction is impossible under that ban, so it always asks, and it asks
  in §4, before the grill discipline starts in §6.
- `SKILL.md:8` says never rely on a previous conversation, which discards the orchestration
  context that is present in actual usage. R4 reported the same thing from the other side.

**Resolved:** narrow the ban to the delivery-and-merged assertion only; apply the grill
discipline from the first question including the decision line; let warm invocation use warm
context with cold invocation still working as a fallback. The delivery-trust boundary itself
is **kept** as chosen.

## Deliberately unresolved (Fog)

- **Is layered defect discovery intrinsic to adversarial review, or an artifact of how
  critics are prompted?** R3 and R5 both name it and neither tested it. It determines whether
  the cap needed changing at all. One deliberate trial.
- **Whether "execute, don't just read" should be an explicit critic instruction.** R3 calls
  it the strongest correlation in its evidence and declines to make it a rule, in case it is
  a property of its own prompting.
- **Where the falsification burden sits**, plan-writer versus critic. R2 and R4 place it
  oppositely with opposite confidence. Blocked on the meter.
- **Whether R5's ceremony defenses are domain-dependence or under-measurement.** Blocked on
  the meter.

## Constraints that earned their place — do not trade away

Multiply credited, several load-bearing for each other:

- Codex authors, Claude gates, orchestrator owns git, state and escalation. R2: this alone
  kept a critic's stray commit (`c9c6e19`) local and reversible, and no executor cut a tag
  despite a repo script printing `git tag pourpoint-v0.2.1` as literal advice on stdout.
- Ref-based grounding on every dispatch, with a ref/checkout mismatch explicitly not a
  finding. The firstmate comparison calls this PCE's single largest advantage; firstmate has
  nothing comparable.
- Cold re-dispatch on every REVISE, never `resume`. R3 verified byte-identical `files_touched`
  across revision rounds in three graphs.
- Executor stop-do-not-improvise, plus `root_cause` in the verdict schema.
- Critics that execute checks rather than read them. R2: the single highest-yield mechanism;
  the cannot-fail class hit 30+ instances and none was found by reading.
- Worktree isolation with the writable parent `.git`; two-tier merge; human-only tag/release
  boundary; the delta mechanism; escalation gates persisted in state rather than in
  conversation; plan self-containment; witness transport with halt-if-absent.
- `self_sufficiency` item (c), assertion blast-radius. R3: items (a) and (b) would not have
  caught `S1-DONUT-GRASS-BREAKS-HOLE-ASSERTION`.

## Success conditions for the eventual Program

1. No finding marked fixed recurs in a subsequent report.
2. The next report's recommendations target frames, verbs, hooks or the per-repo contract
   file rather than `SKILL.md` prose.
3. The meter has a per-dispatch cost baseline.

Explicitly **not** success conditions: fewer escalations, fewer rounds. R5 shows cap
exhaustion routed to a human beating more automation, and R1 shows an escalation that paused
a run for days being the mechanism working.

## Delivery plan: PCE modifies PCE

The implementation goes through the normal flow, so each Effort ticket gets its own
`/work-ticket` → `/to-vision` → `/pce` → `/land-ticket` cycle. The work is clearly
multi-vision (event log plus derived state, dispatch verb with assembly, role frames, hooks,
the per-repo contract file, Phase 0 gate execution, the `/land-ticket` fixes), which is what
the Program layer exists for.

**Self-modification hazards.** This repo's skills and binary are the ones the orchestrator is
running from: `install.sh` symlinks `skills/*` into `~/.claude/skills/` and the release binary
into `~/.local/bin/pce`. Consequences for ticket ordering:

- **The binary takes effect immediately on merge.** `SKILL.md` is loaded once at invocation, so
  a mid-run change to it does not retroactively alter the running orchestrator. The binary is
  invoked per dispatch, so a merged CLI change applies to every subsequent dispatch in the same
  run. A run can therefore use the old dispatch verb for its first milestones and the new one
  for its last, which is a real inconsistency, not a theoretical one.
- **A hook can deny the orchestrator's own commands.** The Agent-tool registration hook breaks
  every subsequent critic dispatch unless the registration verb is already live. That is a hard
  blocking edge: the verb ships before the hook that depends on it, never the reverse.
- **Global hooks live outside this repo**, so no merged PR can activate them. The deliverable is
  the hook scripts plus `install.sh` wiring; activation needs a human `install.sh` run. Any
  ticket whose acceptance depends on a hook actually firing has a manual step in it.
- **Frames and the skill should land late**, after the verbs they call exist, for the same
  dependency reason.

The safe general ordering is: event log and derived state, then dispatch verb without assembly,
then assembly and frames, then hooks last. Treat this as input to charting, not as a decided
graph.

## Next round

Run PCE across the active repos, have each orchestrator write a report into
`orchestrator-feedback/` using `TEMPLATE.md`, then re-review with this file as the baseline.
The specific thing to look for: whether the new reports raise findings already resolved here
(which tests nothing new) or findings these five could not see (which is what more runs are
for).

Terminology from this session is recorded in root `CONTEXT.md`: seven canonical terms, two
aliases to avoid, five relationships, one ambiguity.
