# WP6 follow-up end-to-end evidence

Run root: `/tmp/pce-wp6-followup-e2e-onal737k`  
PCE binary: `/tmp/pce-wp6-follow-final3-test/debug/pce`

The worker created `known.txt` containing `wrong` and committed built artifact `e3340b3f7b04dcb1c758b41032f59f19b8b1e3ae`. The package
floor required only that the file exist, so its observation held while the artifact violated the
vision goal. WP6 did not execute that criterion.

The gate then authored exactly two commits in repository `fixture`:

1. Witness `35135e7141e0eebbf0c45ad80d428d4cce48fc58` added only `proposed-criterion.sh`.
2. Repair `cef43d377ff3d3cd47163419a3a7357de3601ae2` changed only `known.txt` from `wrong` to `known`.

The repair's direct parent is the witness, and `git merge-base --is-ancestor 35135e7141e0eebbf0c45ad80d428d4cce48fc58 cef43d377ff3d3cd47163419a3a7357de3601ae2`
returned 0. `pce package gate-agent` independently resolved both refs and accepted that ancestry.
It recorded:

```json
{
  "findings": [
    {
      "description": "known.txt contains wrong instead of known",
      "repair": "replaced only known.txt contents with known",
      "proposed_criterion_command": "./proposed-criterion.sh",
      "repository_refs": [
        {
          "repository": "fixture",
          "witness_ref": "35135e7141e0eebbf0c45ad80d428d4cce48fc58",
          "repair_ref": "cef43d377ff3d3cd47163419a3a7357de3601ae2"
        }
      ]
    }
  ]
}
```

Inspection, without executing `./proposed-criterion.sh`, showed the identical script at both refs:

```sh
#!/bin/sh
test "$(cat known.txt)" = known
```

At the witness ref, `known.txt` is `wrong`, so the command's comparison is false. At the repair ref,
`known.txt` is `known`, so the comparison is true. This semantic observation is evidence only; no
WP6 code executed the proposed criterion.

Recorded artifacts:

- `/tmp/pce-wp6-followup-e2e-onal737k/vision.md`
- `/tmp/pce-wp6-followup-e2e-onal737k/graph.json`
- `/tmp/pce-wp6-followup-e2e-onal737k/repo`
- `/tmp/pce-wp6-followup-e2e-onal737k/worker.sh`
- `/tmp/pce-wp6-followup-e2e-onal737k/worker.brief.md`
- `/tmp/pce-wp6-followup-e2e-onal737k/worker.outcome.json`
- `/tmp/pce-wp6-followup-e2e-onal737k/gate.sh`
- `/tmp/pce-wp6-followup-e2e-onal737k/gate.brief.md`
- `/tmp/pce-wp6-followup-e2e-onal737k/gate.outcome.json`
- `/tmp/pce-wp6-followup-e2e-onal737k/gate-final3-validation.outcome.json` (final ref reachability, direct-parent, and ancestry validation)
- `/tmp/pce-wp6-followup-e2e-onal737k/evidence.json`
