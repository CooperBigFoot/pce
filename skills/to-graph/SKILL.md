---
name: to-graph
description: Turn the current conversation into a vision and a frozen work-package graph. Writes vision.md, authors the graph, checks it mechanically, renders it, discusses it with the human until it is settled, and freezes it as an immutable plan version. Use for `/to-graph "<name>"` to start from the conversation, or `/to-graph <vision-dir>` to convert an in-flight vision from where it currently stands.
---

# Author a work-package graph

A **work package** is the largest unit whose acceptance criteria can be written as executable
commands. A **graph** is the set of packages and their typed edges. A **plan version** is a frozen
graph, immutable for its lifetime.

This skill takes a conversation all the way to a frozen plan: it writes the vision, authors one
graph, settles it with the human, and freezes it. It implements nothing, dispatches nothing, and
resolves nothing.

Run every command from the repository the vision belongs to.

## 1. Establish the vision and its current state

`$1` is either a vision name or an existing vision directory.

**If it is a name, or absent and the conversation has reached shared understanding, materialize the
vision first.** Invoke and follow the sibling `to-vision` skill to create the directory and write
`vision.md` against its fixed template. Do not restate or reimplement that template here; it is
normative there, and a second copy of it drifts. Return here once `vision.md` has passed its check.

Never invent a vision from a thin conversation. If the grill has not settled what is being built,
say so and stop — a graph authored over an unsettled vision decomposes a guess.

**If it is an existing vision directory**, read `vision.md` with the Read tool. A hook refuses Bash
access to it during an active run. Extract the acceptance-criteria block, the scope, the
constraints, and the explicit non-goals.

Determine what has already landed:

```bash
pce status --file <vision-dir>/events.jsonl --vision-dir <vision-dir>
```

An in-flight vision is converted **from where it stands**, not re-authored from the beginning.
Work that has merged is out of scope for the new graph. Say explicitly which criteria are already
satisfied by merged work and exclude the packages that would have carried them.

Nodes present in the event log but absent from any approved graph are runtime delta stubs. They
carry real work and no durable representation. Fold that work into the package it belongs to and
discard the stub; never reproduce it as a package.

## 2. Read each repository's source at its ref

The graph is grounded in what each repository's code is, not in what the vision says about it.
Before partitioning, resolve and record one exact commit OID for every repository the graph will
name. Read each repository at its own recorded OID for every claim the packages will depend on —
the modules a criterion names, the interfaces an edge will cite, and the paths a package will
delete. Never use one repository's ref as the ground for another repository.

A package or an edge that cannot cite a repository, a file, and that repository's recorded ref is
not yet authored. This is the descent obligation and it is not optional. The repository names
recorded here must match the names in package `repositories` and the keys in `authored_at_refs`
exactly.

## 3. Partition the acceptance criteria

**Packages come from the criteria, never from the scope list.** The scope list states intent; only
a criterion states runnability. Authoring from the scope list reproduces the vision's own
narrative order and is the failure this step exists to prevent.

For every acceptance criterion, ask exactly one question:

> What must exist before this criterion can be run?

Group the criteria by that answer. Each group is a candidate package.

Rules that decide the hard cases:

- **A criterion may split.** If half of a criterion is testable against fixtures and half needs
  real inputs, it is two criteria in two packages. Name the halves. A criterion held whole because
  splitting is awkward defers a check that could have run early.
- **Every package owns at least one criterion.** A package with none is not a package; it is scope
  that belongs inside another one.
- **Every criterion lands in exactly one package**, or in named halves across two. An orphaned
  criterion means the partition is incomplete.
- **Literal reading wins.** If a criterion names a provider, a file, or a command, that name is a
  requirement. It frequently reorders work relative to the vision's own suggested sequence, and the
  criterion is right.
- **Fixtures beat real inputs.** A criterion runnable against a committed fixture belongs to an
  earlier package than one needing a real source, even when the vision groups them together.
- **Every name needs an owner.** If any package's data or metadata names a thing — a file format
  detail, a binding, a dataset version, a validation rule — some package's criteria must own
  making it real. A name whose owner is nobody is discovered as a mid-run park: the first worker
  that needs the named thing refuses to invent it. The incidence run stalled three workers on one
  unowned name before the graph was revised to add its owning package.

Give each package an id, a title, the repositories it touches, and its criteria.

## 4. Write each criterion as a command

A criterion carries `name`, `input`, `observation`, and `command`. The `input` and `observation`
pair is what a human reads. The `command` is what the driver runs, and its exit status decides
whether the package is complete.

A criterion with no command that could be written is a defect to report, not a criterion to
approximate. Do not invent a weaker command that passes. Say which criterion cannot be expressed
and what would make it expressible — an injectable probe, a controlled environment, a seam that
does not exist yet — and let step 6 fail on it.

**Every command re-executes.** The driver runs a criterion command at package judgement, again for
each parent when a conflicted join is re-verified, again against the final assembly, and again
after any plan-version carry-forward. A command must therefore be re-runnable and read-only toward
anything outside its clone: a verb that refuses to run twice, or whose second run would mutate or
double-register external state, can never be a criterion command. When the observed thing was
produced by a one-shot act, the package must deliver a read-only re-verification verb and the
criterion runs that — recomputing the observation, never reading a receipt.

**Author for the driver's failure model.** A worker process can die *after* performing its act but
before reporting; the driver records an environment failure and dispatches a fresh worker
automatically. For any package containing a hard-to-reverse act — minting an immutable artifact,
registering, publishing — the criteria's `input`/`observation` prose must license the successor to
resume by verification: finding the act already performed and provably correct is success to
attest, not an obstacle to fail on. The license must also require a successor to establish that no
predecessor process is still writing before failed attestation permits removal or repetition. Use
the act's completion report and status file as the natural evidence: an absent or stale terminal
status together with a running predecessor means WAIT, not permission to remove or repeat the act.
Only after the predecessor is known to have stopped may failed attestation authorize the recovery
action named by the package. A one-shot act with no stated race-safe resume protocol turns the
driver's ordinary retry into a guaranteed park or makes it race work still in flight.

## 5. Derive the edges, typed

For each ordered pair of packages, decide whether an edge exists and which kind it is. There are
exactly three kinds and only two of them bind the scheduler.

**`buildability`** — B does not compile or run until A has merged. Must name the code fact: the
symbol imported, the interface called, the path removed. Forced.

**`safety`** — B performs an act that cannot be undone, and A must exist to protect it. Must name
the irreversible act. Nothing about B fails to build; the cost of getting the order wrong is
unrecoverable. Forced.

**`risk-ordering`** — B follows A so a defect is discovered cheaply. A deliberate choice, not a
constraint. The scheduler may override it when throughput matters more than rework. Must state
what is learned by ordering it this way.

Do not use `buildability` for an ordering you merely prefer, and do not use `risk-ordering` for
anything whose violation destroys data. Conflating the three is how a scheduler runs an
irreversible act in parallel with the thing that was supposed to protect it.

Omit transitively implied edges — with one exception. **Declare `buildability` for direct API
consumption even when the provider is transitively present.** Workers are boundary-disciplined:
one whose package consumes an upstream package's API without a declared edge refuses and parks
with a missing-dependency claim rather than crossing the boundary. If package B calls what
package A built, the edge exists whether or not A's work would already be in B's composed base.
Two of the incidence run's parks were exactly this.

## 6. Write the graph, then run the mechanical check

Author criterion commands from commands you have executed successfully against the named input.
Never write a command because a tool or flag is expected to exist. Recorded evidence must establish
that the exact command surface exists before it becomes a criterion. A negative-form criterion must
first assert that its tool exists, for example `test -x ./tool && if ./tool verify; then exit 1; fi`;
otherwise a missing tool can skip the `then` branch and false-pass the criterion.

A frozen criterion is immutable for every machine role. Only an explicit human-ratified revision at
a later freeze may edit or remove it, and that ruling forfeits the affected package's carried
completion. Gate-earned amendments still carry as criteria into the fresh attempt; repair commits
from the forfeited attempt do not carry into its rebuilt lineage. This skill never authors or
supplies a criterion revision record.

Write the draft to `<vision-dir>/graph.json` in the shape given in step 8. It is a draft until it is
frozen; nothing reads it as authority yet.

```bash
pce graph check --file <vision-dir>/graph.json \
  --repository <NAME>=<ABSOLUTE_PATH> \
  [--repository <NAME>=<ABSOLUTE_PATH> ...]
```

Supply exactly one path for every repository named by the graph. The check resolves each
repository's authored ref in that repository. A missing path, unknown path name, or unresolvable
ref is one graph error naming the repository and ref. The driver repeats this preflight before it
can dispatch work or spend an environment-failure attempt.

The graph is not reviewed for taste. It is checked against a predicate that terminates:

1. Every criterion has a `command`.
2. Every package owns at least one criterion.
3. Every criterion, or named half, lands in exactly one package.
4. Every edge declares a kind, and carries the justification that kind requires — a code fact for
   `buildability`, an irreversible act for `safety`, what is learned for `risk-ordering`.
5. The graph is acyclic.
6. The union of package criteria covers every vision criterion not already satisfied by merged
   work.

On failure, name the offending package or criterion and the rule it broke, correct it **once**, and
re-run the check. If it fails again, stop and report what cannot be expressed. Do not iterate. A
predicate that needs three rounds is not being checked, it is being negotiated.

## 7. Render it, then settle it with the human

Render with the binary rather than by hand, so the picture is deterministic and identical to what
the driver will later show:

```bash
pce package render --graph <vision-dir>/graph.json --output <vision-dir>/graph.html
```

Publish that file as an artifact and give the human the link.

Then **discuss it, and expect more than one round.** This is the last cheap moment before work
starts, and every serious defect found in a graph so far was found by a human looking at the drawn
picture rather than by anything reading the JSON — a missing safety edge that would have permitted
building on an unvalidated layout, and a dependency stated more strongly than its criterion needed.

Two rules keep the discussion honest, and they are not in tension:

**Do not ask them to ratify technical content.** Never ask whether an edge kind is right, whether
the partition is correct, or whether a criterion is well formed. Those are the predicate's job, and
a human approving them launders a machine decision into a human one. Open with what the graph says
is being built and what it leaves out — not with a request for validation.

**Answer everything they ask, and act on what it exposes.** A question is not ratification. When the
human asks why two packages are separate, or why one thing waits for another, explain it plainly. If
the answer reveals a real defect, fix it, re-run the check, re-render, and say what changed. That is
the human finding a bug, not the human approving a design — the opposite of the failure the first
rule guards against.

Iterate until the human is settled. **The one-correction limit in step 6 governs the mechanical
check, never this conversation.** A predicate that needs three rounds is being negotiated; a human
who needs three rounds is being served.

State plainly, unprompted, what the graph does **not** cover: criteria already satisfied by merged
work, work deliberately excluded, and anything the check could not express.

Surface any `risk-ordering` edge on or near the critical path as an open choice with its cost both
ways. It is the one thing on the page that is a decision rather than a fact, and it is theirs.

## 8. Freeze

Only once the human is settled. For plan version 1, or a later version with no criterion changes,
freeze normally. A later draft that edits or removes frozen criterion bytes will refuse unless the
human independently writes and explicitly supplies the exact attributed revision record. Do not
create that record for them. A mechanical freeze, when available, is definition-preserving and can
never include a revision record.

The frozen shape is:

```json
{
  "vision": "<vision-dir-name>",
  "plan_version": 1,
  "authored_at_refs": {
    "RivRetrieve": "5e2bf7fc51177f63ca670218accbc27ba8d8c4ec"
  },
  "packages": [
    {
      "id": "RR1",
      "title": "The shared store reader",
      "repositories": ["RivRetrieve"],
      "criteria": [
        {
          "name": "Unrecognised manifest is refused",
          "input": "Hand-edit a valid store's manifest to declare an unknown format version, then query it",
          "observation": "The read refuses and names the rebuild command; no download begins",
          "command": "uv run pytest tests/store/test_manifest_refusal.py"
        }
      ],
      "depends_on": []
    },
    {
      "id": "RR2",
      "title": "The certification harness",
      "repositories": ["RivRetrieve"],
      "criteria": [
        {
          "name": "Verification catches a mutated store",
          "input": "Alter one staged value after writing and before verification",
          "observation": "Read-back fails and the staged store is not published",
          "command": "uv run pytest tests/store/test_certification.py -k mutated_store"
        }
      ],
      "depends_on": [
        {
          "id": "RR1",
          "kind": "buildability",
          "reason": "the read-back comparison calls the shared reader's query path"
        }
      ]
    }
  ]
}
```

`authored_at_refs` has exactly one entry for every repository named by any package and no extra
entries. Each key is the exact repository name used in package `repositories`; each value is the
exact commit OID recorded while reading that repository in step 2. Use this map even for a
single-repository graph. Never author a new graph with the legacy scalar `authored_at_ref`. Frozen
legacy graphs that carry that scalar remain readable: the scalar applies to every repository they
name. Do not rewrite one merely to change its shape.

`kind` is one of `buildability`, `safety`, `risk-ordering`. Freeze it:

```bash
pce graph freeze --vision-dir <vision-dir> \
  --repository <NAME>=<ABSOLUTE_PATH> \
  [--repository <NAME>=<ABSOLUTE_PATH> ...]
pce log --file <vision-dir>/events.jsonl --kind planning-artifact-approved --node graph
```

Freeze repeats repository-ref resolution; it never freezes a map whose repository set and
authored-ref key set differ. Repository names supplied here and to the driver must be byte-for-byte
identical to `authored_at_refs` keys.

A frozen plan version is immutable. A structural change, including a change to any repository's
authored ref, mints version `n+1` by re-entering step 3 for the affected region; it never edits a
frozen graph in place. A ref-only change does **not** invalidate an otherwise unchanged completed
package. The journal's recorded `base_oid` preserves the exact historical ground; assembly
recomposes the carried package branches at the new repository ref and re-runs every effective
criterion. Re-render every new version. Notify the human only when the new version changes **which
vision criteria are covered** — that is checkable, and it is the only deviation they need to see.

## 9. Resolve nothing

Never implement a package, dispatch a worker, run a criterion command as if it were the driver, or
mark a package complete. Leave the graph frozen and unstarted.

Conclude at intent altitude: what the vision now says it is building and which package is worth
taking first, in plain language, and the command that would run it. That command's `--graph` names
the frozen artifact — `graph.v<N>.json` — never `graph.json`; the working copy is never run. Give
it one `--repository` and one `--prepare` flag for every repository the graph names, and none for
any repository it does not. Then, marked as skippable: the artifact URL, the vision and graph
paths, the frozen plan version, the ready set, the edge kinds used, and anything the check could
not express.
