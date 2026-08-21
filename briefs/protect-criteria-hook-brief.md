# Brief: the criterion-protection hook must judge accurately

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine
the evidence, decide, implement, and record what you decided and why in your completion
report and CONTEXT.md. Boundary: if a decision would change ratified doctrine (an ADR, a
frozen criterion), stop and report. Written 2026-08-19.

## Two defects in one hook, both long-ledgered, now actively blocking ruled work

The hook: `hooks/pce-protect-criteria.sh` in the pce repo, installed at
`~/.local/bin/pce-protect-criteria` (sh wrapper around an inline python PreToolUse guard).
Its purpose is right and stays: during an active run, ratified criteria in a vision document
may not be removed, reordered, or changed; additive criteria enter via
`pce log --kind criterion-added` first.

**Defect 1 — substring matching.** The Bash-refusal path matches the literal substring
`vision.md` anywhere in a command, so any command naming `supervision.md` — which the
work-graph skill REQUIRES appending to before every chat message — is refused. Confirmed on
at least five occasions across three visions (ledger; most recently it refused a supervisor
writing a graph draft whose criterion PROSE contained the word "supervision.md", and a
RivRetrieve feedback report documents the cwd-dependence that makes it look sporadic —
`orchestrator-feedback/2026-08-19-a-fixture-is-a-recording.md`, finding 1). Fix: match paths,
not substrings — the guard should trigger only when the command actually references the
active run's vision document path (basename-anchored, e.g. a path component exactly
`vision.md`), never on `supervision.md` or prose containing the string.

**Defect 2 — the verifier judges empty stdin.** The hook's accepting decision comes from
`pce criteria check`, whose CLI contract reads the PROPOSED DOCUMENT FROM STDIN
(`run_criteria_check(&log_path, &recovery_log_path, &vision_dir, input)`). Evidence from
pourpoint (2026-08-19): the orchestrator recorded five additive criteria via
`pce log --kind criterion-added` (exits 0), then had both its vision edits — text matching
the logged criteria, in log order, plus a Scope-In prose edit — REFUSED with "could not
obtain an accepting decision". The pourpoint orchestrator independently diagnosed the same
class in its own direct invocation: run with no stdin, criteria check parses empty input and
reports zero headers. Read the hook's actual subprocess call; if it fails to feed the
constructed proposed document on stdin (or constructs it wrongly — the CONSTRUCT path globs
too), an edit that is legitimately additive can never be accepted, which is the observed
behavior. Fix the invocation so the verifier judges the real proposed document; add a
diagnostic line to the refusal naming WHICH criterion comparison failed, so a refused author
learns the reason instead of guessing.

**Regression guard:** a test harness for the hook (the repo already has hook tests near
`hooks/`, or add one) covering: (a) a command touching `supervision.md` passes; (b) a
command touching the active `vision.md` by path is refused; (c) an Edit adding criteria that
were first recorded via criterion-added is ACCEPTED; (d) an edit that removes or reorders a
ratified criterion is refused with the named criterion; (e) prose-only edits (Scope-In)
that leave the criteria list byte-intact are accepted.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, main at c151487 or later. Merge only in
  `/Users/nicolaslazaro/Desktop/work/pce-integration`; full suite there; fast-forward main;
  push origin. Do NOT install and do NOT `cargo build --release` in the MAIN checkout; build
  only in pce-integration. Hook installation is via ./install.sh which the supervisor runs.
- Key code: `hooks/pce-protect-criteria.sh` (inline python: CONSTRUCT path, VERIFIER path,
  BASH_REFUSAL substring check), `pce criteria check` / `run_criteria_check` in src/main.rs
  for the stdin contract.
- The waiting consumer: pourpoint's orchestrator holds a ruled, fully-designed vision
  amendment (five additive criteria logged, publish-first design ready) that the hook
  refuses. It has explicitly declined to bypass the hook with globs — honor that by making
  the guard judge accurately. Known flake: tests/dispatch.rs
  gate_execution_echoes_large_input_without_deadlock under full parallel load.
