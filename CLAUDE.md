# Claude Code Guidance

Follow `AGENTS.md`.

Claude Code is an authoring environment for PCE. It installs and uses five skills:

- `grill-me` for canonical intent discovery;
- `to-vision` for standalone vision authoring;
- `chart-program` for approved Program Maps and Effort tickets;
- `grill-ticket` for one explicit Effort's discovery and vision;
- `land-ticket` for evidence-first Effort landing and Program evolution.

Use the standalone workflow for one coherent idea:

```text
grill-me → to-vision → implement-vision
```

Use the Program workflow for a large outcome:

```text
chart-program → grill-ticket → implement-vision → land-ticket
```

Claude Code owns authoring, Program discovery, and ticket landing. Implementation planning, delegation, PR execution, independent review, and Effort delivery records belong to a root Prime Agent through `implement-vision`.

## Durable handoffs

For tracked work, invoke the root workflow as `implement-vision <Effort number or canonical URL>` or pass its linked `planning/visions/<vision>.md` path. A number uses the current repository. A canonical URL supplies its repository identity. An Effort-derived path recovers that same identity from canonical provenance and still runs the full ticket validation. A path without Effort provenance remains standalone.

Input and durable linkage are validated before a persistent goal is created. A vision is ready only after its exact regular-file copy is verified on the intended target branch. A first standalone run can publish a new local draft before implementation. An Effort vision must already be published by `grill-ticket`. Each invocation reconstructs durable Git and GitHub evidence so it can start or resume without the previous session. Isolated PCE worktrees belong under `<repository>/.worktrees/visions/`; merged clean worktrees are removed, while incomplete or uncertain evidence is preserved.
