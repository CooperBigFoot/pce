# The mandated three `--env` entries strand `TMPDIR`, and pce's two sandboxes then disagree about which temp root is writable

**Vision:** `2026-08-11-the-store-is-the-only-copy`, node `m8-s1`
**Date:** 2026-08-11
**Severity:** blocking for every Codex executor dispatch on macOS, with a cause that points at the wrong thing

## Summary

`pce dispatch` and `pce contract check` are two surfaces of the same binary that measure the same
gates in the same repository, and on macOS they permit **disjoint** temporary directories:

| Surface | Writable temp root | Mechanism |
|---|---|---|
| `pce contract check` | `$TMPDIR` of the **caller** (`/var/folders/…/T`) | Seatbelt profile, temp root from the caller env |
| `pce dispatch codex` | `/tmp` only | codex sandbox; `$TMPDIR` is **unset in the child**, so its `$TMPDIR` rule matches nothing |

Any tool that needs a writable cache outside the repository — `uv` is the case here, and it is the
case for every Python repository pce is likely to drive — therefore cannot be configured to satisfy
both. Whichever root you choose, one of the two surfaces refuses.

This is a direct consequence of the current dispatch-route contract, which mandates **exactly**
three environment entries and treats the cardinality as falsifiable from both sides:

> Every anchored route contains exactly three environment entries, `--env {{PATH_ENV}} --env
> {{HOME_ENV}} --env {{USER_ENV}}` … Two entries are as invalid as one, so cardinality is
> falsified from both sides.

So `TMPDIR` cannot be forwarded without violating the route contract. The previous report from
`2026-08-05` asked for `TMPDIR` in a default allow-list; the contract has since gone the other way
and made forwarding it structurally impossible.

## What happens

Phase 0 `pce contract check` failed with the cache at `/private/tmp`:

```
error: Failed to initialize cache at `/private/tmp/pce-uv-cache`
  Caused by: failed to open file `/private/tmp/pce-uv-cache/sdists-v9/.git`:
             Operation not permitted (os error 1)
Error: failed to measure tracked repository contract
Caused by: stated gate command `uv run ruff format` exited with status 2
```

Moving the cache under `$TMPDIR` made contract check green — and then the first `step-executor`
dispatch blocked before running a single gate:

```
INPUT:       uv sync
OBSERVATION: Exited 2: Failed to initialize cache at
             `/var/folders/9m/…/T/pce-uv-cache`; opening `sdists-v9/.git` failed with
             Operation not permitted (os error 1).
```

Each sandbox refuses precisely what the other requires.

## Measurement

Both failures were on paths named `sdists-v9/.git`, which strongly suggests a `.git`-write guard.
It is not. A bounded diagnostics dispatch through the `codex-diagnostics` purpose anchor
(issuance 48, node `m8-s1`) discriminated the two hypotheses:

| Probe | Result |
|---|---|
| `printenv TMPDIR` | **exit 1 — unset** |
| `touch /tmp/pce-probe-a/w` | `OK-TMP` |
| `touch /tmp/pce-probe-a/sub.git/f` | **`OK-TMP-DOTGIT`** |
| `touch /var/folders/…/T/pce-probe-b/w` | `DENY-VARFOLDERS` |
| `touch /var/folders/…/T/pce-uv-cache/sdists-v9/.git` | `DENY-CACHE-DOTGIT` |

> Conclusion: TMPDIR is unset; this sandbox permits writes under /tmp (including .git-named paths)
> but denies writes under /var/folders/…/T, so the denial is **path-root based, not .git-name
> based**.

The child's writable set is workdir + `/tmp`. Its `$TMPDIR` rule is inert because the binary
clears the environment and the route may only restore three variables, none of which is `TMPDIR`.

## Workaround

Environment-only, no tracked file touched, verified from both sides:

```sh
# 1. cache under /tmp, which the codex sandbox permits
printf 'cache-dir = "/private/tmp/pce-uv-cache"\n' > ~/.config/uv/uv.toml

# 2. override TMPDIR on every contract-check invocation so the Seatbelt root becomes /tmp too
TMPDIR=/private/tmp pce contract check --file <root>/.pce/repository-contract.json --repo-root <root>
```

Verified: contract check exits 0 with all five gates green under that invocation, and the codex
probe writes `/tmp` freely. Every subsequent executor dispatch in this run succeeded.

The workaround is not discoverable. It requires knowing that `pce contract check` derives its
Seatbelt temp root from the caller's `TMPDIR`, which is not documented and not visible in any
error message.

## Why the failure points at the wrong thing

`uv`'s message names a cache path and a permission error, so it reads as a machine
misconfiguration. It is not: the same cache path is writable from the operator's shell, writable
under one pce surface, and denied under the other. Nothing in either error mentions a sandbox, a
temp root, or an environment policy.

The executor behaved correctly — the repository contract's hazard note says a cache-init permission
error is an environment fault and must never be worked around by editing project files, and it
reported `BLOCK` / `root_cause=execution` rather than touching the gates. But a correct executor
still burned a full dispatch, and the orchestrator still had to spend a diagnostics dispatch to
find out that a *third* variable was missing.

## Suggested changes, cheapest first

1. **Make the two sandboxes agree.** Whatever `pce contract check` treats as its writable temp root
   should be what a dispatched child gets. Today one reads the caller's `TMPDIR` and the other
   cannot see `TMPDIR` at all.

2. **Add `TMPDIR` as a fourth mandated entry, or synthesize it.** The route contract already
   justifies each of its three entries by what breaks without it (`PATH` → spawn fails; `HOME` →
   fabricated git identity; `USER` → keychain OAuth). `TMPDIR` belongs in exactly that list:
   *without it, a child's sandbox cannot grant its own platform temp directory.* If four entries
   is unacceptable, have the binary set `TMPDIR` itself to a root it guarantees is writable, and
   say so in the contract.

3. **Name the cause when a gate dies on a permission error under a sandboxed root.** A message of
   the form `gate 'uv run ruff format' failed writing under <path>, which is outside this
   sandbox's writable roots (<roots>)` would have replaced roughly forty minutes of bisection.

4. **Document that `pce contract check` reads the caller's `TMPDIR`.** It is load-bearing and
   currently invisible.

## Two smaller findings from the same run

### `pce contract refresh` dirties the tracked contract it is forbidden to change

After the milestone-8 merge, `pce contract refresh --file … --repo-root … --node m8-s1` exited 0
and left the tracked file modified:

```diff
   }
-}
\ No newline at end of file
+}
```

A trailing-newline normalization only — no `stated` field changed — but the skill forbids a run
from editing that file, so the orchestrator must notice and revert it. If refresh is going to
rewrite the tracked file, it should be byte-idempotent when nothing has changed.

### `pce dispatch check-in --file` requires an absolute path; its siblings do not

```
$ pce dispatch check-in --file planning/<vision>/events.jsonl
Error: dispatch check-in event-log path must be absolute: planning/<vision>/events.jsonl
```

`pce log`, `pce log read`, `pce status` and `pce ready` all accept the same relative path happily.
The error is clear and recoverable, but the inconsistency is a papercut in a loop where check-in
is called dozens of times.

### An appendable hazard entry can be added but never retracted

`.pce/repository-contract.json` carries this appendable `environment_hazards` entry, written by an
earlier run:

> The machine therefore carries `~/.config/uv/uv.toml` setting cache-dir to a warm shared cache
> under `$TMPDIR`, which **both sandboxes permit**.

That claim is now false on this host — it is the exact configuration that blocks every executor.
The contract lifecycle offers `pce contract learn` to **add** an appendable entry, but nothing to
mark one falsified or retract it, and a run may not edit the tracked file. So a hazard note that
was true when written accumulates indefinitely and actively misleads later runs. Recording the
correction as a `key-finding` in the event log (which is what this run did) keeps it out of the
contract where the next run will look first.

Consider a retraction path — `pce contract learn --retract`, or a supersedes field — so appendable
knowledge can be corrected rather than only accreted.

## Related

- `2026-08-05-dispatch-spawns-with-an-empty-environment.md` — the original empty-environment
  finding, which asked for `TMPDIR` in a gate allow-list. The route contract subsequently fixed
  cardinality at three and made that impossible; this report is the consequence.
- `2026-07-31-the-binary-owns-every-dispatch.md`
