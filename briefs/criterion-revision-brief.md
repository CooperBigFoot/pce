# Brief: a human may revise a frozen criterion; nothing else may

Status: READY TO DISPATCH — no grill. The doctrine below is already ruled by the human
(Nicolas, 2026-08-19, recorded here and in this brief's lineage); do not re-litigate it.
Every mechanism decision is evidence-resolvable: examine the evidence, decide, implement,
and record what you decided and why in your completion report and CONTEXT.md. Boundary: if
a mechanism choice would force a doctrine different from the ruling below, stop and report.
Written 2026-08-19.

## The ruled doctrine

Criteria invariance stays: no machine — worker, gate, supervisor, driver — may ever weaken,
edit, or remove a frozen criterion. That anchor of promotion-without-review is untouched.

But the current implementation locks the human out too, and lies about it. The plan-advance
door (`criteria_invariance_violation`, `crates/core/src/work_package_graph.rs:578`, call site
`src/main.rs:~4460`) requires every predecessor criterion to survive byte-identically
somewhere in the successor, package identity ignored — so no criterion can ever be edited OR
removed across sequential plan versions, by anyone. The refusal text says "freezing the
revised version is the human's ruling," and `skills/work-graph/SKILL.md` section 6
choreographs exactly that handoff (side-by-side texts, present the freeze command, stop) —
a door the binary then keeps shut.

The ruling: a criterion revision becomes possible through exactly one path — explicit human
ratification, recorded durably and attributed, with the affected packages' completions
forfeited and the revision surfaced verbatim all the way into the promotion PR. As durable
and auditable as a park-overrule rationale, and no easier.

## Evidence

- **gridded-statics WP3 (2026-08-19):** frozen criterion invoked
  `orthographos reissue seal --verify-only`; the delivered verb had no such flag. The human
  green-lit a corrective freeze dropping the flag; graph.v3.json was minted and the advance
  was refused anyway. The never-journaled dead file had to be retired by rename
  (`graph.v3.superseded-never-journaled.json` in the vision dir) — had the refusal come after
  journaling, the chain would be poisoned forever. Recovery was additive (WP4 delivers the
  flag), tolerable only because the missing surface was cheap and worth building. Full
  account: that vision dir's supervision.md, 2026-08-18/19 sections.
- **pourpoint GD7 (2026-08-19):** criterion frozen in v3 requires refinement from a transport
  that replays null bytes — unsatisfiable as written; the orchestrator's own authoring error,
  admitted in its report. Additive recovery (recorded-real-bytes corpus) exists but is heavy.
- **The unfixable case, so far only near-missed:** a criterion asserting a falsehood (wrong
  path, wrong constant) has no world that can conform. Today that kills the journal and every
  carried completion. Two independent authoring defects in two days says it will come.

## What to build (decide the mechanism yourself)

1. **Ratified revision at freeze.** A freeze whose draft changes or removes existing
   criterion bytes succeeds only when accompanied by an explicit human revision record —
   per revised criterion: the predecessor bytes, the successor bytes (or removal), and a
   human-authored rationale. Decide the capture mechanism (freeze flag(s), a sidecar file the
   human writes, an interactive refusal that names exactly what needs ratifying — your call),
   but the record must be durable, attributed to the human not an agent, and impossible to
   supply implicitly. A freeze with no criterion changes behaves exactly as today.
2. **The advance door honors it.** `ensure_driver_plan_version` accepts a criterion change
   only when the successor's freeze carries the matching revision record; the journal gains a
   typed event (criterion-revised or similar) so the fold and render see it. A revised or
   removed criterion forfeits the owning package's carried completion (revising bytes MUST
   drop it from `unchanged_package_ids` carry — verify this composes with the existing
   whole-package identity check rather than restating it). Amendments carried from a
   forfeited completion: decide their fate from the amendment-portability code and record why.
3. **Freeze-time guard, unconditionally.** `pce graph freeze` runs the invariance check
   against the highest frozen predecessor itself and refuses to mint an unadvanceable
   version (naming each offending criterion and the ratification path) — this kills the
   poisoned-chain trap even for humans who never use revision.
4. **Honest refusal text** at the advance door: name the real exits (additive
   world-conformance, or human-ratified revision at freeze) instead of the current
   implication that any freeze suffices.
5. **Promotion surfaces it.** The PR body generation (work-graph skill section 9, and any
   binary support it leans on) must quote every criterion revision verbatim — old bytes, new
   bytes, rationale — in a fence, like overrule rationales. A revision that cannot be
   correlated to its record is a refusal, not an omission.
6. **Skill texts.** `skills/work-graph/SKILL.md` section 6: the choreography (draft, check,
   side-by-side, present command, stop) is now truthful — keep it, but state that the freeze
   will demand the human's revision record and that the skill itself never supplies one.
   Never grant the supervisor revision authority; mechanical freezes (see
   briefs/mechanical-freeze-brief.md, possibly already landed — reconcile, don't collide)
   remain definition-preserving by definition. `skills/to-graph/SKILL.md`: authoring doctrine
   note that a frozen criterion is revisable only by human ratification with completion
   forfeiture — write commands you have executed, not commands you hope exist (both defects
   above were authoring-time).

**Decide the edge cases and record them:**
- removal vs edit: same record shape or distinct; whether a package whose criteria all vanish
  may itself be removed (recommend: yes, with the same ratification);
- multiplicity: the current matcher consumes successor occurrences one-for-one — keep that
  discipline for unrevised criteria;
- a revision record naming bytes that don't match the predecessor: refuse at freeze, exactly;
- interaction with `--mechanical` freeze if landed: a mechanical freeze with a revision
  record is a contradiction — refuse.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at 2fd2512 or later. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration`; full suite there; fast-forward main;
  `./install.sh` from the MAIN checkout only. Do NOT install — live drivers; the supervisor
  coordinates installs at fleet-quiet boundaries.
- Key code: `criteria_invariance_violation` and `unchanged_package_ids`
  (`crates/core/src/work_package_graph.rs:578` and below), `ensure_driver_plan_version` and
  the advance refusal (`src/main.rs:~4430-4480`), `run_graph_freeze` (`src/main.rs:~8798`),
  amendment carry from the amendment-portability change, journal fold
  `derive_driver_snapshot`, render.
- Live journals must keep deriving: incidence v1–v5 (finished, promoted), gridded-statics
  v1–v3 (running), pourpoint/bluesmith/hfx (various). Every existing advance event predates
  revision records — absence of a record on historical advances must parse as "no revision,"
  not an error.
- Tests: existing invariance and plan-advance coverage near the code above;
  `tests/skill_dispatch_review.rs` asserts on skill text. Add a test that a freeze of a
  criterion-editing draft WITHOUT a record refuses, WITH a record mints, and that the advance
  then carries everything except the revised package's completion.
