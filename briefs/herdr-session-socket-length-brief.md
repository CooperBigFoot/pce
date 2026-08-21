# Brief: pce accepts a herdr session name whose socket path cannot be bound

Status: READY TO DISPATCH — no grill. The decisions below are evidence-resolvable; examine the
evidence, decide, implement, and record what you decided and why in your completion report and
`CONTEXT.md`. Boundary: if a decision would change ratified doctrine (an ADR, a frozen criterion),
stop and report. Written 2026-08-20, before the session feature has been installed anywhere.

**Fix this before the feature ships.** `main` is at `8e4e611` and carries named-session dispatch, but
the installed binary is still `1be14320` and has no `--herdr-session` flag. No run can hit this yet.
The next install is what exposes it.

## The defect

`HerdrSessionName::parse` (`crates/core/src/herdr_dispatch.rs:54-67`) validates the *name* — non-empty,
not `.` or `..`, ASCII alphanumeric plus `. _ -`, and at most `MAX_HERDR_SESSION_NAME_LEN = 64`
(`:42`). It says nothing about the **socket path** herdr derives from that name, which is what
actually has to bind.

herdr places a named session's socket at
`~/.config/herdr/sessions/<name>/herdr.sock` — established live:

```
$ herdr --session pce-work workspace list
{"id":"cli:workspace:list","error":{"code":"server_not_running","message":
 "no herdr server is running at /Users/nicolaslazaro/.config/herdr/sessions/pce-work/herdr.sock; …"}}
```

A unix domain socket path is bounded by `sun_path`, which is **104 bytes on macOS**. Measured for the
natural default a supervisor would choose, `pce-workers-<vision-slug>`:

```
/Users/nicolaslazaro/.config/herdr/sessions/pce-workers-2026-08-20-silence-means-the-run-has-stalled/herdr.sock
= 111 characters, against a 104-byte limit
```

The name is 56 characters — comfortably inside our 64-character bound — and the path is 7 bytes over.
So pce accepts the name, and herdr fails before reaching any server. Reported by the palaestra
orchestrator on its first `/work-graph` invocation of
`planning/2026-08-20-silence-means-the-run-has-stalled`, which correctly declined to use the feature
and dispatched into the default session instead.

The bound is also wrong in kind, not only in size: 64 characters is a fact about names, and the
constraint is a fact about the *absolute path* the name lands in, which varies with `$HOME` and with
herdr's session directory. A name that is fine for one operator can be too long for another.

## What to change

Validate the derived socket path, not the bare name. Reject at parse time when the path herdr would
build exceeds the platform's `sun_path` limit, and say so in the error — name, computed path, its
length, and the limit — so the author shortens the name instead of guessing.

**Decisions delegated to you:**

1. **How the path is derived.** It must match herdr's own construction. The error message above is
   the authority; confirm it against herdr's documentation (`herdr --skill`, the guide index the
   0.8.2 CLI help points at) rather than hardcoding a layout inferred from one error string. If the
   base directory is configurable, the check has to read the same configuration herdr does, or bound
   conservatively and say why.
2. **The platform limit.** 104 on macOS, 108 on Linux, both including the terminating NUL. Decide
   whether to detect it or to bound by the smaller value everywhere. Recommendation: use the smaller
   constant with a named explanation — a session created on one machine should not become unusable
   when the same repository is driven from another.
3. **Whether `MAX_HERDR_SESSION_NAME_LEN` survives.** It is redundant once the path is bounded, and
   keeping both means two limits that can disagree. Recommendation: drop it and let the path check be
   the single rule, unless you find a herdr-side name-length constraint that is independent of the
   path — in which case keep both and cite it.
4. **Whether the skill should suggest a shorter default.** `skills/work-graph/SKILL.md` documents the
   `herdr_session` field as of `2aa72f6`. If it recommends a name shape, that shape must fit —
   something like `pce-<short-slug>` rather than `pce-workers-<vision-slug>`. Say what you chose.

## Tests

A name that passes the character grammar but produces an over-long path is **refused**, with the
computed path and both lengths in the message. Use the real measured case from palaestra as the
fixture: `pce-workers-2026-08-20-silence-means-the-run-has-stalled` under a home directory of
realistic length. Also assert that a short name still passes and that the composed argv is unchanged
for it. The existing exact-argv tests in `crates/core/src/herdr_dispatch.rs` and
`tests/driver_dispatch.rs` are the right neighbours.

## Environment facts

- pce repo `/Users/nicolaslazaro/Desktop/work/pce`, `main` at `8e4e611`, pushed and clean.
- Merge only in `/Users/nicolaslazaro/Desktop/work/pce-integration`, branch
  `integration/work-package-harness` — and **edit there too**, not in the main checkout. Full suite
  there: `cargo fmt --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`.
  Known parallel-load flake: `tests/dispatch.rs gate_execution_echoes_large_input_without_deadlock`.
- Do **not** `cargo build --release` in the main checkout and do not run `./install.sh`; that symlink
  is a fleet install across five runs. The supervisor owns installation.
- Installed herdr is **0.8.2**; pce enforces `>=0.8.2,<0.9.0`.
- Key code: `crates/core/src/herdr_dispatch.rs:42` (`MAX_HERDR_SESSION_NAME_LEN`), `:44-73`
  (`HerdrSessionName`), and `src/main.rs:7673` (`require_herdr_session_running`), which is where a
  path that cannot bind currently surfaces as a running-server failure rather than as a bad name.

## The waiting consumer

palaestra `planning/2026-08-20-silence-means-the-run-has-stalled`, six packages with no dependency
edges, launching into the default session because this feature is not installed. When it is, that
run is the first that would reach for it — and the natural name for it is precisely the one that
fails. A healthy first pass: `pce package driver-run --herdr-session
pce-workers-2026-08-20-silence-means-the-run-has-stalled` refuses at parse time naming the 111-byte
path and the 104-byte limit, and a shortened name is accepted and dispatches normally.
