# Brief: the driver blames a conflicted join for a criterion that was already failing

Status: READY TO DISPATCH — no grill. **This spent a package's entire ladder on a defect that did not
exist.** The decisions below are evidence-resolvable; examine the evidence, decide, implement, and
record what you decided and why in your completion report and `CONTEXT.md`. Boundary: if a decision
would change ratified doctrine (an ADR, a frozen criterion), stop and report. Written 2026-08-20.

## The defect

When a package's composition requires a conflicted join, the driver re-runs each parent package's
effective criteria against the joined tree (`src/main.rs:2714-2747`). Any failure produces:

```rust
DriverEvent::PackageFailed {
    package: package_id.to_owned(),
    reason: format!("conflicted join broke parent criteria: {}", failed.join(", ")),
}
```

Nothing establishes that the criterion **passed before the join**. A parent criterion that is already
red at the parent's own ref fails again in the joined tree, and the driver reports the join as the
cause. The claim is asserted, never tested.

## The evidence

hfx `planning/2026-08-07-close-the-seven-basin-coverage-gap`. Package SB9 burned issuances **26, 27
and 28** on the identical assertion and then parked with `recovery spending exhausted after 3
attributable failures; re-author as plan version n+1`. Each of the three workers received a brief
naming its own merge as the culprit.

The supervising orchestrator falsified the claim directly: it ran SB7's criterion at **SB7's own
untouched ref, with no SB9 work present**, and got the identical failure and the identical message.
The join broke nothing. The criterion was already failing against the current evidence root, for an
unrelated reason (a check script pinned to a previous campaign's recorded numbers).

Cost: three dispatches, a full recovery ladder, a park, and a plan version — all spent hunting a
defect that did not exist, with each worker actively misdirected by the brief it was handed.

## What to change

Before attributing a parent-criterion failure to the join, **run that criterion at the parent's own
ref**. Attribute to the join only when it passes there and fails in the joined tree. When it fails in
both, the join is innocent: report a different, truthful reason naming the parent criterion as
already failing, and decide whether that is the joining package's failure at all.

**Decisions delegated to you:**

1. **What the truthful outcome is when the parent criterion is already red.** The joining package did
   nothing wrong, so charging its ladder is the same mis-attribution in a quieter form.
   Recommendation: emit a distinct typed event (a parent-criterion-already-failing shape) and park
   without charging the joining package — the fix belongs to the parent, and a human or the parent's
   own ladder should own it. Decide and justify; this is the substantive half of the brief.
2. **Whether the pre-join execution is always run or only on failure.** Running it only when the
   joined execution fails is strictly cheaper and sufficient to attribute correctly. Recommendation:
   only on failure.
3. **What the worker brief says.** Three workers were told their merge broke a parent criterion. The
   brief text for a conflicted join should not name a cause the driver has not established. Fix the
   wording alongside the attribution so a worker is not sent after a phantom.
4. **Whether `effective_criteria` at the parent's ref needs its own materialization.** It does — the
   parent's tree, not the joined one. Reuse the existing materialization machinery rather than
   inventing a second path, and say what you reused.

## Tests

The decisive test is the falsification the orchestrator performed by hand: a parent criterion that
fails at the parent's own ref, and a joining package whose merge is clean, must **not** produce
`conflicted join broke parent criteria`, and must not charge the joining package's recovery. Pair it
with the true-positive case — a criterion green at the parent's ref and red after the join — which
must still attribute to the join exactly as today. Either test alone passes trivially; the pair is
what makes them meaningful.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, `main` at `6f56851`, pushed and clean.
- Merge only in `/Users/nicolaslazaro/Desktop/work/pce-integration`, branch
  `integration/work-package-harness` — and **edit there too**, not in the main checkout. Full suite
  there: `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`.
  Known parallel-load flakes in `tests/dispatch.rs`
  (`gate_execution_echoes_large_input_without_deadlock`,
  `gate_execution_drains_three_pipes_concurrently`) — verify in isolation before blaming a change.
- Do **not** `cargo build --release` in the main checkout and do not run `./install.sh`; that symlink
  is a fleet install across five runs. The supervisor owns installation.
- Installed binary digest `d4b8a13b…`; installed herdr is 0.8.2 and pce enforces `>=0.8.2,<0.9.0`.
- Key code: `src/main.rs:2690-2747` (the join re-proof loop, `JoinCriterionExecuted`, and the
  `PackageFailed` attribution), `effective_criteria` and `execute_effective_criterion` in
  `crates/core/src/package_driver.rs`, and the conflicted-join section of
  `compose_package_worker_brief` in `crates/core/src/package_worker.rs`.
- Evidence: hfx's journal — SB9 issuances 26, 27, 28, each `PackageFailed` with the same reason, then
  `recovery-parked`.

## The waiting consumer

hfx, plan version 10, blocked. SB9 is parked with its ladder exhausted and a plan version 11 pending
that will give a package authority to reconcile the check script with the criterion it implements —
the *real* blocker. This brief exists so the next conflicted join in that run, or any other, does not
spend three more dispatches on a cause nobody verified.

A healthy first pass: a joining package whose merge is clean, against a parent whose criterion is
already failing, parks with a reason that names the parent criterion as the cause and leaves the
joining package's ladder intact.
