# ADR-0008: A gate is measured in the environment that enforces it

## Status

Accepted

## Context

The repository contract's stated half is falsified by executing each gate command at base on the
orchestrator's host. Executors run those same commands inside `codex exec --sandbox workspace-write`.
Nothing reconciles the two, so a measured exit status certifies an environment that is not the one
enforcing the gate.

This has now cost two repositories with unrelated toolchains. On `grant-proposal-nik`, 2026-07-29,
the `tectonic` build gate exited 101 inside the sandbox and 0 outside it in the identical worktree,
burning two execution rounds on the run's one irreversible step before a human authorised
`danger-full-access`. On `RivRetrieve`, 2026-07-31, `uv build` failed the same way for four
executors, producing three verdicts that needed manual reclassification and one executor that
withheld a commit on complete, green work because its instructions require every gate to pass first.

Both reports independently recommend the same pair: run gate measurement through the executor's own
sandbox, and add an `environment` value to the verdict schema's `root_cause` enum so a sandbox fault
is not attributed to `step_plan` and routed to replanning a correct plan. The Program's out-of-scope
list rules out changing `root_cause`, so the two recommendations cannot both be taken as written.

## Decision

Gate commands are measured in the environment that enforces them, through the same dispatch envelope
the executor receives. `root_cause` gains no `environment` value. A sandbox that degrades mid-run is
established by re-running the gate in the identical shape and comparing against the base measurement.

## Consequences

Measured where it is enforced, `uv build` and `tectonic` fail at base, and the stated half's existing
rule disposes of them without new vocabulary: a command that fails at base is not a gate. Neither
enters a contract, neither reaches an executor, and there is nothing left to misattribute. The enum
value describes a situation this prevents from occurring.

The residual is a sandbox that works at base and degrades later — observed on 2026-07-29, where the
same gate passed on two early steps before failing. There the binary re-runs the gate as it ran it at
base; green in the sandbox at base and red in the sandbox now is a disagreement between two of the
binary's own measurements rather than a claim an executor makes about its own environment, which puts
the classification where the Enforcement split already puts verifications.

What this does not address is the executor withholding a commit on green work when one gate is
unsatisfiable. That is the binding executor prompt text, which belongs to the role-frame ticket.
