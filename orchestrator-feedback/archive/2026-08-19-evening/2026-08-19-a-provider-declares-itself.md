# PCE workflow feedback: 2026-08-19-a-provider-declares-itself

- Date: `2026-08-19`
- Orchestrator: `Claude Code (Opus 5), /work-graph skill`
- Run: `planning/2026-08-19-a-provider-declares-itself` in `RivRetrieve/RivRetrieve`, frozen graph `graph.v1.json` (plan_version 1)
- Outcome: `in flight` — filed during supervision, before any terminal driver status

## Executive summary

Two findings from the setup phase of a `/work-graph` run, both about the supervisor's ability to
observe and record rather than about package work.

The high-severity one is a false positive in the `pce-protect-criteria` PreToolUse hook: it tests
`"vision.md" not in command` as a plain substring, so it refuses every Bash command that mentions
`supervision.md` — the file the `/work-graph` skill requires the supervisor to append to before
putting anything in chat. The protection and the skill's core recording obligation collide on a
substring.

The second is a hazard, not a defect: probing the installed `pce` while its build artifact was
being rewritten returned a usage string missing a flag, and the supervisor recorded a wrong
conclusion about its own authority into `supervision.md` before catching it. No workflow rule
caused this, but the run's durable record briefly carried a false capability claim.

## Evidence reviewed

- `/Users/nicolaslazaro/.local/bin/pce-protect-criteria` (hook source, `handle_bash`)
- `planning/2026-08-19-a-provider-declares-itself/supervision.md` (this run's supervision record)
- `planning/2026-08-19-a-provider-declares-itself/run.json`, `driver-journal.jsonl`
- `pce` repo `HEAD` = `e647d1d`; commit `6e7d420 feat: mechanical freeze for definition-preserving plan bumps`
- `/Users/nicolaslazaro/Desktop/work/pce/target/release/pce` (symlink target of `~/.local/bin/pce`), mtime 2026-08-19 15:56 local
- `pce` source `src/main.rs:9616`
- `~/.claude/skills/work-graph` skill text, sections 2–6

## What worked

### Single-frozen-version resolution

- Evidence: only `graph.v1.json` existed; `pce graph check --file .../graph.v1.json` exited 0 with
  `{"packages":5,"plan_version":1,"refs_verified":false,"valid":true}`; `diff graph.json graph.v1.json`
  was empty.
- Effect: the "unapproved working draft" branch of section 1 resolved in one command pair with no
  ambiguity, and no version-mixing risk existed for later commands.

### `run.json` precedent reuse

- Evidence: `planning/2026-08-19-a-fixture-is-a-recording/run.json` already encoded the same single
  repository `RivRetrieve` with `prepare` = `uv sync --all-extras` and an empty `environment`.
- Effect: the section 2 launch-configuration question became a one-question confirmation instead of
  a four-part interrogation, and the empty environment declaration was independently corroborated by
  a harness survey (`os.environ` appears only in `br_ana`/`no_nve` `generate_catalogue.py`, which no
  frozen criterion command exercises).

## Friction and failures

### The criterion-protection hook refuses Bash on `supervision.md`

- Severity: `high`
- Phase: `supervision / state persistence`
- Observation: a Bash heredoc appending the launch record to
  `planning/2026-08-19-a-provider-declares-itself/supervision.md` and writing
  `supervision-state.json` was refused with
  `REFUSED: Bash may not access vision.md during an active run; use Read for inspection, Edit or
  Write for a proposed change, and pce log --kind criterion-added for additive criteria.`
  The command never referenced `vision.md`.
- Evidence: `pce-protect-criteria`, `handle_bash`, line 108:
  `if "vision.md" not in command or root is None: return`. This is an unanchored substring test on
  the whole command string. `supervision.md` contains `vision.md` at offset 5. The guard then fires
  because `glob(<root>/planning/*/events.jsonl)` matches — which it always does during any active
  run, since `events.jsonl` is exactly what marks a run active.
- Inference: the substring test was written to catch paths like `planning/<x>/vision.md` and did not
  anticipate that the `/work-graph` skill's own required artifact is named `supervision.md`. The
  same false positive would fire on any `supervision.md` read, `grep`, `cat`, `wc`, or `git`
  invocation, and on any future file whose name ends in `vision.md`.
- Impact: the supervisor cannot use Bash for the one file section 4 requires it to append to
  "before putting it in chat". Every append must route through Edit/Write, which costs a Read of the
  current tail first, and read-only inspection (`tail`, `wc`, `grep`) of the supervision record is
  blocked outright. The refusal message actively misdescribes the situation, naming a file the
  command did not touch, which invites an orchestrator to conclude the hook is broken and look for a
  way around it. An orchestrator that took the message at face value could plausibly try string
  splitting (`"super""vision.md"`) to satisfy the tool, defeating the protection for real
  `vision.md` accesses too. This run did not attempt that and recorded the decision not to.

### A mid-rebuild binary probe produced a false capability claim in the durable record

- Severity: `low`
- Phase: `orientation`
- Observation: the first `pce` invocation of the session printed a usage block whose freeze line was
  `pce graph freeze --vision-dir <VISION_DIR> [--criterion-revisions <HUMAN_RECORD_PATH>]
  --repository ...` with no `--mechanical`. The supervisor recorded in `supervision.md` that
  definition-preserving mechanical freeze authority was unavailable for the whole run. Re-probing
  later returned
  `pce graph freeze --vision-dir <VISION_DIR> [--mechanical] [--criterion-revisions <HUMAN_RECORD_PATH>] --repository ...`.
- Evidence: `~/.local/bin/pce` is a symlink to `.../pce/target/release/pce`, mtime 2026-08-19 15:56
  local, which falls inside the window of the first probe. `strings` on the current artifact yields
  the `[--mechanical]` usage line plus `mechanical freeze refused because package definitions differ
  or graph metadata changed...` and `mechanical freeze cannot carry a human criterion revision
  record`. `6e7d420` is an ancestor of `pce` `HEAD`; `src/main.rs:9616` gates `--mechanical` on
  `FreezeAuthority::Human`.
- Inference: the binary was being rewritten by a `cargo build` while the supervisor probed it, so
  the probe read a stale or partially written image. Distinguished from observation: no build log was
  inspected, so "a rebuild was in progress" is inferred from the mtime coincidence alone.
- Impact: bounded. The wrong claim lived only in `supervision.md` and was corrected there before any
  freeze was attempted, so no authority was wrongly exercised or wrongly withheld. The general shape
  is what matters: section 1 tells the supervisor to treat binary refusals as the authority test,
  and a `--help` probe is not a durable capability proof.

## Recommendations

### Match the hook on a path boundary, not a bare substring

- Addresses: the `supervision.md` false positive.
- Change: in `handle_bash`, replace `if "vision.md" not in command` with a test that requires
  `vision.md` to be preceded by a path separator or a token boundary — the cheapest correct form is
  a regex such as `(?:^|[\s'"=/])vision\.md(?:$|[\s'"])`, which still catches `planning/x/vision.md`
  and bare `vision.md` while letting `supervision.md` through. Keep the existing
  `glob(planning/*/events.jsonl)` active-run condition unchanged.
- Location: `~/.local/bin/pce-protect-criteria`, `handle_bash`, line 108.
- Trade-off: a regex is marginally more expensive than `in` and needs a matching test. A command
  that constructs the path by concatenation still evades it — but that was already true of the
  substring form, so no protection is lost.
- Confidence: `high`

### Name the offending token in the refusal message

- Addresses: the same finding; the message misdescribes what was matched.
- Change: include the matched substring and its offset in `BASH_REFUSAL`, e.g. append
  `(matched "vision.md" at offset N in the command)`.
- Location: `~/.local/bin/pce-protect-criteria`, `BASH_REFUSAL` constant, line 20.
- Trade-off: a slightly longer message.
- Confidence: `high`

### Have the work-graph skill state that supervision.md is Edit/Write-only

- Addresses: the same finding, as belt-and-braces if the hook fix lands later.
- Change: add one sentence to section 4 next to "Append every narration ... to `supervision.md`
  **before** putting it in chat": note that this file is written with Edit/Write, never with shell
  redirection, and that a Bash refusal naming `vision.md` on a `supervision.md` command is a known
  hook false positive to be filed, never worked around by obscuring the string.
- Location: `~/.claude/skills/work-graph/SKILL.md`, section 4.
- Trade-off: adds a line of ceremony that becomes redundant once the hook is fixed.
- Confidence: `medium`

### Prove binary authority from a refusal, not from `--help`

- Addresses: the mid-rebuild false capability claim.
- Change: section 1 already says the authority "must be refusals in the installed `pce` binary" and
  that "a prose promise or harness hook is not a substitute". Make the check operational: state that
  the supervisor establishes mechanical-freeze authority at the moment it needs it, by observing the
  binary's actual refusal or acceptance, and that a usage or `--help` string is not that proof.
- Location: `~/.claude/skills/work-graph/SKILL.md`, section 1, the sentence beginning "Criteria
  invariance and definition-preserving mechanical freeze must be refusals...".
- Trade-off: none material; it removes an eager check that can be read at the wrong instant.
- Confidence: `medium`

## No-change decisions

- The stale `.pce-graph-freeze.lock` (0 bytes) left in the vision directory by the freeze that
  produced `graph.v1.json` was observed and left alone. It is not a driver input and did not block
  anything. Not worth a workflow change unless a later run shows a stale lock blocking a freeze.
- `pce graph check` reporting `refs_verified: false` when invoked without `--repository` is correct
  behavior, not a defect. The supervisor's section 1 check does not pass repository mappings, and
  the frozen graph's `authored_at_refs` were verified separately with `git rev-parse`.
- The driver serializing RR1 first despite all five packages declaring empty dependency sets was
  observed but not investigated. Risk ordering is the documented reason; no evidence here shows it
  is wrong.

## Suggested follow-up

- A regression test for `pce-protect-criteria` covering the boundary cases: `supervision.md` must be
  allowed, `planning/x/vision.md` must be refused, bare `vision.md` must be refused. The hook already
  honors `PCE_TEST_PYTHON`, so it appears designed to be tested.
