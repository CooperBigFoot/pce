# PCE workflow feedback: hardening invalidation is charged to the wrong package

- Date: `2026-08-21`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `pourpoint/planning/2026-08-07-declare-grit-d8-live-and-prove-released-reader-refinement-across-a-row-seam`, plan version 19
- Outcome: `blocked` — GD6 recovery-parked after two invalidations it did not cause and cannot fix
- Severity: `high` — the wrong package's ladder is spent, and the remedy the park names would spend a
  human ruling to reproduce the same failure

## Executive summary

At assembly the driver checks that each hardening repair is still load-bearing by reverting it and
confirming the criterion then fails. For GD15's repair `7b68223` that revert no longer applies:

```
package-hardening-invalidated  package: GD6  hardened_package: GD15  repository: pourpoint
  repair_ref 7b68223  gate package-gate-23-1  finding 0
  detail: Auto-merging scripts/released_wheel_proof.py
          CONFLICT (content): Merge conflict in scripts/released_wheel_proof.py
          error: could not revert 7b68223... fix: gate direct worker reads from production trace
```

The conflict is real. **The attribution is not.** `scripts/released_wheel_proof.py` was rewritten by
twelve commits from GD2, GD16, GD17, GD19, GD20 and GD21 after `7b68223` landed. **GD6 does not touch
that file at all** — its two repairs are documentation and `scripts/check_grit_attribution.py`.

GD6 was charged for this twice, which with one genuine failure exhausted its ladder:

```
recovery-parked GD6  "recovery spending exhausted after 3 attributable failures in this journal
                      recovery epoch; supply an attributed --recovery-reset record to open a fresh ladder"
```

A reset would hand GD6 a fresh ladder to spend on a conflict it cannot resolve, because nothing in
GD6's scope reaches the file.

## Evidence reviewed

- two `package-hardening-invalidated` events, both `GD6 -> GD15`, identical detail, at journal
  positions before issuance 46 and after issuance 47
- GD6's repair commits:
  ```
  8cc00abe  crates/python/README.md, docs/guide/datasets.md, docs/guide/staged-api.md,
            docs/quickstart.md, scripts/check_grit_attribution.py       (6 files)
  38169868  docs/credits.md, docs/how-it-works.md, docs/raster-cache.md,
            scripts/check_grit_attribution.py                            (4 files)
  ```
  Neither touches `scripts/released_wheel_proof.py`.
- `git log --oneline 7b68223..fdd55c476a50 -- scripts/released_wheel_proof.py` → twelve commits,
  including `4088000` (GD16), `1884cba`/`87252a3` (GD17), `411667f` (GD19), `4f5d316`/`654013a` (GD20),
  `a0e5a13` (GD21), `c460cbd`/`470ea2a` (GD2)
- `recovery-parked GD6`, whose recorded attempts are `retry 46`, `local-patch 47`, `replan`
- the second failure GD6 *was* fairly charged for: `package-failed` at issuance 46, three carried
  amendments exiting 2 because their test scripts were not in the fresh attempt's tree

## What worked

### The invalidation check itself is a good idea

- Evidence: it asks whether a hardening repair is still necessary rather than assuming it, by
  reverting and re-testing.
- Effect: it is the only mechanism in this run that examines whether a *completed* package's proof
  still holds against the assembled whole. Reports 3, 7 and 8 all describe defects it is aimed at.

### The new recovery-parked message names the epoch and the door

- Evidence: `"…in this journal recovery epoch; supply an attributed --recovery-reset record…"`,
  where the previous binary said `"re-author as plan version n+1"` (report 11).
- Effect: the remedy is now the one that actually works. In this case it is the right door aimed at
  the wrong package, which is a separate defect.

## Friction and failures

### 1. The invalidation is attributed to the package being assembled, not the one that caused it

- Severity: `high`
- Phase: `assembly`
- Observation: `package: GD6` on both events; GD6's commits do not touch the conflicted file.
- Evidence: the two commit stats above against the twelve-commit log.
- Inference: attribution appears to be positional — the package whose assembly step ran when the check
  failed — rather than derived from which commits touch the conflicted paths. The information needed
  to attribute correctly is present: the conflict names one file, and `git log <repair>..<assembly>`
  over that file names the packages that rewrote it.
- Impact: GD6 was charged twice for a conflict outside its scope, exhausting a ladder that had one
  legitimate failure on it. The graph is now blocked on a package that cannot fix what it is blamed
  for.

### 2. An unrevertable repair is treated as a package failure rather than an assembly finding

- Severity: `medium`
- Phase: `assembly`
- Observation: the outcome is `package-hardening-invalidated`, a charged failure, dispatching a worker.
- Inference: "this repair can no longer be reverted cleanly" is a fact about the *lineage*, not about
  any package's work. A shared file rewritten by six packages will always reach this state eventually;
  it is the normal end state of a file under sustained repair, not a defect.
- Impact: every graph with a heavily-edited shared file will spend recovery budget here, and will
  spend it on whichever package happens to assemble last.

### 3. The remedy the park names cannot work for the parked package

- Severity: `high`
- Phase: `recovery`
- Observation: the park directs a human to supply a `--recovery-reset` for GD6.
- Inference: a fresh ladder returns GD6 to the same assembly step, where the same revert will conflict
  for the same reason. The only actions that could clear it are outside GD6 — changing the assembly
  order, or not requiring a clean revert of a repair whose file has since been rewritten.
- Impact: a human ruling would be spent producing no change. This is the second time in this run a
  park has named a remedy that cannot resolve it; report 11 was the first.

## Recommendations

### Attribute the invalidation from the conflicted paths

- Addresses: findings 1 and 3
- Change: when the revert conflicts, resolve the conflicting paths from `git`'s own output and
  attribute to the packages whose assembly inputs modify those paths. If that set does not include the
  package currently being assembled, do not charge it.
- Location: the assembly hardening-invalidation path in `package_driver.rs`.
- Trade-off: the correct attribution may be several packages, or all of them, in which case there is
  no single package to charge — which is finding 2's point and argues for treating it as an assembly
  finding instead.
- Confidence: `high` — the attribution inputs are already in hand at the moment of failure.

### Distinguish "revert conflicts" from "repair is no longer necessary"

- Addresses: finding 2
- Change: a clean revert whose criterion still passes means the repair is genuinely redundant, which is
  worth reporting. A revert that *cannot be applied* proves nothing about necessity; record it as an
  assembly observation and continue, rather than charging a package.
- Location: same path.
- Trade-off: a repair that has become genuinely unnecessary could be missed when its file has drifted.
  The alternative is what happened here.
- Confidence: `high`

### Do not name a remedy without checking it applies

- Addresses: finding 3
- Change: `recovery-parked` should not direct a human to reset a package whose recorded failures are
  attributed to paths outside that package's inputs.
- Location: the `recovery-parked` reason construction.
- Confidence: `medium` — a full check may be expensive; even naming the conflicted file in the park
  reason would let a supervisor see the mismatch without reading the source.

## No-change decisions

- **Charging GD6 for issuance 46.** Correct. That failure was real: three carried amendments exited 2
  because the test scripts their earlier repairs created were not in the fresh attempt's tree. GD6
  fixed it at issuance 47 on the local-patch rung, which is the ladder working as designed.
- **Re-running assembly criteria against the composed whole.** Correct and valuable — it caught a real
  cross-package conflict in this same assembly (GD4's authority check against a file GD6 rewrote),
  which no package-level gate could have seen.
- **The recovery-epoch design.** Correct, and better than the plan-version scoping I proposed in report
  11. Nothing here argues against it.

## Suggested follow-up

- **Check whether any repair in this run is still cleanly revertable.** `scripts/released_wheel_proof.py`
  carries repairs from seven packages; if most are now unrevertable, the check is close to
  unconditionally failing for this file and finding 2 is urgent rather than theoretical.
- **The second assembly failure is unrelated and is now settled.** GD4's `verify-authority.py --public`
  fails because GD6 rewrote `hosting/grit-hfx-v0.3.0/README.md` from 17601 to 7406 bytes while the
  script pins the 17601-byte identity at line 24. **Correcting this report's original framing:** I
  called it a conflict needing a human ruling. It needed neither a ruling nor a criterion revision —
  GD4's criterion command is unchanged, and `LOCAL_IDENTITIES["README.md"]` pins *which local file was
  compared to the hosted observation*, so when GD6 legitimately rewrites that file the pin follows it
  to stay true. The graph is untouched; the fix is one tuple, to be made by GD6 when it next
  dispatches.
