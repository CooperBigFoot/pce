# Make PCE Harness Agnostic

PCE should run on any agent harness that can spawn subagents, run shell commands, and use Git and GitHub. Today the workflow is tied to Prime Agent: `implement-vision` is installed only there, its text tells the root to create and close the harness's persistent goal, and the authoring skills and documentation address a "root Prime Agent." Prime Agent's goal feature has open reliability problems, and the user does not want PCE's correctness to rest on any single harness feature.

## Desired outcome

All six PCE skills read identically and work in Claude Code, Codex, and Prime Agent. `implement-vision` no longer depends on a persistent goal, a heartbeat, or any other harness-specific mechanism. Its reliability comes from what PCE already requires: reconstructing progress from Git and GitHub evidence on every invocation.

This deliberately reverses the "explicit non-goal" in `planning/visions/2026-08-31-skill-only-pce.md` that excluded direct implementation through Claude Code or the Codex harness. That decision is superseded.

## Settled decisions

### Pure agnostic skill text

Skill text names only capabilities every supported harness has: spawning subagents, running commands, reading and writing files, and using Git and GitHub. It must not mention goals, heartbeats, scheduled wake-ups, or any harness by name, and must not contain conditional language such as "if your harness offers X, use it." The same instructions must read correctly in every environment, so agents behave the same way regardless of where they run.

### Reliability model

The root agent works within its active turn. Subagent completions wake it. When it needs human input it stops cleanly, explains what it needs, and ends the turn. Every invocation of `implement-vision` reconstructs prior progress from the vision, target branch, branches, PRs, issue comments, delivery records, and structured worktrees, exactly as the skill already requires. If a session is lost, a person re-invokes `implement-vision` and it resumes from that evidence. Nothing in the harness is treated as the durable truth of the workflow.

The existing ordering rule survives in agnostic form: complete input validation and the target-branch durability gate before any planning, delegation, branch creation, or substantive implementation. A rejected input ends the turn normally with one precise explanation.

### Subagents are a precondition

The user only runs PCE on harnesses that can spawn subagents. The skill therefore assumes delegation and a fresh independent reviewer are available. Do not design a degraded path for harnesses without subagents.

### Installation matrix

The installer links all six skills into all three known environments:

| Environment | Skills |
| --- | --- |
| Claude Code (`~/.claude/skills`) | all six |
| Codex (`~/.codex/skills`) | all six |
| Prime Agent (`~/.prime/agent/skills`) | all six |

The installer's deliberate removal of `implement-vision` from the Claude Code and Codex directories as a "wrong placement" is dropped. All other installer guarantees are unchanged: links to this checkout only, safe reruns, refusal of ambiguous conflicts before partial installation, preservation of unrelated user state, and cleanup of proven legacy artifacts.

## Repository evidence

- `skills/implement-vision/SKILL.md` names the harness's "native goal and progress capabilities" in its frontmatter description, its "Validate the resolved input" section, its "Establish the persistent goal" section, and its closing paragraph, which also says "Do not recreate ... state machines that the Prime Agent harness already supplies."
- `skills/to-vision/SKILL.md` and `skills/grill-ticket/SKILL.md` address "a fresh Prime Agent."
- `README.md` and `CLAUDE.md` describe handing visions to "a root Prime Agent" and document a Prime-only row for `implement-vision`.
- `install.sh` builds a matrix with five authoring skills for Claude Code and Codex and six for Prime Agent, and lists the Claude Code and Codex `implement-vision` paths under `wrong_placements`.
- `tests/test_skill_contracts.py` asserts the strings "root Prime Agent" and "Prime Agent installs all six PCE skills globally," parses the Prime Agent table row, and checks that "Do not create a persistent goal" appears between the validation section and the "## Establish the persistent goal" heading.
- `tests/test_install.py` encodes the current five-versus-six matrix.

## Required changes

1. Rewrite `implement-vision` so its description, validation section, planning section, and closing paragraph express the reliability model above without any harness-specific mechanism. The planning section should still require: investigate before asking; decide reversible details from evidence; ask the human only about intent, priorities, outcome trade-offs, credentials, authority, or exceptional irreversible acts; and choose one PR or several coherent vertical slices. The closing paragraph should instruct the root to use the harness's own planning and delegation rather than re-inventing them, without naming any harness.
2. Replace every "Prime Agent" phrase in `to-vision`, `grill-ticket`, README, and CLAUDE.md with harness-neutral wording such as "the implementing agent" or "a fresh implementing agent." CLAUDE.md and README should state that all six skills are available in every supported environment and that any harness meeting the capability floor can run either workflow.
3. Update `install.sh` to the six-by-three matrix and remove the `implement-vision` wrong-placement cleanup.
4. Update contract and installer tests to assert the agnostic wording, the full matrix, and the validation-before-planning ordering. Tests must not require any harness name inside skill text.

Everything else in `implement-vision` is unchanged in substance: input resolution, the target-branch durability gate, reconstruction on every run, worktree and preservation policy, the naming boundary, delegation with complete context, independent review and merge rules, delivery records, and cleanup.

## Scope boundaries

- Do not add a runtime, scheduler, hooks, or generated state. PCE remains skill-only.
- Do not add support for additional harness directories beyond the three listed.
- Do not change the Program or Effort workflows, vision provenance rules, or delivery record format.
- Do not rename PCE's skills or workflow artifacts.

## Completion evidence

1. `python3 -m unittest discover -s tests -v` passes on the accepted `main` state.
2. No file under `skills/` contains the strings "Prime Agent", "goal", or "heartbeat" in a harness-mechanism sense. A mention of Prime Agent in README or CLAUDE.md is acceptable only as one of the supported environments in the install table.
3. Running `./install.sh` against an isolated temporary `HOME` produces six owned links in each of the three environment directories and preserves unrelated state.
4. A fresh session in Claude Code can discover `implement-vision` and its text gives that session no instruction it cannot follow.
