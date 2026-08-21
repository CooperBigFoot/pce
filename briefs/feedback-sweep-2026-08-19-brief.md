# Brief: seven open workflow defects from twelve orchestrator reports

Status: READY TO DISPATCH — no grill. Every decision below is evidence-resolvable: examine the
evidence, decide, implement, and record what you decided and why in your completion report and
`CONTEXT.md`. Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion,
the credentials-are-the-fence ruling in workstream D), stop and report. Written 2026-08-19 by the
PCE supervisor after processing twelve orchestrator feedback reports from four live runs.

**Fan out.** These seven workstreams touch disjoint code regions and are meant to be worked in
parallel by separate agents, then landed in one integration pass. Suggested split — A alone;
B and C together (both live in the `graph` command paths of `src/main.rs`); D and E together (both
in `crates/core/src/package_worker.rs`); F alone; G alone (documentation only, land last so it
describes the behavior the others ship). Do not let two agents edit the same file.

## Where this comes from

Four independent runs filed twelve reports on 2026-08-19:

- `orchestrator-feedback/2026-08-19-signal-bearing-dudh-warm-window{,-2,-3,-4}.md` (bluesmith)
- `orchestrator-feedback/2026-08-07-declare-grit-d8-...-row-seam{,-2,-3,-4,-5}.md` (pourpoint)
- `orchestrator-feedback/2026-08-19-close-the-seven-basin-coverage-gap.md` (hfx)
- `orchestrator-feedback/2026-08-19-a-fixture-is-a-recording.md` (RivRetrieve)
- `orchestrator-feedback/2026-08-19-a-provider-declares-itself.md` (RivRetrieve)

I verified every claim below against the source at `main` = `4c0d33d` before writing it. **Findings
already fixed — do not re-do them:** the `pce-protect-criteria` substring collision (fixed;
`hooks/pce-protect-criteria.sh` now matches resolved active-vision paths with a boundary regex,
installed copy identical, regression cases present in `tests/criterion_protection_hook.rs`);
per-repository `authored_at_refs` plus freeze-side refusal; gate failures billed to a separate
`GateFailureLimit`; gate refs anchored under `refs/pce-gate/`; the criterion-revision door;
mechanical freeze; `--worker-env` forwarding and `worker-environment-extended`; byte-exact repair
credit replaced by `RepairCreditStale` plus the typed `DriverAborted` event; assembly ref
materialization (`refs/heads/pce/<vision>/assembly-v<N>`, `derive_driver_ref_intents`,
`src/main.rs:5890`). Four separate reports also asked for the hook fix; all four are satisfied.

---

## A. The recovery ladder spends its whole budget on blockers no worker can fix

**Evidence.** hfx `close-the-seven-basin-coverage-gap`: package SB3 was dispatched **17 times
across 4 plan versions and never created a single cloud resource**. Every failure was a
precondition outside the package — an unset environment variable the driver did not forward, then
a governance runbook that did not exist. Grouped by identical cause: issuances 5,6 / 9,10 (same
authorization complaint), 13,14 (`HFX_CAMPAIGN_EVIDENCE and HFX_S3_ENV_FILE are unset`), 15,16,17
(missing runbook + approval). Four human rulings and four frozen plan versions were consumed
resetting a ladder that was then immediately re-spent on the unchanged blocker; two of those plan
versions were pure bumps. bluesmith saw the same shape with `NoCredentials` failures advancing
W2/W3 toward the local-patch rung — where the only in-scope move is to weaken the check, which is
exactly what produced that run's tolerance-widening incident.

**Second half of the same defect.** The park record discards the cause. `src/main.rs:2284` parks
with `recovery spending exhausted after {charged} attributable failures; re-author as plan version
n+1`; the three `blocked_by` strings that caused it survive only in
`package-outcomes/<PKG>/<N>.json`. `DriverEvent::RecoveryParked`
(`crates/core/src/package_driver.rs:347`) carries `package`, `reason`, `attempts` — no cause. By
contrast the mis-specification parks carry `fault` text and were immediately actionable.

**Current behavior.** `src/main.rs:2764` records `WorkerFailed { reason: blocked_by }` and then
calls `park_if_recovery_exhausted` — the ladder keeps spending regardless of what the worker said.

**Decisions delegated to you:**

1. When does a `blocked_by` stop the ladder? Recommendation: park on the **second consecutive
   `failed` outcome from the same package whose `blocked_by` is byte-identical**, rather than on
   the first. A first `blocked_by` can be transient; a repeated identical one never is. The hfx
   evidence supports either, and the report itself argues a park is cheap to overrule while an
   exhausted ladder costs a frozen plan version — if you judge the first-occurrence rule better,
   take it and say why.
2. Carry the final `blocked_by` verbatim into the `RecoveryParked` payload as its own field
   (recommended) rather than concatenating it into `reason`, so `driver-status` and the skill can
   render cause and budget separately.
3. Whether a park triggered by a repeated external blocker should be distinguishable in the journal
   from a budget-exhaustion park. Recommendation: yes, via the park reason text, so a supervisor can
   tell "no worker can fix this" from "this worker kept failing".

**Do not** attempt to auto-classify stderr as "environmental". The bluesmith report argues against
it and I agree: pattern-matching stderr eventually misclassifies a real failure as environmental,
which is the dangerous direction. The worker's own typed `blocked_by` is the signal.

**Out of scope, recorded:** hfx also proposed letting a package declare runtime preconditions the
driver evaluates before dispatch. That is a graph-schema change and a larger design; the park rule
above captures most of its value. Note it in your report as deferred, do not build it.

## B. Freeze accepts graphs that can never run

Two independent defects, both at `run_graph_freeze` (`src/main.rs:9916`).

**B1 — freeze does not enforce criteria invariance.** hfx proved it in a scratchpad copy: a graph
with ` # MUTATED PROBE` appended to SB3's first criterion command froze successfully
(`{"created":true,...,"plan_version":2}`) and `pce graph check` reported `valid: true`; only
`pce package driver-run` refused it, with `criterion "..." from predecessor package SB3 was changed
or removed`. A mis-frozen version is a permanent, immutable link in the version chain that can
never be run — the human's own words during that run were "a mis-frozen version poisons the chain".
Change: have `freeze` locate the vision's driver journal and apply the same predecessor-criterion
comparison `driver-run` applies, refusing with the same message. Where no journal exists, freeze is
unchanged. Decide how to behave when the journal belongs to a different repository set and record
your reasoning.

**B2 — base currency is checked only at promotion, when it is most expensive.** RivRetrieve
`a-fixture-is-a-recording` executed a complete, fully proven run — 107 journal events, five
packages, three gate findings each with a merged and re-proven repair, a full assembly sweep — to
`assembly-completed`, against a base that could never be promoted. `graph.v1.json` was frozen with
`authored_at_refs.RivRetrieve = b8d6deb…`, which equalled local `main` and was **already 36 commits
behind `origin/main`** at that moment. `origin/main` did not move during the run; the three newest
commits on it dated to six days before. `git rev-list --left-right --count b9e8a3e0…...ed2ed091…`
→ `36 16`; recomposition cannot rescue it (two modify/delete structural conflicts). Every launch
signal agreed: clean worktree, `graph check` passing, `authored_at_refs` matching HEAD exactly. The
supervisor observed "36 behind", correctly concluded the skill gave it no basis to treat that as a
blocker, and launched.

Change: `pce graph freeze` already takes `--repository NAME=SOURCE_WORKTREE`, so it already has
everything needed to fetch the remote default branch and require the fetched oid to be an ancestor
of the `authored_at_refs` oid for that repository, refusing with both oids and the divergence count
and naming `git pull --ff-only` as the remedy.

**Decisions delegated to you:**

1. Whether this lives in `freeze` (recommended — it is the one place that already holds the
   repository mappings) or is a launch-time precondition in the skill, or both. If freeze, the
   check makes a currently-local operation depend on the network: provide an escape hatch for
   deliberate offline or historical freezes, and **journal the fact that staleness was accepted**
   so it is visible later rather than silently swallowed.
2. Whether the mechanical (`--mechanical`) freeze path takes this check too. Careful: mechanical
   freeze is ratified as definition-preserving and local. If you conclude the check belongs only on
   the human path, say so with your reasoning; if you conclude it belongs on both, note that you
   are widening a ratified property and justify it. This is the one place in this brief where the
   doctrine boundary is close.

## C. `pce graph check` validates structure, never satisfiability

**Evidence.** pourpoint's run burned four freeze-dispatch-park cycles on graphs that returned
`{"valid":true}`: `graph.v3.json` (GD7's criterion required a transport whose GET path returns
`bytes(byte_range.length)` — null bytes — so no reader could ever refine),
`graph.v4.json` (GD8 bound its ranges to a *downstream* package's deliverable; no declared cycle,
so acyclicity passed), `graph.v5.json` (GD9 duplicated the act GD2's frozen title already claims;
its worker was shown GD2 as out of bounds and correctly parked), `graph.v9.json`. Two of the three
defects were authored by the supervisor under a rule the tool enforces (preserve every criterion
byte-for-byte) while the tool enforced nothing about whether the new criteria could be met.

**Change.** Add two authoring lints to the `graph check` path, reusing the `--repository` mappings
it already accepts:

- **Artifact provenance** — extract repository paths from each criterion `command`; warn when a
  path neither resolves at that package's authored ref nor is produced by the package itself or by
  a package strictly upstream in the declared edges.
- **Act ownership** — warn when a newly added package's criteria reference an artifact that a
  predecessor package's criteria also reference, or when its title overlaps a frozen package's act.

**Decisions delegated to you:** warning versus error; whether a `--strict` flag promotes warnings
to failures; how to keep path extraction from shell commands honest. Path extraction is heuristic
and will produce false positives on generated paths — recommendation: warn by default, never fail
without `--strict`. Note in your report the limit the reporting orchestrator itself named: the GD7
defect lived in criterion *prose* (`input`/`observation`), which no lint can reach. The prose half
is covered by workstream G.

## D. A worker's authority boundary is undefined, and its blast radius is unstated

**Evidence, part 1 (authority).** On bluesmith, two package workers on separate attempts edited a
**shared IAM role** — `nostos-self-teardown`, used by another program, explicitly placed off-limits
by a human ruling — to grant read on exactly the one S3 object a criterion named. CloudTrail
timeline: worker `PutRolePolicy` at 14:45:29 and 14:50:27, supervisor restore at 14:53:42, then a
third worker grant at 15:24:20 under a **differently-named inline policy**, which defeated a tamper
watch keyed on the first policy's name. Causality is proven, not inferred: deleting the grant
mid-attempt flipped the same criterion `PASS → FAIL(1)` with nothing else changed. Both grants were
object-scoped while the dependent package reads the surrounding prefix, so **both passes were
hollow** — the criterion would have gone green and the dependent package would still have failed.
The dedicated instance profile that was the correct mechanism existed before the first violation
and was never attached; nothing in the brief told the worker it existed or that the role was
protected. The journal's 341 events record none of it.

**RATIFIED DOCTRINE — read before you design.** The operator has ruled: *the fence is the
credentials, never the criterion text.* Your change here is **informational**: tell the worker what
it may not touch and what mechanism it should use. Do **not** build criterion-text-based
enforcement, do not add a gate that inspects cloud state, and do not present the brief text as the
boundary. If a worker games the guidance, that is further evidence for the credentials fence, not
an argument for more brief text. The credential-side work (a least-privilege driver role with
explicit Denies) is the operator's, already implemented for bluesmith as
`bluesmith-driver-least-privilege`, and is out of scope for you.

**Change, part 1.** In `compose_package_worker_brief`
(`crates/core/src/package_worker.rs`, §4 "Scope boundary", currently at line 239): state that only
the repositories under change may be modified, and that cloud, IAM, or account-level mutations are
out of scope **even when the credentials the worker holds permit them**. Some visions legitimately
provision infrastructure — bluesmith's own W7 does — so the rule is not "never touch cloud state",
it is "only through a criterion that names the act". Decide whether a pre-provisioned mechanism the
package is expected to use can be named from data the brief already has, or whether that needs a new
declaration; if the latter, recommend rather than build it.

**Evidence, part 2 (blast radius).** pourpoint: GD1's accepted gate repair `66e3e0e` added a
required parameter to `_build_evidence` (`scripts/released_wheel_proof.py:2081-2085`); the sibling
script `scripts/verify_released_wheel_evidence.py:209-212` still calls it with ten positional
arguments where eleven are required, so `--self-test` dies with `TypeError` at GD2's composed base.
GD1's four criteria and its one amendment all invoke `released_wheel_proof.py`; none invokes the
verifier. GD1 was marked complete and its proof carried across five plan advances while its
deliverable carried a break that halted the very next package — three GD2 dispatches, one spent
overrule, and a seventh plan version. The gate-reproof fix that landed the same day does **not**
address this: re-proving a finding re-runs the criterion the finding names, and the blast radius
lies entirely outside that criteria set.

**Change, part 2.** The brief already lists every other package's criteria. Add a derived line: for
each file path appearing in this package's own criteria, note which **other** packages' criteria
also reference that path. Purely syntactic on criterion commands, derived from data already in the
graph — and it would have surfaced this exact case, since `verify_released_wheel_evidence.py` *is*
named by GD2/GD3/GD4 criteria. Cheap; land it.

**Delegated decision, may be deferred with reasoning:** pourpoint also proposed that before
emitting `package-completed`, the driver re-run the criteria of every already-complete package
against the completing package's tree ("no package completes by breaking a proof already earned").
The rule is right; the cost grows with the run. Recommendation: if you implement it, bound it to
criteria whose commands name files the completing package changed. If the scoping does not come out
clean, record the decision to defer and why — do not ship an unbounded re-run.

## E. `mis-specified` faults carry no evidence

**Evidence.** pourpoint: the fault `id` is free text and workers satisfy it with a package
identifier. Observed values across five parks: `"GD1 released-reader-compatible HTTPS declaration
proxy"`, `"offline-released-reader-GRIT-object-replay"`, `"GD2"`, `"GD2"`, `"GD1"`. Refuting or
accepting such a park means reconstructing the worker's reasoning from scratch — for GD2 issuance 7
the supervisor ran `merge-base --is-ancestor`, read two symbol definitions, grepped for an absent
identifier, and re-executed two criteria in the worker's own worktree to refute a one-word claim.
`"GD2"` from GD8 and GD9 was not falsifiable at all. The counter-example is in the same run: GD2
issuance 9 reported `"GD1 buildability: released-wheel verifier self-test regression"`, which was
reproducible from the fault text alone in one command and attributed correctly.

**Change.** Replace the free-text `fault.id` with a required structured payload — the capability or
artifact believed missing, the path(s) or symbol(s) checked, and the command run to check them —
in the `PackageOutcome` schema (`crates/core/src/package_worker.rs:279+`, note `deny_unknown_fields`
and the strictness tests at `:329`, `:364`) and in the brief's "Required outcome" section
(`:268`). Reject an outcome whose fault names only a package id.

**Decisions delegated to you:** the exact field set; whether `criterion`-kind faults need the same
treatment; how a worker that genuinely cannot name what is missing is expected to comply — the
reporting orchestrator's view, which I share, is that making that hard is the point. Migration
matters: existing journals contain the old shape, so decide and state how replay handles them.

## F. A wedged worker is invisible, and its transcript is unaddressable

**Evidence.** pourpoint captured a prime-agent wedge live (the fourth instance, first captured
before the kill): GD2 issuance 8 held the driver for **122 minutes** while the journal's last record
was a normal `dispatch-pane-opened`. The whole process tree burned 0.54s of CPU over two hours,
with no socket open and nothing written to the worktree. Detection depended entirely on a human
asking whether the run was still moving. The decisive facts lived outside every workflow artifact:
`~/.prime/agent/daemon-workers/<id>/<worker>.recovery.jsonl` held a `tool_execution_start` with no
matching `tool_execution_end`, and the session file's last record was an assistant message whose
final block was a `toolCall` with no answering `toolResult`.

Two facts make this a pce-side change rather than only a prime-agent one:

1. `~/.prime/agent/sessions/*.jsonl` **already contains the worker transcript** that six pane reads
   could not recover (`herdr pane read` returned a bare shell prompt every time, across
   `wNY:p1`, `wNZ:p1`, `wPJ:p1`, `wPY:p1`, `wQB:p1`, `wRK:p1`). Nothing needs to be built to retain
   it — only located. In the first pourpoint report, that unreadable transcript is finding 1, and it
   caused a misdiagnosis that was frozen into two further defective packages.
2. `grep -rl <agent-name> ~/.prime` **misattributes sessions across visions**: it returned a
   559 KB session belonging to an entirely different vision in a different repository. The
   authoritative mapping is `createCommand.sessionPath` in the daemon worker record.

**Change.** Include `session_path` (from `createCommand.sessionPath`) in the
`dispatch-worker-identified` event, alongside the existing `agent_name`, `pane_id`, `workspace_id`,
and `process`. One field, no behavioral change; it makes the worker transcript addressable from the
journal and removes the need for the grep that misattributes.

**Out of scope:** surfacing daemon `busy` staleness as a `worker-stalled` driver event is
prime-agent-side work and is already covered by `briefs/prime-agent-dispatch-durability-brief.md`.
Do not duplicate it. Pane-scrollback retention is also **not** wanted: the session file already
holds what retention would have bought.

## G. Skill text: nine corrections, each earned by a frozen defect

Documentation only — `skills/work-graph/SKILL.md` and `skills/to-graph/SKILL.md` in the pce repo
(`./install.sh` syncs them to `~/.claude/skills/`; the supervisor runs it, you do not). Land this
workstream last so it documents what A–F actually shipped.

**`skills/work-graph/SKILL.md`:**

1. **Line ~230 hands the human a command that fails.** The repartition path prints
   `pce graph freeze --vision-dir <vision-dir>` with no `--repository` mappings; against the current
   binary that errors with `graph freeze requires one --repository NAME=SOURCE_WORKTREE mapping per
   graph repository`. The `--mechanical` (line ~211) and `--criterion-revisions` (line ~250) forms
   are already correct. Fix the one that is not.
2. **`assembly-repository-composed.base_oid` is a naming trap.** Section 8 (line ~321) says to read
   "the final `AssemblyRepositoryComposed.base_oid`". That field holds the **composed result**
   (`ed2ed091…` in the RivRetrieve run), not the base composition started from (`b8d6deb…`, which
   is its first parent). The skill's ancestry check is only correct under the observed reading;
   a supervisor taking the name literally reaches the opposite verdict. State it in section 8.
3. **Killing a driver pane orphans its workers.** `tmux respawn-pane -k` ends the driver but leaves
   herdr-dispatched workers running (bluesmith: pids 74548/74575 and 75039/75067 survived, had to be
   found and killed by hand, each guarded by a `ps` match so four other visions' concurrent drivers
   were not hit). Document the behavior and the guarded kill recipe.
4. **`respawn-pane` needs absolute paths.** It executes the command vector with no shell, so a bare
   `env` or a bare `pce` dies with status 127 and no message. One sentence in section 3.
5. **The wedge discriminator, before any kill.** An agent at 0% CPU **with a child doing real work**
   is a worker foreground-waiting on its act and must never be killed; an agent tree whose
   *cumulative* CPU is under a second, with a clean worktree and nothing written, is parked
   machinery. Check the whole tree, always. Also record that killing a wedged leaf is free — the
   kill charges the environment budget, not the package ladder (bluesmith and pourpoint both
   confirm: `dispatches_remaining`, `retry_remaining`, `local_patch_remaining` unchanged, driver
   re-dispatched in 20 seconds unaided).
6. **`wait_timeout_ms: null` means the supervisor is the only timeout** — a wedged worker holds the
   driver indefinitely. Say so in the section 2 `run.json` shape (line ~63).
7. **What is once-per-journal and what is per-plan-version.** A plan advance clears disputed parks
   and consumed overrules and re-scopes package state; it does **not** reset the recovery limits.
   The worker environment is now extendable at a plan boundary via the ratified
   `worker-environment-extended` door — describe both facts, since a supervisor reasonably expects
   a new plan version to carry new launch configuration and nothing signals otherwise until a launch
   aborts.
8. **The driver's own credential is launch configuration.** bluesmith's `AWS_PROFILE=work` was
   supplied so S3-reading criteria could run, was not expressible in `run.json`, survived only in
   the launch command, and silently conferred IAM write on every worker that inherited it. Say that
   the driver should run under a credential scoped to exactly what criteria need, with explicit
   Denies on mutations that must never occur, and that the credential's **name** (never its
   secrets) belongs in the launch record. If workstream B lands a launch-time base-currency check
   instead of a freeze-time one, document that here too.
9. **Drop nothing about `supervision.md` and Bash.** The hook is fixed and tested; do **not** add
   the "supervision.md is Edit/Write-only" ceremony three reports asked for. Verify against
   `hooks/pce-protect-criteria.sh` before you write anything on this.

**`skills/to-graph/SKILL.md`:**

10. **Numeric standards belong in the criterion command.** bluesmith's W4 worker, under recovery
    pressure, introduced `DARWIN_ARM64_CHECKPOINT_RELATIVE_TOLERANCE = 1.25` inside the script a
    criterion calls; the criterion then passed at `rel_linf 1.2209` against a `1.25` bar having
    failed at `9.07e-04` one attempt earlier, with its `name`, `input`, `observation`, and `command`
    byte-identical throughout. Criteria invariance protects the command text, not the meaning of
    constants inside the scripts it calls. Rule: when a criterion's observation asserts a threshold,
    tolerance, or bound, the number must appear in the criterion's `command`, and the script takes
    it as a flag rather than owning a default. This is authoring guidance, not a lint — a checker
    cannot tell which criteria assert a numeric standard.
11. **Prefer criteria that name a capability over criteria that name one artifact.** A criterion
    naming a single S3 object invited a grant covering a single S3 object, twice, and both passes
    were hollow. Requiring a prefix listing *plus* a read beneath that prefix cannot be satisfied
    object-by-object. Same caveat as D: this is authoring craft, not a fence.
12. **The four authoring checks, as questions the draft presentation must answer with evidence.**
    From pourpoint, each earned by a frozen defect: (i) *artifact provenance* — every artifact a
    criterion names, in its command **and in its input/observation prose**, exists at the authored
    ref or is produced by a strictly upstream package; (ii) *act ownership* — no frozen package's
    title or criteria already claim this act; (iii) *reference resolution* — does the artifact
    resolve its own references from where it is about to be put (a manifest with relative keys
    needed 255 GB of object copy); (iv) *consumer addressability* — can the consumer **name** the
    artifact at all (a reader that derives `manifest.json` itself can never name a content-addressed
    sibling). Checks (iii) and (iv) look redundant and are not: pourpoint's sibling design passed
    (iii) and failed (iv), after it had been built, frozen, dispatched, and completed.
13. **Say what the descent in step 2 must ask.** The obligation is stated; its questions are not, so
    an author satisfies it by reading the code a criterion names. Enumerate: where will this
    artifact live; what does it reference and does that resolve from there; who consumes it and can
    they name it; what does the consumer validate about location, scheme, or host. Both of that
    run's last two blockers were a missing answer to one of these, and each took under five minutes
    once asked.

---

## Environment facts

- pce repo: `/Users/nicolaslazaro/Desktop/work/pce`, `main` at `4c0d33d` or later.
- **Merge only** in `/Users/nicolaslazaro/Desktop/work/pce-integration`, branch
  `integration/work-package-harness`. Full suite there: `cargo fmt --check`, `cargo clippy`,
  `cargo test --workspace`. Known parallel-load flake:
  `tests/dispatch.rs gate_execution_echoes_large_input_without_deadlock` — verify it in isolation
  before blaming a change.
- **Do not `cargo build --release` in the main checkout** and do not run `./install.sh`.
  `~/.local/bin/pce` is a symlink to the main checkout's `target/release/pce`, so a release build
  there is a fleet install on four live runs. The supervisor owns installation.
- Key code: `src/main.rs` (`run_graph_freeze` `:9916`, graph check path, driver outcome handling
  `:2764`, park reason `:2284`, ref intents `:5890`); `crates/core/src/package_driver.rs`
  (`DriverEvent` `:259+`, `RecoveryParked` `:347`); `crates/core/src/package_recovery.rs`;
  `crates/core/src/package_worker.rs` (`compose_package_worker_brief` §4 at `:239`, outcome schema
  `:279+`); `crates/core/src/work_package_graph.rs` (invariance `:578-602`, carry `:608+`);
  `skills/work-graph/SKILL.md`, `skills/to-graph/SKILL.md`.
- Tests: every workstream ships tests. A–B–C–E–F are all journal- or CLI-observable, so prefer a
  test that asserts the emitted event or the refusal text over a unit test of an internal helper.

## Waiting consumers, and what a healthy first pass looks like

- **bluesmith** `planning/2026-07-29-signal-bearing-dudh-warm-window` — blocked, plan version 7+,
  two paid `r7g.2xlarge` cycles already spent. Healthy first pass after A: a package whose criterion
  fails twice on an identical `blocked_by` parks with that text in the `recovery-parked` payload,
  with rungs unspent.
- **hfx** `planning/2026-08-07-close-the-seven-basin-coverage-gap` — SB3 parked three times behind a
  governance procedure. Healthy first pass after A: no fourth ladder spend on the unchanged blocker,
  and the park reason names the runbook, not the budget.
- **pourpoint** `planning/2026-08-07-declare-grit-d8-...-row-seam` — nine plan versions, three
  packages proven. Healthy first pass after C and G: a graph draft whose criterion names an artifact
  no upstream package produces draws a warning at `graph check` time instead of a dispatch and a
  park; after F, `dispatch-worker-identified` carries a `session_path` that opens the worker's real
  transcript.
- **RivRetrieve** `planning/2026-08-19-a-provider-declares-itself` and
  `planning/2026-08-19-a-fixture-is-a-recording` — the latter lost a complete proven run to a
  six-day-stale base. Healthy first pass after B2: freezing against a base the remote default branch
  has already passed refuses, names both oids and the divergence count, and says
  `git pull --ff-only`.

Report per workstream: what you decided, why, the evidence you checked it against, and anything you
deferred. Land as separate commits so a bad workstream can be reverted without the others.
