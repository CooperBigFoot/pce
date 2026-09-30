# Read-only Effort validation

Required before Effort discovery in either mode and freshly before publication issue mutation. Apply state and ownership checks only; do not assign, mutate, or invoke the interview through this reference.

Before questions or mutation, inspect the repository and use `gh` to load the issue, its comments and timeline, authenticated user, linked Program Map, all member Efforts and dependencies, relevant repository evidence, delivery records, and any linked vision. Require all of the following:

- the issue is open, has label `pce:effort`, and contains `<!-- pce:effort -->`;
- it has exactly one valid `Program: <canonical GitHub issue URL>` line pointing to an issue with label `pce:program` and `<!-- pce:program -->`;
- it has exactly one `Depends on:` line naming `none` or valid Efforts in that same Program, and the complete Program dependency graph is acyclic;
- the Program Map contains this canonical Effort URL exactly once as an open member and not as a landed outcome or duplicate entry;
- it has exactly one `Vision:` line that is `pending` or identifies one repository vision;
- a linked vision contains exactly one canonical `Program:` line and exactly one canonical `Effort:` line, both matching the ticket, Map, and repository identity.

Stop without mutation and report the malformed, closed, duplicate, mismatched, foreign-Program, cyclic, missing-membership, or otherwise ambiguous state. Do not claim or mechanically repair missing or duplicate Map membership. If any other user is assigned, stop without mutation in either mode.

Blockers constrain delivery, not discovery; read-only validation does not require blockers to have landed.
