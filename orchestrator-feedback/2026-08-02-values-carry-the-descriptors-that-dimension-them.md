# PCE workflow feedback: values carry the descriptors that dimension them

- Date: `2026-08-02`
- Orchestrator: `Claude Code (Opus 5), session 8dc0cfbd`
- Run: `palaestra/planning/2026-07-31-values-carry-the-descriptors-that-dimension-them`
- Outcome: `paused` — milestones 1-6 merged; m7 executed and produced a finding but no verdict; m8 terminated

This is the second report for this vision. The first (`2026-07-31-...md`) covered
orientation through milestone 4. This one covers milestones 5-7, whose defining
feature was a pre-registered discovery gate.

## Executive summary

Milestones 5 and 6 merged and restored metis `main`, which had not compiled since
milestone 4 landed in orthographos. Milestone 7 built a pre-registration gate,
sealed it before any adjudication, executed it against real data, and discovered a
genuine metis defect — then could not classify that defect under its own frozen
predicate.

The highest-impact findings are about pre-registration as a mechanism:

1. Sealing a predicate also seals every incidental environmental fact captured
   alongside it, and those become irreversible constraints on later steps. One such
   fact (a working directory) would have deadlocked the milestone unrecoverably.
2. A two-arm predicate keyed on "a stage exited nonzero" cannot express "every stage
   succeeded and the output is wrong" — which is exactly what the gate found.
3. A test suite authored alongside the validator it tests proves only their mutual
   consistency. Four green review rounds and 39 passing tests did not detect a
   checker that could never have returned its own success outcome.

The gate's *integrity* property held completely: across four plan-critic rounds,
three PR-review rounds, and five execution attempts, no agent ran `hdx validate`,
`tethys data branch`, or any materialization before the predicate was sealed.

## Evidence reviewed

- `planning/2026-07-31-.../events.jsonl` — 240+ events across the run
- `milestone-7/step-1/plan.md` and `review-1.json`, `review-pr.json` (7 review rounds)
- `milestone-7/step-2/plan.md` and `review-1.json` (2 review rounds)
- metis commits `debcc1d`, `0073e24`, `ed84461`, `1e7bc85`, `07923a1`
- metis `crates/core/src/prepare.rs:734-743`, `split.rs:24-31`, `recipe.rs:126-127`
- hdx `a37277e` `crates/core/src/validate.rs`, `crates/core/src/error.rs`
- tethys-cli `12ba931` `src/tethys/commands/data/branch.py`, `metis.py`, `resolver.py`
- The minted slot at `camels-pur/data/m7-small-cohort/v1` and its manifests

## What worked

### Pre-registration sealed before the outcome was knowable

- Evidence: m7-s1 merged as `07923a1` with canonical digest
  `3c792746…` recorded in the commit body and surviving the squash. m7-s2's
  preflight proved `git merge-base --is-ancestor 07923a1 HEAD` exit 0 and
  recomputed the digest from committed bytes before the first domain command.
- Effect: when the gate later produced an uncomfortable result, there was no
  question whether the predicate had been shaped to fit it. That is the property
  the whole milestone exists to buy, and it survived intact.

### The no-outcome prohibition propagated to every agent, including critics

- Evidence: one plan critic abandoned a source investigation mid-way rather than
  resolve a stale hdx doc comment, because doing so would have revealed part of the
  outcome. The final PR reviewer argued its most critical finding deliberately on
  two type mismatches that hold for *every* report entry, specifically so the
  finding could not depend on the trunk's conformance.
- Effect: the discipline was not merely stated but internalized, including at the
  moment an agent had a good reason to break it.

### Adversarial rounds converged rather than churned

- Evidence: m7-s1 took 4 plan rounds and 3 PR rounds, closing 13 blocking findings.
  Findings narrowed monotonically: "judgment left to whoever knows the answer" →
  "frozen rules admit environmental faults" → "the map is incomplete" → "is this
  specific row correct".
- Effect: each round found the defect the previous fix introduced one level down.
  Expensive, but the sequence terminated.

### Executors refused rather than improvising

- Evidence: five clean refusals across the run. m7-s1's first attempt declined to
  substitute system Python for a missing project dependency. m7-s2's first attempt
  refused to resume after its own recorder halted it. m5-s2's executor halted on a
  cross-repository scope conflict rather than reaching into another repo.
- Effect: each refusal preserved a finding that an improvisation would have erased.

## Friction and failures

### Sealing a predicate also seals incidental environmental facts

- Severity: `high`
- Phase: `step planning` / `execution`
- Observation: m7-s1's `preregister.json` froze a per-stage `cwd`. For four of five
  stages that value was the *m7-s1 worktree* — where m7-s1 happened to run. `check`
  compares it exactly. m7-s2's plan stated only the Tethys stage's cwd and declared
  the m7-s2 worktree in its sandbox roots, never m7-s1's.
- Evidence: `milestone-7/step-2/review-1.json` finding `M7S2-R1-B1`; the five frozen
  cwd values in the committed `preregister.json`.
- Inference: the cwd is not part of the scientific predicate — it is an incidental
  fact about the authoring environment that got frozen alongside the cohort, the
  window, and the tick vector, and inherited the same immutability.
- Impact: an honest cwd record would have been rejected at adjudication — *after*
  the immutable slot was minted. The no-repair rule and the frozen
  `output_slot exists:false` clause then make a second run impossible. The milestone
  would have reached a state with no verdict and no possible future verdict, where
  the only exit available to an executor was writing down a cwd it did not use.
  Thirteen blocking findings had been closed on m7-s1 and none examined this value,
  because it was not a *claim* about anything.

### A two-arm predicate could not encode the defect it found

- Severity: `high`
- Phase: `execution`
- Observation: m7-s2's four domain stages all exited 0, but both emitted label
  directories contained all four basins where the recipe declared two each.
  `proved_conformant` is false; `found_non_conformant_and_chartered` is defined by a
  *first failing stage* and none exists.
- Evidence: the minted slot's `train/` and `test/` each containing four `basin=`
  directories; the emitted version manifest's `split.entries` recording both labels
  with the full cohort; all four stage receipts at exit 0.
- Inference: the pre-registration operationalized "non-conformant" as "a stage
  exited nonzero", which is strictly narrower than "the output violates the
  predicate". Content non-conformance under a clean exit falls between the arms.
- Impact: the gate discovered a real defect and could render no verdict about it.
  The executor correctly refused to force either arm — recording observed
  inventories yields `E_COHORT`, an artifact rejection rather than a domain outcome;
  substituting expected inventories would falsify evidence.

### A test suite authored alongside its validator proves only mutual consistency

- Severity: `high`
- Phase: `execution` / `review`
- Observation: m7-s1's committed checker validated HDX reports against a schema hdx
  does not emit — `checks` as an object rather than an array, `depth` as an integer
  rather than a string, `detail` as a string rather than nullable. Every entry of
  every real report violates the last two unconditionally, so `proved_conformant`
  was unreachable and m7-s2 would have exited 2 for a reason unrelated to the trunk.
- Evidence: `milestone-7/step-1/review-pr.json` finding `M7S1PR-B1`; hdx
  `a37277e:crates/core/src/validate.rs:425-465`; hdx's own conformance goldens.
- Inference: both fixtures were authored to the invented shape, so the tests and the
  implementation agreed with each other and nothing anchored either to the external
  system being modeled.
- Impact: four green plan-critic rounds, 39 passing tests including 37 named
  rejections, and an orchestrator verification that both fixtures drove exit 0 and
  exit 1 all passed on a checker that could never have returned its own success
  outcome. The eventual anchor was hdx's own golden reports.

### Making a dead clause load-bearing reveals its frozen value was always wrong

- Severity: `medium`
- Phase: `review`
- Observation: three times in one step, a correction that made an unread clause live
  immediately exposed that its value was incorrect — the `CoreError` map (incomplete
  by 13 of 28 variants), the frozen argv/cwd comparison (never compared), and the
  output-slot path (missing the `v` prefix Metis mints).
- Evidence: `review-pr.json` findings `M7S1PR-B3` and `M7S1PR-R2-B1`; the orchestrator's
  own enumeration diff of the `CoreError` enum against the plan.
- Inference: a frozen map presented as a table of rows looks complete at any length.
  Nothing makes a missing row visible.
- Impact: each instance cost a correction round. The remedy that worked was requiring
  the artifact to state a verified total and list every member exactly once, so
  completeness is checkable by counting rather than by inspection.

### A step's own recording code can impose constraints stricter than the seal

- Severity: `medium`
- Phase: `execution`
- Observation: m7-s2's first attempt halted `PLAN_INFEASIBLE` after the slot was
  minted, because its recorder required `split_labels` order `["train","test"]` while
  Tethys emitted `["test","train"]`.
- Evidence: the halted attempt's report; `check:53-61` showing `exact_keys` is
  set-based; the frozen checker never reads `split_labels` from Tethys stdout at all.
- Inference: the executor wrote a validation stricter than the predicate it was
  serving, and the one-shot latch then correctly refused to resume.
- Impact: required a human decision to authorize resumption from stage 3. Recoverable
  only because no domain command needed re-running and nothing sealed had changed.

### Orchestrator did not consult its own recorded hazard at dispatch time

- Severity: `low`
- Phase: `execution`
- Observation: m7-s1's first attempt failed because the sandbox omitted
  `~/.cache/uv`, breaking `uv run`. That exact hazard was already recorded in the
  orchestrator's persistent memory.
- Inference: a recorded hazard only helps if it is consulted at the moment of use.
  Dispatch construction is that moment and had no checklist.
- Impact: one wasted execution attempt.

## Recommendations

### Require a pre-registration to separate its predicate from its environment

- Addresses: "Sealing a predicate also seals incidental environmental facts"
- Change: split the frozen artifact into a `predicate` block (cohort, fields, window,
  outcome definitions) and an `environment` block (paths, cwd, binary locations).
  Seal both, but require the step that consumes them to re-verify every `environment`
  value against the environment where it will *run*, and permit an environment value
  to be corrected — with its own digest bump and a recorded reason — without
  reopening the predicate. Add an explicit gate at seal time: "for each environment
  value, state where it will be true when consumed."

### Require the outcome enum to cover clean-exit content failure

- Addresses: "A two-arm predicate could not encode the defect it found"
- Change: require any pre-registration whose failure arm is keyed on process exit
  status to also define an arm for "all stages succeeded and a predicate clause is
  violated". A gate that can only report failure when something crashes cannot report
  the most interesting class of defect.

### Require at least one fixture derived from the real external system

- Addresses: "A test suite authored alongside its validator"
- Change: when a step validates another system's output format, require the schema to
  be read from that system at a pinned ref, and require at least one fixture to be
  real captured output (or a committed golden from that system's own suite) rather
  than authored from the validator's expectations. State this as a done criterion, not
  a suggestion.

### Require frozen enumerations to be countable

- Addresses: "Making a dead clause load-bearing"
- Change: any frozen map over an external enum must state the verified total member
  count and list every member exactly once across its partitions, so completeness is
  verified by arithmetic. Add a reviewer instruction to extract the enum at the pinned
  ref and diff it against the artifact.

### Add a dispatch preflight checklist derived from recorded hazards

- Addresses: "Orchestrator did not consult its own recorded hazard"
- Change: before each `codex exec` dispatch, require the orchestrator to enumerate the
  repository contract's `environment_hazards` and its own persistent memory entries
  for that repository, and state which apply. The hazards were correct and recorded;
  they were simply not read at the point of use.

## Cost note

Milestone 7 step 1 consumed 4 plan-critic rounds and 3 PR-review rounds to produce
one commit of scripts and fixtures. That is high, and roughly half the findings were
introduced by the fix for a previous finding. It was nonetheless proportionate here:
the artifact is the adjudicator for an irreversible one-shot gate, and two of the
findings (unreachable success outcome, environment fault laundered into a domain
verdict blaming an external maintainer) would each have invalidated the milestone's
result while every mechanical check still passed.
