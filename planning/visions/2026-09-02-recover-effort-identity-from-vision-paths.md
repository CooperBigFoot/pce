# Recover Effort identity from vision paths

`implement-vision` should treat an Effort-derived repository vision path as a recoverable locator, not reject it merely because the caller did not repeat the Effort issue identity.

## Context

The Program workflow currently produces a durable vision with canonical `Program:` and `Effort:` provenance, publishes it on the target branch, links it from the Effort, and tells the user to invoke implementation with the Effort number or canonical URL. `implement-vision` also accepts repository-relative paths, but classifies them as standalone-only inputs. If a supplied path contains an `Effort:` line, it stops and asks the user to invoke the workflow again with the issue identity.

This distinction caused a valid handoff for RivRetrieve Effort 17 to fail. The supplied vision was a regular target-branch file, its provenance uniquely identified Effort 17, and its content matched the ticket's commit-pinned link and `origin/main`. The agent successfully identified all of this, yet refused to proceed because the input was the path rather than `17`. It then repeatedly emitted the same rejection because it had created a persistent goal before resolving the input.

The durable records already contained the identity needed to perform the safe workflow. Requiring the user to transcribe it again added no validation.

## Desired behavior

Accept all three useful forms as entry points to the same Effort implementation workflow:

- an Effort number in the current repository;
- a canonical Effort issue URL;
- a repository-relative `planning/visions/` path whose content has one canonical `Effort:` provenance line.

For an Effort-derived path, read the provenance, derive the canonical repository and Effort identity, and then run the complete Effort validation. The path is only a locator. It does not make the document authoritative and does not bypass the ticket.

Continue only when the derived identity and every durable record agree. The issue, Program Map, vision link, Program and Effort provenance, repository identity, dependency graph, commit-pinned content, and target-branch copy must still satisfy the existing Effort contract. Stop when provenance is missing where required, duplicated, malformed, foreign, ambiguous, or inconsistent with any of those records.

A path with no `Effort:` provenance remains a standalone vision and follows the existing standalone workflow. The change must not manufacture Program or Effort provenance for standalone work.

## Failure behavior and goal lifecycle

Resolve and validate the supplied input before creating the harness's persistent vision-level goal. A rejected input should produce one precise explanation and end normally. It must not create an active goal that causes automatic continuations to repeat an input request the agent cannot satisfy itself.

Once a valid path has been promoted to its derived Effort identity, goal tracking and all later reconstruction, implementation, review, delivery-record, and landing boundaries remain unchanged.

## Evidence of success

The skill contract and its regression coverage should demonstrate that:

- the three accepted forms above converge on the same validated Effort workflow;
- an Effort-derived path is never treated as standalone;
- automatic recovery still stops on ambiguous or conflicting provenance and linkage;
- validation precedes persistent goal creation;
- invalid input terminates without a continuation loop;
- existing standalone-path, canonical-URL, cross-repository, durability, dependency, and delivery safeguards remain intact.

The repository remains a skill-only distribution. This work must not add an orchestration runtime, compiled application, package manager, CI workflow, or generated runtime state.
