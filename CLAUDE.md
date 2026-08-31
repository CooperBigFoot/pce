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
