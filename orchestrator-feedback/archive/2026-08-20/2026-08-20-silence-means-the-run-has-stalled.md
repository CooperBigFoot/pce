# PCE workflow feedback: 2026-08-20-silence-means-the-run-has-stalled

- Date: `2026-08-20`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill, session 11f7b79a`
- Run: `/Users/nicolaslazaro/Desktop/work/palaestra/planning/2026-08-20-silence-means-the-run-has-stalled`, graph.v1 -> graph.v2, plan versions 1 and 2
- Outcome: `blocked` (driver-status `outcome: "blocked"`; H1, P1, P2 complete; P3 parked twice; N1, N2 pending; a plan-version-3 repartition drafted and awaiting the human freeze)

## Executive summary

The driver completed three of six packages unattended with gate-found repairs on each, then parked
the fourth twice on two genuinely different faults. Both parks were correct refusals of unbuildable
work, and neither consumed a recovery rung. The mechanical-freeze and plan-advance path worked
exactly as specified: a definition-preserving successor was accepted by the binary, carried three
completions and one amendment forward, and recomposed the parked package on the corrected base.

The highest-impact findings are not about the driver. Three concern the **launch environment**, and
they compound: the worker-environment set is journal-scoped and declared at first launch, the
supervisor's shell is not the driver's shell, and the skill's own recording rule contradicts its own
secret-handling rule when values are passed through argv. Together these produced a driver running
with six environment variables set to the empty string — a state worse than unset, because an empty
value satisfies a presence check.

A fourth finding is a near-miss in the skill's prescribed cleanup procedure: it mandates building a
descendant tree "by recursively calling `/usr/bin/pgrep -P`" without specifying how, and the obvious
shell implementation is wrong in a way that would have selected processes outside the tree.

A fifth is a gate gap with a two-package blast radius: P2 passed both its criteria while
monkeypatching the exact cross-repository boundary the vision exists to establish, so the
integration was never executed and the defect surfaced in P3.

## Evidence reviewed

- `planning/2026-08-20-silence-means-the-run-has-stalled/driver-journal.jsonl` (68 records, 35,423 bytes)
- `graph.v1.json` (6 packages, plan version 1), `graph.v2.json` (6 packages, plan version 2, sha256 `4f526fa4b7dc50371df7c06d10c72f2d5e68d76d5500e1145c356760ed10639e`), `graph.json` (7-package plan-version-3 draft)
- `events.jsonl` (2 records)
- `package-outcomes/P3/4.json`, `package-outcomes/P3/5.json`
- `.pce/package-briefs/P3/4.md` (18,963 bytes), `.pce/package-briefs/P3/5.md`
- `pce package driver-status --graph graph.v2.json --journal driver-journal.jsonl` (two invocations)
- `herdr pane read w14Q:p1 / w14R:p1 / w15F:p1 / w15G:p1`; `herdr workspace get`; `herdr worktree list`
- Git observations at palaestra `674a92a4`, `2e3c9b08`, `4262f38a`, `5327fe30`; hydria `0738efe1`, `4bfddda8`
- `supervision.md` (50,488 bytes, this run's captured evidence), `supervision-state.json`
- `evidence/P3-issuance-4/`, `evidence/P3-issuance-5/`, `evidence/P3-issuance-5-root-argv.txt`
- `/bin/ps`, `/usr/bin/pgrep` observations of worker root pids 93946 and 92718
- `nostos/RUNBOOK.md:59-82`; `hydria/src/hydria/serving.py:294,305,312,328-331,337,341`; `palaestra/pyproject.toml:99`; `palaestra/uv.lock:406`
- `camels-trust/data/trust-gridded-525/v3/train/basin=*/gridded_dynamic/*.zarr/chirps/zarr.json` (8 basins sampled of 368 present)

## What worked

### Mechanical freeze and plan advance

- Evidence: draft differed from `graph.v1.json` in exactly two hunks (`plan_version` 1->2,
  `authored_at_refs.palaestra` -> `2e3c9b08`), `packages` deep-equal `True`.
  `pce graph freeze --mechanical` returned
  `{"created":true,"mechanical":true,"plan_version":2,"sha256":"4f526fa4..."}`. The subsequent
  journal record is `plan-version-advanced {"from_plan_version":1,"to_plan_version":2,
  "carried_completions":["H1","P1","P2"],"carried_amendments":[["H1",{...}]]}`.
- Effect: a repository-contract fix was absorbed without re-running three completed packages and
  without a human freeze. This is the single most valuable mechanism exercised in the run.

### Parks did not consume recovery rungs

- Evidence: after both parks, `driver-status` reports P3
  `{"dispatches_remaining":2,"retry_remaining":1,"local_patch_remaining":1}` — identical to the
  never-dispatched N1 and N2.
- Effect: the supervisor could investigate without a clock running, and a correct refusal was not
  punished as a failure.

### Journal reconciliation of an outcome written while the driver was dead

- Evidence: the issuance-5 worker was orphaned when the driver was stopped; it exited on its own and
  wrote `package-outcomes/P3/5.json`. The journal still held 67 records ending at
  `dispatch-pane-opened`. On relaunch the driver journaled `package-parked` for P3 issuance 5 as
  record 68 and exited blocked (status 0) **without redispatching**.
- Effect: no duplicate dispatch raced for the outcome path, and the admissible proof was repaired
  from disk rather than lost. This behaviour is not documented in the skill and is worth stating
  explicitly, because the supervisor guessed at it.

### Gate-then-reproof on every completed package

- Evidence: 3 `gate-dispatched`, 3 `gate-finished`, 3 `gate-reproof-executed`, 1 `finding-replayed`,
  1 `package-repair-merged`. H1 carries amendment `gate:package-gate-1-1:finding:0` ->
  `uv run pytest tests/test_serving_index_staging_progress.py -q` with witness `28dbaf6c` and repair
  `3c74b0a5`, and that amendment survived the plan advance.

### Pane evidence survived both parks

- Evidence: all four `herdr pane read` calls returned content (693, 82, 613, 82 bytes). The
  worker's free-text summary in `w14Q:p1` and `w15F:p1` carried the reasoning that the typed outcome
  JSON does not have a field for.

## Friction and failures

### F1. The worker-environment set is declared at first launch, and there is no way to rehearse it

- Severity: `high`
- Phase: `launch configuration`
- Observation: the skill requires declaring every needed worker environment name at first launch,
  refuses to declare a name that is unset, and makes later additions cost a human-ratified
  `worker-environment-extended` record at a plan-version boundary. At first launch the six
  `NOSTOS_*` names required by `nostos/RUNBOOK.md:59-82` were unset in the operator's shell. The only
  available moves were to block indefinitely or to launch with an empty set and accept the
  ratification cost.
- Evidence: journal record 2 is `worker-environment-declared {"names":[]}`. `run.json` records
  `"environment": []`. The six names were verified unset three separate times before launch.
- Inference: the declaration is created as a side effect of the first `driver-run`, so there is no
  way to discover "which names are unset" without also committing the set. The supervisor cannot
  separate the check from the commitment.
- Impact: this run permanently carries an empty declared set. If N2's worker (as opposed to N2's
  criteria) turns out to need AWS names, recovering them costs a ratification. The operator reports
  another run lost a day to exactly this.

### F2. "Record the exact argv" and "never write an environment value" contradict each other

- Severity: `high`
- Phase: `launch / supervision record`
- Observation: section 3 requires recording "the exact shell-escaped argv" in `supervision.md`.
  Section 2 forbids writing any environment value to `supervision.md`. A launch that passes values
  through `/usr/bin/env NAME=VALUE ...` cannot satisfy both rules.
- Evidence: the first relaunch argv contained six `NAME="$VALUE"` interpolations. Recording it
  verbatim would have written six infrastructure identifiers into `supervision.md`; redacting it
  would have violated "exact".
- Inference: the skill assumes the driver inherits its environment rather than receiving it through
  argv, but never says so.
- Impact: forced an undocumented improvisation. The resolution adopted — invoke
  `/bin/zsh -c "... exec <abs-pce> package driver-run ..."` so `~/.zshenv` supplies the names — keeps
  argv value-free and satisfies both rules, but it was discovered rather than prescribed.

### F3. `tmux respawn-pane` inherits the tmux server environment, not the supervisor's shell

- Severity: `high`
- Phase: `launch`
- Observation: the driver was launched with all six `NOSTOS_*` names set to the **empty string**.
- Evidence: reading the driver process environment returned `NOSTOS_AWS_PROFILE= [len=]` and five
  identical results. Cause: the supervisor's Bash tool shell is non-interactive and does not source
  `~/.zshrc`, where the exports had been written, so every `$VAR` expanded to empty. A probe session
  confirmed the fix: with the block moved to `~/.zshenv`, `/bin/zsh -c` inside the tmux server
  reports `6 of 6 non-empty via zshenv`.
- Inference: two distinct traps compose here — the supervisor's shell is not a login shell, and the
  tmux server's environment is older than any edit made during the session.
- Impact: an empty value is worse than an unset one; it satisfies a presence check and fails deep
  inside a provider call. Had the chain reached N2 in that state, three live EC2 criteria would have
  failed with an obscure error attributed to the package rather than to the launch.

### F4. The prescribed descendant-tree enumeration has no safe implementation, and pids can be lower than the root

- Severity: `high`
- Phase: `recovery / cleanup`
- Observation: section 3 requires building the descendant tree "only by recursively calling
  `/usr/bin/pgrep -P <parent-pid>`" but specifies no implementation. The natural recursive shell
  function is wrong: in `sh`/`zsh` the loop variable is global, so recursion clobbers it.
- Evidence: the first enumeration returned
  `94838 77589 77590 77590 77590 97012 97012 97012 97012 97012` — duplicates, and pids numerically
  below the root 93946. Nothing was signalled from that output. A correct recursive walk in a
  separate interpreter returned the real tree
  `[94838, 77589, 77590, 63070, 15803, 97012, 95046, 94490, 94126, 94122]`.
- Inference: the low pids are genuine post-wraparound descendants, not foreign processes. But a
  supervisor reading the buggy list has two ways to go wrong: signal a duplicate/foreign pid, or
  dismiss a genuine descendant as foreign because its pid is smaller than the root's.
- Impact: near-miss only, because the tree was re-derived before any signal. Section 3 states this
  cleanup "must not reach any concurrent vision", and the buggy list is exactly how it would.

### F5. The skill never says to assess the worker before stopping the driver

- Severity: `medium`
- Phase: `recovery`
- Observation: to relaunch with a corrected environment, the supervisor sent `/bin/kill -TERM` to the
  driver first, reasoning that the driver should not observe its worker die and charge P3 a recovery
  rung. Only afterwards was the worker measured — and it was doing real work.
- Evidence: whole-tree cumulative CPU 31.98 s (a `find` at 14.96 s, an agent at 13.83 s), far above
  the one-second parked-machinery threshold. Driver pid 92718 gone, pane dead.
- Inference: the skill's liveness discriminator is written for "should I kill this worker", not for
  "should I stop the driver that owns it", so the ordering never comes up.
- Impact: an actively-working worker was orphaned for roughly ten minutes. It recovered — it exited
  normally and wrote its outcome, which the relaunch reconciled — but the outcome was luck, not
  design. Under `wait_timeout_ms: null` the same move against a longer job would strand it.

### F6. Both of P2's criteria simulate the cross-repository boundary, and the gates accepted it

- Severity: `high`
- Phase: `execution / gates`
- Observation: P2 spans palaestra and hydria and exists to establish the staging-callback contract
  between them. Both of its criteria monkeypatch that exact boundary.
- Evidence: `tests/test_staging_progress.py:28` —
  `monkeypatch.setattr(loader.hydria, "make_loader", fake_make_loader)` where `fake_make_loader` is a
  locally-defined function that declares `staging_hook`. Line 62, the `-k stale_hydria` criterion —
  `monkeypatch.setattr(loader.hydria, "make_loader", stale_make_loader)`, a local function omitting
  it, then `pytest.raises(TypeError)`. Neither test calls the real `hydria.make_loader`. P2 is
  `state: complete` with a gate finding and reproof.
- Inference: the gate evaluated the criteria as written and had no rule requiring a package that
  declares two repositories to execute across them.
- Impact: P2 proved that palaestra's *call site* passes `staging_hook`, and nothing about the hydria
  it resolves. The vision's criterion "A stale hydria fails immediately" passed while the property
  failed: the environment genuinely was a stale hydria (`pyproject.toml:99` pinned the absolute
  source checkout at `0738efe1`), and it produced no `TypeError` at loader construction — it produced
  a parked package two nodes downstream. Cost: one full park, one investigation, one mechanical
  freeze, one relaunch.

### F7. `run.json`'s mandated `herdr_session` field cannot be honoured by the installed binary, and pce accepts a session name herdr cannot bind

- Severity: `medium`
- Phase: `launch configuration`
- Observation: the skill specifies `herdr_session` in `run.json`, a `herdr --session <name> workspace
  list` preflight, and a `--herdr-session` flag on `driver-run`. The installed binary has no such
  flag.
- Evidence: `shasum -a 256 ~/.local/bin/pce` -> `1be14320f734f50d...`;
  `pce package driver-run 2>&1 | grep -c "herdr-session"` -> `0`. Named-session dispatch is on `main`
  at `8e4e611`, deliberately not installed. Separately, the skill's default name
  `pce-workers-<vision-slug>` yields a 111-byte socket path against macOS's 104-byte `sun_path`
  limit for this 43-character slug, while the 56-character name passes pce's own 64-character bound —
  `herdr` fails with `local socket name length exceeds capacity of sun_path of sockaddr_un` before
  reaching a server.
- Inference: the skill documents an unreleased feature as mandatory, and the two components disagree
  on the maximum session name length.
- Impact: the supervisor initially reported omitting `herdr_session` as a judgement call; the
  operator corrected that it was forced. Low practical cost this run (the default session was
  running), but the skill's text asserts a capability the environment does not have.

### F8. A park's durable explanation is one typed fault string with no free-text field

- Severity: `medium`
- Phase: `execution / escalation`
- Observation: `package-outcomes/P3/*.json` carries `outcome`, `fault.kind` and `fault.id`
  (`missing`, `checked`, `command`). The worker's reasoning, partial results and what it *did*
  accomplish exist only in the pane.
- Evidence: `package-outcomes/P3/5.json` records the missing capability but not that the worker had
  committed `a1e0afe` and `07851d0`, that 358 tests passed, or that the largest-basin measurement
  **succeeded at 3.66 s** — the one criterion of three that is satisfiable today. All of that came
  from `herdr pane read w15F:p1`.
- Inference: this corroborates the same finding in
  `2026-08-19-incidence-python-binding.md`; it is not specific to this run.
- Impact: the 3.66 s result is a real measurement produced by the run and is invisible to every
  durable artifact except the pane capture the supervisor happened to take. Had
  `dispatch-pane-cleanup` won the race, it would be gone.

### F9. The graph's dependency field is unnamed in the skill, and a wrong guess is silent

- Severity: `low`
- Phase: `orientation`
- Observation: the supervisor parsed `packages[].dependencies`, which does not exist in the schema,
  got `[]` for all six packages, and reported to the operator that the graph had no dependency edges
  and would run fully in parallel. The real field is `depends_on`, and the graph is a chain
  (`H1,P1 -> P2 -> P3 -> N1 -> N2`, with N2 also on P2).
- Evidence: `list(g['packages'][0].keys())` -> `['criteria','depends_on','id','repositories','title']`.
  The error was self-corrected only when `package-base-composed` for P3 showed a non-empty
  `dependencies` array in the *journal* record.
- Inference: the journal record and the graph schema use different key names for the same concept
  (`dependencies` in the event, `depends_on` in the graph), which actively rewards the wrong guess.
- Impact: one incorrect statement to the operator about run shape. Self-correcting here, but the
  same mistake would silently misinform any risk-ordering judgement.

## Recommendations

### R1. Add a launch preflight that reports unset worker-environment names without creating the declaration

- Addresses: F1
- Change: a `--check-worker-env` (or `--dry-run`) mode for `pce package driver-run` that resolves
  `--worker-env` names against the launch environment, prints which are unset, and exits **without
  writing `recovery-configured` or `worker-environment-declared`** to the journal.
- Location: `pce package driver-run`; `/work-graph` section 2 ("Capture launch configuration once")
- Trade-off: one more flag, and a supervisor could still skip the preflight.
- Confidence: `high`

### R2. Forbid environment values in the launch argv; require inheritance from a universally-sourced profile

- Addresses: F2, F3
- Change: state in section 3 that the driver **inherits** its environment and that the launch argv
  must contain no `NAME=VALUE` assignment; require the names to be exported from a file sourced by
  non-interactive shells (`~/.zshenv`, not `~/.zshrc`); note that `tmux respawn-pane` runs in the
  tmux server's environment, which predates any edit made during the session, so the launch must
  invoke an absolute shell (`/bin/zsh -c "... exec <abs-pce> ..."`) rather than relying on the
  supervisor's own shell. Add: verify by reading the launched process's environment for
  **non-empty** values, since an empty string satisfies a presence check.
- Location: `/work-graph` section 3 ("Launch and relaunch the foreground driver")
- Trade-off: couples the skill to a shell that sources a global rc file; a `bash`-only operator needs
  `~/.bash_profile` guidance instead.
- Confidence: `high`

### R3. Specify the descendant-tree enumeration, and warn that pids wrap

- Addresses: F4
- Change: replace "by recursively calling `/usr/bin/pgrep -P <parent-pid>`" with an explicit
  requirement that the walk run in a single program with local recursion state (not a shell
  function), that the resulting pid list be de-duplicated and each pid re-verified as reachable from
  the guarded root before signalling, and an explicit note that **a descendant's pid may be
  numerically lower than the root's after pid wraparound**, so numeric ordering must never be used to
  judge membership.
- Location: `/work-graph` section 3, cleanup paragraph
- Trade-off: longer procedure text in an already dense section.
- Confidence: `high`

### R4. Order the recovery rules: assess the worker before stopping the driver

- Addresses: F5
- Change: add to section 3 that before stopping or respawning the driver pane for any reason other
  than a driver that has already exited, the supervisor must apply the whole-tree liveness
  discriminator to every in-flight worker, and record the result. If any tree is doing real work,
  either wait for it or record explicitly why orphaning it is acceptable.
- Location: `/work-graph` section 3
- Trade-off: slows down an environment-repair relaunch, which is exactly when the supervisor feels
  urgency.
- Confidence: `high`

### R5. Require a multi-repository package to execute across its repositories in at least one criterion

- Addresses: F6
- Change: a gate rule — for any package declaring two or more `repositories`, at least one criterion
  must exercise the real cross-repository symbol, and a criterion that replaces that symbol
  (`monkeypatch.setattr`, `unittest.mock.patch`, or equivalent) does not satisfy it. Where the graph
  is authored (`/to-graph`), warn when every criterion of a multi-repository package names only one
  repository's test path.
- Location: gate rules; `/to-graph` authoring checks
- Trade-off: some multi-repository packages legitimately cannot integrate locally (a live-box
  package such as N2). The rule needs an explicit escape recorded in the package, not a silent one.
- Confidence: `medium`

### R6. Reconcile the skill's herdr-session text with the installed binary, and align the name bound

- Addresses: F7
- Change: mark `herdr_session` in section 2 as applying only when `driver-run` exposes
  `--herdr-session`, and instruct the supervisor to verify the flag before writing the field rather
  than reporting its absence as a decision. Separately, pce's 64-character session-name bound should
  be reduced to whatever keeps the derived socket path within `sun_path` (104 bytes on macOS), so pce
  cannot accept a name herdr cannot bind.
- Location: `/work-graph` section 2; pce session-name validation
- Trade-off: the length bound becomes platform-dependent.
- Confidence: `high`

### R7. Give the package outcome a free-text field for what the worker did accomplish

- Addresses: F8
- Change: add an optional `notes` (or `partial_results`) string to the package-outcome schema,
  written by the worker alongside `fault`, and surface it in the `package-parked` journal record.
- Location: package-outcome schema; `package-parked` payload
- Trade-off: a free-text field invites narration; cap its length and state that it records
  **measurements and commits made**, not reasoning.
- Confidence: `medium`

### R8. Name the dependency field in the skill, or unify it with the journal's

- Addresses: F9
- Change: either state `packages[].depends_on` explicitly in section 1, or rename the
  `package-base-composed` event's `dependencies` key to `depends_on` so the two agree.
- Location: `/work-graph` section 1; `package-base-composed` payload
- Trade-off: renaming a journal key is a schema change affecting existing journals.
- Confidence: `medium` (naming it in the skill: `high`)

## No-change decisions

- **`wait_timeout_ms: null` installs no driver timeout.** It did not bite this run — both parks
  produced clean blocked exits at status 0 — and the skill's whole-tree evidence rules are the right
  substitute for a blind timer. No change recommended.
- **Default recovery limits** (`retry_attempts: 1`, `local_patch_attempts: 1`,
  `environment_failures: 6`, `gate_failures: 3`) were never approached. No evidence either way.
- **The driver exits between plan versions, requiring a supervisor relaunch.** This looked like
  ceremony but proved useful: both exits were the moment the supervisor could inspect state and, in
  the second case, correct the environment. Keep.
- **The one-shot overrule per package** was never spent, and twice the correct answer was *not* to
  spend it. The constraint did its job by making the supervisor verify before refuting. Keep.

## Suggested follow-up

- **A vision-authoring check for criteria that presuppose capabilities no package builds.**
  `vision.md:100-102` asks to time "the largest per-basin grid window in isolation against the
  smallest", which presupposes per-basin windows of differing sizes; hydria's loader refuses them at
  `serving.py:328-331`, and no package in the frozen graph touched loader geometry. The freeze
  accepted this. The prior report
  (`2026-08-19-incidence-python-binding.md`) records a structurally identical finding: a package
  depending on work no criterion obliged its dependency to build. Two independent occurrences
  suggest a check worth designing, not a one-off.
- **Verify the 525 / 368 basin-count discrepancy.** The vision and package titles say 525 basins;
  `trust-gridded-525/v3/train` holds 368 basin directories. The remainder may be in other splits.
  Recorded as an observation only — not verified, and not a workflow finding.
