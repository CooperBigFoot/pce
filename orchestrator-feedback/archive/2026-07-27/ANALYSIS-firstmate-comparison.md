# firstmate vs. PCE — mechanism analysis

Source repo: `/Users/nicolaslazaro/Desktop/thirdparty/firstmate/`
Contrast repo: `/Users/nicolaslazaro/Desktop/work/pce/`

## 1. What firstmate is, in 5 sentences

**Correction to the brief first:** firstmate does *not* decompose a vision into milestones and steps. Its unit of work is **one dispatched task** — `ship` (produces a project change) or `scout` (produces a report at `data/<id>/report.md`) — handed to one autonomous agent in a disposable git worktree; there is no graph, no `depends_on`, no decomposition artifact anywhere in the repo.

Its state model is **a directory of small files**: `state/<id>.meta` (durable facts), `state/<id>.status` (append-only *wake-event* log, explicitly *not* current state), `state/.wake-queue` (durable TSV queue written *before* detector state advances), and `data/backlog.md` (the queue of work items) — all crash-safe, greppable, and reconstructible with no parse step.

It enforces quality by **outsourcing it entirely** to `no-mistakes`, a separate product (a git-proxy push gate that runs AI review/fix/test/lint/document/CI in its own worktree and opens the PR); firstmate itself contributes only the *authority boundary* around that pipeline (`/Users/nicolaslazaro/Desktop/thirdparty/firstmate/.no-mistakes.yaml`, `bin/fm-gate-refuse-lib.sh`) and the merge gate.

The determinism split is aggressive and explicit: **bash owns all state, liveness, classification, and destructive-action refusal; the model owns only intake, routing, authority judgment, and human-facing translation** (`bin/fm-crew-state.sh` header: *"The determinism lives entirely here — only run-step / pane / log reads plus fixed mapping logic, no heuristics and no LLM"*).

Roughly 90% of the ~92 `bin/` scripts and ~90% of its git history are terminal-multiplexer/harness plumbing (tmux, herdr, zellij, orca, cmux × claude, codex, grok, pi, opencode, kimi), not workflow logic.

## 2. Mechanism inventory

### State / progress persistence

- `/Users/nicolaslazaro/Desktop/thirdparty/firstmate/AGENTS.md` §2 — one file per fact, one owner per file. `state/<id>.meta` is `key=value` lines appended by whoever learns the fact (`fm-spawn` writes `window=/worktree=/harness=/kind=/mode=`; `fm-pr-check.sh` appends `pr=`/`pr_head=`; `fm-x-link.sh` appends X fields). No single document any agent rewrites wholesale.
- **The event-log/current-state split is the load-bearing idea.** `state/<id>.status` is append-only and holds only *supervisor-actionable transitions*. `AGENTS.md` repeats four times: "a status line is a wake event, not current-state truth." `/Users/nicolaslazaro/Desktop/thirdparty/firstmate/bin/fm-crew-state.sh` is the only current-state authority.
- **Durable wake queue**: `/Users/nicolaslazaro/Desktop/thirdparty/firstmate/bin/fm-wake-lib.sh` — `epoch<TAB>seq<TAB>kind<TAB>key<TAB>payload` in `state/.wake-queue`, written *before* the detector advances its suppression markers, so a missed process exit is recoverable by draining (`docs/architecture.md:14`).
- **Versioned read-only projection**: `bin/fm-fleet-snapshot.sh --json` emits schema `fm-fleet-snapshot.v1`; `fm-fleet-view.sh` and `fm-bearings-snapshot.sh` both consume that one contract instead of re-parsing raw state.
- **Restart is a non-event by design** (`AGENTS.md` §5): "durable state and live backend inventory, not conversation memory, are authoritative." Session start is one composed digest (`bin/fm-session-start.sh`) with a hard **read-once** rule.

### Agent handoff

- One mechanism: a scaffolded brief file. `/Users/nicolaslazaro/Desktop/thirdparty/firstmate/bin/fm-brief.sh` writes `data/<id>/brief.md` from a fixed template with `{TASK}` as the only hole. The scaffold — not prose instruction — carries the safety contract: worktree-isolation assertion, status protocol, escalate-on-second-obstacle, definition-of-done shaped by delivery mode. `AGENTS.md` §11: "The scaffold is a safety contract, not a suggestion." It **refuses to overwrite** an existing brief.
- Steering is *one line* through `bin/fm-send.sh`, which **fails closed unless `FM_HOME` is explicit** so a steer cannot land in a sibling home. Long instructions must go in a file.
- Return channel is never chat: `AGENTS.md` hard rule 4 — crewmates never address the human; secondmates answer through a status line carrying a `corr=<id>` correlation token, with detail in a doc the status line points at (`bin/fm-pending-reply-lib.sh` owns correlation/recovery/escalation/retention).
- Operational vs. human input is **structurally distinguishable**: `bin/fm-operational-input.sh` prefixes machine injections with U+2063 + `FIRSTMATE_OP: ` — an untypable marker, so a human can never forge one and the agent never mistakes a daemon escalation for a user message.

### Verification gates

- Quality gates are `no-mistakes`', not firstmate's. firstmate's contribution is the **authority boundary**: `.no-mistakes.yaml` sets `disable_project_settings: true`, honored *only from the trusted default-branch copy* — a pushed branch cannot enable its own project instructions during its own validation. Independently, `bin/fm-gate-refuse-lib.sh` makes `fm-spawn`/`fm-send`/`fm-teardown` exit 3 inside a gate environment so a gate agent can never drive the fleet.
- `bin/fm-lint.sh` is the **single owner** of the lint definition (file set + config + *pinned shellcheck version*, refuses to run under any other) and both CI and the pre-push gate invoke it, so local and CI cannot diverge (asserted by `tests/fm-lint.test.sh`).
- `bin/fm-test-run.sh --check-coverage` is a partition proof: portable shards + serial lane + Herdr lane must equal the full inventory, so a test cannot fall out of CI silently.
- Deliberate anti-gate rule (`AGENTS.md` §7): "Never hold work outside no-mistakes for a manual clean verdict, stack serial manual reviews, or infer authority for one from security, architecture, or risk alone." One rigor owner per path, no bolt-on reviewers.

### Failure / retry / escalation

- Escalation triggers live in the *brief*, not the supervisor: "If you hit the same obstacle twice, append `blocked:` and stop." **Repetition, not a round cap.**
- Vocabulary distinguishes `paused:` (bounded external wait, expected, absorbed and rechecked hourly) from `blocked:` (supervisor must act). Both are keyed and must be closed by a same-key later state or `resolved [key=…]` (`bin/fm-classify-lib.sh`).
- `bin/fm-crew-state.sh` reconciles: if the log's last line says `needs-decision` but the pipeline run moved on, the log is **deterministically flagged superseded** — no LLM guessing whether an old escalation is still live.
- Wedge escalation is *counted*: repeated stale wakes on an unchanged provably-working pane add an escalation count and, at `FM_WEDGE_DEMAND_INSPECT_COUNT`, a `demand-deep-inspection` marker.
- Recovery playbook is a skill with a hard precondition: `/Users/nicolaslazaro/Desktop/thirdparty/firstmate/.agents/skills/stuck-crewmate-recovery/SKILL.md` — before relaunch, *prove* no live agent owns the recorded task and the worktree is still available; "do not use a fresh generic spawn while the recorded worktree is unaccounted for, because allocating another worktree can split one task across two copies." Also: "A low context reading is not wedging."
- **Decision-hold lifecycle** — the strongest mechanism here. `/Users/nicolaslazaro/Desktop/thirdparty/firstmate/.agents/skills/decision-hold-lifecycle/SKILL.md` + `bin/fm-decision-hold.sh`: every unresolved human decision discovered by an investigation becomes a **keyed, idempotent, structured backlog hold** (`<origin-id>-decision-<key>`) that *blocks* dependent work. `complete` is a mandatory attestation gate (`--none` is an explicit semantic assertion, not a default), `verify` is called by teardown so teardown *cannot erase the source before the gate passes*, and `resolve` requires the decision be written to a file and every routed task exist and be blocked by the hold before the hold closes. The script deliberately never reads report prose to infer decisions — the agent does the semantic inventory, the script owns identity and ordering.
- **ask-user-authority** (`/Users/nicolaslazaro/Desktop/thirdparty/firstmate/.agents/skills/ask-user-authority/SKILL.md`): an 8-step procedure for the gray zone between "correction within accepted intent" (autonomous) and "contract expansion" (escalate), with the sharpest rule in the repo: *"Treat labels such as correctness, security, fail-closed, high-risk, or required as evidence about the finding, never as authority to broaden the task"* and *"Repeated same-theme findings require escalation before another Fix when incremental corrections are preserving a questionable abstraction rather than closing independent defects."* Plus: the implementation worker never answers its own finding.

### Context grounding

- Weak, and firstmate's weakest axis relative to PCE. There is no ref-based grounding: crewmates are told to trust their worktree. Grounding is instead *placement discipline* — `AGENTS.md` §6 routes each fact to its most specific owner (project `AGENTS.md` / `data/captain.md` / `data/learnings.md` / backlog item / scout report), and **inspect-then-update** is mandatory: read the destination, rewrite or prune in place, never append by default. `/stow` is a session-end sweep that routes uncaptured knowledge to disk.
- Absence is signalled explicitly: the session digest prints `ABSENT` markers, never conflating a missing file with an empty one, because absence has different meaning per file.

### Preventing an agent from lying about success

Three independent mechanisms, all deterministic:

1. **Never trust the agent's own status line.** `bin/fm-crew-state.sh` attributes a pipeline run to the crew only if the run head equals the worktree HEAD or HEAD is an ancestor of it — *branch name alone is insufficient*, so a historical run on a reused/rewritten branch cannot be claimed as current success. Local work that advanced past or diverged from the run head invalidates attribution.
2. **Landed-work proof before destruction.** `/Users/nicolaslazaro/Desktop/thirdparty/firstmate/bin/fm-teardown.sh` refuses cleanup unless work is provably landed: reachable from *any* remote-tracking branch, **or** its PR is merged and GitHub reports a head containing the local work, **or** its content is already in the up-to-date default branch. `gh` failure falls back to the content check; still inconclusive ⇒ **refuse**. Uncommitted changes are never landed. Scout tasks carve out only after the report exists *and* the decision-hold gate passes.
3. **Content-binding for anything the watcher will execute.** `bin/fm-check-register.sh` writes a sha256 of the exact bytes; the watcher executes a custom check only against its registered hash, and PR polls are byte-identical copies of a repo script with task data in a private sidecar, never interpolated into shell source.

Plus a cultural rule with teeth: `AGENTS.md` §8 "never report an unchanged fleet as progress"; §9 "empty polls, elapsed time, and no-change updates are not captain-facing progress."

### Hooks / CI enforcement — the most transferable part

- `/Users/nicolaslazaro/Desktop/thirdparty/firstmate/.claude/settings.json`: `SessionStart` nudge; two `PreToolUse` Bash hooks; one `PreToolUse` `.*` hook; two `Stop` hooks.
- **subagent guard** (`bin/fm-subagent-pretool-check.sh`, `docs/subagent-guard.md`) — the best-documented incident in the repo. On 2026-07-22 the primary ran four workers through Claude's built-in subagent tool instead of `fm-spawn.sh`; no `state/<id>.meta` was created, so the in-flight count stayed at 0, **so the turn-end guard's in-flight branch became structurally inert** and supervision stayed down 73 minutes unnoticed; two workers died on restart. The doc's conclusion is the reusable insight: *"No additional guard keyed on task metadata can catch this class of failure, because the failure is precisely the absence of that metadata."* The fix denies by **tool-name shape** (stem list: `agent subagent task workflow cron schedul worktree delegate spawn dispatch handoff remote sendmessage monitor`) with a `.*` matcher so future tool names are caught, and a deliberate allowlist for *observe/stop* tools so a runaway task can always be killed.
- **cd guard** (`bin/fm-cd-pretool-check.sh`, `docs/cd-guard.md`) — denies a `cd`/`pushd`/`popd` that persists to the parent shell, because a stray `cd projects/foo` once caused a home-owned backlog write to execute inside a project clone. Allows `git -C`, `make -C`, absolute paths, subshells. Inert in linked worktrees.
- **arm guard** (`bin/fm-arm-pretool-check.sh` + 38 KB `bin/fm-arm-command-policy.mjs`) — rejects command *shapes* (background operator, pipeline, redirection, wrapper) that would hide watcher-arm failure. Transport fails open on malformed input; semantics fail closed.
- **turn-end guard** (`bin/fm-turnend-guard.sh`, `docs/turnend-guard.md`) — "no turn ends blind": when work is in flight and no identity-matched watcher has a fresh beacon, block the Stop (exit 2) or force one bounded follow-up. Budgeted (`FM_CLAUDE_TURNEND_BLOCK_BUDGET`, default 3, below Claude's 8-block override) then allows degraded with a visible message.
- **CI as contract enforcement**: `.github/workflows/no-mistakes-required.yml` fails any PR to `main` whose body lacks the deterministic `no-mistakes` signature, with a per-event immutable concurrency group so a later body edit cannot collapse an earlier pending check. Bots exempt.

## 3. Ideas worth stealing, ranked by expected value

Ranked against PCE's own recorded failures in `/Users/nicolaslazaro/Desktop/work/pce/orchestrator-feedback/`.

**1. Filesystem-first result channel with queue-before-advance.**
*Problem:* the highest-severity recorded finding — `orchestrator-feedback/2026-07-27-no-d8-raster-lies-silently.md:68` — "Subagent final-message delivery failed completely and silently"; both Phase 0 Explore agents went idle with no report and a filesystem channel was improvised for all fourteen later dispatches. firstmate never reads an agent's result message: the artifact is a file, the notification is a separate durable queue record written *before* state advances.
*Steal:* make the delegation contract's "Output format" clause mandate a **path** for every Claude subagent verdict (`review-<n>.json`), and make the orchestrator read the file with an explicit halt-if-absent — never the result message. `codex exec` already does this via `-o`; the asymmetry is the bug.
*Cost:* ~10 lines in `/Users/nicolaslazaro/Desktop/work/pce/skills/pce/SKILL.md` §"The delegation contract". Near-zero. **Do this first.**

**2. Event-log ≠ current-state, with a deterministic derived-state reader.**
*Problem:* `orchestrator-feedback/2026-07-27-no-d8-raster-lies-silently-2.md:160` "`state.json` counters drifted from reality." `state.json` is one document the orchestrator rewrites from memory, so it is exactly the artifact most likely to encode a hallucinated success. firstmate splits the two and forbids inferring state from the log tail.
*Steal:* split `state.json` into (a) append-only decision/event records the orchestrator only ever *appends* to, and (b) node status **derived on read** from git and `gh` (branch exists? merged into `milestone-<m>`? tag points at HEAD?) rather than stored. A `pce state <vision-dir>` verb — the Rust CLI already exists — makes this cheap and un-fakeable.
*Cost:* one CLI verb + a `state.json` shape change; touches resume logic. Medium, highest structural payoff.

**3. Landed-work proof before any destructive step.**
*Problem:* `gh pr merge --delete-branch` colliding with an active worktree is recorded **twice** as a recurrence (`2026-07-27-no-d8-raster-lies-silently.md:156`, `2026-07-27-post-2019-deep-learning-directions.md:152`), and Phase 3 step 6 removes the worktree and deletes the branch on the orchestrator's own say-so.
*Steal:* `bin/fm-teardown.sh`'s three-way proof, reduced to PCE's shape — before `git worktree remove` / `git branch -d`, require: PR reports merged **and** the squash commit's content is reachable from `milestone-<m>`; otherwise refuse and escalate. Inconclusive ⇒ refuse, never proceed.
*Cost:* ~15 lines in Phase 3 step 6, or a `pce` subcommand. Small, and it closes a *recurring* defect.

**4. Decision-hold lifecycle for escalations.**
*Problem:* `2026-07-27-post-2019-deep-learning-directions.md:116` "The escalation terminal state is undefined." `state.json.escalations[]` is an append-only list with no close condition, no key, and no blocking edge.
*Steal:* firstmate's model verbatim — each escalation gets a stable key, is idempotent on retry, **blocks** the dependent node, and can only close when (i) the human's answer is written to a file and (ii) every dependent node is named and unblocked. Make the "done" check refuse while any hold is open.
*Cost:* moderate — schema addition + a routing rule; mostly discipline, not code. High value for unattended runs.

**5. Escalate on repetition, not on a round cap.**
*Problem:* recorded in three separate reports: "Step-plan cap of 3 fires on convergence, not on stuckness" (`2026-07-26-tile-count-independent-planetary-cog-reads.md:69`), "Four artifacts consumed exactly the 3-round cap" (`2026-07-27-no-d8-raster-lies-silently.md:139`), "Cap exhaustion occurred exactly where the prior report predicted it" (`2026-07-27-post-2019-deep-learning-directions.md:107`). firstmate has **no round cap**; the trigger is "the same obstacle twice" plus a *counted* wedge escalation on an unchanged surface.
*Steal:* invert the cap and stuck-detector — make the stuck-detector (substantially identical `blocking_issues`) the primary escalation trigger and raise/remove the numeric cap. This is the change PCE's own evidence most directly supports.
*Cost:* a few lines in §"Routing, caps, and adaptation". Small.

**6. PreToolUse hooks for invariants the model repeatedly violates.**
*Problem:* the recurring traps in the feedback reports are all mechanically identifiable command shapes — `--delete-branch`, `codex exec` without `< /dev/null` (stdin hang, rediscovered by failure), backtick substitution under zsh. Prose in a 283-line SKILL.md is the wrong enforcement layer for a command shape.
*Steal:* a `PreToolUse` Bash hook in the target repo's `.claude/settings.json` that denies `gh pr merge` containing `--delete-branch`, and denies `codex exec` with neither `< /dev/null` nor a pipe into it. Note firstmate's transport rule: **fail open on malformed hook input, fail closed on a semantic match** — a broken hook must not deny every Bash call.
*Cost:* ~40 lines of bash + a settings block. Small, and it converts three recurrences into zero.

**7. Phase 0 must execute the gates it records.**
*Problem:* `2026-07-26-tile-count-independent-planetary-cog-reads.md:78` "Phase 0 recorded acceptance gates it never executed." firstmate's equivalent guarantee is structural: `bin/fm-lint.sh` is the single owner invoked by *both* CI and the local gate, so a recorded command that does not work cannot survive.
*Steal:* require the orientation sweep to **run** each `format/lint/typecheck/test/build` command at the orientation ref and record its exit status in the contract; a command that was never executed is not eligible to be a gate.
*Cost:* one clause in Phase 0 + orientation wall-clock. Small; kills a whole defect class (plans quoting commands that cannot pass).

**8. Repo contract as a tracked file in the target repo, honored only from the default branch.**
*Problem:* the best-praised mechanism (`2026-07-27-no-d8-raster-lies-silently.md:61`, `repo_contracts.pourpoint.version_bump.orchestrator_ruling` overriding the skill's mandatory version-bump) is discovered fresh every run and lives in gitignored `state.json`, so it is re-derived (or missed) every time. firstmate's `.no-mistakes.yaml` is tracked, and *trusted only from the default-branch copy* so a branch cannot relax its own gate.
*Steal:* a committed `.pce.yaml` in each target repo carrying `commands`, `version_bump` policy, and an `environment_hazards` list (`2026-07-26-tile-count-independent-planetary-cog-reads.md:132` asks for exactly this slot), read only from the default branch; Phase 0 verifies rather than derives.
*Cost:* a new file format + Phase 0 change. Medium; compounding return across runs.

**9. Versioned read-only projection instead of ad-hoc state reads.**
`fm-fleet-snapshot.v1` gives every consumer one schema. PCE has no equivalent — every resume re-reads `state.json` by convention.
*Steal:* a `pce status --json` with a schema id. *Cost:* small. *Value:* moderate; mostly pays off when more skills read run state.

**10. `metadata.internal` + one-owner discipline for PCE's own skill set.**
`/Users/nicolaslazaro/Desktop/thirdparty/firstmate/.agents/skills/firstmate-coding-guidelines/SKILL.md` exists because `AGENTS.md` grew 585→958 lines from conditional detail added inline. It ships a 7-tier knowledge-placement tree, a one-owner rule ("every contract stated in full exactly once; every other mention is a one-line cross-reference"), and an inline-stub pattern. `/Users/nicolaslazaro/Desktop/work/pce/skills/pce/SKILL.md` is at 283 lines and already duplicates the repos-block YAML shape into `/Users/nicolaslazaro/Desktop/work/pce/skills/to-vision/SKILL.md` with a hand-written "normative if this ever conflicts" disclaimer — precisely the drift the one-owner rule forbids.
*Cost:* discipline only. *Value:* moderate now, high in six months.

## 4. Deliberate divergences — and who is right

| Axis | firstmate | PCE | Better |
|---|---|---|---|
| **Authorship / gating split** | One crewmate writes *and* validates via an external pipeline; no adversarial separation inside firstmate | Codex authors, Claude gates, orchestrator owns git — different models, structurally unable to approve their own work | **PCE, decisively.** The feedback reports show 17 pre-merge catches in one run and zero escapes. firstmate has no equivalent evidence because it has no equivalent mechanism. |
| **Ground truth for delegates** | "You are in a worktree at detached HEAD"; the checkout *is* the truth | Every dispatch names an exact ref + the `git show <ref>:<path>` command; a ref/checkout mismatch is explicitly *not a finding* | **PCE.** Single largest advantage; firstmate has nothing comparable. Keep it. |
| **Verdict encoding** | Free-text `verb: prose` status lines + a bash verb classifier | JSON `--output-schema` with six required keys | **Both, at different layers.** PCE's schema is right for gates. firstmate's cheap verb line is right for *wake triage* — schema-parsing every heartbeat would be absurd. Neither should adopt the other. |
| **State store** | Many small append-only files, one writer per fact | One `state.json` the orchestrator rewrites | **firstmate.** PCE's own feedback records counter drift; a document an LLM rewrites from memory is the least trustworthy artifact in the system. |
| **Loop termination** | Repetition-triggered ("same obstacle twice") + counted wedge escalation, no numeric cap | Numeric caps (3) + stuck-detector | **firstmate**, per PCE's own three-report evidence that the cap fires at convergence. |
| **Agent persistence** | Crewmates live in tmux windows with durable state and survive an orchestrator restart | Subagents die with the session; only `state.json` survives | **firstmate for long runs.** The paused multi-day run survived via `state.json`, which worked — but in-flight subagent work does not. |
| **Human contact** | Chatty supervisory relationship, whole etiquette layer, `/ahoy`, `/bearings` | Escalate only on `BLOCK`, cap exhaustion, or `root_cause: vision` | **PCE.** firstmate's §9 is ~20 lines of forbidden-vocabulary mapping. |
| **Concurrency admission** | Overlap is "a risk signal, not a reason to wait"; serialize only on true semantic dependency | Parallelize only on **disjoint `files_touched`** | **firstmate is more permissive and probably right**, but only because its worktrees are fully isolated and its merge path reconciles rebases. PCE's disjointness rule is cheap and safe; the version-bump serialization carve-out is the same insight. Not worth changing. |
| **Where the workflow lives** | An "agent distro" — a repo you clone and launch a harness inside | Installed skills + a small CLI | **PCE.** firstmate's model means the workflow and the user's private fleet state occupy one directory, which is why `.gitignore` is doing safety work. |

## 5. Anti-patterns — do not import

1. **The persona tax.** `/Users/nicolaslazaro/Desktop/thirdparty/firstmate/AGENTS.md:7` — "Address the user as 'captain' at least once in every response … This is mandatory respectful address" — plus nautical seasoning rules, plus §9's ~20-line forbidden-vocabulary translation table (`teardown → cleanup`, `worktree → local copy`, `fail-closed → stops safely when something goes wrong`). Not just ceremony: the translation table **actively degrades** evidence-first reporting, which is exactly the property PCE's gates depend on. PCE's reports cite exact paths, refs, and sha256 values; firstmate forbids relaying them.
2. **52 KB always-loaded instruction file.** Paid by every session and every turn of every fleet member. firstmate's own guidelines skill exists as an apology for it. `skills/pce/SKILL.md` at 283 lines is loaded only on `/pce`. Do not converge.
3. **A control plane that violates its own YAGNI rule.** `AGENTS.md:238` — "Do not build wrappers, control planes, policy layers, custom verifiers, or automation unless the direct path exposes a concrete blocker." The repo is 92 bin/ scripts including `fm-supervise-daemon.sh` (72 KB), `fm-spawn.sh` (69 KB), `fm-fleet-snapshot.sh` (68 KB), `fm-teardown.sh` (54 KB), `fm-pr-check-migrate.sh` (43 KB). Steal the *mechanisms* above at 20–40 lines each; do not steal the surface area.
4. **The backend × harness matrix.** Five session backends (tmux, herdr, zellij, orca, cmux) × six harnesses (claude, codex, opencode, pi, grok, kimi), each with its own busy-signature, composer classifier, submit path, and verification doc. `git log --oneline | head -30` is ~25 commits of adapter plumbing and near-zero workflow improvement. This matrix *is* the project now.
5. **Terminal-scrollback state inference.** `bin/fm-composer-lib.sh`, ANSI-aware composer classification, pane busy-signature matching, "Kimi's rotating idle tip `ctrl+c: cancel` borrowing Grok's busy token" (`docs/architecture.md:98`). PCE reads structured verdict files from disk. Never trade that for screen-scraping.
6. **The check-execution security model.** Byte-hash trust records, device and link-count validation, `.pr-check-quarantine/`, `.pr-check-migration-scan-v1` markers, identity-bound retirement receipts, 43 KB of non-executing migration code — a supply-chain threat model for a single-user local tool. The *idea* (hash-bind anything you will execute) is worth one line; the implementation is not.
7. **`fm-arm-command-policy.mjs`** — 38 KB of shell tokenizer with ANSI-C and locale-quoting decoders, whose own doc concedes "the seatbelt's threat model is agent mistakes: no one accidentally writes an ANSI-C-obfuscated watcher path." It then builds the decoder anyway and adds a tripwire clause about when to delete it. Take the *hook shape*, not the tokenizer.
8. **X mode.** An autonomous agent that answers public Twitter/Discord mentions and posts up to three unattended public follow-ups per task within a 7-day window. Roughly a dozen `state/x-*` artifacts, a relay protocol, dry-run outbox, split-reply budgets, and carry-count recovery. Wrong direction for anything PCE does.
9. **Secondmates.** A second, isolated firstmate home per domain, with inherited-config allowlists (`fm-config-inherit-lib.sh`, 44 KB), cross-home liveness sweeps, correlated pending-reply records, and guarded fast-forward version convergence. This is a distributed-systems problem invented to avoid a queue. PCE's Program layer solves the same "many efforts, one context" problem with GitHub issues and no daemon — that is the better answer.

---

**If you take one thing:** #1 (file-based subagent verdicts with halt-if-absent) closes the highest-severity recorded failure for ~10 lines.

**If you take one structural thing:** #2 (derive node status from git/`gh`; make `state.json` append-only) removes the artifact most able to encode a hallucinated success — which is the entire point of firstmate's `bin/fm-crew-state.sh`, and the mechanism PCE most conspicuously lacks.
