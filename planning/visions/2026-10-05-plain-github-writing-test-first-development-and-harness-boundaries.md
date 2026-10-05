# Plain GitHub writing, test-first development, and harness boundaries

PCE should keep its working six-skill workflow while improving the writing people read on GitHub, the way agents develop and review tests, and the boundary between workflow guidance and harness instructions.

The owner should be able to read an issue or PR and explain what it is about to another person without rereading it or asking an agent for a translation. Tests should protect intended behavior rather than confirm whatever an agent happened to implement. The skills should supply the owner's standards, not duplicate the harness's execution instructions.

## Write for the person reading

Apply shared writing principles wherever the workflow authors GitHub issues, PR bodies, and delivery or landing summaries. Start with a short, plain explanation of the problem or change and why it matters. Use concrete actions and examples instead of dense technical noun phrases. Technical precision remains important, but a reader should not need implementation knowledge to understand the point.

Simple issues may be reminders, placeholders, or brief bug reports. Include what is needed to take up the work; do not force discovery, a solution design, or a detailed acceptance checklist into every issue. The later grill is where deeper discovery happens. This does not remove the established Program and Effort contracts.

Do not mandate a heading set, diagram, glossary, evidence section, or risk label. Use structure only when it helps this particular explanation. No GLOSSARY.md convention is required. Visuals and before/after comparisons are optional tools, not obligations.

Put necessary technical detail in optional, clearly named HTML `<details>` blocks that GitHub renders collapsed. Organize it by its use, such as reproduction, investigation findings, or verification, rather than pasting working notes. Do not create an appendix just to satisfy a template. Existing durable records may be linked instead of duplicated. Current blockers, breaking changes, and material limitations must remain visible in the main explanation.

Describe the current state, not an accumulating diary of intermediate test runs and review attempts. Preserve required verification evidence, reproduction information, and durable recovery records; simplify their presentation rather than deleting them. Keep private material private. Preserve canonical links, markers, provenance, dependencies, authoritative delivery records, and no-auto-close safeguards. Readable writing must not weaken the workflow's machine-checkable contracts.

For example, a technical phrase such as “request-bounded, source-faithful reading of compiled stores” can become “Reading one month of data should not require checking the whole national dataset.” Follow with the observed problem, the intended improvement, and any important constraint. This is a writing illustration, not a domain requirement or fixed template.

## Develop behavior test-first

Make test-first development the default for bug fixes and new behavior. Work in small increments: choose a meaningful behavior, write a focused test with an independently established expected result, observe that it fails for the intended reason, and implement the behavior. Improve the code while keeping the tests passing. Do not write a huge batch of tests followed by a huge implementation.

The test must detect the claimed defect or missing behavior, not fail because of an unrelated setup error. Expected answers come from requirements, independently worked examples, or suitable independent evidence, not from the implementation being tested. Ordering alone does not make a test useful.

Choose the simplest sufficient level of testing. Reuse existing coverage when it already proves the promise. Add cases for distinct, relevant failures rather than every imaginable input combination. Avoid duplicate coverage, fragile assertions about incidental implementation details, and elaborate fixtures or parallel implementations where a small example would suffice.

Allow narrow, explained exceptions when a new failing test adds no value, including prose-only changes. Existing tests may already protect a behavior-preserving refactor. Do not require a new test for every edit, a mandatory test inventory, a separate approval gate, or repeated full-suite runs solely to satisfy an instruction. Repository requirements and the actual changed behavior determine necessary validation.

Read the target repository's testing guidance when implementing or reviewing tests. A short governing principle and a contextual pointer in AGENTS.md can coexist with a detailed project testing guide; PCE should not require relocating that guidance or impose a particular file layout. Project-specific testing rules remain in the target project.

## Review the proof and the scope

Retain fresh independent full-diff review. The reviewer must assess both whether the change delivers the vision and whether it fits the repository without unnecessary behavior or complexity.

Review tests as part of the design, not as a passing-count report. Check the promise each meaningful new case protects, whether its expectation is independent, whether it reaches the intended rule, and whether a plausible broken implementation could still pass. Challenge unnecessary duplication and implementation-shaped assertions. When coverage is removed or changed, verify that its useful guarantee remains protected or is intentionally no longer required.

Large PRs should prompt a necessity review: what can be removed without weakening the agreed outcome? This is not a minimal-patch mandate or a reason to reject justified redesign. Avoid a second mandatory review team, a long checklist report, or a new testing bureaucracy. Concise instructions should establish the standard and leave ordinary engineering judgment to the agents.

## Let the harness own execution

This is the owner's personal workflow, not a product requiring a community-support matrix. The owner uses Prime Agent and guarantees delegation and independent-review capabilities are available. Keep skill instructions harness-agnostic: do not name Prime Agent or embed its API recipes in the skills, and do not add capability detection or fallback machinery for environments the owner does not use.

Inspect all six skills and their supporting references for general delegation and execution instructions already supplied by the harness. Remove redundant advice about when to delegate, spawning and messaging agents, waiting and resumption, nonblocking execution, routine progress reporting, and generic persistence or planning mechanics. The harness already decides when independent substantive work benefits from delegation and how to coordinate it. A replacement incentive to delegate is unnecessary.

Keep workflow-specific requirements that apply when delegation happens: complete vision and relevant constraints, ownership and checkout-preservation rules, and the context needed to perform independent review. Independent review by someone other than the implementer remains mandatory; it is a quality requirement, not an orchestration recipe. Preserve publication, implementation, merge, and landing authority boundaries and verified completion criteria.

Do not remove the assumption that delegation is available in the name of portability. Remove competing execution instructions, not the workflow responsibilities they happened to surround.

## Keep the change lean

The existing implementation skill already requires requirement-to-evidence reasoning and independent review. Its delegation and progress language overlaps harness instructions. The shared vision-publication procedure also contains nonblocking execution advice. These are concrete places to inspect, not permission to replace the workflow.

Use concise, narrowly triggered guidance and one authoritative definition of shared rules. Load supporting material only when relevant. Choose the smallest suitable arrangement within the existing six-skill distribution; no additional user-facing workflow step is needed. Avoid turning these outcomes into elaborate itineraries or mandatory output sections.

Matt's skills supplied useful ideas, not a workflow to import wholesale. Do not add mandatory retrospectives, an integration-branch delivery model, a glossary, a tracker abstraction, new orchestration machinery, or a broad rewrite of unrelated instructions. Do not change the established Program/Effort semantics, target-branch durability, independent review, preservation rules, or authority boundaries.

RivRetrieve was an example used to investigate unreadable GitHub text and a useful testing philosophy. Do not introduce its library, provider, archive, source-certification, or scientific-data requirements into PCE. Do not edit that repository or rewrite its existing GitHub content as part of this work.

## Observable completion

The six skills and applicable references consistently express these writing, test-first, review, and harness-boundary decisions without conflicting duplicate instructions. Representative simple bug, reminder, implementation PR, and documentation PR examples should make the problem or change understandable without opening technical details; any required metadata and important blockers remain intact. Examples are a validation aid, not mandatory templates or a new test framework.

Implementation and review guidance should distinguish meaningful test-first work from unnecessary testing, preserve repository-specific rules, and challenge both weak proof and redundant coverage. Existing workflow and safety requirements remain effective after execution-mechanics cleanup. Validate the changes with proportional standard-library tests and the repository's existing command:

```bash
python3 -m unittest discover -s tests -v
```

Do not add a compiled application, package manager, CI workflow, orchestration runtime, generated runtime state, or retired PCE machinery. This vision authorizes only its described future implementation when explicitly handed to implement-vision; publishing it does not start that implementation.
