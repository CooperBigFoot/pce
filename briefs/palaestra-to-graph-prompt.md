We are converting an in-flight vision into a work-package graph. Its vision lives at:

  /Users/nicolaslazaro/Desktop/work/palaestra/planning/2026-08-09-gridded-statics-self-name-to-every-consumer

I have rebuilt the software I use to develop and implement visions. PCE no longer decomposes a
vision into milestones and steps driven by an orchestrator agent. It now turns a vision into a
work-package graph, and a driver in the binary walks that graph: it composes each package onto its
dependencies' completed work, dispatches prime-agent workers concurrently, executes each package's
acceptance criteria as real shell commands in detached clones, sends a gate to attack the built
artifact, and finally assembles every package and re-runs all criteria against the assembly.
Afterward a supervising skill (`/work-graph`) can promote a finished assembly mechanically.

Run `/to-graph` pointed at that directory. That skill does the whole job in one go: it reads the
vision, works out what has already landed, partitions the remaining acceptance criteria into work
packages, writes each criterion as an executable command, types the dependency edges, checks the
graph mechanically, renders it as an artifact for me to look at, discusses it with me until it is
settled, and only then freezes it.

## This vision spans three source repositories and one external data location

- `/Users/nicolaslazaro/Desktop/work/palaestra` — Python, uv. Currently at `c702f65`, clean.
- `/Users/nicolaslazaro/Desktop/work/hdx` — Rust, cargo. Currently at `a21e5fc`, which IS this
  vision's milestone 2 merged via PR #21 on its remote.
- `/Users/nicolaslazaro/Desktop/work/orthographos` — Rust, cargo. Currently at `d521471`
  (`fix(reissue): attest manifest-only dataset shape (#71)`), and it carries 8 local branches
  named for this vision — recent merges there are re-issue capability work from this vision.
- `/Users/nicolaslazaro/Desktop/work/camels-trust` — an external mutable data-project location,
  NOT a repository and NOT a package owner. Registration, rebranch, and the C2 rerun happen there
  through the tethys, metis and palaestra CLIs; the vision's constraints section says exactly how
  (pre/post hashes, no commits there).

Each source repo has a tracked contract at `.pce/repository-contract.json`; read all three. The
vision's constraints block also names repos that must NOT own packages (`camels-us`, `metis`,
`temenos`) — respect that.

## The old run got far; establish ground truth before partitioning

The previous milestone/step run reached milestone 4 step 2 before the workflow was retired.
Milestone 2 (hdx spec + validator side) is demonstrably merged; much of the orthographos re-issue
capability appears merged through its recent PRs. Do not trust `pce status` merge summaries —
establish what actually landed from git and GitHub in all three repositories, and state plainly
which vision acceptance criteria are already satisfied by merged work and are therefore carried by
no package. Getting this wrong in either direction is expensive: claiming something landed that did
not leaves a hole, and rebuilding something that did wastes a package.

## Hard-won authoring rules from the last conversion (violating these cost that run four parks)

- **Every named reference needs an owning package.** If model or file metadata names a thing
  (a band description convention, a dataset version, a validator rule), some package's criteria
  must own making it real. A dangling name discovered mid-run stalls the whole graph.
- **`depends_on` must cover direct API consumption, not just build order.** Workers refuse to
  consume an upstream package's API when the edge is undeclared — they park with
  "missing dependency" rather than crossing a boundary. If package B calls what package A built,
  declare the edge even when A's work would be transitively present anyway.
- **Criteria are bets.** Most test files a criterion names will not exist yet; making them exist
  and pass is the worker's job. A criterion may never be satisfied merely by a test existing.
- **Sibling packages editing one crate will conflict at the join** (e.g. both adding a module
  declaration). That is now handled — the dependent's worker resolves the conflict as its first
  task — but partition to minimize gratuitous overlap anyway.

## Things specific to this vision to think through and discuss with me

- **This is the first genuinely multi-repository graph.** The graph schema has a single
  `authored_at_ref`, which cannot say which repository a ref belongs to — this is a known open
  limitation. If it bites, report it as a finding and propose how you worked within it; do not
  work around it silently.
- **The re-issue and everything after it are expensive and hard to reverse** (a ~19 GB
  copy-on-write re-issue, registration of an immutable v3, rebranching a live calibration
  project). The edge vocabulary has a `safety` kind for exactly this — an irreversible act binds.
  Think about which edges are safety edges, and remember the vision's own constraint that disk,
  time and network are measured and reported before the re-issue executes.
- **Criteria run as real commands in detached clones of the repos**, but several vision criteria
  observe the external data location. Write those criteria so the command is executable from a
  repo clone (CLIs with absolute paths are fine) and so re-running it is safe — the driver
  executes criteria more than once (package time, possibly join time, assembly time). A criterion
  whose re-execution would mutate v3 or double-register it is wrong; probe/verify commands must
  be read-only against the data location.
- **The red-first regression constraint** (composition check demonstrated failing on the pre-fix
  writer) may already be satisfied by merged work — check before carrying it into a package.

## Things to know

- Convert from where the work stands, not from the beginning.
- Do not run the old `/pce` orchestrator on this vision, and do not modify `events.jsonl` or the
  milestone directories; they are history.
- When the graph runs, each repository gets its own preparation command (`uv sync` for palaestra,
  `cargo fetch` for the Rust repos, or whatever the contracts say). State what you would use for
  each; I need them on the command line and they will be persisted into the run's `run.json`.
- I want to collaborate on this. Expect me to ask questions about the rendered graph and to push
  back. Answer them plainly and fix anything my questions expose. Do not ask me to approve edge
  kinds, the partitioning, or how criteria are worded — those are yours.

## Report back with

- What the graph says is being built, and what it deliberately does not cover.
- Which vision criteria are already satisfied by merged work in each repository, and how you
  established that.
- The dependency spine, which edges are safety edges and why, and where the parallelism is (if
  any — the vision itself says the core is a hard chain).
- The exact `pce package driver-run` command that would start the run, with a `--repository` and
  `--prepare` flag for every repository the graph actually names — and none it does not.
- Anything the multi-repository shape made awkward in the tooling, as findings to report rather
  than problems you quietly absorbed.
