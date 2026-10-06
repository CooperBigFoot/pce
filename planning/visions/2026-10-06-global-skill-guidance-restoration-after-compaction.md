# Global skill guidance restoration after compaction

PCE's GitHub writing and test-first development standards should remain available after Prime Agent compacts a conversation. The owner observed an issue written after compaction that no longer followed the carefully authored writing guidance. Keep task-triggered pointers in Prime Agent's persistent global instructions, with the detailed rules in dedicated skills, rather than relying on a conversation summary to remember them.

The solution should work across the owner's Prime Agent projects after one global installation. Do not require edits to existing projects' `AGENTS.md` files, project-local hook configuration, repeated commands, or a new user-facing workflow step. Keep the six existing planning, discovery, implementation, and landing workflows intact.

## One source for each standard

Extract the GitHub writing rules currently in `skills/to-vision/publication.md` into a dedicated Markdown skill. Extract the test-first development rules currently in `skills/implement-vision/SKILL.md` into a separate dedicated Markdown skill. These are supporting guidance skills, not new workflows. Update the existing skills and publication reference to load the applicable skill rather than retain competing copies of its rules. Relative references between sibling installed skill directories should work both from this checkout and through the globally installed links.

Preserve the substance of the current standards. GitHub text should explain the problem or change and why it matters in plain, concrete language. Structure is optional, necessary technical detail can live in clearly named collapsed details, and current blockers and material limitations remain visible. Preserve required evidence, canonical links, markers, provenance, dependencies, delivery records, privacy, and no-auto-close safeguards.

Test-first development means small meaningful red/green/refactor increments, independently established expectations, and observing a failure for the intended reason before implementing behavior. Use the target repository's testing guidance and simplest sufficient coverage. Preserve the rules against unnecessary test combinations, duplicate coverage, incidental implementation assertions, elaborate fixtures, and parallel implementations. Retain narrow explained exceptions, including prose-only changes and refactors already protected by coverage. Do not turn this into a generic “use TDD” reminder or new testing bureaucracy. Keep the existing independent-review requirements and test-quality review effective.

Preserve the skill-authoring principles already used by PCE and reinforced by the owner's supplied OpenAI guidance: short descriptions with precise task triggers, concise instruction bodies, progressive disclosure, and no elaborate itineraries or redundant scaffolding. Do not broaden descriptions so that unrelated work loads these skills. Longer supporting references, if needed, remain progressively loaded; these two short single-purpose skills do not need an artificial router or extra documents just to demonstrate that pattern. Do not add blanket repository-reading rules, repeated test rituals, model-specific prompting, or new approval gates. Preserve actual workflow authority and safety contracts rather than using prompt cleanup to weaken them. Merely loading a guidance skill must not start publication, implementation, or another workflow.

## Global installation without project edits

Extend the existing installer to install the two supporting skills globally using the current linked-installation approach. The installer already links PCE skills into Prime Agent, Claude Code, and Codex; keep those installations working and make the supporting skills available wherever the existing workflow references them. Skill directories point back to this checkout, which must remain in place. Do not maintain generated or copied versions of the detailed guidance.

For Prime Agent, install a small, clearly PCE-owned section in the global `~/.prime/agent/AGENTS.md` that tells the agent when to read each skill. Use the installed skill names and resolvable locations. The intended instructions are:

> Before drafting or revising a GitHub issue or PR body, read and follow the GitHub-writing skill.
>
> Before implementing or reviewing code or tests, read and follow the test-first-development skill.

Choose concise wording and concrete links consistent with the installed skill names. Keep the detailed rules in the skills, not in the global file. Do not add “after compaction” wording, repeat the entire workflow, or require reading both skills for unrelated tasks. Existing workflow references should continue to cover publication, delivery, and landing summaries where the current writing rules apply.

Prime Agent loads the global instructions alongside discovered project instructions into its system prompt. Compaction retains that loaded prompt; it does not reread `AGENTS.md` from disk. The persistent task-triggered pointers are the agreed mechanism for retrieving the relevant skill bodies when needed. This design relies on the agent following those instructions; it does not promise automatic reinjection or permanent retention of full skill bodies. No compaction-triggered reload is required. Explain `/reload` or restart only for refreshing installed resources or global instructions changed during an existing session.

No project-by-project installation, opt-in, configuration, or edits to downstream projects' `AGENTS.md` files are required. Project-specific instructions and testing guidance stay in their existing repositories. Do not modify other harnesses' global instruction files; the global instruction integration here is for Prime Agent.

Preserve unrelated user content in the global `AGENTS.md`, as well as unrelated files, links, extensions, and settings. Create the global file when absent. When present, add or update only PCE's clearly delimited section; do not replace the whole file, take ownership of unrelated instructions, or accumulate duplicate sections on rerun. Handle conflicting or ambiguous ownership safely rather than guessing or silently overwriting content. Preserve the existing installer's protection against replacing foreign skill paths. Validate all installer mutations with an isolated temporary `HOME`, never the owner's real global instructions as a test fixture.

Update PCE's own documentation and repository instructions where needed to describe the additional supporting skills and the global instruction section. Keep installation and updates simple. Do not add an extension, compaction/context hooks, pending-restoration state, session tracking, a compiled application, package manager, CI workflow, orchestration runtime, generated runtime state in the repository, or retired PCE machinery.

## Read the harness documentation directly

Before implementing the global instruction integration, the implementing agent must read the relevant installed Prime Agent documentation directly at:

```text
~/.local/share/prime-agent/releases/0.9.8-darwin-arm64-078c9abd519978ef27f6404367e37a2981267db47f1b95b60940ea7fe6ead8b9/docs/
```

Read `skills.md` and `compaction.md`, and the shipped README or relevant referenced documentation covering global/project `AGENTS.md` discovery, system-prompt inclusion, installed resource paths, and `/reload`. Reviewers must independently consult the relevant contracts. This is a required investigation step: this vision and prior agents' summaries are not substitutes for reading the documentation. Inspect the released source where the documentation leaves an important question unanswered. If the installed release has changed, read the documentation for the runtime actually used rather than treating the older summary as authoritative.

Useful released references are [skills.md](https://github.com/PrimeIntellect-ai/prime-agent/blob/v0.9.8/packages/coding-agent/docs/skills.md), [compaction.md](https://github.com/PrimeIntellect-ai/prime-agent/blob/v0.9.8/packages/coding-agent/docs/compaction.md), and the [shipped README](https://github.com/PrimeIntellect-ai/prime-agent/blob/v0.9.8/packages/coding-agent/README.md). The implementation does not depend on the extension API and does not require an upstream harness fix or a compatibility project.

## Superseded hook design

This revision replaces the earlier extension-based design and its daemon/inline scope distinction. Implementation investigation reported that request-only context injection was not retained, while persistent delivery started an extra turn despite `triggerTurn: false`; inline delegation also had session-isolation problems. These explain why the owner chose persistent global instructions instead. Do not resume hook development, reproduce those probes as a prerequisite, repair Prime Agent, or carry the old restoration-state and delegate-hook acceptance requirements into this simpler solution. Preserve any existing investigation evidence without treating it as product machinery.

## Evidence of completion

Demonstrate the agreed result with focused, proportional checks:

- The writing and TDD guidance each have one authoritative supporting skill, preserving their existing substance and concise, task-specific descriptions. Existing workflow references resolve both from the repository and through installed global skill links.
- One installer run makes the supporting skills available and creates or updates exactly one PCE-owned section in Prime Agent's global `AGENTS.md`. Repeated runs do not duplicate it. Unrelated content and user state remain unchanged; conflicts do not cause destructive replacement.
- An isolated-home check verifies that the global guidance pointers and a fixture project's local instructions can both be loaded into Prime Agent's system prompt. Establish from the actual harness contract that compaction retains this prompt rather than relying on a conversation summary. Do not impose a new live-model compaction benchmark or extension test matrix.
- The global section uses task triggers, not special compaction instructions. There are no extension files, hook registrations, automatic reloads, or edits to downstream projects needed for this feature.
- Documentation explains installation, source-checkout retention, and when an existing session needs `/reload` or restart. It distinguishes persistent pointers from on-demand skill bodies without claiming automatic skill invocation.

Run `python3 -m unittest discover -s tests -v` and test installer behavior with an isolated temporary `HOME`. Use meaningful behavioral checks rather than only literal string inventories, but do not introduce a package-managed application or testing bureaucracy. Keep independent review and all existing workflow safety and authority boundaries.

This vision authorizes its described future implementation only when explicitly handed to `implement-vision`. Publishing this revision does not install skills, modify the owner's global instructions, edit workflow skills, or start implementation.
