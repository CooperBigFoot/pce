# Clean Branches and Expose All PCE Skills to Prime Agent

PCE should have one durable branch, `main`, and Prime Agent should be able to discover every skill distributed by this repository. The current branch inventory is visually noisy and the current installer intentionally exposes only part of PCE to Prime Agent, which leaves workflows such as `chart-program` unavailable there.

## Desired outcome

After the work is complete:

- `main` is the only local branch in this checkout and the only branch on the `origin` remote.
- All six tracked PCE skills are installed in Prime Agent's global skill directory, `~/.prime/agent/skills/`:
  - `chart-program`
  - `grill-me`
  - `grill-ticket`
  - `implement-vision`
  - `land-ticket`
  - `to-vision`
- Claude Code and Codex retain their existing supported PCE authoring skills.
- Re-running the installer remains safe and preserves unrelated user state.
- Repository documentation describes the actual supported installation matrix.

The skill change must reach `main` before the final branch deletion pass. Any temporary implementation branch must also be removed locally and remotely after it lands, so the final branch inventory still contains only `main`.

## Repository evidence

The repository distributes exactly six skills under `skills/`. On inspection, `install.sh` installed the five authoring skills for Claude Code and Codex but only `implement-vision` for Prime Agent. The actual Prime Agent global directory contained links for `to-vision`, `grill-ticket`, `implement-vision`, and `land-ticket`, while `chart-program` and `grill-me` were absent. A copy of `grill-me` under `~/.agents/skills/` does not satisfy the Prime Agent global installation requirement.

An unpublished branch named `feat/install-prime-authoring-skills` contains a partial installer change. It still excludes `chart-program` and `grill-me`, so it does not satisfy this vision as written. Its useful ideas may be inspected, but the branch itself does not need to be retained.

At discovery time, the repository had 17 local branches besides `main`, 39 remote branches besides the remote's symbolic listing, three clean branch-linked worktrees, and three detached worktrees containing untracked review artifacts. Local `main` was four commits behind `origin/main`.

## Branch-cleanup decisions

The cleanup applies to both local and remote branches. No non-`main` branch needs to be retained, including branches Git reports as unmerged. Loss of branch names and branch-only commits is accepted.

Before deletion, refresh remote evidence and ensure the intended skill change is safely present on `main`. Then remove every non-`main` local branch and every non-`main` branch from `origin`. The final state must be verified from both local Git references and the remote host, rather than inferred from deletion command output.

Clean worktrees attached to branches may be removed when they block branch deletion. Detached worktrees with untracked review artifacts are not branches and must be preserved. Their files must not be deleted merely to make the branch list look clean.

`origin/main` is the sole remote branch to preserve. The remote's symbolic `origin/HEAD` reference is not a separate branch and should continue to identify `origin/main`.

## Installation boundaries

Prime Agent support is additive. All six PCE skills belong in `~/.prime/agent/skills/`; existing Claude Code and Codex support remains in place. The installer must continue to create links to this checkout, remain safe to rerun, reject ambiguous conflicts before partial installation, and preserve unrelated skills, files, directories, links, settings, and other user state.

Installer tests must use an isolated temporary `HOME`. They should prove the complete Prime Agent matrix, exact supported placement, safe reruns, conflict behavior, and preservation of unrelated state. Repository contract tests and documentation must be updated where they encode the old Prime-only subset.

After the accepted change is on `main`, run the installer for the real user home and verify that each of the six Prime Agent global entries resolves to its matching tracked skill directory. Validation should account for the possibility that an already-running Prime Agent session does not refresh its discovered skill list until a new session starts.

## Completion evidence

The outcome is complete only when all of the following are observable:

1. The repository's full test command, `python3 -m unittest discover -s tests -v`, passes on the accepted `main` state.
2. Installer tests run through isolated homes and prove unrelated state is preserved.
3. Each of the six paths under `~/.prime/agent/skills/` is an owned link resolving to the corresponding directory under this checkout's `skills/`.
4. A fresh Prime Agent session can discover all six skills, including `chart-program` and `grill-me`.
5. Local branch enumeration contains only `main`.
6. GitHub branch enumeration contains only `main`.
7. `main` matches the accepted `origin/main` state after publication.
8. Detached worktrees with untracked review artifacts remain intact.

Do not add an application, package manager, CI workflow, orchestration runtime, or generated runtime state. This is an installer, documentation, test, and repository-maintenance outcome within the existing skill-only PCE surface.
