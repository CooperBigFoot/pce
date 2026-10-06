# Global skill guidance restoration after compaction

PCE's GitHub writing and test-first development standards should remain available after Prime Agent compacts a conversation. The owner observed an issue written after compaction that no longer followed the carefully authored writing guidance. Preserve the instructions themselves rather than relying on a conversation summary to remember them.

The solution should work across the owner's Prime Agent projects after one global installation. Do not require edits to existing projects' `AGENTS.md` files, project-local hook configuration, repeated commands, or a new user-facing workflow step. Keep the six existing planning, discovery, implementation, and landing workflows intact.

## One source for each standard

Extract the GitHub writing rules currently in `skills/to-vision/publication.md` into a dedicated Markdown skill. Extract the test-first development rules currently in `skills/implement-vision/SKILL.md` into a separate dedicated Markdown skill. These are supporting guidance skills, not new workflows. Update the existing skills and publication reference to load the applicable skill rather than retain competing copies of its rules. Relative references between sibling installed skill directories should work both from this checkout and through the globally installed links.

Preserve the substance of the current standards. GitHub text should explain the problem or change and why it matters in plain, concrete language. Structure is optional, necessary technical detail can live in clearly named collapsed details, and current blockers and material limitations remain visible. Preserve required evidence, canonical links, markers, provenance, dependencies, delivery records, privacy, and no-auto-close safeguards.

Test-first development means small meaningful red/green/refactor increments, independently established expectations, and observing a failure for the intended reason before implementing behavior. Use the target repository's testing guidance and simplest sufficient coverage. Preserve the rules against unnecessary test combinations, duplicate coverage, incidental implementation assertions, elaborate fixtures, and parallel implementations. Retain narrow explained exceptions, including prose-only changes and refactors already protected by coverage. Do not turn this into a generic “use TDD” reminder or new testing bureaucracy. Keep the existing independent-review requirements and test-quality review effective.

Preserve the skill-authoring principles already used by PCE and reinforced by the owner's supplied OpenAI guidance: short descriptions with precise task triggers, concise instruction bodies, progressive disclosure, and no elaborate itineraries or redundant scaffolding. Do not broaden descriptions so that unrelated work loads these skills. Longer supporting references, if needed, remain progressively loaded; these two short single-purpose skills do not need an artificial router or extra documents just to demonstrate that pattern. Do not add blanket repository-reading rules, repeated test rituals, model-specific prompting, or new approval gates. Preserve actual workflow authority and safety contracts rather than using prompt cleanup to weaken them. Merely loading a guidance skill must not start publication, implementation, or another workflow.

## Global installation without project edits

Ship a small Prime Agent extension in this repository and extend the existing installer to install it globally alongside the supporting skills. Use the current linked-installation approach: skill directories point back to this checkout, and the extension is linked into Prime Agent's global extension discovery directory, `~/.prime/agent/extensions/`. The rules remain in the skill files, not duplicated in the extension or a generated prompt copy.

The existing installer links skills into Prime Agent, Claude Code, and Codex. Keep those installations working and make the new supporting skills available wherever the PCE workflow references them. The compaction extension is Prime Agent-specific; do not build equivalent hooks or compatibility machinery for other harnesses. No project-by-project opt-in is required: installing this global extension enables restoration across the owner's Prime Agent projects. Each standard applies to its relevant activity, and project-specific testing rules remain in their existing repositories.

Preserve unrelated user files, links, extensions, and settings. Keep installation safe to rerun and refuse conflicting foreign paths rather than replacing them. Explain the one-time installation, the need to retain the source checkout, and how existing sessions load installed or changed extensions through `/reload` or restart. Reading guidance at restoration time should use the current installed skill files, so changing guidance does not require maintaining a second copy or editing every project.

This extends PCE's current six-skill-only distribution with supporting skills and one narrowly scoped extension. Update PCE's own documentation and repository instructions where needed to describe that agreed surface; do not edit downstream projects' `AGENTS.md` files. Do not add a compiled application, package manager, CI workflow, orchestration runtime, generated runtime state in the repository, or retired PCE machinery.

## Restore once after compaction, not at every message

Use Prime Agent's compaction and context extension events as a coordinated pair:

1. A successful compaction marks restoration as pending for that session.
2. At the next context event, before the model resumes work, load the full bodies of the two supporting skills and restore them to its context.
3. Clear the pending state once restoration succeeds. Later context events add nothing until another successful compaction arms restoration again.

The context callback may be called before every model request, but that must not cause injection on every request, user message, or tool step. Restore both short guidance skills rather than constructing an elaborate active-skill detector. Do not load the entire PCE workflow or all installed skills.

Load the actual Markdown instructions, with their source identity and reference base preserved. In this harness a Markdown skill invocation reads its body into context; a literal `/skill:name` string inside a custom message is not sufficient evidence of invocation. Do not ask the agent merely to remember to retrieve a pointer, rely on a summary preserving the rules, or maintain a separately summarized version of them.

The restored guidance must remain available for subsequent work, not only the first request after compaction. Prime Agent's context hook modifies a request copy non-destructively, so implementation must verify retention rather than assume a one-off request mutation persists in the session. Meet both constraints: effective restoration and no reinjection on ordinary subsequent requests. Prevent stale or duplicate restoration blocks from accumulating across repeated compactions.

Restoration must not start an extra model turn, interrupt or restart a workflow, or change publication and merge authority. Cancelled, skipped, or failed compaction must not be treated as a successful restoration boundary. Do not silently mark restoration complete if a required skill cannot be loaded; make the failure visible and retain a recoverable state rather than claim success.

Keep restoration state isolated to the correct session. Cover delegated agents as well as the parent, and verify the relevant Prime Agent delegate paths rather than assume a parent's extension closure is safely inherited. Loading or restarting the extension must not create periodic or every-message injection. Preserve pending restoration across a relevant reload or resume boundary so the next model request cannot silently skip it.

## Harness evidence and implementation boundary

The owner's installed stable Prime Agent is the TypeScript/Bun `v0.9.8` release. The official stable release manifest was checked during discovery and matched that installation. Its extension API supports `session_compact` after successful compaction and `context` before model calls. Its automatically loaded `AGENTS.md` contents stay in the system prompt, but compaction does not reread those files from disk. Full skill bodies read into conversation history can be summarized away.

### Mandatory documentation read before implementation

Before designing or implementing the extension, the implementing agent must read the installed Prime Agent documentation directly at:

```text
~/.local/share/prime-agent/releases/0.9.8-darwin-arm64-078c9abd519978ef27f6404367e37a2981267db47f1b95b60940ea7fe6ead8b9/docs/
```

Read `extensions.md`, `compaction.md`, and `skills.md` in full, and follow applicable references for message/session persistence, extension loading, and reload behavior. Reviewers must independently consult the relevant documented contracts when verifying the extension. This is a required investigation step: this vision's API descriptions and the discovery agent's summary are not substitutes for reading those documents. Verify assumptions against the documented contract and inspect the released source where the documentation leaves an important question unanswered. If the installed release has changed, locate and read the documentation for the runtime actually being used; do not assume the older summary remains correct.

Use the documentation and source for the actual installed release, not an old working checkout or an unrelated development architecture:

- [Official extension documentation](https://github.com/PrimeIntellect-ai/prime-agent/blob/v0.9.8/packages/coding-agent/docs/extensions.md), including global locations, compaction events, and context events.
- [Official skill documentation](https://github.com/PrimeIntellect-ai/prime-agent/blob/v0.9.8/packages/coding-agent/docs/skills.md).
- [Released session implementation](https://github.com/PrimeIntellect-ai/prime-agent/blob/v0.9.8/packages/coding-agent/src/core/agent-session.ts), including Markdown skill expansion and post-compaction message reconstruction.

At discovery time upstream `main` had moved to a Rust implementation without the released TypeScript extension API. Do not turn that development change into a compatibility project, require a harness migration, or ask the owner to choose internal hook mechanics. Verify the supported API against the runtime used for implementation and validation. If that runtime no longer supports the necessary behavior, report the concrete blocker instead of shipping an inert extension or silently weakening restoration.

## Evidence of completion

Demonstrate that one global installation makes the guidance available across projects without editing their instructions. Verify sibling skill references through installed links and that changes to the source guidance are read at the next restoration. Test installer success, reruns, conflicts, and preservation with an isolated temporary `HOME`.

Verify the extension behavior with the installed harness's actual contract, using focused, proportional tests and an integration check where needed:

- A normal context call before compaction adds no restoration block.
- Successful manual and automatic compaction restore both complete skill bodies before the next model request, including automatic continuation without a new user prompt.
- Later model requests retain the restored guidance without another injection; another successful compaction permits exactly one fresh restoration without accumulating duplicate blocks.
- Failed or cancelled compaction does not arm restoration; missing guidance does not silently consume a pending restoration.
- Session reload/resume and delegated execution preserve the required behavior without cross-session state leakage or extra agent turns.

Use tests that prove these behaviors, not only literal string inventories. Preserve the repository's standard-library validation command, `python3 -m unittest discover -s tests -v`, and keep any additional extension validation lean and isolated rather than introducing a package-managed application. Check representative GitHub text and test-first behavior against the preserved guidance without introducing mandatory output templates.

This vision authorizes its described future implementation only when explicitly handed to `implement-vision`. Publishing it does not install the extension, edit the workflow skills, update other projects, or start implementation.
