# 0015 — A conflicted join is the dependent worker's first task

When composing a dependent package's base from parallel siblings produced a textual merge
conflict, the driver recorded a terminal `composition-failed` state and the run died at
exactly the point parallelism paid off, exitable only by journal surgery. We decided that a
conflicted join is a starting condition, not a failure: the dependent package's worker
receives the conflicted worktree with git's conflict evidence in its brief, resolves it as
its first task, and every parent's criteria re-run against the merged base before the
dependent's own work is judged — rejecting the alternative of synthesizing a merge-package
at run time, which would put an unauthored package into a frozen graph. The reason is that
the frozen plan version is doctrine (no package the human never saw may appear at run time),
while the parent re-proof keeps "a completed package stays proven on every base built on it"
true at the first moment it is at risk instead of at assembly.
