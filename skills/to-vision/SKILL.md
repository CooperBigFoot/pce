---
name: to-vision
description: Turn confirmed understanding into a published repository vision, or a local draft when requested. Use when asked to write or revise a vision.
---

# To Vision

Turn the completed discussion into one published handoff for a fresh implementing agent that cannot see the conversation. Invoking `to-vision` authorizes publication of the confirmed vision, including the ordinary documentation PR merge, unless the user explicitly requests draft-only output. It does not authorize implementation or merging research or implementation PRs. `grill-me` confirmation alone does not invoke this workflow or authorize publication.

## Name and path

When `$ARGUMENTS` contains a useful human-readable name, use it unchanged as the vision name. When it is empty, derive a concise, descriptive name from the confirmed shared understanding. Ask for a name only when the conversation does not contain enough information to choose a meaningful one. Do not reopen the completed discussion merely to name the file.

If revising an existing vision, reuse that same regular `planning/visions/` path, including on a later invocation to publish a draft. Refuse symlinks, symlinked path components, and paths outside `planning/visions/`. For a new vision only, from the target repository root run the deterministic creation helper with the explicit or derived name:

```bash
python3 <path-to-this-skill>/scripts/create_vision.py "<vision name>"
```

The helper prints `planning/visions/YYYY-MM-DD-<slug>.md`. It creates an empty file only when the path does not exist. A repeated call returns the existing path without changing its content.

Read an existing file before editing it. Never silently replace prior content. Reconcile the current shared understanding with it and preserve still-valid information.

## Authoring contract

Choose the structure that best communicates this specific work. There is no required template or heading set.

Write enough context that the fresh agent can implement without the prior conversation. Where relevant, communicate:

- the desired outcome and why it matters;
- externally observable evidence of success;
- scope boundaries and explicit exclusions;
- constraints and settled decisions;
- important repository facts learned during the grill;
- material risks or genuine uncertainty that remains.

Translate intent into useful technical context, but leave reversible mechanisms to the implementing agent. Do not invent unresolved questions to fill a section. Do not reopen decisions settled during the grill.

## Authoring checks

Investigate source evidence against the confirmed requirements before authoring. After drafting, compare each confirmed decision with the vision for omissions, weakened constraints, or contradictions, including draft-only output. Check proposed requirements against confirmed intent: distinguish ordinary technical elaboration from unapproved outcomes or constraints. Investigate uncertain or conflicting sources and repair mismatches before publication review. These checks do not grant publication or implementation authority.

This same authoring check applies when called by `grill-ticket` or when `chart-program` converges on a standalone vision. Do not repeat completed checks for the same unchanged draft within one invocation.

## Draft-only output

When the user explicitly requests draft-only output, write or revise the local draft and perform no commit, push, PR, issue mutation, or merge. Report its path and concise summary. State that it is not yet verified on the target branch and is not implementation-ready. A later authoring invocation can publish the same path. Stop without an implementation handoff.

## Publish and verify

For publication or resumed publication, load and follow the [vision publication procedure](publication.md) after the authoring checks. Execute it once and consume its verified or precise partial-failure result. Draft-only output stops above without loading the procedure. This reference owns publication, not implementation.
