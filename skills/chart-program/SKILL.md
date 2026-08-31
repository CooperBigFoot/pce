---
name: chart-program
description: Chart a large outcome as an approved GitHub Program Map and contained Effort tickets, or re-survey one explicit Program. Use for /chart-program with a large idea or Program issue URL or number.
---

# Chart Program

Turn a large outcome into a concise GitHub Map. `$ARGUMENTS` must contain either the idea for a new Program or the issue number or URL of one Program to re-survey. Ask for that missing identity only when the conversation does not supply it. Never infer a repository-wide current Program. Multiple open Programs are valid.

Keep discovery in GitHub issues and repository visions. Do not invoke `grill-with-docs`, edit `CONTEXT.md`, create ADRs, or restore domain-modeling or retired PCE runtime machinery.

## Investigate first

From the repository root, use repository files and `gh` to identify the repository and authenticated user. Read relevant code, documentation, issues, labels, comments, linked visions, PRs, delivery records, and existing Program Maps. For re-survey, validate the explicit issue as an open or closed Program and reconstruct all linked Efforts, dependencies, landed outcomes, and Fog. Spend enough effort to recover complete current state. Do not ask for facts that this evidence or established practice can answer.

Load and follow PCE's canonical `grill-me` skill. It owns the interview rounds, question format, investigation policy, and confirmation gate; do not restate or replace its loop here. Run it at Program altitude and breadth-first across destination, boundaries, candidate Efforts, genuine delivery dependencies, success, exclusions, risks, and Fog. An Effort is an ambitious, independently meaningful outcome suitable for one contained vision, not an implementation task or technical layer. Do not deeply design an Effort. On re-survey, preserve settled outcomes unless evidence conflicts and grill only additions, changed boundaries, conflicts, and Fog that may now be sharp.

## Approval gate

Before any GitHub mutation, present one intent-level proposal:

- **Destination:** at most three sentences.
- **Effort tickets:** title and one-sentence outcome question for each.
- **Dependencies:** one line per genuine blocker.
- **Fog:** short bullets.
- **Exclusions:** short bullets when useful.
- **Re-survey changes:** only substantive additions, removals, conflicts, or boundary changes.

Do not show generated bodies, identifiers, API calls, unchanged inventories, or placeholder mechanics unless requested. The canonical grill confirmation is the publication approval for this explicitly requested charting workflow only when the summary contains this complete proposal. Otherwise ask for concise approval. Do not mutate before approval.

If the survey finds one coherent Effort, do not create a Program or Effort issue. Continue the same explicitly requested workflow by running canonical `grill-me` at standalone-vision depth. After its confirmed summary, use the `to-vision` creation mechanism with a descriptive derived name, author the standalone vision, and stop with its path and exact `implement-vision` handoff. Do not start implementation.

## Publish deterministically

Use ordinary `gh` issue commands and preserve unrelated issue content. Use these minimum cold-session contracts:

- Program issues have the `pce:program` label and `<!-- pce:program -->` marker.
- Effort issues have the `pce:effort` label and `<!-- pce:effort -->` marker.
- Each Effort body contains exactly one `Program: <canonical GitHub issue URL>` line.
- Each Effort body contains exactly one `Depends on: <none or comma-separated canonical Effort URLs>` line. Dependencies may refer only to Efforts in that Program and must not form a cycle.
- Each Effort body initially contains exactly one `Vision: pending` line.
- The Map links every Effort by canonical issue URL and separates open Efforts, Frontier, landed one-line outcomes, Fog, and exclusions. Frontier contains only open Efforts whose listed dependencies are closed.

Create labels if absent without changing unrelated labels. For a new chart, create the Program first, then Efforts, then replace the approved Map placeholders with their canonical URLs. For re-survey, edit the explicit Map and affected open tickets in place; never create duplicates for unchanged Efforts. Preserve closed outcomes and auditable comments. Do not turn Fog into speculative tickets.

After mutation, reload every changed issue from GitHub. Validate markers, labels, unique Program and dependency lines, Program membership, links, dependency acyclicity, Map completeness, and computed Frontier. Report exact partial failures and the state that exists; never claim an unverified mutation. Finish with the Program URL, created or changed Effort URLs, Frontier, retained Fog, and no implementation start.
