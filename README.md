# PCE

PCE provides six workflow skills for turning ideas into delivered work, plus two supporting guidance skills.

## Install

With Bash and Python 3 available, run from this checkout:

```bash
./install.sh
```

All eight skills are linked into Claude Code (`~/.claude/skills`), Codex
(`~/.codex/skills`), and Prime Agent (`~/.prime/agent/skills`). Keep this checkout
in place: the links load its files, not copied guidance. Pull updates here and
rerun the installer. It replaces checkout-owned links and refuses foreign paths.

For Prime Agent only, installation creates or updates one section delimited by
`<!-- pce:guidance:start -->` and `<!-- pce:guidance:end -->` in
`~/.prime/agent/AGENTS.md`. It points to writing guidance before GitHub drafting
and test-first guidance before code or test implementation and review. Unrelated
instructions, files, links, extensions, and settings remain unchanged. Ambiguous
section markers, linked global instruction files, or alternate global instruction
files that would be hidden are refused; inspect the reported conflict before
retrying. No downstream project edits are needed.

Prime Agent includes global and project instructions in its system prompt.
Compaction retains that loaded prompt, not a fresh disk read. These persistent
pointers rely on the agent following them to read skill bodies on demand; they
do not automatically invoke skills or keep their full bodies in context.
Use `/reload` or restart an existing session to refresh changed instructions or
installed resources, not after each compaction.

## Skills

- [grill-me](skills/grill-me/SKILL.md): Clarify an idea through focused questions.
- [to-vision](skills/to-vision/SKILL.md): Author, publish, and verify a durable vision.
- [implement-vision](skills/implement-vision/SKILL.md): Implement a vision with delegation and independent review.
- [chart-program](skills/chart-program/SKILL.md): Chart a large outcome as a Program and Effort tickets.
- [grill-ticket](skills/grill-ticket/SKILL.md): Discover one Effort and publish its vision.
- [land-ticket](skills/land-ticket/SKILL.md): Verify delivery, land an Effort, and update its Program.

Supporting guidance (loading it starts no workflow):

- [github-writing](skills/github-writing/SKILL.md): Write clear GitHub bodies and publication, delivery, and landing summaries.
- [test-first-development](skills/test-first-development/SKILL.md): Implement and review code and tests with small, meaningful test-first increments.

## Workflows

```text
grill-me → to-vision → implement-vision
chart-program → grill-ticket → implement-vision → land-ticket
```
