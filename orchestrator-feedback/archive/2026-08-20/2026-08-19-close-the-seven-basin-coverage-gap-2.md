# PCE workflow feedback: close-the-seven-basin-coverage-gap (2)

- Date: `2026-08-19`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill, same session as report 1`
- Run: `hfx: planning/2026-08-07-close-the-seven-basin-coverage-gap`, plan versions 5 and 6, issuances 18-21
- Outcome: `in progress` — SB0/SB1/SB2/SB6/SB7 complete, second paid campaign running under SB3 issuance 21

## Executive summary

Report 1 covered plan versions 1-4, where 17 dispatches burned against external preconditions. Its two headline recommendations have since landed in the binary (park on the second identical worker blocker; carry `blocked_by` verbatim into the park record), so this report does not restate them.

Two new findings, both `high`-adjacent and neither about the graph model:

1. **Worktrees are never reclaimed.** 200 directories, ~270 GB, across five visions. Free disk fell to 27 GB while a paid campaign ran toward a preservation step that writes several GB locally. A successful compile that cannot preserve its outputs is money spent for nothing.
2. **`find` on this machine is `bfs`, which silently mis-answers `-newermt`.** A recency classification built on it reported "0 active, 199 stale" while a live worktree sat in the set. Trusting it would have deleted this run's own campaign worker plus four other sessions' in-flight work.

One structural observation worth recording: correcting a completed package additively (SB7 superseding SB1) produces a guaranteed join conflict, because both edit the same code the second exists to replace.

## Evidence reviewed

- `driver-journal.jsonl` at plan versions 5 and 6, offsets 113085-147465
- `package-outcomes/SB7/20.json`, SB7 `criterion-executed` and `finding-replayed` records
- `join-conflict-SB3.md`, `park-evidence-SB3-v5.md` (written by this orchestrator)
- `du`/`df`/`os.lstat` measurements of `/private/tmp/pce-work-package-worktrees`
- `hcloud server list` / `volume list` sampled across both campaigns

## What worked

### The park record named its own dependency

- Evidence: `package-outcomes/SB3/17.json` -> `{"outcome":"mis-specified","fault":{"kind":"missing-dependency","id":"maintainer-approved SB3 compile-campaign runbook and tracked-provisioner credential-transfer authorization"}}`
- Effect: unlike the `recovery spending exhausted` parks of plan versions 1 and 2, this one was immediately actionable and became plan version 5 without further investigation.

### The gate attacked the finding a reviewer would doubt

- Evidence: SB7's three criteria all passed with every real case resolving to `endpoint=1`. Its gate then proposed and merged `source_downstream_endpoint_cannot_be_masked_by_tolerance_overlap`.
- Effect: six identical answers is exactly the pattern that could mean "correct convention" or "rule always returns 1". The gate turned that suspicion into a test rather than leaving it to a human reviewer who might not look.

### Re-proof of all criteria after a repair merge

- Evidence: after `package-repair-merged` for SB6, four `gate-reproof-executed` records ran all three authored criteria plus the new amendment.
- Effect: a repair that silently broke an already-passing criterion could not survive. Cheap and load-bearing.

### A structurally malformed gate finding was rejected, and the check was not lost

- Evidence: `finding-rejected` on `package-gate-18-1` — `finding exceeds package repository scope: package gate finding names untouched repository /tmp/pce-work-package-worktrees/pce-0c836.../00-hfx`. The successor gate `package-gate-18-2` re-proposed the same check well-formed and it was accepted.
- Effect: schema enforcement without permanent loss of the check. Worth noting the near-miss: the substance survived only because a second gate re-derived it.

## Friction and failures

### Worktrees are never reclaimed, and accumulated to 270 GB under a paid campaign

- Severity: `high`
- Phase: `execution`
- Observation: `/private/tmp/pce-work-package-worktrees` held 200 directories totalling about 270 GB, spanning five visions: `close-the-seven-basin-coverage-gap`, `signal-bearing-dudh-warm-window`, `declare-grit-d8-live...`, `gridded-statics-self-name...`, `incidence-core`. Free disk fell from 58 GB to 27 GB over roughly two hours while a paid campaign was running.
- Evidence: `du -sh` -> 270G over 200 entries; per-directory `os.lstat` classification -> 154 untouched for 6h or more, 222.1 GB; `df -g` 27 GB free before removal, 232 GB after.
- Inference: one worktree is created per issuance and nothing reclaims them, within a vision or across visions. This run alone produced 21 issuances.
- Impact: SB3's third criterion requires that every produced output survive the machine that made it, and preservation writes several GB locally. A campaign that compiles successfully and then cannot preserve is money spent for nothing. The margin was roughly 27 GB.

### `find` on this machine is `bfs` and silently mis-answers `-newermt`

- Severity: `medium`
- Phase: `supervision`
- Observation: `find <dir> -type f -newermt '-3 hours' -print -quit` emitted `bfs: error: Invalid timestamp.` on stderr, produced no stdout, and exited such that a naive caller saw "no recent files". Every directory was therefore classified stale, including a worktree modified minutes earlier.
- Evidence: `bfs: error: bfs -S dfs -regextype findutils-default <dir> -type f -newermt "-3 hours" -print -quit` / `bfs: error: Invalid timestamp.`; the classification reported `active (touched <3h): 0` / `stale: 199`.
- Inference: `find` resolves to `bfs`, which does not accept GNU relative-timestamp syntax and fails silently to stdout-only callers.
- Impact: none realised — the zero count was implausible on its face and was rechecked with `os.lstat`, which found 46 active directories. Had it been trusted, the deletion would have destroyed this run's live campaign worker and four other sessions' in-flight worktrees.

### Additive correction of a completed package guarantees a join conflict

- Severity: `low`
- Phase: `execution`
- Observation: `package-join-conflicted` for SB3 — `CONFLICT (content): Merge conflict in adapters/tdx-hydro/build_adapter.py`.
- Evidence: `git diff` hunk ranges — SB1 attempt-4 edits 3868-3892 of `_build_compact_topology`; SB7 attempt-20 rewrites 3868-3895 and 3878-3927.
- Inference: SB7 exists to supersede the rule SB1 delivered. Because criteria are immutable and completions carry, the only way to correct SB1 is a new package editing the same lines, so the conflict is structural rather than incidental.
- Impact: none here — issuance 21 resolved it and provisioned. But the resolution silently decides which of two proven packages wins in the merged tree, and nothing re-proves SB1's frozen criteria against the resolved result before the campaign spends money.

## Recommendations

### Reclaim a worktree when its issuance reaches a terminal state

- Addresses: "Worktrees are never reclaimed"
- Change: remove an issuance's worktree when its package completes or the issuance is superseded. The commits live in the owning repository, so the checkout is disposable; `dispatch-pane-cleanup` shows the driver already tracks the lifecycle this needs. Retaining the most recent N per package would keep debugging value.
- Location: driver cleanup path in `pce`, alongside `dispatch-pane-cleanup`
- Trade-off: inspecting a superseded attempt then needs `git worktree add` at the recorded branch.
- Confidence: `high`

### Re-prove a package's criteria after a join conflict is resolved

- Addresses: "Additive correction of a completed package guarantees a join conflict"
- Change: when `package-join-conflicted` is resolved, re-run the frozen criteria of every package whose files the resolution touched, as the post-repair `gate-reproof-executed` pass already does after a gate repair. Here that would re-prove SB1's three criteria against the tree that actually goes to the campaign.
- Location: join-resolution path in `driver-run`
- Trade-off: extra criterion executions on every conflicted join; here that is three unit tests against a several-euro campaign.
- Confidence: `medium`

### Prefer `stat`-based recency over `find -newermt` in workflow instructions

- Addresses: "`find` on this machine is `bfs`"
- Change: where any workflow or skill instructs an agent to select files by recency, specify `stat`/`os.lstat` comparison rather than `find -newermt`, or require the caller to check stderr and exit status.
- Location: skill and runbook text that selects by recency
- Trade-off: slightly more verbose instructions.
- Confidence: `high`

## No-change decisions

- **Runtime-precondition declarations in the graph schema.** Report 1 recommended these at `medium` confidence; the maintainer considered and deliberately deferred them, on the ground that the second-identical-blocker park rule captures the value. Agreed on this run's evidence: SB0 and SB6 achieved the same effect as ordinary packages with ordinary criteria, and both passed first try.
- **The per-package one-shot overrule.** Report 1's experiment suggestion still looks unnecessary. The overrule spent in this run was true, admissible, and insufficient; a wider budget would not have helped, because the blocker was invisible from the tree.

## Suggested follow-up

- The two campaigns cost roughly EUR 1.30 and EUR 0.35 so far, both bounded by the runbook's own gate (`{"projected_gross_total_eur": "20.99", "gross_cost_ceiling_eur": "40.0", "decision": "permit"}`). That gate ran before the expensive phase and is worth keeping as a pattern for any future paid package.
- The `pce` binary was replaced three times during this run, twice while a driver was live. The live driver keeps its loaded code, so campaigns were unaffected, but a supervisor cannot assume the binary it verified is the binary a later command will use. Recording each digest before consequential commands worked; a `pce --version` carrying the integration head would make it cheaper.
