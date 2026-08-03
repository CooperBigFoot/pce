# PCE workflow feedback: the binary owns every dispatch

- Date: `2026-07-31` — completed `2026-08-03`
- Orchestrator: Claude Code (Opus 5), single session, compacted repeatedly
- Run: `pce`, `planning/2026-07-31-the-binary-owns-every-dispatch`
- Outcome: **delivered.** `main` at `e8aef92790637d61c7c4912a73f77255a8916b3e`. Eight milestones,
  all nine acceptance criteria discharged. 437 workspace tests green, `pce contract check` exit 0,
  `SKILL.md` carries zero `codex exec` and zero `claude -p` against 17 `pce dispatch` references.

This replaces the mid-run version of this report, whose findings on vacuity are confirmed and
strengthened below.

## Executive summary

The dominant finding is unchanged from mid-run and got worse with data: **vacuity is the defect class
the workflow exists to catch, and it is the defect class the workflow keeps reintroducing while
catching it.** Final count: **fourteen** measured vacuous assertions, **ten of them introduced while
remedying an earlier one**. The mid-run report recorded eight and three.

The second-order rule — *a remedy is a new claim, not a closure* — is the most transferable output of
this run, and it generalised past code. Two of the final three PR blockers were false sentences
written **inside disclosures added to prevent false statements**.

The second dominant class was new, and it is mine: **the orchestrator asserting instead of
measuring.**

## The orchestrator's own error class

Concretely, each one a single command away from being known:

- **Six binding decisions issued from a review's prose without testing them. Five were wrong.** One
  claimed every production gate anchor writes `{{OUTPUT}}` beneath `{{CWD}}`; in `SKILL.md` both are
  independent placeholders and nothing binds one beneath the other. Cost: a full plan round.
- **`--disallowedTools` placed before the prompt.** It is *variadic* and consumed the prompt;
  `claude` replied `Input must be provided either through stdin or as a prompt argument`. One
  `claude --help` would have shown it. Cost: a gate dispatch and a campaign stop.
- **`LIVE_ENV={PATH, HOME}` asserted sufficient for a gate child.** Measured: `Not logged in · Please
  run /login`, `duration_api_ms: 0`. `USER` is required. Graph note NB3 had established those two as
  *necessary*; nobody tested *sufficient*. Cost: a second dispatch and a second stop.
- **"437 tests pass" measured in the executor's working directory**, where the untracked `.agents/`
  tree exists. Three tests hardcode a path into it, so from a clean checkout the suite was **red**.
  Cost: a PR review round.
- **Five manifest values transcribed as measured that were inferred**, contradicted by the run's own
  receipts.

## Rules that earned their keep

**Prove the mechanism once, cheaply, before committing the rest.** The `m8-s2` gate probe was designed
to run alone so a broken write mechanism could not burn all five gate dispatches; it caught two
defects for the price of one. Later, validating the gate against a *throwaway* log before the clean
re-run stopped a transient failure from tainting a fresh campaign.

**Check for pre-emption on every falsification row.** If an earlier assertion or a parse error reds
first, the named assertion never ran and the cycle is void. Five were caught preventively during
`m8-s1` execution — one *predicted* from the prior session's lesson. Every earlier instance in this
run had been found after the fact by a critic, and one cost four false ledger records.

**A named impossibility beats an asserted property.** Criterion 4 does not claim every descendant is
gone — user-space polling cannot guarantee that. It claims something narrower and checkable.

**Freeze the parser before spending the irreversible resource.** The `m8-s2` manifest schema was frozen
against a *working* parser before any quota. Roughly a dozen shape errors were then caught offline
rather than after the window was spent. The freeze also refused a late `measurement_provenance` key —
working exactly as designed.

**Two-stage receipts where an actor structurally cannot measure its own work.** The executor sits at
Seatbelt nesting level 1 and the strict harness must skip there. Eight `X` packets were authored by
the executor and replayed by the orchestrator at level 0 — which caught `X02`, the only
production-weight falsifier, being **completely inert**: it opened a hardcoded fixture path that never
existed, so the claim it guarded had no falsifier at all. Invisible at level 1 by construction.

## Findings for the skill

1. **`--add-dir` scope is not obvious and bit three times.** Being inside the `-C` root is not
   sufficient for write access; `mkdir` succeeding is not evidence that file creation will; and a
   linked worktree shares the **main** repository's object database, so `git add` needs the primary
   `.git` granted explicitly.
2. **Gate children are unsandboxed.** `pce` adds no sandbox, no permission flag, no tool restriction —
   `current_dir(cwd)` is the only spatial binding and it does not bound `Bash`. With the operator's
   real `HOME` required for keychain OAuth, `~/.claude/settings.json` grants stay live. Measured:
   `~/.claude/skills/pce` is a **symlink into the repository**, a literal path from a child's home
   into the tree. Pin `--disallowedTools` on every real gate.
3. **The subscription window is shared three ways** — orchestrator session, subagents, and
   `pce`-spawned children. It was exhausted once by review work alone, mid-gate. Serialize heavy
   reviews; never run a quota-bearing campaign concurrently with one.
4. **`gate_duration_compares_binary_measurements` is a genuine intermittent.** Failed first-try on two
   independent bases, passed 5/5 in isolation, green on retry both times. Tracked separately; not
   introduced by this vision.
5. **The identity scheme has no provision for re-issuing a canonical route.** The retry namespace
   covers *substitutes* only. Two setup-defect re-issues left one canonical identity appearing three
   times, which the frozen verifier rejects at `fixture/identities/canonical-unique`. The choice was
   loosen the frozen parser after seeing the data, or re-run cleanly. Re-running was correct; a
   re-issue namespace would have avoided the dilemma.
6. **Snapshot exclusions must be scoped to what the snapshotter itself writes.** A first attempt
   reddened on the orchestrator's own `events.jsonl` appends and on peer `orchestrator-feedback/`
   files. A snapshot that fires on the snapshotter aborts a quota-bearing campaign for nothing.

## What a reviewer should distrust

The committed fixture cannot be proven a real measurement from a clean checkout; a hand-authored JSONL
file satisfies the same offline assertions. The adversarial reviewer nonetheless corroborated it
independently: raw `ps` captures whose SHA-256 digests match the manifest exactly, real PIDs, real
`claude` argv bearing the campaign's own absolute paths, no `ANTHROPIC_API_KEY`, and a direct-child
`lstart` matching an issuance timestamp to the second.

The manifest states which projection fields are authored rather than captured, that `raw` and
`normalized` carry the same bytes so those six sites are self-consistency checks, and which PID values
were inferred from a single reading. Those disclosures are the honest part of the artifact and should
be read before its numbers.
