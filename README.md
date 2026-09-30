# PCE

PCE is a lean distribution of six Agent Skills for turning ideas into delivered work.

## Install

With Bash and Python 3 available, run from this checkout:

```bash
./install.sh
```

All six skills are linked into Claude Code (`~/.claude/skills`), Codex
(`~/.codex/skills`), and Prime Agent (`~/.prime/agent/skills`). Keep this checkout
in place. The installer is safe to rerun, replaces links owned by this checkout,
and refuses conflicting files, directories, or foreign links. Unrelated files
and settings are left unchanged.

## Skills

- [grill-me](skills/grill-me/SKILL.md): Clarify an idea through focused questions.
- [to-vision](skills/to-vision/SKILL.md): Author, publish, and verify a durable vision.
- [implement-vision](skills/implement-vision/SKILL.md): Implement a vision with delegation and independent review.
- [chart-program](skills/chart-program/SKILL.md): Chart a large outcome as a Program and Effort tickets.
- [grill-ticket](skills/grill-ticket/SKILL.md): Discover one Effort and publish its vision.
- [land-ticket](skills/land-ticket/SKILL.md): Verify delivery, land an Effort, and update its Program.

## Workflows

```text
grill-me → to-vision → implement-vision
chart-program → grill-ticket → implement-vision → land-ticket
```
