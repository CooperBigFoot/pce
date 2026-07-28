# ADR-0004: Session-start rehydration injection

## Status

Accepted

## Context

ADR-0001 proposed context injection through both `PostCompact` and `SessionStart`. The
`PostCompact` mechanism is impossible: its documentation states, “Supports additionalContext: No. PostCompact is used for side effects only”
and “Decision control: None”. `PostCompact` is absent from the documented list of events
supporting `hookSpecificOutput.additionalContext`: SessionStart, Setup, SubagentStart, UserPromptSubmit, UserPromptExpansion, PreToolUse, PostToolUse, PostToolUseFailure, PostToolBatch, Stop, SubagentStop.

`SessionStart` supports `additionalContext`. Its documented matchers are `startup`, `resume`,
`clear`, `compact`, and `fork`; the `compact` matcher covers “Auto or manual compaction”. The
2026-07-28 human ruling dropped `PostCompact`. Recovery after compaction and on resume remains
fully served by `SessionStart` with only the `compact` and `resume` matchers.

The rejected alternatives are relying on the orchestrator to notice lost context and invoke
status itself, treating injection as a tool-boundary prohibition, configuring `PostCompact`
despite its lack of injection support, and firing on a cold `startup`.

## Decision

The enforcement split gains a fourth layer: context the orchestrator must hold but cannot be
relied on to fetch is injected at the harness boundary by a hook that fails open.

The hook uses `SessionStart` matching `resume` and `compact` only and deliberately excludes
`startup`. It configures no `PostCompact` behavior, including side-effect-only behavior. It
runs the status verb in the current repository and injects the run snapshot's recovery digest
through `hookSpecificOutput.additionalContext`. It selects the run by the most recent
event-log record without a run-marker file, and silently injects nothing when there is no
candidate or the choice is ambiguous.

Fail-open distinguishes context injection from a prohibition: failure to inject loses a
convenience, while a prohibition that silently fails to fire is a broken gate. Marker-free
selection is acceptable for this fail-open injection layer and remains forbidden for
prohibition enforcement.

## Consequences

Recovery no longer depends on the orchestrator remembering to recover when its context is
weakest, and the status verb remains the single computation serving ordinary reads and
recovery.

The hook lives outside this repository, so no merged pull request activates it. Later
repository deliverables are the hook script and `install.sh` wiring, and activation requires a
human `install.sh` run. Any acceptance claim that depends on the hook actually firing therefore
remains manual.

A fresh session inspecting a paused run receives nothing automatically. The bounded digest can
make an imperfect judgment and must name what it elided. Future work may depend on this layer,
making reversal costly.
