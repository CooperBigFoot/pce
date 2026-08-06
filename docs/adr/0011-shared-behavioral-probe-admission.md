# ADR-0011: Shared behavioral probe admission

## Status

Accepted

## Context

A repair-sensitive replay establishes what one recorded stimulus actually did at the broken and
repaired refs. The campaign mechanism was delivered with critic dispatch intended to hide the
campaign side. The critic is handed only its own single-commit repository, an opaque
transport-only recorder client, and a probe-only executable, and the harness does not materialize
its two-ref replay repository until both critics have exited. However, the harness executes
arbitrary critic-supplied programs. Operating-system-visible channels remain open: the process
table exposes the campaign argv, including `--repo-root` naming the source worktree and its labelled
execution-subject fixtures, and the operator's real `HOME` exposes agent-runtime transcripts.
Side-blindness cannot be certified by a harness that does not control the machine.

Four live runs against the same subject produced different admitted outcomes:

```text
campaign 12: BLOCK / APPROVE, witnesses [],                 decision REFUSE
campaign 13: BLOCK / APPROVE, witnesses [],                 decision REFUSE
campaign 14: BLOCK / APPROVE, witnesses [execution-000001], decision APPROVE
campaign 15: BLOCK / APPROVE, witnesses [],                 decision REFUSE
```

Campaign 15 is the only live campaign of the delivered build and the only campaign that exercised
the delivered opaque recorder client. The retained capture is campaign 14, produced before that
recorder-client change by binary
`d3d3e2a4f64f206354596c7ab967890991327ee23be50c83116255bbf7c0d2d1` at head
`118630b39d571822b5037037759b18444b0b3110`; it is not a live capture of the delivered build.
Campaign 14's witness is nevertheless genuine: all four broken-side executions ran inside the
checkout with repository-relative programs, and the harness classified the cited primary as
`repair-sensitive`. The harness's recording, replay, classification, and admission fold are
deterministic. The free-running critic is not: in some runs it executes the subject, while in
others it establishes the same correct defect by reading and therefore supplies no qualifying
primary witness. A passing campaign does not predict the next campaign, and a refusing campaign
does not prove that the subject cannot be manifested.

The ten side channels found while reviewing the campaign are:

1. **Closed — multi-ref repository history.** Each critic checkout contains one commit.
2. **Closed — opposite-side sibling checkout.** The sides run sequentially and the first checkout
   is removed before the second is materialized.
3. **Closed — two-ref replay repository.** It is materialized only after both critics exit.
4. **Closed — side-labelled campaign temporary paths.** Critic-visible dispatch paths and checkout
   names are opaque.
5. **Closed — readable prior critic artifacts.** Verdict, evidence, schema, and event-log bytes are
   captured and removed before the later critic runs.
6. **Closed — side-labelled direct-parent argv.** Opaque dispatch filenames remove the side from
   the critic's direct parent command line.
7. **Closed — campaign binary and probe strings.** The critic receives a probe-only executable
   that embeds no refs, fixture paths, replay classifications, or expected outcomes.
8. **Accepted open — shared `HOME`.** Claude Code writes an agent-runtime session transcript beneath
   `$HOME/.claude/projects/<slug>/`, where a later critic can read a prior critic's transcript.
9. **Closed — recorder-client path and bytes.** The client is an opaque transport-only launcher in
   the critic checkout and neither names nor embeds the source worktree or labelled fixtures.
10. **Accepted open — process table.** An arbitrary submitted program such as `ps` can read an
    ancestor's campaign argv, recover `--repo-root`, and inspect the labelled execution-subject
    fixtures there.

The two open channels are one accepted machine-isolation limitation. No run of this campaign may
be described or certified as side-blind while gate children are unsandboxed and the harness does
not control the machine.

The replacement subject is a deliberately constructed executable pair. Its ordinary `gate` fails
on a plain input at the broken ref and the identical command passes at the repaired ref; the repair
changes only the incorrect output token. The source tree remains necessarily visible to a
stimulus, so the clean-ref isolation established by m1-s4 cannot prevent a stimulus from
discriminating on arbitrary source content.

The rejected alternatives were trusting verdict prose, which supplies no execution provenance;
accepting any single repair-sensitive report, which proves direction but not that the stimulus
exercised the claimed behavior; accepting different stimuli on the two sides, which permits
arbitrary tree discriminators; and putting an expected side or outcome into the gate-visible
prompt, which leaks the parent-owned oracle.

## Decision

The paired campaign admits a broken finding and a repaired approval only when independently
recorded instances of the same binary-owned ordinary-gate probe are repair-sensitive and match
after replacing only their repository roots. A qualifying stimulus runs within its repository and
names its program and inputs relative to that repository, making it rebaseable onto a clean
checkout. An unreplayable record is reported and excluded rather than aborting the campaign. The
gate-visible mandate requires the same probe on both sides without supplying a side, repair,
classification, expected outcome, opposite-side source, or correctness claim. Verdict prose and
replacement-only evidence cannot substitute for the cited primary execution.

## Consequences

The mechanism retains both recorder collections, reports unreplayable records, replays eligible
cited broken primaries and repaired records, and keeps the replay oracle outside the gate-visible
process ancestry. Because a source tree is necessarily visible to its stimulus, m1-s4 cannot
prevent content discrimination; paired root-relocated probe identity is the additional admission
boundary. Campaign 14 passes that boundary, while campaigns 12, 13, and 15 correctly refuse
because the critic's correct reading cannot be laundered into an execution-backed proof. The
retained passing capture demonstrates one admitted run, not a repeatability guarantee. Vision
criteria 1 and 2 remain unpaid because the deliberately constructed subject does not have the
anchored subject provenance those criteria require. In addition, the shared-`HOME` transcript and
process-table channels prevent any run of this campaign from being certified side-blind unless the
harness gains control of the machine and isolates both channels.
