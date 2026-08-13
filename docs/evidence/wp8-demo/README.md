# WP8 real render evidence

`run.html` is the output of:

```bash
pce package render \
  --graph docs/evidence/wp8-demo/graph.json \
  --journal docs/evidence/wp8-demo/journal.jsonl \
  --output docs/evidence/wp8-demo/run.html
```

The journal was produced against a temporary real Git repository. `package replay-finding` checked
`test "$(cat value)" = repair` against the recorded witness and repair commits and accepted the
finding. `package criteria-run` then executed B's authored criterion, captured `real failure output`
on stdout and `real stderr` on stderr, observed exit 7, ran the accepted amendment successfully, and
failed B. A is complete. The temporary Git repository and detached materializations are not retained.
The graph, journal, gate input, decisions, stopped snapshot, and rendered page are retained here.

Two consecutive render commands produced byte-identical output. The final file SHA-256 is
`43a2cda52d271ce279adae1337073777c71194ba2f9fdeace4a4e8681b6d1178` (update this line if the
checked-in renderer changes the evidence page).

## Theme and greyscale checks

The same `file://` page was rendered with headless Google Chrome in four modes: default light,
default dark (`--force-dark-mode`), explicit `data-theme="light"` while dark preference was active,
and explicit `data-theme="dark"` while light preference was active. The background, surfaces, text,
SVG nodes, edges, markers, and evidence panels changed together in both explicit and default modes.
The explicit theme overrode the opposing preference. Greyscale legibility was inspected from the
screenshots: edge kinds retain solid/thick/dashed strokes plus B/S/R labels; critical edges retain a
dotted underlay and CP label; states retain distinct symbols and border patterns plus text labels.

## Journal limits

The journal records no timestamps, so the page shows none. A replayed finding retains its gate and
ordinal, command, repository refs, executions, and decision, but not the original finding description
or repair prose. Those omitted fields cannot be rendered without changing WP7's journal format.
