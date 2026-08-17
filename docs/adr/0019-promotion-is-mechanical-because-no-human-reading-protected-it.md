# Promotion is mechanical because no human reading protected it

Doctrine held that nothing merges and that promotion is a human act, but the human who
authorises a run cannot review its graph for what an agent omitted and cannot read the
thousands of lines a run produces, so the gate was a rubber stamp at both ends and its only
real effect was to imply a review that never happened.

A run reaching `assembly-completed` therefore pushes its attempt branches, opens one pull
request from the composed assembly, and merges it without waiting for approval, as a single
merge commit that one `git revert -m 1` undoes in full.

The alternatives were to keep the human gate, which protects nothing it cannot read, or to
promote each package as it completes, which the run that motivated this decision refutes:
it ended `assembly-failed` with every authored criterion green, so per-package promotion
would have landed seven unproven merges before anything noticed the composition was broken.

## Consequences

The safety argument now rests entirely on execution, which obliges promotion to prove the
thing it actually merges: the current default branch is composed into the assembly and every
assembly criterion re-executed against that composition, followed by the repository's own
declared gate commands. A failure there reports and stops, because reconciling with work the
graph never saw would require a new criterion, and a new criterion is a frozen plan version.

Because reversal replaces review, the merge is atomic while publication is phase-recorded. The
assembly and retained final attempt refs are pushed together with an atomic push. The skill then
records PR creation, required-check completion, and merge separately; a failure reports exactly
which phases succeeded and is never retried into a different route.

The authenticated GitHub actor initiates a merge commit and GitHub supplies its committer metadata;
the skill does not claim a human author or reviewer. Required checks are allowed to finish, but a
failed, cancelled, or timed-out required check stops promotion. The default branch is fetched again
after checks and any movement stops promotion rather than rebasing or resolving at this layer.

The PR body is the bounded human-legible record, with SHA-256 identities for both the journal and
frozen graph. It carries final criterion outcomes, accepted witness/repair refs, plan changes, and
verbatim overrules; the hashed local journal retains full streams and transient execution paths.
