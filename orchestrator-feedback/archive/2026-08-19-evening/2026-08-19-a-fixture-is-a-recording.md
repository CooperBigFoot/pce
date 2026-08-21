# PCE workflow feedback: 2026-08-19-a-fixture-is-a-recording

- Date: `2026-08-19`
- Orchestrator: Claude Code (Opus 5), `/work-graph` skill, session `f4b1593b`
- Run: `planning/2026-08-19-a-fixture-is-a-recording` in `/Users/nicolaslazaro/Desktop/work/RivRetrieve`
- Outcome: driver finished (`assembly-completed`); promotion stopped at a terminal check, nothing pushed

## Executive summary

One high-severity defect found in the run's supporting tooling, not in the skill text: the
`criterion_protection` PreToolUse hook refuses Bash on any command whose text contains the
substring `vision.md`, and `supervision.md` contains that substring. Because `/work-graph`
mandates appending to `supervision.md` before every chat message, the protection hook
intermittently blocks the skill's own mandatory bookkeeping step. The failure is cwd-dependent,
which makes it look sporadic rather than deterministic.

No findings yet about the driver itself; it has produced six journal events and is still running.

## Evidence reviewed

- `~/.local/bin/pce-protect-criteria` (hook source, read in full)
- Hook refusal text returned to the Bash tool at approximately 11:40 local
- `planning/2026-08-19-a-fixture-is-a-recording/supervision.md` (partial; append blocked)
- `planning/2026-08-19-a-fixture-is-a-recording/driver-journal.jsonl` (6 events, 1694 bytes)
- `planning/2026-08-19-a-fixture-is-a-recording/graph.v1.json`, `events.jsonl`, `run.json`
- `/Users/nicolaslazaro/.claude/skills/work-graph/SKILL.md` sections 1–5

## What worked

### Frozen-graph resolution is unambiguous

- Evidence: `pce graph check --file planning/2026-08-19-a-fixture-is-a-recording/graph.v1.json`
  returned exit 0 with
  `{"packages":5,"plan_version":1,"refs_verified":false,"valid":true,...}`; `diff graph.json
  graph.v1.json` produced no output.
- Effect: the "unapproved working draft" branch of section 1 resolved in one command pair with no
  judgement call required.

### The empty worker-environment contract is journaled explicitly

- Evidence: first journal record is `{"event":"worker-environment-declared","names":[]}`.
- Effect: the decision to declare no `--worker-env` names is durable run proof rather than a claim
  in a supervision log. A later relaunch that silently added a name would be visible as a changed
  contract.

### `authored_at_refs` gave a checkable launch precondition

- Evidence: `graph.v1.json` records `authored_at_refs.RivRetrieve =
  b8d6deb3b75ff889eac048e23b20359412d53fcb`; `git -C <source worktree> rev-parse HEAD` returned the
  same oid; `package-base-composed` for REC1 then recorded that oid as `base_oid`.
- Effect: three independent sources agreed before any worker ran, and the agreement was cheap to
  establish.

## Friction and failures

### `criterion_protection` refuses Bash on `supervision.md` because of a substring collision

- Severity: high
- Phase: supervision / state persistence (applies to every phase of `/work-graph`)
- Observation: a Bash call appending a section to
  `planning/2026-08-19-a-fixture-is-a-recording/supervision.md` was refused with:
  `REFUSED: Bash may not access vision.md during an active run; use Read for inspection, Edit or
  Write for a proposed change, and pce log --kind criterion-added for additive criteria.`
  The command did not reference `vision.md`. It referenced `supervision.md`.
- Evidence: `~/.local/bin/pce-protect-criteria`, function `handle_bash`:

  ```python
  def handle_bash(payload, root):
      ...
      command = tool_input.get("command")
      ...
      if "vision.md" not in command or root is None:
          return
      pattern = os.path.join(root, "planning", "*", "events.jsonl")
      if any(os.path.isfile(candidate) for candidate in glob.glob(pattern)):
          sys.stderr.write(BASH_REFUSAL + "\n")
          raise SystemExit(2)
  ```

  The test is a bare substring containment check against the entire command string. `"vision.md" in
  "supervision.md"` is `True`. The `Edit`/`Write` branch of `main()` does not have this problem: it
  resolves `file_path` and calls `active_vision()`, which requires `os.path.basename(normalized) ==
  "vision.md"`, so `supervision.md` correctly passes there.
- Observation (second, compounding): the refusal is not deterministic across otherwise identical
  commands. Two earlier appends to the same file succeeded; the third failed. The three differed
  only in the shell's working directory. `project_root()` returns the payload `cwd`, and the glob is
  `<root>/planning/*/events.jsonl`. From inside the vision directory the glob matches nothing and
  the hook returns early; from the repository root it matches
  `planning/2026-08-19-a-fixture-is-a-recording/events.jsonl` and the hook refuses. The harness
  reported `Shell cwd was reset to /Users/nicolaslazaro/Desktop/work/RivRetrieve` immediately
  before the failing call.
- Inference (distinguished from the above): the substring test was almost certainly written to
  catch commands like `cat planning/*/vision.md` and `sed -i ... vision.md` without paying for
  argument parsing. `supervision.md` appears to be an unforeseen collision rather than an intended
  block — the refusal message itself recommends "Edit or Write for a proposed change", which is
  advice about editing a vision, not about a supervision log. I did not find a test covering this
  case, but I did not search the `pce` test suite exhaustively.
- Impact: `/work-graph` section 4 requires appending every narration, theory, question, command,
  and result to `supervision.md` *before* putting it in chat, and section 5 requires appending
  perishable evidence *before* analysis. A supervisor operating from the repository root — the
  documented posture, since section 1 says "Run commands from the repository that owns it" — cannot
  perform that mandatory step through Bash. Worse, section 5's evidence capture is time-critical
  against `dispatch-pane-cleanup`; a refusal there costs the evidence, not just a retry. The
  cwd-dependence means a supervisor may append successfully several times, form the belief that
  Bash appends work, and then lose an evidence write during the one interrupt that mattered.

## Recommendations

### Match the path, not the substring, in the Bash guard

- Addresses: the substring collision finding above.
- Change: in `handle_bash`, replace `if "vision.md" not in command` with a test that requires
  `vision.md` to appear as a path component rather than a suffix of a longer basename. The smallest
  correct form is a regex requiring a start-of-string or non-filename character before it, e.g.
  `re.search(r'(?:^|[^\w.-])vision\.md\b', command)`. This keeps the cheap no-parse design and its
  deliberate over-approximation, and only removes basenames that merely end in `vision.md`.
- Location: `~/.local/bin/pce-protect-criteria`, embedded Python, `handle_bash`.
- Trade-off: still over-approximates (a command mentioning `vision.md` inside an unrelated string
  is still refused), which is the intended conservative posture. Adds one `re` import.
- Confidence: high.

### Add a regression case for `supervision.md`

- Addresses: the same finding; prevents reintroduction.
- Change: a hook test asserting that a Bash payload whose command names
  `planning/<vision>/supervision.md`, with `cwd` set to a repository root containing an active
  `planning/*/events.jsonl`, is allowed — and that the same payload naming `vision.md` is refused.
  The pairing is what makes the test meaningful; either half alone passes trivially today.
- Location: the `pce` test suite covering `criterion_protection`.
- Trade-off: none beyond the test's own maintenance.
- Confidence: high.

### State the hook interaction in the skill, or drop the Bash dependency

- Addresses: the operational surprise, independent of whether the hook is fixed.
- Change: `/work-graph` section 4 could state that `supervision.md` appends go through `Write`/`Edit`
  rather than shell redirection. This is worth doing even after the hook fix, because `Write`/`Edit`
  are the tools the protection design already models precisely, whereas the Bash branch is a
  deliberate approximation that will keep producing collisions.
- Location: `~/.claude/skills/work-graph/SKILL.md`, section 4, near "Append every narration ...
  **before** putting it in chat."
- Trade-off: conflicts with a session-level instruction preferring Bash for file mutation; the skill
  would need to say the hook overrides that preference.
- Confidence: medium. The hook fix is the required change; this is defence in depth.

## No-change decisions

- `recovery-configured` reported limits (`retry_attempts` 1, `local_patch_attempts` 1,
  `environment_failures` 6, `gate_failures` 3) that `run.json` did not set, and `/work-graph`'s
  `run.json` shape has no `gate_failure_limit` field although `pce package driver-run --help`
  documents `--gate-failure-limit <N>`. This is a real gap between the skill's stored launch
  authority and the binary's flag surface, but nothing in this run needed a non-default gate-failure
  limit, so I am recording it rather than recommending a change on zero evidence of harm. It becomes
  a recommendation the first time a run needs that limit and cannot persist it.
- `pce graph check` reported `refs_verified: false` while `events.jsonl` records the human ratifying
  the same file with "`pce graph check` exit 0 with 5 packages and refs_verified **true**". The
  difference is explained by `--repository` being absent from my invocation, which section 1
  prescribes. No defect; noting it only because the two recorded values differ for the same file and
  a future reader could mistake that for tampering.

### The driver never creates the assembly branch section 8 requires

- Severity: high
- Phase: promotion (section 8, "Establish the exact promotion inputs")
- Observation: `/work-graph` section 8 requires resolving "assembly branch
  `pce/<vision>/assembly-v<plan_version>` at the recorded assembly oid", and states that "A missing
  or mismatched ref is a terminal promotion failure, not a reason to reconstruct history." No such
  branch exists after a fully successful run.
- Evidence:
  - `git rev-parse --verify pce/2026-08-19-a-fixture-is-a-recording/assembly-v1^{commit}` →
    `fatal: Needed a single revision`
  - `git for-each-ref --format='%(refname) %(objectname:short)' | grep -i a-fixture-is-a-recording`
    lists exactly ten refs, all attempt branches
    (`refs/heads/pce/<vision>/REC{1..5}/attempt-{1..5}` and
    `refs/heads/pce/<vision>-gate-<g>/REC<n>/attempt-<m>`), and no assembly branch.
  - The assembly commit itself does exist and is well-formed:
    `git log -1 --format='%H parents=%P' ed2ed091f7c7c0b93f94d5268ce2bcc4c62c8e77` →
    `parents=b8d6deb3b75ff889eac048e23b20359412d53fcb 90bce2f1a6f814244603c209a5d9a2fa2235a4bc`,
    subject `Merge commit '90bce2f1…' into HEAD`, epoch-fixed date `2000-01-01`.
  - `driver-status` reported `assembly: {"state":"complete"}` and the journal's final record is
    `assembly-completed`, so the run is exactly the case section 8 is written for.
- Inference (distinguished from the above): the driver composes the assembly as a commit and records
  it as `assembly-repository-composed.base_oid`, but branch creation appears to be assumed by the
  skill rather than performed by the binary. I did not read the driver source to confirm which side
  is meant to own the ref, so I cannot say whether the correct fix is in `pce` or in the skill text.
- Impact: section 10 pushes `<oid>:refs/heads/<name>` refspecs and section 9 names the assembly
  branch in the PR body, so a run that passes every other gate still cannot promote. Any
  `/work-graph` run reaching a clean terminal state hits this. It was masked here only because the
  ancestry check fails first; on a run whose base had not moved, this would be the sole blocker, and
  the prescribed response — treat a missing ref as terminal, never reconstruct — means the
  supervisor must stop rather than `git branch` its way past it.
- Note on the adjacent field: section 8 says to read "the final `AssemblyRepositoryComposed.base_oid`
  for the repository". In this journal `base_oid` holds the *composed result* (`ed2ed091…`), not the
  base the composition started from (`b8d6deb…`, which appears as its first parent). The skill's
  ancestry check is only correct under the reading actually observed here, but the field name invites
  the opposite reading, and a supervisor who took the name literally would compare the fetched
  default branch against the pre-composition base and reach the wrong verdict.

### Base currency is checked at promotion, when it is expensive, and never at launch, when it is free

- Severity: high
- Phase: launch configuration (section 2/3), with the consequence surfacing at promotion (section 8)
- Observation: the run executed to a complete, fully proven `assembly-completed` against a base that
  could never be promoted, because the source worktree's `main` was 36 commits behind
  `origin/main` before the driver ever started. Section 8 detects this correctly — but only after
  the whole run is spent.
- Evidence:
  - `gh pr view 157 --json mergedAt` → `2026-08-13T17:57:29Z`; `git log -1 --format=%cI b9e8a3e0…`
    → `2026-08-13T19:57:28+02:00`. The three newest commits on `origin/main` all date to
    2026-08-13, six days before this run. `origin/main` did not move during the run.
  - `graph.v1.json` was frozen at 11:32 on 2026-08-19 with
    `authored_at_refs.RivRetrieve = b8d6deb…`, which equalled the local `main` HEAD and was already
    36 commits behind the remote default branch at that moment.
  - `git rev-list --left-right --count b9e8a3e0…...ed2ed091…` → `36  16`; merge base `b8d6deb…`.
  - The run that was wasted was not a small one: 107 journal events, five packages, three gate
    findings each with a merged and re-proven repair, and a full assembly criterion sweep.
  - The cost is not recoverable by recomposition.
    `git merge-tree --write-tree --name-only b9e8a3e0… 90bce2f1…` reports four conflicts, two
    structural: `src/rivretrieve/_internal/providers/ca_eccc/fetch.py` deleted on `origin/main` and
    modified by this vision's packages, and `tests/test_usgs_nwis_observations.py` deleted by this
    vision's packages and modified on `origin/main`.
- Inference (distinguished from the above): section 8's ancestry check exists precisely because a
  stale base invalidates a proof. Nothing about that check depends on the run having happened yet.
  It reads as though it were written for the narrow in-flight-movement race, and the far more common
  case — a worktree that was simply never pulled — falls through to the same terminal failure at the
  most expensive possible moment.
- Impact: one complete run discarded. The failure mode is silent at launch: `git status` reports the
  worktree as clean and healthy, `pce graph check` passes, `authored_at_refs` matches HEAD exactly,
  and every one of those signals agrees while the base is six days stale. A supervisor has no
  prompt to look further. This supervisor did in fact observe "36 behind" at launch, reported it to
  the human, and classified it as something that "matters only at promotion" — the skill gave no
  basis for treating it as a blocker, and section 8's placement actively suggests it is a
  promotion-time concern.
- Note: section 1 requires re-resolving the frozen graph before every command, and section 3
  requires re-checking launch preconditions on relaunch, so the skill is already comfortable with
  cheap repeated verification. Base currency is the one precondition it verifies exactly once, at
  the end.

### Repair attribution requires byte-exact survival of the repair hunk, and the driver's own resolution worker breaks it

- Severity: high
- Phase: assembly gating, plan version 2
- Observation: the plan-version-2 driver aborted with exit status 1 and
  `Error: could not attribute unconstructable repair 7a34e785f972a7ab9716f68b8ba50b0e5ef801a6 for REC3`,
  after the resolution worker had successfully closed every conflict and after nine assembly
  criteria — including the REC3 amendment being attributed — had all executed and passed.
- Evidence:
  - Pane exit `pane_dead_status = 1`. `driver-status` afterwards reports `outcome: running`,
    `assembly: {"state":"gating"}`: the driver died mid-assembly without recording a terminal state.
  - Nine `assembly-criterion-executed` records, all `{"kind":"exited","code":0}`. The last one
    before the abort is REC3's `gate:package-gate-3-1:finding:0` —
    `uv run pytest tests/test_boundary_probe_harness.py -k probe_that_does_not_replay` — exit 0.
    The very repair the driver then declined to attribute had just been re-proven against the
    composed whole.
  - Not a missing or unreachable ref: all three carried witness refs and all three repair refs are
    reachable ancestors of the resolved base `7056e1e4…` (`git merge-base --is-ancestor` true for
    each of `9000e93c…`, `1f4d6f75…`, `de81203f…`, `28630a10…`, `7a34e785…`, `90bce2f1…`).
  - The repair touches `src/rivretrieve/_internal/boundary_probes.py` and
    `tests/test_boundary_probe_harness.py`. Between the v1 assembly and the v2 resolved base the
    test file is byte-identical and `boundary_probes.py` differs by
    `1 insertion(+), 5 deletions(-)`.
  - No package commit caused it:
    `git log --oneline 7a34e785…..90bce2f1… -- src/rivretrieve/_internal/boundary_probes.py`
    returns nothing. The resolution worker introduced the change, in a file outside the four
    conflicted paths it was dispatched to fix.
  - The change in full is a generator expression collapsed onto one line:

    ```diff
    -        return tuple(
    -            request
    -            for request in self._declared_requests
    -            if request not in self._replayed_requests
    -        )
    +        return tuple(request for request in self._declared_requests if request not in self._replayed_requests)
    ```

    Parsing both forms and comparing ASTs gives `AST identical: True`. `line-length = 120` is
    unchanged across the two bases and the collapsed line is 103 characters, so this is `ruff
    format` doing exactly what the repository configures it to do — the worker's own report line
    reads `Ruff format and lint: passed`.
- Inference (distinguished from the above): attribution appears to be implemented as byte- or
  hunk-exact recovery of the repair's diff in the composed tree, rather than as "the amendment
  criterion passes against the composed whole". Under that implementation any semantically neutral
  reformat defeats it. I did not read the attribution code, so I cannot confirm the mechanism —
  only that reachability is satisfied and byte-identity is not, and that the run died on exactly
  that difference.
- Impact: a run that had genuinely succeeded was aborted. Every package was complete, every carried
  amendment re-proven so far, all conflicts resolved, and the full suite green on the resolution
  commit (`1616 passed, 2 skipped`). The failure is also self-inflicted and repeatable: the driver
  dispatches a resolution worker, that worker runs the repository's mandated formatter, and the
  formatter's output then fails the driver's own attribution check. Any project whose lint
  configuration would reformat a gate repair hits this on every conflicted assembly.
- Second-order impact: because the driver aborts rather than parking, `driver-status` is left
  reporting `running` on a dead driver. A supervisor that trusted the status over the pane's exit
  code would conclude the run was still in progress and wait indefinitely.

### Recommendations

### Attribute a repair by re-proving its criterion, not by matching its bytes

- Addresses: the finding immediately above.
- Change: treat a carried amendment as attributed when its criterion command executes successfully
  against the composed tree. That evidence already exists in the journal at the moment the driver
  aborts — `assembly-criterion-executed` for `gate:package-gate-3-1:finding:0` with exit 0 is the
  record immediately preceding the error. If a byte-level check is wanted as a supplementary
  signal, it should downgrade to a journaled warning rather than abort the run.
- Location: `pce` driver assembly gating, repair attribution.
- Trade-off: a criterion can pass for reasons unrelated to the repair surviving, so this is a weaker
  guarantee than hunk recovery. It is the guarantee the frozen criteria actually encode, and the
  gate authored that criterion precisely to detect the regression the repair fixes.
- Confidence: high that the current check is wrong; medium on this being the right replacement.

### Fail by parking, never by aborting mid-assembly

- Addresses: the second-order impact above.
- Change: an unattributable repair should emit a typed event (`assembly-attribution-failed` or a
  park) and let the driver record a terminal state, so `driver-status` reports something other than
  `running` for a process that has exited. The current behaviour makes a dead driver
  indistinguishable from a live one by status alone.
- Location: `pce` driver assembly gating error path.
- Trade-off: none identified; this is strictly more information in the journal.
- Confidence: high.

### Constrain the resolution worker to the conflicted paths

- Addresses: the root trigger, independently of how attribution is fixed.
- Change: the resolution worker was dispatched to resolve four named paths and additionally
  reformatted a fifth file that was not in conflict. Either scope its writes to
  `conflicted_paths`, or have it skip repository-wide format/lint passes. Its brief should say that
  resolving a conflict is not an invitation to normalise the surrounding tree.
- Location: `pce` assembly resolution worker brief.
- Trade-off: the resolved commit may then fail a repository's format check, which some projects
  gate CI on. That is a visible, fixable failure, unlike silently rewriting proven repair hunks.
- Confidence: medium — this may be deliberate, since a merge that leaves the tree unformatted is
  also a defect. If so, the fix belongs entirely in attribution.

### Check base currency before launch, not only before push

- Addresses: the finding immediately above.
- Change: add to section 3, before the first `driver-run` launch, the same check section 8 performs:
  `git fetch <remote> <default-branch>` in each configured source worktree, then require the fetched
  default-branch oid to be an ancestor of the graph's `authored_at_refs` oid for that repository.
  On failure, stop before launching and report the two oids and the divergence count, exactly as
  section 8 does. The remedy at that point is a `git pull --ff-only` plus a mechanical re-freeze,
  both of which are human actions and both of which are nearly free before a run.
- Location: `~/.claude/skills/work-graph/SKILL.md`, section 3, before "Build an argument vector".
- Trade-off: adds one network round-trip per repository per launch, and will refuse launches that
  today start and sometimes succeed — specifically, runs whose packages happen not to collide with
  what landed on the default branch. That refusal is the point, but it does convert a
  sometimes-works case into an always-stop case, so the message must name the `git pull --ff-only`
  remedy rather than leaving the human to infer it.
- Confidence: high.

### Have `/to-graph` refuse to freeze against a stale base

- Addresses: the same finding, one step earlier in the pipeline.
- Change: the real defect is upstream of `/work-graph` — a graph should not be frozen with
  `authored_at_refs` pointing at a commit the remote default branch has already passed. Either
  `pce graph freeze` should fetch and refuse, or `/to-graph` should require the check before
  presenting the freeze command. `pce graph freeze` already takes `--repository NAME=SOURCE_WORKTREE`
  and so already has everything it needs to perform it.
- Location: `pce graph freeze` ref verification, or `~/.claude/skills/to-graph/SKILL.md`.
- Trade-off: makes freezing depend on network reachability, which is a new failure mode for an
  operation that is currently purely local. An escape hatch would be needed for deliberate offline
  or historical freezes, and any such flag would need to be journaled so the staleness is visible
  later rather than silently accepted.
- Confidence: high on the diagnosis; medium on `pce graph freeze` being the right home, since that
  is the one place a mechanical freeze is supposed to be definition-preserving and local.

### Decide which side creates `pce/<vision>/assembly-v<N>` and make it do so

- Addresses: the missing assembly branch finding.
- Change: either have the driver create the branch at the composed oid when it emits
  `assembly-repository-composed`, or add an explicit `assembly-branch-created` style event the skill
  can read. If the skill is meant to own it, section 8 must say so and must stop calling a missing
  assembly ref a terminal failure — those two clauses cannot both stand.
- Location: `pce` driver assembly composition, or `~/.claude/skills/work-graph/SKILL.md` section 8.
- Trade-off: creating the branch in the driver adds a repository mutation to a component that
  otherwise only writes refs for attempts; that is a small widening of its write surface.
- Confidence: high that the current pair is inconsistent; medium on which side should change.

### Rename or document `assembly-repository-composed.base_oid`

- Addresses: the naming note above.
- Change: `base_oid` holds the composed result. Either rename it (`composed_oid`, `assembly_oid`) in
  a future journal schema version, or state in section 8 that despite its name it is the assembly
  commit and that the pre-composition base is its first parent.
- Location: `pce` journal schema, or `~/.claude/skills/work-graph/SKILL.md` section 8.
- Trade-off: a rename is a journal-schema change affecting replay of existing journals; the
  documentation-only fix costs nothing but leaves the trap in place.
- Confidence: medium.

## Suggested follow-up

- Whether the driver should be able to recompose an assembly onto a moved default branch without a
  new freeze is a real design question this run surfaced, but it needs a decision rather than a
  report finding. Note that it would not have helped here: the conflicts are structural
  (two modify/delete pairs), so recomposition alone cannot produce a promotable tree. Suggested as a
  separate vision or issue.

## Post-fix outcome (appended 2026-08-19, after the fix landed)

The attribution fix and the resolution-worker scoping landed and were confirmed installed by digest
(`sha256 64a45a98…`, size 10783408, resolved through the `~/.local/bin/pce` symlink to
`pce/target/release/pce`). Relaunching against the unchanged `graph.v2.json` produced
`outcome: finished`, `assembly: {"state":"complete"}`, pane exit status 0, journal ending in
`assembly-completed`.

The attribution finding is **resolved and verified in practice**: the former fatal abort now appears
as a typed `repair-credit-stale` record (`package REC3`, `gate package-gate-3-1`, `finding 0`,
`repair_ref 7a34e785…`, `lineage_oid 7056e1e4…`, `reason "counterfactual-unconstructable"`), the
run continued past it, and `driver-status` reports REC3's amendment with `repository_refs: []` —
byte-exact credit dropped, criterion retained and proven by execution. Cold resume worked: the
driver resumed at REC4, the exact abort point, without re-running the nine already-green criteria.
All 12 assembly criteria across the plan-version-2 pass are exit 0.

Two notes for the fix's authors:

1. **The absent-assembly-branch finding below is now confirmed blocking, not moot.** It was
   originally recorded as masked by the ancestry failure. With the base corrected and attribution
   fixed, it is the sole remaining obstacle: `git rev-parse --verify
   pce/2026-08-19-a-fixture-is-a-recording/assembly-v2^{commit}` → `fatal: Needed a single
   revision`, on a run that reached `Finished`. A fully proven assembly cannot be promoted.
2. **Resolution-commit anchoring did not cover this commit.** The install report described
   resolution commits as now anchored under `refs/pce-assembly-resolutions/*`. For assembly oid
   `7056e1e4…`, `git for-each-ref` matching that namespace returns nothing, and
   `git for-each-ref --points-at 7056e1e4…` returns only the supervisor's manually created retention
   tag. Theory, not established: anchoring applies to resolutions the fixed binary creates, and this
   one was produced pre-fix and resumed over rather than re-created. If so, a resumed run leaves its
   resolution commit unanchored, which is worth handling since resume is exactly the path a
   post-fix upgrade takes.

## Disposition

Ruled by the human on 2026-08-19: fix repair attribution (the first recommendation under the
attribution finding), with constraining the resolution worker folded in as a secondary fix. Both are
in flight in the `pce` repository, merged into one fix brief together with an identical-principle
defect another run hit the same day — stale repair credit after a rebuilt lineage, and the same
missing abort event. That second run's independent arrival at the same principle is corroboration
that this is a general defect in how repair credit survives a rebuilt lineage, not a quirk of this
vision's conflict.

Hand-promotion was offered as an option and refused: the journal proof is the product, so an
assembly that the driver did not certify is not promotable by other means. The vision holds at plan
version 2 until the fixed binary lands by the standard land-install-confirm path. `graph.v2.json`
is correct and stays frozen; no new plan version is required. The resolution commit `7056e1e4…` is
retained (it was dangling — see the vision's `supervision.md` for the retention tag and its undo).

The remaining findings in this report — base currency checked only at promotion, the absent assembly
branch, and the `base_oid` naming trap — were not part of this ruling and remain open.

## Correction to an earlier draft of this report

An earlier revision of this file described the promotion stop as `origin/main` advancing "while this
graph was in flight", and listed it under no-change decisions as the workflow behaving correctly
against an unavoidable race. That was wrong. `gh pr view 157 --json mergedAt` places the merge at
2026-08-13, six days before the run; the remote default branch never moved. The stale base was
present at freeze time and observable at launch. The finding "Base currency is checked at promotion,
when it is expensive, and never at launch, when it is free" replaces that no-change decision, and it
is the highest-impact item in this report.
