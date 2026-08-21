# Brief: /work-graph — supervising a driver run is a skill, not a human chore

Status: GRILLED 2026-08-17, ready to dispatch. Supersedes the draft of the same name.
Written at the end of the second real-world driver run (vision `2026-08-10-incidence-core`),
in which every act below was performed by hand by the human or a supervising session.

This brief covers **two pieces of work with a mandatory order**:

1. **The driver refusal** — `pce` enforces criteria invariance at the plan-version advance.
2. **The `/work-graph` skill** — the supervising agent.

Piece 2 must not ship before piece 1. The skill's whole safety argument is that a supervisor
empowered to reshape a frozen graph cannot weaken what the graph promises, and that argument
is a computation in the binary, not a rule in prose. Building the skill first creates an agent
with the motive and the means to make a blocked run go green by redefining done.

---

## The problem

`/to-graph` ends at a frozen artifact; from there the human is on their own. In the observed
run that meant:

- Hand-typing a six-flag `driver-run` into tmux, five times across three plan versions.
- Hand-typing `render` — once against a stale `graph.v1.json`, earning a version-mismatch
  error — and re-running it repeatedly to see current state.
- Reading raw journal JSON to learn a package had parked twenty minutes earlier.
- Retyping `--repository` / `--prepare` from memory; one launch carried a `taqsim` repository
  that contributes no packages, purely from paste inertia.
- Depending on a second session to capture a worker's tmux pane before it vanished, inspect
  its worktree, and translate `"reason":"replan: missing dependency: IC2"` into something a
  human could rule on.
- Two sessions — the authoring one and the supervising one — each holding half the story.

The driver's outputs are evidence, not communication. Nothing in the loop is a human's job
because it needs a human; it is a human's job because nobody wrote the agent.

---

## Piece 1 — criteria invariance is a driver refusal

### What is true today

`ensure_driver_plan_version` (`src/main.rs:3507`) is the single door every plan-version
advance passes through. It already:

- reads the frozen predecessor `graph.v{n}.json` and successor `graph.v{n+1}.json` from the
  graph's own directory;
- refuses a non-sequential version, and a predecessor whose `vision` does not match;
- refuses to advance while any package attempt is `Running` or `Judging`;
- computes `unchanged_package_ids` and carries completions for packages whose definition is
  byte-identical, invalidating unchanged packages whose dependencies were revised.

It does **not** open `vision.md`, and it does **not** compare the two versions' criteria.

`pce graph check` (`src/main.rs:7452`) reads the graph file and nothing else. The rule stated
in `skills/to-graph/SKILL.md` step 6 — "the union of package criteria covers every vision
criterion not already satisfied by merged work" — is performed by whichever agent authors the
graph and verified by no machine anywhere.

`hooks/pce-protect-criteria.sh` is a Claude Code `PreToolUse` hook. It intercepts `Edit` and
`Write` to a `vision.md` under an active run, constructs the proposed document, and pipes it
to `pce criteria check`, refusing anything that removes, reorders, or changes a ratified
criterion. It fires in Claude Code and nowhere else — the driver already dispatches Codex
workers (`pce dispatch codex`) that never trigger it, and prime-agent
(`/Users/nicolaslazaro/Desktop/thirdparty/prime-agent`) carries its own unrelated
`beforeToolCall` / `afterToolCall` extension points in `AgentOptions`.

### The hole, concretely

A supervising agent, blocked on its third park of the same package, authors plan version 4.
It keeps every criterion's `name` identical so nothing looks dropped, and rewrites one
`command` from the failing test to a narrower one that passes. `pce graph check` prints
`valid: true`. `pce graph freeze` accepts it. `ensure_driver_plan_version` advances. The run
goes green, carries completions forward, and reports done. `vision.md` was never touched, so
the hook never fired.

That is not an agent misbehaving. It is an agent doing exactly what it was asked — adapt the
graph until the goal is reached — with no machine in the loop that can distinguish *reached
the goal* from *redefined it*.

### What to build

Enforce **criteria invariance** inside `ensure_driver_plan_version`, before any advance is
journaled:

> Every criterion present in version `n` survives in version `n+1` with its `name`, `input`,
> `observation`, and `command` byte-identical, in whatever package now owns it.

- **Packaging beneath it is free.** Packages may split, merge, be renamed, be re-edged, be
  added. Which package carries a criterion is a claim about how work is reached, not about
  what is being built.
- **Criteria may be added.** Additions cannot make done easier; this matches the
  additions-only floor already imposed on ratified vision criteria.
- **Refuse a criterion changed, removed, or split into named halves.** The refusal names the
  criterion and the package that held it in version `n`, and states that freezing the revised
  version is the human's ruling.
- The existing package-identity rules for carrying completions are unaffected. A split
  package's prior attempts correctly do not carry: repartitioned work is new work.

`--override-risk-ordering` and every other existing flag keep their meaning. There is no flag
that relaxes this refusal; a flag that turns the floor off is the floor not existing.

Consider also making `pce graph check` read the vision when given one, so the coverage rule in
`to-graph` step 6 stops being prose — but the load-bearing change is the driver refusal, which
holds in any harness because the driver *is* the binary.

### The hooks

Both hooks are artifacts of the pre-graph era, when the orchestrator was a Claude Code session
that dispatched steps and could lose its own memory to compaction. `pce-rehydrate.sh`
re-injects a run snapshot into a context window the graph design no longer has. Once the
criterion floor is a driver refusal, `pce-protect-criteria.sh` is a per-harness duplicate of a
binary-owned verification.

**Do not delete them in this work.** Five repositories have vision directories touched within
five days of writing — `RivRetrieve`, `palaestra`, `hfx`, `pourpoint`, `bluesmith` — carrying
old-orchestrator runs that *are* Claude Code sessions, for which the criterion hook does fire.
Deleting now strips criterion protection from live in-flight work. Hook deletion follows the
retirement of the pre-graph orchestrator and is separate work. See ADR 0016.

---

## Piece 2 — the `/work-graph` skill

`/work-graph <vision-dir>` supervises a driver run end to end.

### 1. Resolve, never guess

Find the highest frozen `graph.vN.json` in the vision directory and use it in every command.
A working `graph.json` newer than the last freeze is reported to the human and never used. The
observed vision directory holds `graph.json`, `graph.json.bak-v1`, `graph.json.bak-v2`,
`graph.v1.json`, `graph.v2.json`, `graph.v3.json` — the stale-version error came from picking
wrong among exactly these.

### 2. Launch from durable configuration

Run flags live in `<vision-dir>/run.json`, captured once on first launch and reused forever.
`/to-graph` does not write it: it does not know local paths or prepare commands, and a file
authored from a guess is how the phantom `taqsim` repository got carried.

The skill derives the repository **names** from the frozen graph's packages and asks only
about repositories the graph actually names. A configured repository absent from the graph is
dropped and reported. This kills the phantom-repository failure by construction rather than by
care.

Launch `pce package driver-run` in a named, reusable tmux session:

```
pce package driver-run --graph <vision-dir>/graph.vN.json \
  --journal <vision-dir>/driver-journal.jsonl \
  --repository <NAME=SOURCE_WORKTREE>... [--prepare <NAME=COMMAND>]... \
  [--override-risk-ordering] [--retry-limit N] [--local-patch-limit N] \
  [--environment-failure-limit N] [--wait-timeout-ms N]
```

`driver-run` is a **foreground loop that exits**: on `Finished` or `Blocked` it prints
`driver-status` and returns (`run_driver_loop`, `src/main.rs:3632`). There is no long-lived
process to supervise. There is a relaunch problem — the observed run exited five times.

The driver opens and cleans up its own tmux panes per worker (`dispatch-pane-opened`,
`dispatch-pane-cleanup`). The skill's tmux session hosts only the driver itself.

### 3. Render continuously

Re-render every ~2 minutes to a stable path so one browser tab is the whole dashboard:

```
pce package render --graph <vision-dir>/graph.vN.json \
  --journal <vision-dir>/driver-journal.jsonl --output <vision-dir>/graph.html
```

### 4. Watch, and interpret

Poll the journal every ~15 seconds. It is well-typed; narration is translation, not scraping.
Event kinds observed in the reference run:

`recovery-configured`, `worker-dispatched`, `dispatch-pane-opened`, `dispatch-pane-cleanup`,
`environment-preparation-executed`, `criterion-executed`, `worker-done`, `gate-finished`,
`finding-replayed`, `package-base-composed`, `package-completed`, `package-parked`,
`package-park-overruled`, `package-join-conflicted`, `worker-environment-failed`,
`plan-version-advanced`.

**Interpret freely while explaining.** The human asked for interpretation, not a faithful
event feed: state theories, name the facts they rest on, label a theory as a theory. A
narrator that reports each event correctly and never notices that the same package has parked
three times with the same complaint is correct at every step and useless across them. In the
reference run, IC6 parked three times in two hours with reasons that restate one complaint.

**Paraphrase while asking.** When a decision reaches the human, restate what the facts say and
stop. Do not argue for an outcome. This asymmetry is the guard against a *laundered decision*
(see `CONTEXT.md`) — the same interpretive power aimed at a question the human must answer is
how a machine's preference gets recorded under a human's name.

### 5. Capture perishable evidence immediately

On `package-parked`, `worker-environment-failed`, or `package-join-conflicted`, capture before
anything is cleaned up, and write it to a supervision file in the vision directory:

- the worker's tmux pane scrollback (`pane_id` and `workspace_id` are on the
  `dispatch-pane-opened` event) — this is urgent: the driver appends `dispatch-pane-cleanup`
  and the pane is gone;
- the worker's worktree ref and `git status`;
- the outcome file and the brief;
- the raw journal event.

The observed `worker-environment-failed` reason was `"package dispatch stopped without a
successful required artifact: Exited { code: ExitCode(1) }"` — no information at all. The real
error existed only in the pane.

### 6. Resolve what preserves the goal

The rule is not "technical versus not". It is **does the goal still get reached**.

**Rule alone, then report afterwards, when evidence at a named ref shows the vision's criteria
still hold:**

- *Refute a worker's spec complaint.* The observed human overrule was itself a finding, every
  clause checkable by reading the composed base:
  > *IC2's work is present in IC6's composed base (60133df) via the IC4/IC5 chains, and its
  > public artifact API suffices for the interpreter; the worker recorded no evidence that an
  > IC2 change is required.*
  An agent with the worktree in front of it could have established that faster than the human
  did. Overrule via `pce package driver-overrule --graph ... --journal ... --package ID
  --rationale TEXT`. The rationale must name the ref and the evidence command, and the journal
  event must identify the agent as its author — an unattributed ruling is the laundered
  decision.
- *Rebuild disposable state.* Re-run a prepare command, delete a corrupt worktree, clear a
  cache. Silent; nothing survives the run.
- *Install a missing tool.* Permitted, reported afterwards, with the exact command recorded in
  the supervision file so it can be undone.
- *Repartition the graph.* Split an untacklable package, merge, re-cut edges, add packages —
  freeze the next version and continue, provided every criterion survives byte-identically.
  Say that a split forfeits that package's carried completions.

**Never modify anything inside a repository under the run.** Never edit a gate command under
any circumstance: a base measurement catches a command that fails and can never catch one that
passes while testing nothing.

**Stop patching on the second identical environment failure.** A recurring environment failure
is usually a *repository-contract defect* — a durable false fact about the repository — and
patching it locally makes the symptom vanish while the defect survives into every future
vision. Bring the recurring fact to the human as real work. Note the observed run's tolerance
was six environment failures before the driver gives up.

**Wake the human for exactly one thing: a criterion that cannot be met as written.** Arrive
with the paraphrase, the park reasons side by side, the drafted and checked revised version,
and the exact `pce graph freeze --vision-dir <vision-dir>` command. The ruling should be
reading one screen, not an hour of archaeology.

An overrule is **one-shot per package**: after an overrule, a second refusal parks with graph
revision as the only exit, because two workers independently refusing a spec outranks one
ruling. A wrong agent overrule does not cost a retry — it spends the cheap door. This is why
the ruling requires evidence at a ref and not a persuasive reading.

### 7. Notify, and stop

Push-notify the human the moment something needs them or the run ends, and stop polling when
the driver stops. Everything narrated is appended to the supervision file **before** it is
said in chat, so nothing important lives only in scrollback and a fresh session resumes from
disk after compaction, sleep, or a kill.

Keep bulk out of the supervising context the way prime-agent's RLM model does: hold working
state outside the context window and delegate anything large — reading a worker's worktree,
diffing a composed base — to a subagent whose output never lands in the supervisor's context.
Take the shape, not the runtime; a state file plus subagents gives the same property.

### 8. Never author, never land

The skill does not write `vision.md`, does not run `/to-graph`, and does not land anything. It
never edits a journal: the append-only record is the only admissible proof, and the observed
un-park performed by deleting three journal events is the failure ADR 0014 exists to prevent.

---

## Shape, tone, installation

- Skills live at `/Users/nicolaslazaro/Desktop/work/pce/skills/<name>/SKILL.md` and are
  symlinked into `~/.claude/skills/` by `./install.sh`. Run `install.sh` **only from the main
  checkout** — running it from a worktree re-points every symlink at that worktree.
- Imitate `skills/to-graph/SKILL.md`, `skills/work-ticket/SKILL.md`,
  `skills/land-ticket/SKILL.md` for shape and tone. Numbered sections, doctrine stated with
  its reason, no restating another skill's normative content.
- The skill's text must never present `pce graph freeze` as something it performs. It prints
  that command for the human. `driver-overrule` it does perform, under section 6.

## Acceptance criteria

| Name | Input | Observation |
|---|---|---|
| Weakened criterion is refused | Freeze `graph.v4.json` identical to v3 except one package criterion's `command` narrowed from the failing test to one that passes; run `driver-run` against it | The driver refuses to advance and names the criterion; no `plan-version-advanced` is appended |
| The floor holds outside the harness | Write that same weakened `graph.v4.json` with a plain shell redirect, using no Claude Code tool and with all hooks removed from settings; run `driver-run` | Refused identically, proving the floor is the binary and not the hook |
| A repartition is admitted | Freeze a version splitting one package into two with every criterion redistributed byte-identically | The driver advances, carries the unchanged packages' completions, and carries none of the split package's |
| A criterion split reaches the human | Freeze a version replacing one criterion with two named halves that cover it | The driver refuses; the supervisor surfaces it as a human ruling with the freeze command |
| The stale graph is unreachable | Invoke `/work-graph` in a vision dir holding `graph.v1/v2/v3.json` and a newer working `graph.json` | Every command issued names `graph.v3.json`; the working draft is reported to the human, not used |
| The phantom repository is dropped | Configure `run.json` with a `taqsim` repository that no package in the frozen graph names | The launched `driver-run` carries only the graph's repositories and states which it dropped |
| Perishable evidence survives cleanup | Force a worker environment failure and let the driver append `dispatch-pane-cleanup` | The supervision file contains that pane's scrollback and the worker's worktree ref |
| An agent ruling is attributable | Read the journal after the supervisor overrules a park | The `package-park-overruled` event names the agent as author and its rationale carries a ref and an evidence command |
| A stop reaches the human | Let the driver exit blocked with no one at the keyboard | A push notification arrives naming the package and the reason, and polling stops |
| The story survives the session | Kill the supervising session mid-run, then invoke `/work-graph` again | The new session's first report names the same park, reason, and evidence as the killed session's last |

Two are checkable only outside the run that delivers them: *the floor holds outside the
harness* needs the hooks already removed from live settings, and *a stop reaches the human*
needs nobody at the keyboard.

## Non-goals, and pending work not to fold in

- Do not delete the hooks (above).
- Do not touch `/to-graph`; authoring ends at the freeze and the two sessions stay separate,
  because in the reference run they held different halves of the story.
- Do not fix rationale-less worker outcomes. Parks arrive unexplained; the skill narrates
  around it today and benefits when it is fixed.
- Do not fix pane cleanup on park and environment-failure paths. Panes currently leak, which
  accidentally preserves evidence; when that is fixed, capture becomes *more* urgent, which is
  why section 5 captures immediately rather than relying on the leak.
- Do not add a spend ceiling. Setting one remains a standing non-goal.

## Reference material

- Worked example: `/Users/nicolaslazaro/Desktop/work/taqsim/planning/2026-08-10-incidence-core/`
  — three plan versions, two parks adjudicated, one composition conflict, five relaunches.
  `driver-journal.jsonl` is 94 events and is the transcript of everything this skill must
  handle.
- `CONTEXT.md`: *Run supervision*, *Goal-preserving resolution*, *Criteria invariance*,
  *Laundered decision*, *Park adjudication*, *Carried completion*, *Enforcement split*,
  *Repository contract*, *Installed surface*.
- ADR 0014 (adjudication enters the journal), 0015 (a conflicted join is the dependent
  worker's first task), 0016 (the criterion floor is a driver refusal), 0017 (the criteria are
  the invariant; the packaging is free).
- `pce` verbs used: `package driver-run`, `package render`, `package driver-status`,
  `package driver-overrule`. Read-only awareness of `graph freeze`, which the skill prints and
  never runs. Binary at `~/.local/bin/pce`, main at `a3d5852` or later.
