# Brief: a definition-preserving freeze is mechanical

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine
the evidence, decide, implement, and record what you decided and why in your completion
report and CONTEXT.md. Boundary: if a decision would change ratified doctrine (an ADR, a
frozen criterion), stop and report. Written 2026-08-18.

## The principle, and its limit

The human gate at freeze exists because freezing ratifies the definition of done — the
anchor of the promotion-without-review safety case (ADR 0019, ADR 0022 lineage: the ruling
is human, at freeze; everything after is mechanical because of it). That gate must stay for
any freeze that changes what "done" means.

But a class of freezes provably changes nothing about done: every package deep-equal to the
predecessor (ids, titles, repositories, criteria, depends_on — byte-identical), with only
`plan_version` incremented and `authored_at_refs` (or legacy `authored_at_ref`) changed.
Such a bump exists to reset plan-scoped recovery/gate budgets after unjust exhaustion, or to
pin repository refs. Today those freezes still interrupt the human — three times in two days
(gridded-statics v2, hfx v2 pending, bluesmith's would-have-been) — for a ruling that rules
on nothing. Each interruption is hours of blocked run when the human is away.

## What to build

**A `pce`-verified mechanical freeze.** The judgment "this freeze preserves the definition"
must be a computation in the binary, not agent prose — the same standard criteria invariance
already meets. Decide the mechanism; candidates:

- a `pce graph freeze --mechanical` (name yours to choose) that refuses unless every package
  in the draft is deep-equal to the highest frozen predecessor's, with only `plan_version`
  (must be predecessor+1) and the authored-ref field(s) differing — and, when refs changed,
  verifies each new ref resolves in its repository (the per-repo machinery from 2fd2512);
- or the same refusal folded into plain `freeze` with the flag only asserting intent.

Note the current freeze does NOT compare packages across versions at all (verified
2026-08-18: `run_graph_freeze` checks schema/vision/predecessor-version only; invariance is
enforced later, at the driver's plan-advance door, and only for criteria — packaging may
legitimately change in a human freeze). The mechanical path needs the stricter, whole-package
comparison — new code, not a reuse of `criteria_invariance_violation`.

**Skill authority.** `skills/work-graph/SKILL.md` currently forbids the supervisor from ever
running freeze (sections 6 and 11). Amend: the supervisor MAY run the mechanical freeze
itself when its draft qualifies, immediately appending the draft, the command, and the
binary's acceptance to supervision.md and reporting it — no prior approval, no notification
gate. Every other freeze remains the human's, exactly as written today. The skill's claim of
authority must cite the binary refusal (as it already does for criteria invariance): a prose
promise is not a substitute.

**Decide the edge cases and record them:**
- refs changing for a repository whose packages are all complete — the plan-advance door
  already decides carry-forward vs recomposition (ref-only revisions retain completions,
  assembly recomposes; landed in 2fd2512); confirm the mechanical freeze composes with that
  rather than restating it;
- legacy scalar `authored_at_ref` → per-repo map in the same bump: definition-preserving or
  not (recommend: yes, it is exactly the pinning case);
- a draft that also fixes a typo in a `reason` on an edge: reasons are justification prose on
  unchanged edges — decide whether deep-equality covers them (recommend: yes it does — byte
  identity keeps the proof trivial; a reason edit waits for a human freeze);
- interaction with `pce log --kind planning-artifact-approved`: does the mechanical path
  append the same approval event, attributed how?

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at 2fd2512 or later. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration`; full suite there; fast-forward main;
  `./install.sh` from the MAIN checkout only. Do NOT install — live drivers; the supervisor
  coordinates installs at quiet boundaries.
- Key code: `run_graph_freeze` (`src/main.rs`, ~:8798 by the latest read), the plan-advance
  door `ensure_driver_plan_version` and `criteria_invariance_violation`, per-repo ref
  verification from commit c741acf; `skills/work-graph/SKILL.md` sections 1 (authority
  preconditions), 6 (the freeze prohibition and the human-command handoff), 11 (boundary).
- Evidence of the interruption cost: gridded-statics v2 (pure bump, human woken for it),
  hfx v2 (authorized bump waiting on human), bluesmith supervision.md. Counter-evidence the
  gate must keep protecting: bluesmith W4's 1.25 tolerance (a semantic revision correctly
  stopped at the human) — cite both in CONTEXT.md so the boundary's why survives.
- Tests: freeze-path tests live near `run_graph_freeze`'s existing coverage;
  `tests/skill_dispatch_review.rs` asserts on skill text.
