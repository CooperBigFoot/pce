# Program layer

The Program layer tracks a large, multi-vision idea above the unchanged
single-vision PCE workflow. Its Map issue is the sole durable Program state;
every invocation reconstructs context from GitHub issues and committed
repository context rather than a Program state file, persistent chat, or
Program CLI verb.

## Lifecycle

1. **Chart.** `/chart-program "<idea>"` surveys the idea breadth-first. After a
   complete review gate, it creates or fully re-surveys the Program Map, creates
   only currently sharp Effort tickets, records real ordering dependencies,
   retains unresolved territory as Fog, and identifies the actionable,
   unblocked Frontier. Chart resolves or claims no ticket and does not decompose
   the Program into implementation milestones or steps.
2. **Work.** `/work-ticket <n>` selects and claims one Effort ticket by assigning
   it to the authenticated GitHub user. It reconstructs cold-session context
   from the complete ticket, the Map's `Notes` and `Decisions so far`, and the
   committed root [`CONTEXT.md`](../CONTEXT.md). It then runs one deep
   Grill-with-docs session toward one ambitious, contained vision. After
   convergence, Work reserves a vision directory with
   `pce vision new "<name>"`, writes exactly one root-level
   `Vision: planning/<YYYY-MM-DD>-<slug>` line on the ticket, and gives the user
   the exact `/to-vision "<name>"` invocation. Work does not invoke
   `/to-vision` or `/pce` automatically and does not write `vision.md`.
3. **Land.** After that vision's independent `/to-vision "<name>"` then `/pce`
   delivery cycle finishes, `/land-ticket <n>` validates the ticket's `Program:`
   and `Vision:` linkages and consumes the binary-owned landing result, which
   proves executed criteria, merge state, and event evidence without asking the
   human to assert delivery. It restates only the linked vision's `Goal / Why`
   and `Scope — In` as one plain-language destination question and asks one
   combined command-and-observation question for each unpaid criterion. Every
   such observation is required before any GitHub mutation. It closes the
   Effort ticket on either destination answer; a negative answer is recorded and
   minted through the reviewed proposal as named Effort work. Land appends or
   reconciles the ticket's one-line landed decision in the Map's `Decisions so
   far`, links applicable committed ADRs, and runs the complete reviewed
   Fog-graduation Grill-with-docs session. Newly sharp Fog becomes lean Effort
   tickets with real blocking relationships; unresolved territory stays Fog.
   After re-fetching state, Land considers the Program done only when no open
   member Effort tickets remain and the Map's Fog is empty. It then presents a
   final summary and asks separately before closing the Map.

Each Effort ticket owns an independent `grill -> /to-vision -> /pce` delivery
cycle. The Program layer selects visions and carries context between them;
`/pce` remains responsible for milestone and step decomposition within one
vision.

## GitHub state model

The single open Map issue labeled `pce:program` is the sole durable Program
state. Discover it with exactly:

```bash
gh issue list --label pce:program --state open
```

Only one open Program is allowed per repository. `/work-ticket` and
`/land-ticket` require exactly one result and stop if there are zero or more
than one. `/chart-program` follows this trichotomy:

```text
ZERO open programs => create a new map; EXACTLY ONE => full re-survey of that existing map; MORE THAN ONE => clear error and stop.
```

The Map body has this normative structure:

```markdown
## Destination
<!-- Describe the big idea's end state. -->

## Notes
<!-- Record durable program context. -->

## Decisions so far
<!-- Maintain the running one-line decision index that landed tickets append to. -->

## Not yet specified
<!-- Keep the fog of un-sharpened future tickets here. -->

## Out of scope
<!-- List explicit non-goals. -->
```

Effort tickets are labeled `pce:ticket`. They are vision-sized questions, not
implementation tasks, and point back to the Map with exactly one root-level
`Program: #N` line. Their normative seeded-but-lean body is:

```markdown
## Question

What single question does this ticket resolve?

## Scope sketch

- Add 2–4 bullets describing the likely scope.

Depends on: #M

Program: #N
```

In a real ticket, replace the instructional question and bullet with one
ambitious, contained question and two to four lean scope bullets, and replace
`Program: #N` with the actual Map number. Include `Depends on: #M` only for a
real fallback dependency; omit it for an unblocked ticket. Prefer native GitHub
issue blocking relationships when the supported GitHub CLI or API can create
and query them. If native blocking is unavailable, use the reviewed edge in the
blocked ticket body with the exact fallback line `Depends on: #M`, where `M` is
the blocking Effort ticket number. A fallback line must never be represented as
a native edge.

Fog is acknowledged Program territory that is not yet sharp enough to state as
one contained, vision-sized question. It lives only in the Map's
`Not yet specified` section. Frontier is the current set of open, actionable
Effort tickets whose blocking dependencies are satisfied; it excludes Fog and
blocked tickets. The Map preserves what is knowable now, so it is neither an
implementation decomposition nor a predetermined roadmap or total ordering.

## Vocabulary and decision propagation

Program context survives cold sessions through complementary channels. The
Map's `Decisions so far` section is a concise cross-ticket index. Land adds one
physical Markdown line in this shape:

```markdown
- #<n> — <one-line landed decision>
```

When the ticket produced an applicable ADR, its repository link is appended on
the same line. The index carries the outcome, not the ADR's rationale.

Committed [`CONTEXT.md`](../CONTEXT.md) carries canonical terms, aliases to
avoid, relationships, and material ambiguities. Committed `docs/adr/` records
only consequential, durable, surprising decisions that arose from genuine
trade-offs. Grill-with-docs combines one-question-at-a-time interviewing with
the domain-modeling discipline and updates these sources during discovery, not
as an afterthought.

GitHub issues remain the sole durable **Program state**. `CONTEXT.md` and ADRs
are durable committed domain knowledge; they complement but neither replace nor
duplicate the Map.

## Installation

[`install.sh`](../install.sh) builds and links the `pce` binary, then symlinks
and verifies all seven skill directories in `~/.claude/skills/`: `pce`,
`to-vision`, `domain-modeling`, `grill-with-docs`, `chart-program`,
`work-ticket`, and `land-ticket`. The five newly wired and verified
Program-layer support skills are `/chart-program`, `/work-ticket`,
`/land-ticket`, `grill-with-docs`, and `domain-modeling`, alongside the existing
`/pce` and `/to-vision` skills.

## Boundaries

The unchanged set is exactly:

- `/pce`
- `/to-vision`
- the seven-section vision template
- `Cargo.toml`
- the Rust CLI and Rust sources

In contrast, `install.sh` changed to wire and verify the new Program-layer
skills.
