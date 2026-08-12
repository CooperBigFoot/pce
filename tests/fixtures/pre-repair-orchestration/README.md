# Pre-repair orchestration regression fixture

This directory is a repository-contained, minimal extraction of the event envelopes and
required-artifact sidecar observations used to diagnose the August 2026 orchestration dead ends.
Tests read only these committed files. Historical absolute artifact names are inert identity
strings and are never opened.

## Provenance and normalization

The retained wire records came from the pre-repair live-derived series named in the repair
investigation. Repository refs, evidence prose, durations, timestamps, and artifact root prefixes
were normalized to remove machine-specific data. Event kinds, node and role identities, lifecycle
relationships, completion outcome fields, the absence of `root_cause`, per-series cardinalities,
and record order retain the source semantics. JSON is stored in the canonical compact ordering
emitted by `serialize_event_line`, making byte-for-byte parse/serialization stability testable.

## Exact contents

- `pre-repair-events.jsonl`: 24 records, comprising 12 dispatches and 12 legacy validated
  completions. Each of `(m1-s2, step-executor)`, `(m3-s4, step-executor)`,
  `(m3-s9, step-plan-critic)`, and `(m3-s11, step-executor)` has exactly 3 completed validated
  productions. None of the legacy completions has `root_cause`.
- `pre-repair-artifact-observations.json`: 12 retained required-artifact sidecar observations,
  one for each issuance.
- `spending-limit-resume-events.jsonl`: 27 records for the retained realistic
  `(m1-s2, step-executor)` route: spends 1-3 repeat the retained legacy records, spends 4-12 are
  deterministic extensions to the automatic limit, followed by one keyed escalation open/close
  pair recording human authorization and exactly one subsequent resumed dispatch. Timestamps
  remain ordered through the recorded resume.
- `spending-limit-resume-artifact-observations.json`: 13 retained required-artifact sidecar
  observations, including the one authorized resume.

The fixture is intentionally independent of status snapshot schema and installed skill text.
