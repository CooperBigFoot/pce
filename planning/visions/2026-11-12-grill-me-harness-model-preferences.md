# Harness model preferences for grill-me research delegation

Update `skills/grill-me/SKILL.md` so its optional bounded research delegation points to the existing harness-specific model preference files through progressive disclosure.

## Context and desired outcome

Recent changes in [PR #233](https://github.com/CooperBigFoot/pce/pull/233) ([lean implementation vision](2026-11-12-lean-implementation-and-capable-model-delegation.md)) added progressive disclosure of harness-specific model-family preferences to `skills/implement-vision/SKILL.md` via `skills/implement-vision/antigravity-cli.md` and `skills/implement-vision/prime-agent.md`. [PR #235](https://github.com/CooperBigFoot/pce/pull/235) ([lean discovery vision](2026-11-12-lean-discovery-and-evidence-handoffs.md)) then updated `skills/grill-me/SKILL.md` to allow delegating substantive, bounded research (such as narrow history questions, source comparisons, or repository investigation) to capable smaller models, without linking the harness preference files.

In harnesses such as Antigravity CLI, subagent and swarm tools default to the lead agent's model unless instructed otherwise. Without a link to the harness preference files, an agent running `grill-me` (or `chart-program`, which loads the canonical `grill-me` interview) knows it may delegate research to a smaller model, but does not see the harness-specific model family preference (Sonnet or Gemini Flash depending on availability in Antigravity CLI; the GPT family in Prime Agent).

## Intended changes

- In `skills/grill-me/SKILL.md` (under `## Investigate before advancing the interview`), instruct the agent to load only the model preference reference matching the current harness when delegating research, linking [Prime Agent](../../skills/implement-vision/prime-agent.md) (`../implement-vision/prime-agent.md`) and [Antigravity CLI](../../skills/implement-vision/antigravity-cli.md) (`../implement-vision/antigravity-cli.md`).
- Reuse the existing `skills/implement-vision/prime-agent.md` and `skills/implement-vision/antigravity-cli.md` files as the single source of truth rather than duplicating files or creating a shared harness framework.
- Keep `skills/grill-me/SKILL.md` itself free of hardcoded model names.
- Preserve all existing `grill-me` discovery and delegation rules: delegation remains optional and judgment-based for substantive, bounded, non-overlapping research questions; findings and uncertainty are reconciled before advancing the interview frontier; and dependent discussion and design judgments remain with the lead.

## Scope and boundaries

- The only required skill edit is in `skills/grill-me/SKILL.md`. Because `skills/chart-program/SKILL.md` loads the canonical `grill-me` skill, it inherits this reference automatically without separate edits.
- Do not modify `skills/implement-vision/`, `skills/to-vision/`, `skills/github-writing/`, `skills/save-discussion/`, or `skills/test-first-development/` unless a concrete link or test consistency check requires it.
- Do not add routing machinery, model discovery procedures, worker quotas, or new reference files.
- Publishing this vision does not authorize implementation or installation into user environments. This is a standalone vision with no associated GitHub issue to close.

## Evidence of success

- `skills/grill-me/SKILL.md` remains model-agnostic in its main body while linking `../implement-vision/prime-agent.md` and `../implement-vision/antigravity-cli.md` with instructions to load only the file matching the current harness.
- All Markdown links across `skills/` and installed skill directories resolve cleanly, verified by running `python3 -m unittest discover -s tests -v` (including `test_supporting_links_resolve_from_each_installed_directory` and `test_documentation_references_resolve`).
