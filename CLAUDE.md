# Claude Code Guidance

Follow `AGENTS.md`.

Claude Code is one supported environment for PCE. It installs and uses all six skills:

- `grill-me` for canonical intent discovery;
- `to-vision` for vision authoring, publication, and verified handoff;
- `implement-vision` for root implementation, delegation, PR execution, independent review, and Effort delivery records;
- `chart-program` for approved Program Maps and Effort tickets;
- `grill-ticket` for one explicit Effort's discovery and vision;
- `land-ticket` for delivery verification, Effort landing, Map updates, and explicitly approved Program completion.

Use the standalone workflow for one coherent idea:

```text
grill-me → to-vision → implement-vision
```

Use the Program workflow for a large outcome:

```text
chart-program → grill-ticket → implement-vision → land-ticket
```

All six PCE skills are available in every supported environment: Claude Code, Codex, and Prime Agent. Any environment that can spawn subagents, run shell commands, read and write files, and use Git and GitHub can run either complete workflow without changing environments. Implementation planning, delegation, PR execution, independent review, and Effort delivery records belong to the root implementing agent through `implement-vision`.

## Durable handoffs

For tracked work, invoke the root workflow as `implement-vision <Effort number or canonical URL>` or pass its linked `planning/visions/<vision>.md` path. A number uses the current repository. A canonical URL supplies its repository identity. An Effort-derived path recovers that same identity from canonical provenance and still runs the full ticket validation. A path without Effort provenance remains standalone.

Input and durable linkage are validated before any planning, delegation, or implementation begins. A vision is ready only after its exact regular-file copy is verified on the intended target branch. `to-vision` publishes standalone visions through an independently reviewed documentation PR, merges under repository policy, and verifies exact target content before handoff. An Effort vision must already be published by `grill-ticket` or revised through the same validated authoring contract. Explicit draft-only output performs no publication and is not implementation-ready; a later authoring invocation publishes the same path. `implement-vision` never publishes drafts and stops when publication is missing. Authoring success reports the path, publication PR, verified Git refs, and exact implementation handoff without starting implementation. Effort publication additionally verifies one commit-pinned link, matching provenance, and an open issue after merge. Both authoring workflows inspect closing references, including negated phrases and explicit closing relationships, before merge; publication never delivers, lands, or closes an Effort. Each invocation reconstructs durable Git and GitHub evidence so it can start or resume without the previous session. Isolated PCE worktrees belong under `<repository>/.worktrees/visions/`; merged clean worktrees are removed, while incomplete or uncertain evidence is preserved.
