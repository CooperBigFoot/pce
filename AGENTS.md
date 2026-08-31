# PCE Repository Instructions

PCE is a lean distribution of three Agent Skills. The tracked product surface is limited to `skills/`, the durable vision in `planning/visions/`, standard-library tests, and repository documentation.

## Validation

Run all local tests from the repository root:

```bash
python3 -m unittest discover -s tests -v
```

Do not add a compiled application, package manager, CI workflow, orchestration runtime, or generated runtime state. Keep skill instructions independent of retired PCE machinery. Installer changes must preserve unrelated user state and must be tested with an isolated temporary `HOME`.
