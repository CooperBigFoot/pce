# PCE workflow feedback: a new dirty-source guard blocks a run on pre-existing unrelated changes

- Date: `2026-08-21`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan version 19
- Outcome: `aborted`, then `resolved` — the driver refused to start twice; the run proceeded only
  after the supervisor stashed both entries under explicit human delegation
- Severity: `medium` — the guard is defensible, its arrival and its breadth are the problem

## Executive summary

Binary `273f5c34` — installed to deliver the recovery-epoch fix from report 11 — also introduced a
guard that aborts `driver-run` when a source repository has any uncommitted change:

```
driver-aborted  "source repository `pourpoint` is dirty; commit, stash, or remove its changes
                 before driver-run"
```

The two changes it objects to are `M CONTEXT.md` and `?? docs/codex-non-interactive.md`. **Both were
present when this session began, and every launch in this run since then has succeeded with them in
place** — including two launches today under binaries `1be14320` and `d4b8a13b`. The guard is new, it
arrived unannounced with an unrelated fix, and it fired on state older than the defect it shipped
alongside.

The verification the run was staged to perform — GD4 dispatching under the corrected harness, its
three frozen criteria passing or failing on their own merits — was delayed by two aborts. It began
only after both entries were stashed, at which point the driver resumed, consumed the human's
`--recovery-reset` record, and dispatched GD4 at issuance 42.

## Evidence reviewed

- `shasum -a 256 ~/.local/bin/pce` → `273f5c34fc96a2b1…`, the announced binary
- the journal's only new event:
  ```
  driver-aborted  {"reason":"source repository `pourpoint` is dirty; commit, stash, or remove its
                   changes before driver-run"}
  ```
  Journal 511 → 512. No `plan-version-advanced`, no `recovery-reset`, no dispatch.
- `git -C pourpoint status --short` → exactly two entries:
  ```
  M CONTEXT.md
  ?? docs/codex-non-interactive.md
  ```
- the session's opening `gitStatus`, which records the same two entries
- `git -C hfx status --short` → nine entries, **all untracked**: two `uv.lock` files,
  `milestones.json`, `steps.json`, and five `tdx-m5-*` evidence directories
- `strings ~/.local/bin/pce | grep -c "is dirty; commit, stash, or remove"` → 1

## What worked

### The abort is a typed journal event, not a crash

- Evidence: `driver-aborted` with a reason naming the repository and the remedy.
- Effect: the journal records why the run did not start, and the state is recoverable rather than
  ambiguous. Diagnosis took one command.

### The guard's intent is sound

- Evidence: this run has already been bitten once by base movement — the `--accept-base-currency-risk`
  record the human authored at plan version 13 exists because an authored ref drifted behind its
  remote.
- Effect: refusing to compose against a moving working tree is the right instinct. Nothing below
  argues for removing the guard.

## Friction and failures

### 1. The guard blocks on paths no criterion or composition can reach

- Severity: `medium`
- Phase: `driver execution`
- Observation: `CONTEXT.md` and `docs/codex-non-interactive.md` are not named by any criterion command
  in `graph.v19.json`, and package bases are composed from committed refs, not the working tree.
- Inference: the guard tests repository cleanliness rather than whether the dirty paths could affect
  what the run composes or verifies.
- Impact: a run 20 packages deep, with 14 completions carried and one package staged for the exact
  verification the new binary was installed to enable, cannot start because of an unrelated edit to a
  context document.

### 2. Untracked files appear to count, which makes hfx the next wall

- Severity: `medium`
- Phase: `driver execution`
- Observation: pourpoint's two entries are one modified file and one untracked file. hfx has nine
  entries, every one untracked, including five retained evidence directories from a different run.
- **Resolved by experiment — untracked files do count.** After stashing only the tracked
  `CONTEXT.md`, leaving one untracked file, the driver aborted again with the identical message. A
  second stash covering `docs/codex-non-interactive.md` cleared it and the run started.
- Correction to a false lead I nearly published: I first reasoned that untracked files could not count,
  because another live `driver-run` (pid 35882, started 00:11:27, twelve minutes after this binary was
  installed at 23:59:33) is working normally against an hfx tree holding nine untracked entries. That
  reasoning is wrong, or at least does not transfer — the observed behaviour on this run is
  unambiguous. Most of those hfx entries are outputs that run created after it started, so its
  preflight likely saw a different tree. **I tested rather than asserting, and the test contradicted
  the inference.**
- Impact: two rounds of intervention were needed, exactly as feared, though the second was benign here.
  The concern about hfx stands in principle: if a supervisor had to clear that tree, the remedy would
  read as "delete another run's retained evidence", which no supervisor should do.

### 3. A behavioural change shipped inside a bug fix, unannounced

- Severity: `low`
- Phase: `orientation`
- Observation: the binary was described as delivering the recovery-epoch fix. The guard was not
  mentioned, and its first appearance is an abort.
- Impact: the supervisor's model of what a relaunch does silently stopped being true. I had verified
  the digest, the reset record, and the frozen graph before launching, and none of those checks could
  have surfaced this.

## Recommendations

### Scope the guard to paths the run actually depends on

- Addresses: findings 1 and 2
- Change: abort only when a dirty path is inside a repository path that a criterion command names, or
  when a tracked file differs from the composed base. Otherwise warn into the journal and continue.
- Location: the `driver-run` preflight that emits `driver-aborted`.
- Trade-off: a narrower guard misses an edit that matters through an indirect route, such as a build
  file. A tracked-files-only rule with a warning for untracked would already remove both walls here.
- Confidence: `medium` — the direction is clear; the exact predicate deserves care.

### Distinguish tracked modifications from untracked files

- Addresses: finding 2
- Change: treat `M`/`D`/`R` as abort-worthy and `??` as a journal warning. Untracked files cannot
  change what `git` composes.
- Location: same preflight.
- Trade-off: an untracked file can still affect a build. The warning keeps it visible.
- Confidence: `high` for the distinction itself.

### Name new preflight guards when announcing a binary

- Addresses: finding 3
- Change: when a binary adds a refusal, say so alongside the fix it ships with.
- Location: the release note or install message.
- Trade-off: none.
- Confidence: `high`

## No-change decisions

- **Aborting rather than warning for genuinely dangerous dirt.** Correct. A tracked source file
  differing from the composed base is exactly the failure this run's base-currency machinery exists to
  prevent.
- **The remedy wording.** "commit, stash, or remove" is the right set of options, and it matters that
  the message names all three: `stash` is the only one that clears the guard without moving `HEAD` off
  the graph's authored ref, which `commit` would have done. The supervisor initially declined to take
  any of them, escalated, and acted only after the human delegated the choice — recorded here because
  a reader should know the tree was cleared by the orchestrator, not by the human.

## Outcome under binary `f83d4548` (2026-08-21)

Two of this report's three recommendations landed. The guard now excludes the driver's own vision
directory, and the refusal **names the offending paths** instead of leaving the supervisor to derive
them:

```
source repository `pourpoint` is dirty outside the driver vision directory; driver-run requires
these paths to be clean while preserving /Users/…/planning/2026-08-07-declare-…-row-seam:
 M CONTEXT.md
?? docs/codex-non-interactive.md
```

It also no longer recommends `git clean` or `git stash -u`, which is the right call — those are
destructive-looking remedies for an orchestrator to be handed.

**Finding 1 stands.** Tested rather than assumed: both entries were restored from their stashes and the
driver relaunched. It aborted again. `CONTEXT.md` and `docs/codex-non-interactive.md` are neither
driver output nor inside the vision directory, so the breadth question — whether a repository-root
documentation edit should block a run that composes from committed refs — is unchanged. Both files
were re-stashed to keep the run relaunchable.

## Suggested follow-up

- ~~Confirm whether untracked files count.~~ **Done — they do.** See the resolution under finding 2.
  The remaining question is whether they *should*: an untracked file cannot change what `git` composes
  into a package base, and both of this repository's blocking entries were documentation.
- **The GD4 verification is still pending.** Reports 9 and 11 both remain unconfirmed by execution;
  the run is staged to confirm both in a single dispatch as soon as it can start.
