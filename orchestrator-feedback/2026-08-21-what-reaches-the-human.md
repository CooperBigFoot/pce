# PCE workflow feedback: what-reaches-the-human (work-graph supervision)

- Date: `2026-08-21`
- Orchestrator: `/work-graph` skill, Claude Code, model claude-opus-5
- Run: `planning/2026-08-21-what-reaches-the-human`, frozen graph `graph.v1.json`, journal `driver-journal.jsonl`
- Outcome: `in flight` — OQ1 and OQ2 completed, OQ3 parked awaiting a human freeze, OQ5 dispatched, OQ4 and OQ6 blocked

This report is filed mid-run per the standing instruction to file workflow problems immediately
rather than reporting them. It covers the workflow, not the product change.

## Executive summary

The highest-impact finding is that a work-package graph can pass `pce graph check` with zero warnings
while a dependency edge's free-prose `reason` promises the dependent package a capability that the
dependency's own criteria never compel. That happened here: OQ3's edge said OQ1 adds a `hold register`
verb, OQ1's criteria only ever required hold-open idempotence and legacy-surface invariance, OQ1
legitimately completed without `register`, and OQ3's worker correctly parked as mis-specified after a
full dispatch. Nothing in the authoring, checking, freezing, or briefing path could have caught this
before a worker burned an attempt on it.

Two mechanisms worked well and should be preserved: package gates found a real defect class that the
authored criteria missed, twice, in two different layers; and the `herdr_session` isolation
requirement did what it was meant to do once the server existed.

One secondary friction: the skill mandates a dedicated Herdr session but the operator cannot start
that session from the shell the skill runs in, because Herdr's nesting guard refuses.

## Evidence reviewed

- `planning/2026-08-21-what-reaches-the-human/driver-journal.jsonl` (45 events at time of writing)
- `planning/2026-08-21-what-reaches-the-human/supervision.md` (this run's captured evidence)
- `planning/2026-08-21-what-reaches-the-human/graph.v1.json`
- `planning/2026-08-21-what-reaches-the-human/package-outcomes/OQ3/3.json`
- `planning/2026-08-21-what-reaches-the-human/.pce/package-briefs/OQ3/3.md`
- `herdr --session pce-reach pane read w5:p1 --source recent-unwrapped --lines 10000 --format text`
- `git show 9aa718f580bc8401fb60e4b5f9d1bc93013779ba:src/main.rs`
- `git show 9aa718f580bc8401fb60e4b5f9d1bc93013779ba:crates/core/src/hold_store.rs`
- `skills/work-graph/SKILL.md`, `orchestrator-feedback/README.md`, `orchestrator-feedback/TEMPLATE.md`

## What worked

### Package gates caught a defect class the authored criteria missed, in two independent layers

- Evidence: journal `finding-replayed` OQ1 gate `package-gate-1-1`, command
  `cargo test --test hold_store -- incomplete_tail_is_not_authoritative`; witness ref
  `5e4d26cbed9f16e2cc6d5119a17de3d89a86dc37` exited 101 with
  `assertion failed: matches!(store.read(key), Err(HoldStoreError::IncompleteTail { .. }))` at
  `tests/hold_store.rs:154`; repair ref `08fb1d8ec36683d57c8abb3139977dceb86817c9` exited 0.
  Journal `finding-replayed` OQ2 gate `package-gate-2-1`, command
  `cargo test --test overseer_rulebook -- interrupted_rulebook_install_recovers_incomplete_tail`;
  witness ref `2c5246419e240c3aaf4cbd012e2006b6c25c0528` exited 101 with
  `a valid prefix with an incomplete tail should resume installation: IncompleteRulebookTail`
  at `tests/overseer_rulebook.rs:97`; repair ref `fbf7bb0d9f5f55d41105f8d944fca90dd9d76358` exited 0.
- Effect: both packages' authored criteria had already passed (`criterion-executed` OQ1 ×2 exit 0,
  OQ2 ×1 exit 0) before the gates ran. Two instances of "a partially written final JSONL record is
  read as authoritative" were repaired and merged (`package-repair-merged` to `08fb1d8e` and
  `fbf7bb0d`) instead of escaping into dependents. Dependents composed on the hardened oids, not the
  pre-repair ones (`package-base-composed` OQ2/OQ3/OQ5 all name `08fb1d8e…`, OQ5 also `fbf7bb0d…`).
- Inference, clearly distinguished: the repetition across two unrelated layers suggests the graph's
  authored criteria systematically omit interrupted-write recovery. OQ4 and OQ5 also write records,
  so a third instance is plausible. This is a product observation, not a workflow defect, and is
  listed under follow-up rather than as a required workflow change.

### Dedicated Herdr session isolation behaved as specified

- Evidence: `run.json` `herdr_session: "pce-reach"`; journal `dispatch-pane-opened` records carry
  `herdr_session: "pce-reach"` for every attempt; `herdr --session pce-reach workspace get w5` returns
  label `OQ3:pce:attempt-3`; the supervising shell is workspace `w1` in the same session, distinct
  from worker workspaces `w2`–`w7`.
- Effect: worker panes and workspaces were addressable and separable from the unrelated workspaces
  visible in the default session, and the label corroborated journal-selected workspace ids without
  ever being used to select them.

### The launch preflight refusal was correct and cheap

- Evidence: `supervision.md` records the prior invocation preserving the typed message
  `{"code":"server_not_running","message":"no herdr server is running at
  /Users/nicolaslazaro/.config/herdr/sessions/pce-reach/herdr.sock; …"}` and refusing to launch, with
  no driver, tmux session, or journal created.
- Effect: no half-started run and no journal to reconcile. The next invocation re-ran the preflight,
  found the server up, and launched into a clean first journal.

## Friction and failures

### A dependency edge's prose `reason` can promise a capability no criterion compels, and nothing checks it

- Severity: `high`
- Phase: `graph authoring / freeze / briefing` — the failure surfaced in execution but originates upstream
- Observation: `graph.v1.json` OQ3 `depends_on[0].reason` reads "the changed skill invokes the hold
  open **and register** verbs that OQ1 adds to the binary". OQ1's two frozen criteria are "One
  question, asked once" (hold-open idempotence) and "A live run is untouched by the new verbs"
  (legacy-surface invariance). Neither names a register verb. OQ1 passed both criteria and its gate
  and completed. OQ3's worker then dispatched, checked, found no register verb, and parked.
- Evidence:
  - `package-outcomes/OQ3/3.json`:
    `{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":{"missing":"the pce hold
    register verb and durable run-registration record …","checked":["src/main.rs::HoldCommand and
    parse_hold_command","crates/core/src/hold_store.rs::HoldRecord and HoldStore"],"command":"cargo
    run --quiet -- hold register …"}}}`
  - `git show 9aa718f5:src/main.rs` — `parse_hold_command` accepts exactly
    `open, list, read, answer, route, close`. `git show 9aa718f5:crates/core/src/hold_store.rs` —
    `HoldStore` exposes `new, root, open, read, list, answer, route, close`. No `register`.
  - `.pce/package-briefs/OQ3/3.md` §3 renders the edge prose verbatim to the worker, and §4 tells it
    "OQ1 … out of bounds … report a missing dependency … instead of crossing the boundary".
  - `pce graph check --file graph.v1.json` → `{"packages":6,…,"valid":true,"warnings":[]}`.
  - Worker pane `w5:p1`: "Recorded a `mis-specified` outcome. The required `pce hold register`
    dependency is absent from the composed OQ1 implementation. The worktree remains clean."
    `git -C <worktree> status --short --branch` and `diff --stat` confirm nothing was written.
- Inference, clearly distinguished from the observation: the edge `reason` is free prose that no
  tool cross-checks against the dependency's criteria, and the brief presents that prose to the
  worker with the same authority as a criterion. The worker had no way to discover the mismatch
  before dispatch, and behaved exactly as instructed once it did.
- Impact: one full worker dispatch consumed on a package that could not be started, OQ3 parked, OQ4
  and OQ6 transitively blocked, and a human non-mechanical freeze now required to unblock a defect
  that is purely an authoring slip. The supervisor's one-shot overrule is not admissible here,
  because the missing work is provably absent rather than present or unnecessary.

### `/work-graph` mandates a dedicated Herdr session the operator cannot start from the session's own shell

- Severity: `medium`
- Phase: `launch`
- Observation: with `herdr_session` configured and the server down, the skill requires refusing the
  launch and leaves starting the server to the operator. The operator ran
  `herdr session attach pce-reach` from the Claude Code shell and Herdr refused with
  `nested herdr is disabled by default … "recursion detected. base case not found. aborting."`.
  The shell reports `HERDR_ENV=1`. `herdr session --help` exposes only `list, attach, stop, delete`,
  with no daemon-only start verb.
- Evidence: `supervision.md`, section "Operator precondition blocked by herdr nesting guard", which
  records the refusal text, `HERDR_ENV=1 HERDR_SESSION=`, and the `session --help` verb list.
- Inference: `/work-graph` is normally invoked from inside a Herdr-managed shell, which is precisely
  the context in which `attach` is refused. The instruction "the operator owns that precondition" is
  therefore unsatisfiable from where the instruction is read.
- Impact: one whole invocation produced no run. It cost a full skill invocation, and was only
  resolved because the operator went to a shell outside any Herdr environment between invocations.
  The skill never says that is required.

### The skill's package-inventory step has no schema anchor, and this supervisor misread the graph

- Severity: `low`
- Phase: `orientation`
- Observation: invocation 1 recorded in `supervision.md` "No package declares dependencies." The
  graph's edges live under `depends_on`; the note was produced by reading a `dependencies` key that
  does not exist in the schema. The error was caught only when the driver's own
  `package-base-composed` events showed OQ2 and OQ3 composing on OQ1, and was corrected in
  `supervision.md` under "Correction to invocation 1's package table".
- Evidence: `supervision.md` invocation-1 "Packages and repositories" table versus the correction
  section; `graph.v1.json` `packages[].depends_on`.
- Inference: §1 and §4 of `skills/work-graph/SKILL.md` name event kinds precisely but name no graph
  field except `packages[].repositories`, so the inventory step is done from guessed field names.
- Impact: for four packages the supervisor reported the run's shape to the human incorrectly and had
  to correct it in chat. No command was run against the wrong reading and no run state was affected.

## Recommendations

### Constrain a dependency edge's `reason` to capabilities the dependency's criteria name

- Addresses: "A dependency edge's prose `reason` can promise a capability no criterion compels"
- Change: instruct the graph author that an edge `reason` may only cite behaviour that appears in the
  named dependency's own criteria, and that a capability the dependent needs but no criterion of the
  dependency compels must be given a criterion in that dependency (or be owned by the dependent).
- Location: `skills/to-graph/SKILL.md`, the section that authors `depends_on` edges
- Trade-off: constrains how edge prose is written and lengthens some reasons; does not change the
  frozen schema or any existing criterion
- Confidence: `high`

### Render the dependency's criteria, not only the edge prose, in the package brief

- Addresses: same finding
- Change: in the brief's `Dependencies` line, print each named dependency's criteria names alongside
  the edge `reason`, so a worker sees what the dependency was actually proved to do before it starts.
  The brief already lists all packages' criteria in §2; this is a locality change, not new data.
- Location: brief generation in `src/main.rs` (the `Dependencies:` block of the package brief)
- Trade-off: a modestly longer §3; some duplication with §2
- Confidence: `medium` — this makes the mismatch discoverable at dispatch time but does not prevent
  it at authoring time, so it complements rather than replaces the recommendation above

### Say where the Herdr session must be started from

- Addresses: "mandates a dedicated Herdr session the operator cannot start from the session's own shell"
- Change: in the preflight paragraph, state that `herdr session attach <name>` must be run from a
  shell outside any Herdr environment (`HERDR_ENV` unset), because the nesting guard refuses it from
  inside one, and have the supervisor report `HERDR_ENV` alongside the preserved `server_not_running`
  message so the operator sees why.
- Location: `skills/work-graph/SKILL.md` §3, the paragraph beginning "Before every launch with
  `herdr_session` configured"
- Trade-off: none identified; it is added guidance on an existing refusal path
- Confidence: `high`

### Name the graph fields the orientation step must read

- Addresses: "package-inventory step has no schema anchor"
- Change: in the step that derives repository names, also name the fields to report:
  `packages[].id`, `title`, `repositories`, `produces`, `criteria[].name`, and `depends_on[].id/kind`.
- Location: `skills/work-graph/SKILL.md` §2, the paragraph deriving repository names
- Trade-off: couples the skill text to the graph schema's field names, which must then be updated
  together
- Confidence: `medium`

## No-change decisions

- **The one-shot overrule did not need loosening.** OQ3's park is exactly the case the restriction
  protects: the missing work is provably absent, so overruling would have required asserting a design
  opinion contradicting the brief the binary itself generated. The restriction produced the right
  refusal and cost nothing beyond a correct escalation. No change recommended.
- **The `mis-specified` fault schema did not need changing.** The brief's requirement that a
  missing-dependency fault name what is missing, at least one checked path or symbol, and the exact
  command used to check made the complaint independently verifiable in three commands. It is the
  reason this report can state the defect as fact rather than as the worker's claim.
- **The driver was not stopped for the park.** OQ2 was in flight and OQ5 was still reachable, so the
  park blocked one branch rather than the run. No workflow change is needed; the existing behaviour
  is correct.

## Suggested follow-up

- A separate issue or vision covering the recurring defect class in the product itself: a partially
  written final JSONL record read as authoritative, found by gates in `hold_store` and in the
  rulebook installer. The remaining record-writing surfaces in this graph (OQ4's server, OQ5's rule
  ledger) should be checked for the same gap rather than waiting for their gates to find it a third
  and fourth time.
- Whether `pce graph check` could mechanically warn when a `depends_on` reason contains a
  quoted verb or symbol absent from every criterion of the named dependency. Mark as an experiment:
  it is a heuristic over free prose and would produce false positives, so it should be trialled as a
  warning, never a refusal.
