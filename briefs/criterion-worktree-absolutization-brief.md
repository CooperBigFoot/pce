# Brief: a criterion cannot reach its own second repository when the driver was launched with relative paths

Status: READY TO DISPATCH — no grill. **This blocks a live run and will block two more packages in
it.** The decisions below are evidence-resolvable; examine the evidence, decide, implement, and
record what you decided and why in your completion report and `CONTEXT.md`. Boundary: if a decision
would change ratified doctrine (an ADR, a frozen criterion), stop and report. Written 2026-08-20.

## The defect

`shell_execution_at` (`src/main.rs:6681-6691`) runs each criterion with the **first** repository's
checkout as its working directory and exports the materialized paths verbatim:

```rust
child.arg("-c").arg(command)
     .current_dir(cwd)                                  // the first repository's checkout
     .env("PCE_WORKTREES", projection);
for (index, path) in paths.iter().enumerate() {
    child.env(format!("PCE_WORKTREE_{index}"), path);   // never absolutized
}
```

Nothing on that path canonicalizes or absolutizes. The values are whatever form the driver's own
launch produced. A criterion that does `cd "$PCE_WORKTREE_1"` therefore resolves a path **relative to
the working directory it was just given**, which is `<materialization>/00-repository`, and fails.
`PCE_WORKTREES` carries the same values, so the documented alternative fails identically.

## The trigger, established from two live runs

The form depends on how the driver was launched, which is invisible from inside the run:

- **pourpoint**, launched with `--graph planning/…` (relative), records
  `"working_directory":"planning/2026-08-07-declare-grit-d8-…/driver-materializations/46258-criteria-…/00-repository"`
  — relative. Its cross-repository criteria fail.
- **hfx**, launched with an absolute `--graph /Users/nicolaslazaro/Desktop/work/hfx/planning/…`,
  records
  `"working_directory":"/Users/nicolaslazaro/Desktop/work/hfx/planning/…/driver-materializations/98399-join-…/00-repository"`
  — absolute. Its criteria work.

So this is not a property of the graph, the package, or the criterion text. **The same frozen
criterion passes or fails depending on how a human typed the launch command**, and nothing in the
run proof explains why. That is the part worth fixing: a criterion's meaning must not depend on the
launch invocation's spelling.

Observed in pourpoint `planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`,
package GD4, which failed identically at issuances 39 and 40 — three criteria, same error each time.
The orchestrator checked every criterion in the frozen graph: **GD4 ×3 and GD6 ×2 reference
`$PCE_WORKTREE_1`**, so a second package is already known to be waiting behind the same wall.

## What to change

Absolutize the materialized repository paths before they become the working directory and before
they are exported, so `PCE_WORKTREE_<n>` and `PCE_WORKTREES` always name paths that resolve from any
working directory.

**Decisions delegated to you:**

1. **Where to do it.** Canonicalizing inside `shell_execution_at` is the narrowest fix and covers
   every caller. Absolutizing further upstream — where the driver materialization root is derived
   from the journal path — fixes the journal's `working_directory` records too, which is what made
   this diagnosable across two runs. Recommendation: upstream, so the recorded evidence is absolute
   as well; say what you chose and why.
2. **Canonicalize versus absolutize.** `canonicalize` resolves symlinks, which changes what a
   criterion sees when a path traverses one — and at least one vision deliberately uses a symlink to
   present a sibling repository. Absolutizing against the process's current directory without
   resolving symlinks is the more conservative choice. Decide, and state the consequence for a
   criterion that depends on a symlinked path.
3. **Whether the driver should refuse a relative launch instead.** Cheaper, but it makes a human's
   typing a launch precondition rather than removing the dependence. Recommendation: absolutize, and
   do not add the refusal.
4. **Whether existing journals need anything.** Records already written carry relative
   `working_directory` values. They stay as they are; decide whether replay or `driver-status` reads
   them in a way that a mixed corpus breaks, and say so.

## Tests

A criterion whose command is `cd "$PCE_WORKTREE_1" && pwd` succeeds when the driver is launched with
a **relative** journal and graph path, and the exported `PCE_WORKTREE_<n>` values are absolute in
both the relative-launch and absolute-launch cases. The existing multi-repository execution tests are
the right neighbours. This is a case where a test that only ever launches with absolute paths passes
while production breaks — the same shape as the herdr argv tests that passed through the 0.8.2
breakage — so make the relative launch explicit in the fixture.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, `main` at `49b7cb3`, pushed and clean.
- Merge only in `/Users/nicolaslazaro/Desktop/work/pce-integration`, branch
  `integration/work-package-harness` — and **edit there too**, not in the main checkout. Full suite
  there: `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`.
  Known parallel-load flakes in `tests/dispatch.rs`
  (`gate_execution_echoes_large_input_without_deadlock`,
  `gate_execution_drains_three_pipes_concurrently`) — verify either in isolation before blaming a
  change.
- Do **not** `cargo build --release` in the main checkout and do not run `./install.sh`; that symlink
  is a fleet install across five runs. The supervisor owns installation.
- Key code: `src/main.rs:6681-6691` (`shell_execution_at`), the caller immediately above it that
  selects the first materialized repository as `cwd`, and the `DriverMaterialization` root derivation
  around `src/main.rs:6460-6475` where the journal's parent becomes the materialization parent.
- Evidence: the two `working_directory` values quoted above, from the two runs' journals.

## The waiting consumer

pourpoint, plan version 16, fourteen packages complete. GD4 is parked after two identical failures
and GD6 carries the same two references. The orchestrator explicitly refused the alternative — a
criterion revision rewriting GD4's three commands and GD6's two — because that would bake a
workaround for a tooling defect into the frozen proof record permanently. It is right, and this
brief exists so it does not have to.

A healthy first pass: relaunch pourpoint's driver unchanged, and GD4's three frozen criteria pass
with no criterion text edited — which is also the cleanest evidence that they were never wrong.
