# ADR-0009: The parent dispatch owns gate execution evidence

## Status

Accepted

## Context

A free-running falsification gate must choose the stimulus that probes a built artifact, but it cannot
also be the authority for what that stimulus observed. If the critic writes its own command result, a
plausible JSON claim is indistinguishable from an execution. If it writes directly to a shared evidence
file, one genuine record can be edited or accompanied by fabricated records. Reading the gate's prose
cannot repair either failure.

The existing gate is an unsandboxed `claude -p` child spawned and measured by `pce`. The parent remains
alive for the complete dispatch, already owns the exact child environment and verdict-validation
boundary, and can retain observations until the child no longer has an opportunity to write the final
record. Alternatives were to trust critic-authored evidence, trace arbitrary descendant processes, or
make an installed hook mediate commands. The first has no independent authority, the second cannot
recover semantic setup and input reliably, and the third would make correctness depend on a human
having activated the installed surface.

## Decision

The binary grants the critic repository inspection, verdict writing, and the exact harness helper.
Because the gate child is unsandboxed and inherits the operator's own permission rules, that grant
narrows the default surface but does not by itself prevent direct execution. Admission therefore rests
on the parent's retained records rather than on tool denial: an execution the harness did not perform
has no reference to cite. The critic submits a closed, shell-free stimulus request to the still-running
parent dispatch through a dispatch-scoped Unix-domain endpoint. The parent alone executes the ordered setup and
command under the request's exact working directory, input and complete explicit environment; assigns
the execution reference; and observes stdout, stderr and terminal status. The critic receives the
reference and parent-observed result but cannot supply the observation.

The parent retains records in memory until the critic exits, then writes one create-new, read-only
sidecar derived from the verdict path. Verdict admission uses the retained records, not bytes the
critic could have placed at that path. Every blocking issue and its executed replacement cite their own
record from the same dispatch. The record contains no expected outcome or interpretation; clean-ref
replay remains a separate computation.

## Consequences

An execution reference now identifies something the harness actually ran, so a reading-only opinion or
fabricated result cannot become a blocking issue merely by filling schema fields. Exact binary input,
environment, setup and output remain available for later repair-sensitive replay.

The parent gains a local request server and must clean up its socket on every exit path. Stimuli are
explicit-environment, shell-free requests rather than arbitrary terminal transcripts, and callers that
need shell behavior must name a shell as the program and its script as an argument. The evidence
sidecar is immutable workflow data but is not an expected-value oracle; later work must replay it at
independent refs before deciding relevance. Activation still follows normal human promotion: this
decision does not assume an installed schema or hook changed during the delivering run.
