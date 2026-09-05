# Remove retired-product cruft from skill text

## Outcome

The six PCE skills describe only the workflow that exists today. No skill line argues with the pre-2026-08-31 orchestrator, names an artifact that product produced, clamps the model's output with a number, or tells the model to try hard. Each instruction is either context only the author knows or a constraint whose reason is visible beside it.

This matters because current models follow instructions closely and literally. A prohibition against a failure the model would never make anchors it toward that failure. A migration-relative sentence written as a diff against an older prompt version introduces alternatives the model never saw. Both make the skills behave worse under every supported environment, not just one.

## Why now

A prompt audit on 2026-09-05 (target model Claude Fable 5.1, the current Claude Code default) scanned `AGENTS.md`, `CLAUDE.md`, and every `skills/*/SKILL.md`. The surface is close to clean: zero caps-lock emphasis, zero thinking scaffolds, zero retired model names, zero update suppressors or anti-formatting rules, no request code, and plain skill trigger descriptions. Every line in scope was written between 2026-08-31 and 2026-09-03. The one fossil is the retired PCE product itself. Five sentences still reference it or carry emphasis written for older models.

## The five edits

These are the settled changes. Each is one hunk. Replacement wording is settled at the level of meaning; the implementing agent may polish phrasing as long as the test contracts below still hold.

1. **`skills/chart-program/SKILL.md`, second paragraph.** Replace "Keep discovery in GitHub issues and repository visions. Do not invoke `grill-with-docs`, edit `CONTEXT.md`, create ADRs, or restore domain-modeling or retired PCE runtime machinery." with a positive statement: GitHub issues and repository visions are the only discovery records, and the canonical `grill-me` interview is the only discovery mechanism.

2. **`skills/grill-ticket/SKILL.md`, second paragraph.** Same change for the ticket: the ticket and its repository vision are the only discovery records, and the canonical `grill-me` interview is the only discovery mechanism. The old prohibition sentence goes.

3. **`skills/to-vision/SKILL.md`, Authoring contract.** Replace "Do not impose fixed headings, a JSON schema, command-level acceptance syntax, or a universal template." with a positive statement that there is no required template or heading set. "JSON schema" and "command-level acceptance syntax" are vocabulary of the retired criteria machinery; the template concern stays.

4. **`skills/chart-program/SKILL.md`, Approval gate.** Replace "**Destination:** at most three sentences." with a qualitative format instruction such as "a brief statement of the outcome." The neighbouring "one-sentence outcome question" stays because it defines what an outcome question is.

5. **`skills/chart-program/SKILL.md`, Investigate first.** Delete "Spend enough effort to recover complete current state." The sentence before lists what to read and the sentence after says what not to ask.

6. **`skills/grill-me/SKILL.md`, round loop.** Delete "If the frontier is already empty, do not manufacture a question merely to create interaction." The Completion Gate already says "When the frontier is empty, do not ask a ceremonial question" where it applies. That copy stays.

(Edits 1 and 2 were one finding in the audit; they are listed separately here because they are separate files.)

## Tests

`tests/test_skill_contracts.py` pins exact prompt phrases. One test currently asserts the retired prohibition: `test_program_discovery_rejects_retired_document_workflow` requires "Do not invoke `grill-with-docs`", "`CONTEXT.md`", "ADRs", and "domain-modeling" in `chart-program` and `grill-ticket`. Replace it with a test that asserts the positive rule is present and that none of `grill-with-docs`, `CONTEXT.md`, `ADR`, and `domain-modeling` appears in either skill. That inverted assertion is what `AGENTS.md` already asks for: skill instructions independent of retired PCE machinery.

No other test pins any removed phrase. The existing assertion in `test_program_skills_compose_canonical_grill_without_copying_its_loop` that "grill-with-docs` skill" is absent stays valid.

Evidence of success: `python3 -m unittest discover -s tests -v` passes with all tests, and a grep of `skills/` for `grill-with-docs`, `CONTEXT.md`, `ADR`, `domain-modeling`, `JSON schema`, `command-level acceptance`, `at most three sentences`, `Spend enough effort`, and `manufacture a question` returns nothing.

## Constraints

- Skill text stays harness-neutral. It runs identically in Claude Code, Codex, and Prime Agent, so no edit may introduce a model name, an API feature, or conditional harness language. The existing `test_skill_text_is_harness_agnostic` enforces part of this.
- This is not a shortening pass. Cruft is specific outdated instruction, not length. Do not trim anything outside the six edits.
- Rewrites beat bare deletions where the instruction has a live purpose. Edits 1 through 4 keep their concern and state it positively.

## Explicitly out of scope

The audit matched these on a grep and kept them on purpose. Do not touch them.

- The `landed` predicate paragraph that appears byte-identically in `chart-program`, `implement-vision`, and `land-ticket`. Each skill runs standalone and a test enforces the match.
- The ban on worktrees under `/private/tmp` in `implement-vision`. The Claude Code harness actively directs temporary files there, so the failure reproduces today.
- The list of GitHub closing keywords in `implement-vision`. Auto-closing an Effort is irreversible; fragile operations keep exact scripts.
- "Investigate before asking" and its siblings across skills. Over-asking is a demonstrated current-model failure in this project.
- The `grill-me` question format block. A human answers by number; it pins a format-sensitive output.
- Two low-confidence flags with no edit: "end the turn normally without a continuation loop" in `implement-vision` (possibly load-bearing on Prime Agent, and test-pinned) and "Keep skill instructions independent of retired PCE machinery" in `AGENTS.md` (a maintainer rule, and the rule these edits enforce).
- `README.md`, `install.sh`, `tests/test_install.py`, and `planning/visions/` still mention `grill-with-docs`. Those describe the installer's legacy cleanup or are historical records, not instructions to the model. Leave them.

## Risk

If a downstream user has a foreign `grill-with-docs` skill installed from another source, the explicit "Load and follow PCE's canonical `grill-me` skill" instruction is what pins the interview mechanism. The audit judged that sufficient. If a later session shows a Program skill wandering into another interview skill, restate that one constraint in its minimal positive form rather than restoring the prohibition list.
