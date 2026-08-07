# `pce dispatch` spawns with an empty environment, and the failure mode is misleading

**Vision:** `2026-07-31-values-carry-the-descriptors-that-dimension-them`, node `m8-s6`
**Date:** 2026-08-05
**Severity:** blocking for both dispatch targets, with a silent-looking cause

## What happens

`pce dispatch` builds its child-process envelope with `"environment": {}` unless `--env` is passed.
The dry-run envelope shows this plainly:

```
$ pce dispatch codex --cwd … --dry-run -- "hello"
{"envelope":{"target":"codex","executable":"codex",…,"environment":{},…}}
```

An empty environment means no `PATH`, so `executable: "codex"` cannot be resolved by name. The
error the operator sees is:

```
Error: failed to spawn `codex`

Caused by:
    No such file or directory (os error 2)
```

That message points at the binary. The binary is fine — `/opt/homebrew/bin/codex` exists, is on
`PATH`, and runs. It cost several minutes to realise the missing file was the *environment*, not
the executable. (The shebang makes it worse: `codex` is a `#!/usr/bin/env node` script, so even a
resolved path would fail without `PATH` for `node` — two layers of the same cause.)

## The gate target fails differently, and worse

Passing a minimal environment fixes `codex`:

```
--env "PATH=$PATH" --env "HOME=$HOME" --env "TMPDIR=$TMPDIR" --env "SHELL=/bin/zsh"
```

The same four variables are **not** enough for `pce dispatch gate`. The `claude` child starts,
runs, and returns a well-formed error envelope containing:

```json
{"result":"Not logged in · Please run /login","terminal_reason":"api_error","subtype":"success"}
```

which `pce` then reports as `Error: invalid Claude result data: claude-error-envelope`. The
dispatch consumed a slot, wrote no artifact, and the stated reason ("not logged in") is false — the
operator *is* logged in. On macOS, credentials live in the Keychain, and reaching it needs the
session variables (`SECURITYSESSIONID`, `USER`, `LOGNAME`, `XPC_SERVICE_NAME`, `LaunchInstanceID`)
that an empty environment drops.

The workaround is to forward essentially the whole environment:

```
python3 -c 'import os; …'  # emit --env K=V for every var except the CLAUDE_CODE_* session internals
```

excluding `CLAUDECODE`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_ENTRYPOINT`, `CLAUDE_PID`,
`CLAUDE_JOB_DIR`, `CLAUDE_EFFORT`, `CLAUDE_CODE_CHILD_SESSION` and friends, so the child does not
mistake itself for the parent session. Forty-seven variables. Building that list by hand, from a
"Please run /login" message, is not a reasonable ask.

## Why the empty default is defensible but the ergonomics are not

A hermetic environment is a *good* default for a tool whose whole purpose is reproducible dispatch
— an inherited `RUST_LOG` or `NO_COLOR` leaking into a measured child is exactly the kind of
contamination this vision spent a milestone eliminating. The problem is not the policy. It is that:

1. the policy is invisible until you run `--dry-run` and notice `"environment":{}`;
2. neither failure names it; and
3. there is no supported way to say "hermetic, plus what the child needs to authenticate".

## Suggested changes, cheapest first

1. **Name the cause in the spawn error.** When spawn fails with `ENOENT` and the envelope
   environment has no `PATH`, say so:
   `failed to spawn 'codex': the dispatch environment contains no PATH (pce dispatch uses a
   hermetic environment; pass --env PATH=… or --inherit-env)`.
2. **Add `--inherit-env`**, forwarding the parent environment minus a documented deny-list of
   `CLAUDE_CODE_*` session internals. This is what every operator will end up hand-rolling.
3. **Forward a small default allow-list for the `gate` target** — `PATH`, `HOME`, `USER`,
   `LOGNAME`, `SHELL`, `TMPDIR`, `LANG`, `SECURITYSESSIONID`, `XPC_SERVICE_NAME`,
   `LaunchInstanceID`, `SSH_AUTH_SOCK` — since `claude` cannot authenticate without them and a
   dispatch that cannot authenticate is never the intent.
4. **Do not report an authentication failure as `invalid Claude result data`.** The child returned
   a perfectly valid envelope whose `result` said what was wrong. Surface that string.

## Related

This is the second orchestrator finding from this vision; see
`2026-08-04-values-carry-the-descriptors-that-dimension-them.md` (`pce ready` cannot classify any
step graph) and `2026-08-04-criticism-belongs-on-artifacts.md`.

One smaller note from the same session: `pce dispatch` requires the logging flags in the exact
order `--log-file, --node, --role, --ref, --evidence`, and rejects any other order with a usage
dump. `--planning-act` must follow `--evidence`. The error message does state the required order,
which is good; but flag order dependence is surprising in a CLI that otherwise parses flags
positionally-independently, and the usage line does not make the ordering visually obvious.
