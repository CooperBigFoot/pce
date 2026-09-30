# Lean skills-only repository

## Outcome

PCE is a distribution of six Agent Skills. Make the repository reflect that purpose: the skills are the product, and everything around them should be small and useful. Remove obsolete repository artifacts and migration machinery without changing skill behavior or rewriting Git history.

## Agreed scope

- Keep all six skills: `grill-me`, `to-vision`, `implement-vision`, `chart-program`, `grill-ticket`, and `land-ticket`. Preserve their current behavior and the helper script under `skills/to-vision/scripts/`.
- Keep a small installer, useful standard-library tests, and minimal repository guidance. Remove duplicated workflow explanations from supporting documentation; detailed operating rules belong in the skills.
- Remove completed historical visions from the current tree. Their history remains in Git. Verify completion before removal; do not discard active or uncertain work. This cleanup vision must remain available for its own implementation handoff and verification.
- Remove installer logic for retired PCE commands, skills, and hooks. The user confirms those legacy artifacts have already been removed; automatic migration cleanup is no longer needed.
- The installer installs only the current skills. Preserve support for Claude Code, Codex, and Prime Agent, safe reruns, unrelated user files and settings, and refusal of conflicting files or foreign links. It must no longer edit old hook settings or maintain retired artifact inventories.
- Delete all local and remote tags, including version tags and `pre-skill-only-pce`. Do not replace them with new archive tags or rewrite history.
- Leave only `main` locally and on `origin` after cleanup work is delivered. Verify other branches before deleting them. Preserve and report any undelivered, unique, or uncertain work rather than silently discarding it; that safety exception takes precedence over achieving a one-branch count. Remove stale remote-tracking refs as appropriate. Temporary publication and implementation branches are not permanent exceptions.

## README

Reduce the README to:

1. One sentence describing PCE.
2. Installation instructions.
3. Six linked skill descriptions.
4. These two workflow arrows:

```text
grill-me → to-vision → implement-vision
chart-program → grill-ticket → implement-vision → land-ticket
```

Do not retain detailed publication, recovery, worktree, migration, or other operating policies in the README. Keep repository agent guidance minimal and consistent with the reduced surface.

## Repository evidence

At discovery, GitHub advertised three branches (`main`, `docs/published-vision-handoff`, and `docs/remove-fog-vision`) and twelve tags, with no open PRs. Local refs included stale remote-tracking branches and one extra version tag. Refresh this inventory before mutations; these counts are observations, not a fixed deletion list.

The current README and `CLAUDE.md` repeat extensive skill rules. `install.sh` contains substantial legacy link and Claude hook cleanup logic. Some tests require detailed README prose and will need adjustment so they protect the intended lean documentation rather than force that duplication back in. Skill behavior checks and useful helper/installer tests should remain.

## Validation and boundaries

Success is a small current tree centered on the unchanged six skills, a concise README, an installer without legacy migration behavior, and no old tags or safely removable non-main branches. Report any branch or historical vision retained to protect unfinished work.

Update affected tests and run from the repository root:

```bash
python3 -m unittest discover -s tests -v
```

Test installer changes with an isolated temporary `HOME`, including safe reruns and preservation of unrelated state and conflicts. Verify the final local and remote Git refs and review the diff for accidental skill behavior changes.

Do not add tooling, a compiled application, a package manager, CI workflows, an orchestration runtime, or generated runtime state. This work is repository cleanup, not a redesign of the skills or a bulk edit of GitHub Programs and Efforts. Preserve unrelated user work. Choose reversible implementation details from repository evidence rather than expanding scope.
