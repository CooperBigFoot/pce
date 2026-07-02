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

## Step 2 — Write vision.md against the FIXED template

Write the conversation's shared understanding into `$VISION_DIR/vision.md`. The file structure is a **fixed, byte-exact template** — identical to the stub the CLI writes:

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
- If `vision.md` is a fresh stub (empty sections), fill each section from the conversation.
- If `vision.md` already has content (same-day rerun), **update it in place**, merging the conversation's current understanding into the existing sections while preserving the template structure exactly.
- Sub-structure inside a section (bullet lists, `###` subsections, tables, fenced blocks) is allowed; the title line and the seven `##` headers are inviolable.

What belongs in each section:

| Section | Content |
|---|---|
| `## Goal / Why` | The problem, the motivation, what success changes. |
| `## Scope — In` | Concrete deliverables, numbered. |
| `## Scope — Out (explicit non-goals)` | What is deliberately excluded, so planners do not drift. |
| `## Constraints` | Technical, process, and environment constraints that bind implementers. |
| `## Acceptance criteria (vision-level "done")` | Objectively checkable outcomes. |
| `## Decomposition hints` | Suggested milestone/step structure, orderings, risky-first slices. |
| `## Open questions / risks` | Unresolved decisions and assumptions needing validation. |

## Step 3 — Report

Tell the user the path `$VISION_DIR/vision.md`, summarize what was captured under each section, and flag any `## Open questions / risks` entries that need their input before an orchestrator run.
