# `pce contract bootstrap` is Rust-only on both derivation paths

**Reported by**: an orchestrator running PCE in a Python/uv repository, 2026-08-01.
**Measured against**: `main` at `07b85ccdff934052badba3ae45463015e5fa2518`.
**Status**: confirmed, wider than reported. Out of scope for the vision in flight
(`2026-07-31-the-binary-owns-every-dispatch`); recorded here for a future effort.

## What was reported

> Real situation: Python/uv repo, no CI workflows. `pce contract bootstrap` only handles CI-less
> Rust repos, so the automated path is unavailable.

## What is actually true

The report understates the limitation. `bootstrap` has two derivation branches and **both** are
hardcoded to cargo, so the automated path is unavailable to a Python repository whether or not it
has CI.

**CI-less branch** — `src/main.rs:1581-1584`:

```rust
let derivation = if workflow_paths.is_empty() {
    if !path_exists_at_default_branch_head(repository_root, branch, "Cargo.toml")? {
        bail!("cannot bootstrap CI-less repository without Cargo.toml at default-branch HEAD");
    }
```

This is the branch the reporting orchestrator hit, and its diagnostic names the real cause. Below
it, the five candidate lists it selects from are cargo-only (`src/main.rs:63-83`): every entry in
`FORMAT_BOOTSTRAP_CANDIDATES`, `LINT_BOOTSTRAP_CANDIDATES`, `TYPECHECK_BOOTSTRAP_CANDIDATES`,
`TEST_BOOTSTRAP_CANDIDATES`, and `BUILD_BOOTSTRAP_CANDIDATES` begins with `cargo`. So even if the
`Cargo.toml` guard were relaxed, this branch could not derive a `uv`/`ruff`/`pytest` gate set.

**CI-derived branch** — `src/main.rs:1621-1638`. The `first` closure that extracts each gate from
workflow `run:` scripts matches on the literal first two words:

```rust
words.next() == Some("cargo") && words.next() == Some(prefix)
```

A Python repository *with* GitHub Actions workflows therefore fails too, with
`CI-derived bootstrap found no format gate command in workflows: <names>` — a diagnostic that
describes the symptom and not the cause. That is the worse of the two failures: the CI-less
diagnostic tells you cargo is required, this one reads as "your workflows are missing a gate."

## Why this is out of scope for the current vision

`2026-07-31-the-binary-owns-every-dispatch` makes `pce dispatch` a verb that builds a shell-free
envelope and spawns children directly. Its nine acceptance criteria concern dispatch envelope
construction, stdin binding, `ANTHROPIC_API_KEY` stripping, lifecycle logging, verdict artifact
validation, sandboxed gate measurement, and the `SKILL.md` cutover. Gate derivation for a
non-cargo stack shares no surface with any of them, and folding it in would widen a milestone graph
that is already approved and six-eighths merged.

## What a future effort would have to decide

Three questions, in the order they bind:

1. **Where does stack detection live?** `derive_bootstrap_contract` currently conflates "which stack
   is this" with "which command in the candidate list passes". A Python repo needs the first
   question answered from `pyproject.toml`, `uv.lock`, `poetry.lock`, or `setup.py` before any
   candidate list is meaningful.
2. **Is `typecheck` universal?** The tracked contract's `stated` block has exactly five gates and
   `deny_unknown_fields`. A Rust repo always has `cargo check`; a Python repo may have `mypy`,
   `pyright`, or nothing. Whether a stack may decline a gate — and how that is spelled, given the
   field is required — is a schema question, not a detection question, and it is the one that
   determines whether this is a small change or a payload-boundary change.
3. **Does the CI-derived path generalize at all?** Matching `cargo <verb>` as two literal words does
   not extend to `uv run pytest`, `uv run ruff check`, or a `Makefile` indirection. A general
   version needs the stack to supply its own recognizer, which is the same dispatch-on-stack the
   CI-less path needs. The two branches should share it rather than each grow a match arm.

Until that exists, the workaround for a non-cargo repository is the path `SKILL.md` already
specifies for a repository whose tracked contract is present: hand-author
`.pce/repository-contract.json`, then let Phase 0 run `pce contract check` against it and append the
measured record. That path is stack-agnostic — only *derivation* is cargo-bound, not measurement or
enforcement. The orchestrator who filed this had already begun doing exactly that
("Gathering what the tracked contract must state").
