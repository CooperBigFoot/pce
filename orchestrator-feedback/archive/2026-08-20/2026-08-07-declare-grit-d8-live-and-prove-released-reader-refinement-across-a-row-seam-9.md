# PCE workflow feedback: `$PCE_WORKTREE_N` is unusable from a criterion command

- Date: `2026-08-20`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan version 18
- Outcome: `blocked` — GD4 failed identically at issuances 39 and 40; ladder now on its last rung
- Severity: `high` — no multi-repository package in this graph can pass a criterion that reaches its
  second repository, and the graph has two such packages

## Executive summary

GD4 is the first two-repository package this run has ever reached. Three of its five criteria do
`cd "$PCE_WORKTREE_1" && …`. All three fail identically, at both issuances:

```
sh: line 0: cd: planning/2026-08-07-declare-…-row-seam/driver-materializations/
             46258-criteria-1787246008574232000/01-repository: No such file or directory
exit 1
```

The directory exists. `environment-preparation-executed` ran successfully *inside it* moments before.
The failure is that `$PCE_WORKTREE_1` holds a path relative to the journal's base, while the criterion
child runs with `current_dir` set to the **first** repository's checkout — so the relative path
resolves against the wrong base and cannot be found.

This is not a defect in the package, the graph, or the criterion text. It is the documented
`$PCE_WORKTREE_N` contract being unusable from the place criteria actually execute.

## Evidence reviewed

From the journal, same materialization, same run:

```
environment-preparation-executed  hfx        wd: …/46258-criteria-…/00-repository   succeeded
environment-preparation-executed  pourpoint  wd: …/46258-criteria-…/01-repository   succeeded

criterion-executed  exit 0  Exact in-place declaration      wd: …/00-repository
criterion-executed  exit 1  Released reader floor observed  wd: …/00-repository
criterion-executed  exit 1  Distant-region reach            wd: …/00-repository
criterion-executed  exit 1  Planetary reads remain bounded  wd: …/00-repository
criterion-executed  exit 0  Failed fire is contained        wd: …/00-repository
```

Every criterion runs in `00-repository`. Both repositories were materialized. The two criteria that
never leave the first repository pass; all three that `cd "$PCE_WORKTREE_1"` fail.

From the source, `src/main.rs:6681-6691`:

```rust
fn shell_execution_at(command: &str, cwd: &Path, paths: &[PathBuf]) -> Result<CriterionExecution> {
    let projection = serde_json::to_string(paths)?;
    let mut child = std::process::Command::new("/bin/sh");
    child.arg("-c").arg(command)
         .current_dir(cwd)
         .env("PCE_WORKTREES", projection);
    for (index, path) in paths.iter().enumerate() {
        child.env(format!("PCE_WORKTREE_{index}"), path);
    }
```

`paths` comes from `materialization.paths()` (`:6722`) and is passed to the child verbatim — never
canonicalized. `cwd` is the per-repository checkout. `PCE_WORKTREES` carries the same relative paths
in its JSON projection, so the documented alternative has the identical problem.

Stated as inference rather than fact: I did not read the environment of the dead child, so the claim
that the values are *relative* rests on the `cd` argument echoed in stderr and on the journal's
`working_directory` fields being relative. Both point the same way, and the source shows no
canonicalization anywhere on the path.

## What worked

### The failure is legible and reproducible

The stderr names the exact path `cd` was given. Two issuances produced byte-identical failures, which
is what let me classify this as a contract defect rather than a flaky environment on the second
occurrence instead of the fifth.

### The criteria that don't cross repositories pass

`verify-authority.py --public` and `upload-r2-grit-d8.sh --self-test` both exit 0 in the same
materialization. The package's own work is sound; only the cross-repository reach fails.

## Friction and failures

### 1. `$PCE_WORKTREE_N` cannot be used for its stated purpose

- Severity: `high`
- Phase: `driver execution`
- Observation: the variable exists precisely so a criterion can reach a repository other than the one
  it runs in. From the criterion's actual working directory, it never resolves.
- Inference: any graph with a multi-repository package whose criteria cross repositories is
  unrunnable, and the failure appears only when that package is first dispatched — here, at issuance
  39 of a run that began at issuance 1.
- Impact: two packages in this graph (GD4, GD6) use this shape. Both were authored at plan version 1
  and passed `pce graph check` at every version since. There is no authoring-time signal.

### 2. The failure burns the recovery ladder

- Severity: `medium`
- Phase: `driver execution`
- Observation: issuance 39 failed, `recovery-rung-attempted rung=retry` dispatched 40, which failed
  identically, and `rung=local-patch` dispatched 41 — the last rung.
- Inference: a deterministic contract defect is indistinguishable, to the ladder, from a flaky worker.
  Every rung is spent re-running the same impossible command.
- Impact: worse than the cost, the `local-patch` rung invites a worker to make the path resolve — by
  a symlink or a copied tree — which would satisfy a frozen criterion by fabricating the environment
  it names. That is a gamed proof, produced by the recovery mechanism rather than by a careless
  worker.

### 3. `graph check` cannot see it

- Severity: `low`
- Phase: `graph authoring`
- Observation: `crates/core/src/graph_authoring.rs:81` already parses `$PCE_WORKTREE_` tokens out of
  criterion commands, so the checker knows which index a command reaches for.
- Inference: it validates the index against the package's repository count but says nothing about
  whether the reference can resolve at execution time, because today it cannot for any index above 0.

## Recommendations

### Canonicalize the worktree paths before handing them to the child

- Addresses: findings 1 and 2
- Change: absolutize `paths` in `shell_execution_at` before setting `PCE_WORKTREES` and each
  `PCE_WORKTREE_{index}`, so the values are independent of the child's working directory.
- Location: `src/main.rs:6681-6691`.
- Trade-off: absolute machine paths become visible to criterion commands, and thence potentially to
  stdout captured in the journal. The journal already stores `working_directory` for every execution,
  so this exposes nothing new in kind.
- Confidence: `high` — the mechanism is fully visible in the source above, and the fix is at the one
  place every criterion and preparation execution funnels through.

### Do not let a `local-patch` rung run against a criterion that failed on its environment

- Addresses: finding 2
- Change: when the recorded failure is an environment resolution error rather than an assertion
  failure, park with a contract fault instead of descending the ladder. The two are distinguishable
  here — `sh: line 0: cd: … No such file or directory` never comes from the verifier.
- Location: the recovery-rung selection path.
- Trade-off: a heuristic on stderr, which can misfire. A conservative pattern would still catch this
  class.
- Confidence: `medium` — the mechanism is right, the detection rule needs care.

### Have `graph check` execute a resolution dry-run

- Addresses: finding 3
- Change: for every criterion command that references `$PCE_WORKTREE_N`, verify at check time that a
  materialization would place that path where the command's working directory can reach it. With the
  first recommendation applied this becomes trivially true, which is itself the argument for doing
  the first one.
- Confidence: `medium`

## No-change decisions

- **Criteria running in the first repository's checkout.** Reasonable default. The defect is the
  unusable escape hatch, not the default.
- **`PCE_WORKTREES` as a JSON projection.** Fine shape; it inherits the same relative-path problem and
  is fixed by the same change.
- **Criteria invariance.** Not implicated. If the harness is fixed, GD4's frozen criteria pass
  unchanged — which is the strongest argument that the criteria were never wrong.

## Suggested follow-up

- **Check GD6 before it dispatches.** It uses `--repo "$PCE_WORKTREE_0" --repo "$PCE_WORKTREE_1"` and
  will fail the same way on the `_1` argument, three packages later, after another long live run.
- **Re-run GD4 unchanged after the fix.** If its five frozen criteria then pass with no criterion
  revision and no graph change, that is the clean confirmation that this report identified the whole
  defect.
