# Brief: remove the vendored hdx fork from orthographos

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine
the evidence, decide, implement, and record what you decided and why in your completion
report. This is ordinary repository work in orthographos — no pce run is active there and no
journal is involved; land it as a normal reviewed PR. Written 2026-08-19.

## What happened

During the gridded-statics pce run, a local-patch worker (orthographos commit 27da84f "Make
Orthographos driver checks self-contained", 2026-08-19 17:10) copied the entire hdx core
crate into `vendor/hdx-core/` (~30k lines, ~380 files) so the cargo workspace would build
without the `../hdx` sibling checkout its prepare step normally links. The behavioral work in
that lineage is correct and proven (the run finished and promoted; orthographos PR #73,
merge 3fadf560). The vendoring is scope creep that no check measured: orthographos main now
carries a divergent snapshot-fork of a live external repo
(`/Users/nicolaslazaro/Desktop/work/hdx`), guaranteed to drift.

## What to do

1. In `/Users/nicolaslazaro/Desktop/work/orthographos` (main at 3fadf560 or later), remove
   `vendor/hdx-core/` entirely and restore the dependency exactly as it was expressed at
   pre-run main (013a69b): inspect `git show 013a69b:Cargo.toml` (and workspace members) for
   the prior hdx reference — the repo's convention is the sibling checkout via the
   `ln -sfn …/hdx ../hdx` prepare step. Restore that shape; do not invent a new dependency
   mechanism (no git dependency, no crates.io) unless the pre-run files already used one.
2. Check for any OTHER residue of the same commit that exists only to serve the vendoring
   (workspace member entries, path rewrites, feature flags); `git show 27da84f --stat` is
   the map. Keep everything behavioral: the reissue verbs, tests, and the verify-only seal
   work stay untouched.
3. Prove behavior unchanged: with the sibling `../hdx` link present, run
   `cargo test --workspace` green, and re-run by hand the orthographos-side assembly
   criteria commands from the promoted run (they are listed verbatim in PR #73's body —
   https://github.com/CooperBigFoot/orthographos/pull/73 — the seal/attest/verify-only
   commands against the external evidence roots; all read-only). Every one must exit 0
   against the de-vendored tree. Paste the commands and exit statuses into the PR body.
4. Open a normal PR to main titled "Remove vendored hdx fork; restore sibling dependency",
   body: what 27da84f did, why (the pce driver's unprepared join bench, since fixed in pce
   c151487), the re-proof results, and a link to PR #73. Merge it (merge commit) after any
   required checks.

## Constraints

- Do NOT touch `/Users/nicolaslazaro/Desktop/work/hdx` itself, the palaestra repo, the pce
  vision dir, or any pce journal.
- Do NOT force-push or rewrite any pce/* branch or ref — the promoted history stays as it is.
- The external evidence roots the re-proof commands read
  (camels-four-quadrant-v2/v3, trust-gridded-525, et20-reissue-evidence) are read-only to
  you; the seal/attest verbs are read-only by design, which is why re-running them is safe.
