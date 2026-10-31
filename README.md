# PCE

PCE provides workflow skills for turning ideas into delivered work, plus supporting guidance skills.

## Install

With Bash and Python 3 available, run from this checkout:

```bash
./install.sh
```

All skills listed below are linked into Claude Code (`~/.claude/skills`), Codex
(`~/.codex/skills`), Prime Agent (`~/.prime/agent/skills`), and Gemini CLI /
Antigravity CLI (`agy`) through their shared `~/.gemini/skills` directory. Keep this checkout
in place: the links load its files, not copied guidance. Pull updates here and
rerun the installer. It replaces checkout-owned links and refuses foreign paths for supported skills.
It removes checkout-owned `land-ticket` and `grill-ticket` links, including dangling
links, but preserves foreign links and user-owned files or directories at those retired paths.

Installation creates or updates one section delimited by
`<!-- pce:guidance:start -->` and `<!-- pce:guidance:end -->` in each of
`~/.prime/agent/AGENTS.md` and `~/.gemini/AGENTS.md`. These independent files
point to their own installed skill paths: writing guidance before GitHub drafting
and test-first guidance before code or test implementation and review. Unrelated
instructions, files, links, extensions, and settings remain unchanged. Ambiguous
section markers, linked global instruction files, or alternate global instruction
files that would be hidden are refused; inspect the reported conflict before
retrying. Gemini’s existing `GEMINI.md` rules are preserved and loaded alongside
`AGENTS.md`; only the alternate spelling `AGENTS.MD` conflicts when `AGENTS.md`
is absent. No downstream project edits are needed.

Prime Agent includes global and project instructions in its system prompt.
Compaction retains that loaded prompt, not a fresh disk read. These persistent
pointers rely on the agent following them to read skill bodies on demand; they
do not automatically invoke skills or keep their full bodies in context.
Use `/reload` or restart an existing session to refresh changed instructions or
installed resources, not after each compaction.

## Skills

- [grill-me](skills/grill-me/SKILL.md): Clarify and design an idea, or resume a saved discussion issue.
- [save-discussion](skills/save-discussion/SKILL.md): Save a discussion to a GitHub issue, or branch an experiment issue from it.
- [to-vision](skills/to-vision/SKILL.md): Author, publish, and verify a durable vision.
- [implement-vision](skills/implement-vision/SKILL.md): Implement a vision with delegation and independent review.
- [chart-program](skills/chart-program/SKILL.md): Chart a large outcome as a Program and its Efforts.

Supporting guidance (loading it starts no workflow):

- [github-writing](skills/github-writing/SKILL.md): Write clear GitHub bodies and publication and completion summaries.
- [test-first-development](skills/test-first-development/SKILL.md): Implement and review code and tests with small, meaningful test-first increments.

## Workflows

```text
grill-me ⇄ save-discussion → to-vision → implement-vision
chart-program → grill-me <issue> → to-vision → implement-vision
```

A discussion is an ordinary GitHub issue with the `discussion` label. Saved grills
and Efforts charted under a Program use the same mechanism: comments append context,
and `grill-me <issue>` resumes from it. Nothing is published until the human invokes
`to-vision`.

When a decision needs evidence, a linked experiment issue carries the question. Its
vision authorizes bounded experimental work, and implementation returns the findings
to the originating discussion, which stays open.

Publication leaves its issue open. Implementation closes only the issue a vision explicitly
designates to complete, after the whole outcome is delivered and verified on the
target branch; partial PRs do not imply completion. Programs are not automatically closed.
