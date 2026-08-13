# `escalation-close` cannot lift an exhausted defect cap, so the documented escalation lifecycle is not executable

**Vision:** `2026-08-11-the-store-is-the-only-copy`, node `m1-s2`
**Date:** 2026-08-12
**Severity:** blocking — a human-authorised resolution cannot be enacted; the run cannot proceed by any sanctioned route

## The lifecycle SKILL.md documents

> Cap exhaustion, `BLOCK`, or vision cause appends `escalation-open` and stops. Resolution appends
> `escalation-close`. Never proceed on an unconverged plan.

That reads as a complete cycle: the run stops, a human decides, the resolution is recorded, and the
run continues under it. The last step does not work.

## What happens

`(m1-s2, step-executor)` reached 3 validated-production defect rounds. The binary refused admission,
correctly:

```
Error: dispatch admission refused: defect-round cap 3 is exhausted for node m1-s2 and role step-executor
```

I appended `escalation-open`, stopped, and put the decision to the operator. The operator authorised
exactly one further dispatch, scoped to the specific repair. I appended `escalation-close` with the
full resolution text. `pce status` confirms the hold is recorded and closed:

```json
{"key":"m1-s2-executor-cap-exhausted-block-a",
 "status":{"node":"m1-s2","sequence":115,"state":"closed","resolution":"Human authorised option (a) …"}}
```

The identical dispatch is still refused, byte for byte:

```
Error: dispatch admission refused: defect-round cap 3 is exhausted for node m1-s2 and role step-executor
```

Admission never consults the escalation. Defect rounds are derived from immutable completion facts in
the log, so the count cannot fall, and nothing an orchestrator may legitimately append changes it.

## There is no documented escape

`pce dispatch codex --help` exposes no `--force`, `--override`, `--admit`, or cap-related flag. The
top-level usage lists no admission verb. Grepping the usage output for `cap|round|admission|override|force`
returns nothing. So after a cap is exhausted, the only remaining routes are:

1. **Abuse a different role.** The cap is keyed `(node, role)`, so dispatching `step-plan-writer` or
   `pr-reviewer` to edit product code would be admitted. This is exactly the role-abuse the anchored
   registry exists to prevent, and it would corrupt every round series and artifact-naming derivation
   that keys on role.
2. **Re-cut the node.** Author a delta stub with a fresh canonical id to obtain a new `(node, role)`
   series. Legitimate under runtime graph adaptation, but it is a decomposition change forced by an
   accounting limit rather than by anything about the work, and it fragments one step's single-commit
   history across two nodes and two PRs.
3. **Apply the fix by hand**, outside the orchestrator, which defeats the point of the run.

None of these is what the operator authorised. The operator authorised *one more executor dispatch*,
which is precisely the thing the tool cannot express.

## Why this bit here, and why the accounting is arguably unfair

The three charged rounds for `(m1-s2, step-executor)` were:

| Round | What happened | Whose defect |
|---|---|---|
| 1 | first execution; PR review found 6 issues | executor / plan |
| 2 | `PLAN_INFEASIBLE` — the orchestrator rebased the worktree onto the merged milestone branch, invalidating baseline-relative criteria | **orchestrator** |
| 3 | PR review found BLOCK-A | executor |

Round 2 is the sharp one. The executor detected a contradiction between the plan and its actual branch
point, refused to work around it, and returned a structured `BLOCK` with `root_cause=step_plan` —
**the exactly correct behaviour**, and the behaviour the skill spends several paragraphs demanding.
It was charged a defect round anyway, because rounds key on `(node, role)` and ignore `root_cause`.

So the accounting charges the executor for:

- a defect the orchestrator introduced, and
- correctly *reporting* that defect rather than papering over it.

A dispatch that returns `root_cause=step_plan` or `root_cause=milestone_plan` is not evidence that the
executor is failing to converge. It is evidence that something upstream is wrong, and the upstream
actor has its own cap. Charging it twice — once to the planner when it re-plans, once to the executor
for noticing — makes the executor cap reachable by upstream churn alone.

## Suggested changes, cheapest first

1. **Make `escalation-close` admit exactly one further dispatch for the `(node, role)` it closes.**
   This is what the documented lifecycle already promises. A closed escalation whose `key` names the
   exhausted `(node, role)` should grant one admission, consumed on use, so the cycle is
   escalate → resolve → one authorised attempt → re-evaluate. Anything more permissive re-opens the
   runaway loop the cap exists to stop.

2. **Do not charge a defect round when `root_cause` is not this role's own.** An executor result with
   `root_cause=step_plan` or `milestone_plan` should charge the plan-owning role's series, or no
   series at all, but not the executor's. The executor did its job by refusing.

3. **Name the escape in the refusal message.** `defect-round cap 3 is exhausted …` should say what an
   operator may do about it — for example `… ; append escalation-close for key <k> to authorise one
   further attempt, or delegate a delta node`. Today the message is a dead end.

4. **If lifting the cap is deliberately not supported, say so in SKILL.md.** The current wording
   ("Resolution appends `escalation-close`") strongly implies the run resumes. If the real contract is
   "a cap exhaustion permanently closes that `(node, role)` and the only remedy is a delta node", that
   should be stated, and the delta-node route should be spelled out as the sanctioned recovery rather
   than left to be inferred.

## Current state of the blocked run

The artifact at `174a921` (PR #150) passes all five gates, passed pre-PR falsification with zero
blocking issues, and carries exactly one commit and the 43 authorised paths. One major PR finding
remains — the conformance suite compares the whole Parquet frame schema for exact equality, so it
rejects any store whose provider-native columns differ from the fixtures', contradicting both the
committed normative document and the approved plan's own inspector contract, which says "Permit
additional columns". The repair is four narrow changes to a comparison, with a tested reproduction
supplied in the verdict. It cannot be dispatched.

## Related

- `2026-08-11-the-mandated-three-env-entries-strand-tmpdir.md` — same run, sandbox/TMPDIR conflict.
- `2026-08-05-dispatch-spawns-with-an-empty-environment.md`
