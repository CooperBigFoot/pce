# PCE Repository Instructions

PCE is a lean distribution of six workflow skills and two supporting guidance skills. The tracked product surface is limited to `skills/`, durable visions in `planning/visions/`, standard-library tests, and repository documentation.

Before implementing or reviewing code or tests, read [test-first development](skills/test-first-development/SKILL.md). Before drafting or revising GitHub text, read [GitHub writing](skills/github-writing/SKILL.md).

## Validation

Run all local tests from the repository root:

```bash
python3 -m unittest discover -s tests -v
```

Do not add a compiled application, package manager, CI workflow, orchestration runtime, or generated runtime state. Keep skill instructions independent of retired PCE machinery. Installer changes must preserve unrelated user state and must be tested with an isolated temporary `HOME`.
