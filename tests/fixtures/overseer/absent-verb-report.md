# Work-graph stop report

Requested command: `pce hold brief --source OQ6`
Observed stop: requested verb `hold brief` is absent from the installed binary surface.
Observed surface: `pce hold --help` lists open, list, register, runs, read, answer, route, and close.
Discriminating fact: the installed binary has no `hold brief` verb.
Reproduction: run `pce hold brief --source OQ6` with the installed binary.

## Operator card

The installed binary does not have the requested `hold brief` verb, so this stop needs a binary defect brief rather than a ruling.
