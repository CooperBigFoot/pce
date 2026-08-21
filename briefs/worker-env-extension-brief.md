# Brief: the worker-environment contract must extend at a plan boundary

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine
the evidence, decide, implement, and record what you decided and why in your completion
report and CONTEXT.md. Boundary: if a decision would change ratified doctrine (an ADR, a
frozen criterion), stop and report. Written 2026-08-19.

## The deadlock, one day after the feature shipped

Full evidence: `orchestrator-feedback/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam-4.md`
and the pourpoint vision dir. `worker-environment-declared` is checked in the pre-pass over
EVERY journal event (package_driver.rs:834-842, beside recovery-configured), so the
declaration is journal-scoped and immutable. Pourpoint's journal declared `{}` on 2026-08-18
— nothing then suggested otherwise; the need (POURPOINT_LIVE_READ_AUTHORIZATION,
POURPOINT_RELEASE_WHEEL, both named by released_wheel_proof.py:199-202) was discovered by
GD2's dispatch on 08-19. Relaunching with `--worker-env` now aborts:

    driver worker environment is already declared as {}, not {"POURPOINT_LIVE_READ_AUTHORIZATION", "POURPOINT_RELEASE_WHEEL"}

The structural fact, in the orchestrator's words: the environment must be declared before
the first dispatch, but which environment a package needs is only discoverable by
dispatching it. The only exit today is a fresh journal, which forfeits GD1's and GD10's
proofs (two accepted gate findings with witness/repair refs included) to add two NAMES.
Disproportionate; the orchestrator correctly held.

## What to build (decide the details yourself)

An extension door at the plan-version boundary, additions-only:

- at driver launch, a declared environment that is a strict SUPERSET of the journal's
  current declaration is accepted when (decide the exact gate) the launch advances or sits
  at a plan-version boundary — the same moment budgets reset and packaging may change;
  record a typed event (worker-environment-extended or similar) naming exactly the added
  names, so the proof shows from which point workers could see them;
- removals or value-form changes stay refused exactly as today — the contract only grows;
- decide whether extension requires a human-ratified record like criterion revisions, and
  record the reasoning either way. Recommendation to weigh: names are operational contract,
  not definition-of-done; additions at a boundary the human already ratifies (freezing the
  plan version that motivates them) with a typed journal event may be sufficient — but if
  you conclude the authorization-evidence role of the declaration (pourpoint uses it to
  scope a human authorization) demands an explicit record, build the small record instead
  and keep the shape parallel to --criterion-revisions;
- mid-plan extension stays refused; state it in the refusal text with the actual exit
  ("extend at the next plan-version boundary");
- `skills/work-graph/SKILL.md` section 2/3: document that worker_env (and recovery limits)
  are once-per-journal-until-extended, and that authoring should read the harness for named
  variables BEFORE first launch — the orchestrator's own cheapest-fix recommendation, filed
  because the requirement was discoverable in advance.

Guard tests: extension accepted at boundary with typed event; superset check exact;
mid-plan refusal message names the exit; old journals (no declaration event) keep deriving;
pourpoint's exact shape — journal with `{}` declared, relaunch at a plan boundary with two
added names — launches and forwards both.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at 9dca854 or later. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration`; full suite there; fast-forward main;
  push to origin after. Do NOT install and do NOT `cargo build --release` in the MAIN
  checkout (the installed binary symlinks to its target/release); build only in
  pce-integration. The supervisor coordinates installs.
- Key code: the declaration pre-pass (crates/core/src/package_driver.rs:834-842), the
  launch-time comparison emitting the abort above, `--worker-env` parsing and
  worker-environment-declared emission from f03f205, plan-advance door
  (`ensure_driver_plan_version`), route_environment (main.rs:~2749) which already scopes
  workers to declared names plus PATH/HOME/USER — unchanged.
- The waiting consumer: pourpoint at plan v6/v7 (GD1, GD10 proven; GD2 parked needing the
  two variables; run.json already carries worker_env and the human authorization scope
  note). After this lands and installs, its orchestrator relaunches at its plan boundary
  with both names — state in your report the exact expected first events.
