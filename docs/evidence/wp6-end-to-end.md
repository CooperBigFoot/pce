> Historical WP6 evidence: this run predates the follow-up repository-qualified witness/repair format. The flat pre/post outcome shown below is no longer accepted. See `wp6-followup-end-to-end.md` for the current two-commit contract.

# WP6 end-to-end evidence

Run root: `/tmp/pce-wp6-e2e-final-dtm0hkbz`  
PCE binary: `/tmp/pce-wp6-target/debug/pce`  

## Seed

The one-package graph criterion was `test -f known.txt`. The deterministic worker received the
`pce package agent` brief, wrote `wrong` to `known.txt`, committed it, and wrote
`{"outcome":"done"}`. The file existed, so the stated floor observation held, while its contents
violated the vision goal. WP6 did not execute the criterion command.

Built artifact ref: `d03cf3f986db3931ba14a87b91ffcea25aee20db`.

## Gate

Invocation shape:

```text
pce package gate-agent --vision /tmp/pce-wp6-e2e-final-dtm0hkbz/vision.md --graph /tmp/pce-wp6-e2e-final-dtm0hkbz/graph.json \
  --package T1 --artifact-ref d03cf3f986db3931ba14a87b91ffcea25aee20db \
  --outcome /tmp/pce-wp6-e2e-final-dtm0hkbz/gate.outcome.json -- /bin/sh -c <recorded deterministic gate script>
```

The gate read the artifact at the built ref, observed `wrong`, replaced it with `known`, committed
only that repair, and wrote:

```json
{
  "findings": [
    {
      "description": "known.txt contains wrong instead of known",
      "repair": "replaced the file contents with known",
      "proposed_criterion_command": "test \"$(cat known.txt)\" = known",
      "pre_repair_ref": "d03cf3f986db3931ba14a87b91ffcea25aee20db",
      "post_repair_ref": "66dd6a0fddb08f4340adbbefdf6885798e945521"
    }
  ]
}
```

The pre-repair blob read through Git is `wrong`; the post-repair blob read through Git is `known`.
Therefore the exposed falsifier `test "$(cat known.txt)" = known` is false for the pre-repair tree
and true for the post-repair tree. WP6 did not execute that proposed command; replay remains the
WP7 driver's responsibility.

## Recorded artifacts

- `/tmp/pce-wp6-e2e-final-dtm0hkbz/vision.md`
- `/tmp/pce-wp6-e2e-final-dtm0hkbz/graph.json`
- `/tmp/pce-wp6-e2e-final-dtm0hkbz/repo`
- `/tmp/pce-wp6-e2e-final-dtm0hkbz/worker.sh`
- `/tmp/pce-wp6-e2e-final-dtm0hkbz/worker.brief.md`
- `/tmp/pce-wp6-e2e-final-dtm0hkbz/worker.outcome.json`
- `/tmp/pce-wp6-e2e-final-dtm0hkbz/gate.sh`
- `/tmp/pce-wp6-e2e-final-dtm0hkbz/gate.brief.md`
- `/tmp/pce-wp6-e2e-final-dtm0hkbz/gate.outcome.json`
- `/tmp/pce-wp6-e2e-final-dtm0hkbz/e2e-evidence.json`

Worker exit: `0`. Gate exit: `0`. Repair ref: `66dd6a0fddb08f4340adbbefdf6885798e945521`.
