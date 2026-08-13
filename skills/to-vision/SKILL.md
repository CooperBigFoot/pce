---
name: to-vision
description: Turn the current conversation's shared understanding into a structured vision.md via the pce CLI. Use when the user asks to capture, materialize, or convert the conversation into a vision, e.g. `/to-vision "auth refactor"` — creates a dated vision directory with `pce vision new` and fills its fixed-template vision.md.
---

You are materializing the current conversation into a **vision document** that downstream planning agents will treat as their single source of truth for *what* to build.

## Argument

- `<name>` = `$ARGUMENTS` — the human-readable vision name (e.g. `auth refactor`). If no argument was given, ask the user for a name and stop until one is provided.

## Step 1 — Create the vision directory with the pce CLI

From the repo root, run exactly:

```bash
VISION_DIR="$(pce vision new "<name>")"
```

substituting `<name>` with the argument. Contract of `pce vision new`:

- Its **sole stdout output** is the relative path of the vision directory (`planning/<YYYY-MM-DD>-<slug>`). All diagnostics and warnings go to **stderr**, so the command substitution above captures exactly the directory path and nothing else. After a zero exit, trust `$VISION_DIR` unconditionally.
- **Different-date duplicate — warn and proceed**: if the same slug already exists under another date (e.g. `planning/2020-01-01-auth-refactor`), the command warns on stderr and proceeds, creating a fresh directory under today's date. Do not treat the warning as an error; do not stop.
- **Same-date rerun — idempotent no-op**: if today's directory already exists, the command leaves its contents — including any existing `vision.md` — **untouched**, and prints the existing directory path. Rerunning `/to-vision` on the same day never destroys prior work.
- **Non-zero exit** (empty or unsluggable name, `pce` not on PATH): show the user the stderr output and stop. If the binary is missing, tell the user to run `install.sh` from the pce repo first.

```bash
VISION_CANDIDATE="$VISION_DIR/.vision.md.candidate"
rm -f "$VISION_CANDIDATE"
```

Every command below runs in its own shell, so substitute the literal path `$VISION_DIR/.vision.md.candidate` in each later command rather than relying on a shell variable or an `EXIT` trap surviving between commands.

## Step 2 — Write vision.md against the FIXED template

Render the conversation's complete proposed shared understanding into the fixed same-directory candidate `$VISION_DIR/.vision.md.candidate` using the normal file-editing capability. Do not write `$VISION_DIR/vision.md` first. The candidate's file structure is a **fixed, byte-exact template** — identical to the stub the CLI writes:

- Title line: `# Vision: <name>` — the name exactly as the user gave it, not the slug.
- Then exactly these seven `##` headers, in this order, byte-for-byte. The dashes in the two Scope headers are em-dashes (U+2014); the quotes around done are straight ASCII double quotes:

```
# Vision: <name>

## Goal / Why

## Scope — In

## Scope — Out (explicit non-goals)

## Constraints

## Acceptance criteria (vision-level "done")

## Decomposition hints

## Open questions / risks
```

Hard rules:

- Content is written **only under the existing headers**. Never add, rename, reorder, or remove headers. Never write free-form prose outside the template sections. The vision is never free-form.
- If `vision.md` is a fresh stub (empty sections), fill each section in the candidate from the conversation.
- If `vision.md` already has content (same-day rerun), merge the conversation's current understanding with the existing sections in the candidate while preserving the template structure exactly. Leave the existing file untouched until the candidate passes its check.
- Before updating a same-day vision, inspect the existing file for a repos block using the content-keyed locator below and reconcile it under the cross-repo rules; do not regenerate the file without diffing the proposed repo entries and consumption edges against that existing block.
- Sub-structure inside a section (bullet lists, `###` subsections, tables, fenced blocks) is allowed except under acceptance criteria, whose sole content is the canonical JSON fence below; the title line and the seven `##` headers are inviolable.

What belongs in each section:

| Section | Content |
|---|---|
| `## Goal / Why` | The problem, the motivation, what success changes. |
| `## Scope — In` | Concrete deliverables, numbered. |
| `## Scope — Out (explicit non-goals)` | What is deliberately excluded, so planners do not drift. |
| `## Constraints` | Technical, process, and environment constraints that bind implementers. |
| `## Acceptance criteria (vision-level "done")` | Exactly one fenced `json` object in the canonical shape below, and nothing else. Each criterion names a concrete `input` and an externally observable `observation`; test existence is never an observation. |
| `## Decomposition hints` | Suggested milestone/step structure, orderings, risky-first slices. |
| `## Open questions / risks` | Risks and assumptions the run must carry, written for the orchestrator rather than for the human. Not a place to reopen decisions the grill settled, and not a question list addressed to the user; empty is correct when the grill left nothing unresolved. |

The acceptance-criteria section must use exactly this canonical shape, including key names and key order:

````markdown
## Acceptance criteria (vision-level "done")

```json
{
  "criteria": [
    {
      "name": "<plain-language criterion name>",
      "input": "<the concrete input or action fed to the finished thing>",
      "observation": "<the externally observable result that settles the criterion>"
    }
  ]
}
```
````

Replace all three instructional placeholders with non-blank authored content and repeat the entry object once per criterion. The JSON object has exactly one top-level key, `criteria`. `criteria` is a non-empty array. Each entry is an object with exactly the required string keys `name`, `input`, and `observation`; unknown keys are refused. Each value is trimmed for the domain value and must contain at least one non-whitespace character. Array order is ratification order and must be preserved. Preserve every criterion ratified in the grill. The terms `test`, `tests`, and `must be tested` are not fields and may not replace `input` or `observation`. A criterion may legitimately execute a test command as its input, but its observation must state what that execution externally does; “a test exists” is not an observation. Prose criteria and parallel restatements outside the JSON fence are prohibited.

### Cross-repo visions

Treat cross-repo scope like every other fact established by the conversation. When the conversation clearly establishes that the work spans one or more additional repositories, emit the repos block below. When the conversation leaves cross-repo scope unclear, ask the user whether additional repositories are in scope and wait for the answer. When there is no cross-repo signal, keep single-repo behavior byte-identical: emit no repos block, ask no repository questions, and make no changes to the Step 3 report.

Before emitting or updating a repos block, resolve the primary repository and every additional repository. Name each repository by its directory basename, including the primary repository. If two repositories have the same basename, ask the user for a distinct disambiguated name and wait for it instead of choosing a name yourself.

Validate every proposed declaration before writing it. Mirror all Phase 0 declaration checks:

- The resolved repository path exists.
- The resolved path is a git repository.
- The repository root contains `AGENTS.md` or `CLAUDE.md`.
- Every additional-repository `path` is relative to the primary repository, never absolute.
- No two declared names resolve to the same repository.

If any check fails, name the repository entry, report the specific failed check, ask the user for a correction, and wait for the correction. Treat every validation failure as a conversation with the author, not an error exit. Apply the same validation to additions and edits on reruns. Never emit an entry that Phase 0 is guaranteed to reject.

Place the repos block as the first content under the existing `## Constraints` header, fenced as `yaml`, with `repos:` as the first top-level key. Never add an eighth header. A vision may contain at most one repos block. Identify that block by content, not position: it is a fenced YAML block whose first top-level key is `repos:`. On a same-day rerun, find it by that rule even if it is no longer first under `## Constraints`, then update and move that one block to the required first-content position rather than adding another.

Use exactly this shape, copied from `skills/pce/SKILL.md` §Cross-repo runs:

```yaml
repos:
  <primary-repo-name>:
    path: .
  <additional-repo-name>:
    path: ../<relative-path-from-primary>
consumption:
  - producer: <producer-repo-name>
    consumer: <consumer-repo-name>
    artifact: <human-readable-artifact-name>
```

The primary repository always has `path: .`. Every additional path is relative to the primary repository. Repeat repo entries and consumption list items as needed. Each consumption edge uses exactly `producer`, `consumer`, and `artifact`. `skills/pce/SKILL.md` §Cross-repo runs is normative if this duplicated shape ever conflicts with it.

When the declared repositories share no artifact flow, omit the `consumption:` key entirely and emit only the `repos:` mapping in this shape:

```yaml
repos:
  <primary-repo-name>:
    path: .
  <additional-repo-name>:
    path: ../<relative-path-from-primary>
```

On a same-day rerun, construct the proposed validated repo entries and consumption edges, then explicitly diff them against the existing content-keyed repos block before writing. Additions and edits require no extra confirmation after validation. Before removing any previously declared repository, obtain explicit user confirmation that names the repository entry being dropped. Before removing any previously declared consumption edge, obtain explicit user confirmation that names the producer, consumer, and artifact being dropped. If confirmation is not given, retain the existing entry or edge. This removal check applies even when the current conversation merely stops mentioning a previously declared item.

After rendering the complete candidate, run exactly:

```bash
pce vision check < "$VISION_DIR/.vision.md.candidate"
```

If `pce vision check` fails with the `usage: pce` text rather than an acceptance-criteria diagnostic, the installed binary predates this verb. Run `rm -f "$VISION_DIR/.vision.md.candidate"`, tell the user to rebuild and rerun `install.sh` from the pce repo, and stop before Step 3; do not author the vision.

If `pce vision check` fails with an acceptance-criteria diagnostic, print the diagnostic, run the removal command below, leave the existing `vision.md` byte-for-byte untouched, tell the user the vision was not authored, and stop before Step 3.

```bash
rm -f "$VISION_DIR/.vision.md.candidate"
```

If `pce vision check` fails with the `usage: pce` text rather than an acceptance-criteria diagnostic, the installed binary predates this verb. Run the removal command below, tell the user to rebuild and rerun `install.sh` from the pce repo, and stop before Step 3; do not author the vision.

```bash
rm -f "$VISION_DIR/.vision.md.candidate"
```

Only a zero exit permits exactly:

```bash
mv "$VISION_DIR/.vision.md.candidate" "$VISION_DIR/vision.md"
```

The checker has no success output. A non-zero result prohibits `mv`, Step 3 reporting, and any downstream invocation.

## Step 3 — Report

Tell the user the path `$VISION_DIR/vision.md` and summarize what was captured under each section.

Ask the user nothing. This skill runs after a grill, where every question was already put to the user and answered; a question here is a question the grill failed to ask, and re-opening it treats settled understanding as unsettled. Never end the report with open questions, things needing their input, points to confirm, or a request to review what was captured. Never invent an unresolved decision to fill `## Open questions / risks`; when the conversation left nothing genuinely unresolved, that section carries only risks the run itself must carry, and may be left empty. The two exceptions are the blocking repository facts named in the cross-repo rules and a failed `pce vision check` — both are missing facts that no default can supply, not reopened decisions.
