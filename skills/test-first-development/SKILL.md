---
name: test-first-development
description: Use when implementing or reviewing code or tests. Develop behavior in small test-first increments with meaningful coverage.
---

# Test-first Development

Loading this guidance does not start implementation or another workflow.

Read and follow the target repository's testing guidance when implementing or reviewing tests; keep project-specific rules in that repository, in its existing layout. Use repository-native tools and the simplest sufficient test level.

For bug fixes and new behavior, default to small test-first increments: choose a meaningful behavior, write a focused test with an independently established expected result, observe that it fails for the intended reason, implement the behavior, then improve the code while keeping tests passing. Derive expectations from requirements, independently worked examples, or suitable independent evidence, never from the implementation under test. An unrelated setup failure is not proof. Do not batch a large test suite before a large implementation.

Reuse coverage that already proves the promise. Add cases for distinct relevant failures, not every input combination. Avoid duplicate coverage, incidental implementation assertions, elaborate fixtures, and parallel implementations where a small example suffices. Allow narrow, explained exceptions when a new failing test adds no value, including prose-only changes and refactors already protected by existing tests. Repository requirements and changed behavior determine validation; no new test per edit, test inventory, extra approval gate, or repeated full-suite run is required merely for this procedure.
