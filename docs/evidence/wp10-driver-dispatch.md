# WP10 driver-owned dispatch evidence

This drive used the WP10 binary from its isolated target directory with installed Prime Agent 0.7.2 and installed Herdr 0.7.1. No adapter script, shell join, external poller, or Herdr completion query participated. This differs from `wp7-driver.md`: the binary composed implementation and gate dispatches, watched durable result files, collected completions, ran criteria, and released dependencies itself.

## Graph and invocation

The graph at `/tmp/pce-wp10-real-1786617661/graph.json` has two packages:

- `A` owns `repo-a`, creates `A.txt` containing `alpha`, and has no dependency.
- `B` owns `repo-b`, creates `B.txt` containing `beta`, and has a buildability dependency on `A`.

The exact invocation is recorded at `/tmp/pce-wp10-real-1786617661/invocation.txt`. It supplied only the graph, driver journal, and repository mappings; it supplied no worker argv.

## Observed sequence

The driver journal at `/tmp/pce-wp10-real-1786617661/driver.jsonl` records:

1. `A` issuance 1.
2. A real Prime Agent implementation completion, followed by a passing `A marker` criterion.
3. A real Prime Agent gate dispatch and `package-gate-1` completion with no findings.
4. `A` completion, which released `B`.
5. `B` issuance 2.
6. A real Prime Agent implementation completion, followed by a passing `B marker` criterion.
7. A real Prime Agent gate dispatch and `package-gate-2` completion with no findings.
8. `B` completion and terminal graph outcome `finished`.

The dispatch event log at `/tmp/pce-wp10-real-1786617661/.pce/package-dispatch.jsonl` records four issuances and four completions: implementation and gate for A, then implementation and gate for B. Durations were 16.213 s, 30.839 s, 21.832 s, and 30.649 s. Each completion had exit code zero and required-artifact presence `present`.

## Artifacts

- Vision: `/tmp/pce-wp10-real-1786617661/vision.md`
- Frozen graph: `/tmp/pce-wp10-real-1786617661/graph.json`
- Driver journal: `/tmp/pce-wp10-real-1786617661/driver.jsonl`
- Dispatch ledger: `/tmp/pce-wp10-real-1786617661/.pce/package-dispatch.jsonl`
- Implementation outcomes: `/tmp/pce-wp10-real-1786617661/package-outcomes/A/1.json`, `/tmp/pce-wp10-real-1786617661/package-outcomes/B/2.json`
- Gate outcomes: `/tmp/pce-wp10-real-1786617661/.pce/package-gate-outcomes/A/1.json`, `/tmp/pce-wp10-real-1786617661/.pce/package-gate-outcomes/B/2.json`
- Durable dispatch results: `/tmp/pce-wp10-real-1786617661/.pce/package-results`
- Captured stdout/stderr: `/tmp/pce-wp10-real-1786617661/stdout.txt`, `/tmp/pce-wp10-real-1786617661/stderr.txt`
- Rendered page: `/tmp/pce-wp10-real-1786617661/wp10-run.html`

## Join ownership

The driver composes an inner implementation argv beginning with its own executable and `package agent`; it writes the fully composed RR2/recovery brief to a binary-owned stable brief file, and `package agent` pipes that file to `prime-agent -p`. The existing package dispatch composer wraps the inner argv with `dispatch package-worker`, creates Herdr worktrees, and carries PATH, HOME, USER, binary-owned TMPDIR, and worktree variables. Gates use the same dispatcher with `package gate-agent` as the inner argv.

The binary registers filesystem notification watches over result directories. Notifications only wake the wait; the atomically published result file remains authoritative and restart collection re-folds the disk ledger. There is no default deadline. A caller-supplied `--wait-timeout-ms` appends `driver-stopped-waiting`, returns the graph as still running, and leaves issuance unaccounted.
