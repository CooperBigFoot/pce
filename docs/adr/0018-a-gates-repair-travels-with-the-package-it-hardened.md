# 0018 — A gate's repair travels with the package it hardened

## Status

Accepted

## Context

A credited gate finding is two commits on a driver-owned gate branch: a witness commit adding the failing check and a repair commit fixing the source. Composition reads the package's attempt branch, which never absorbs that gate branch, so no join and no assembly is built from a tree containing the hardening. The 2026-08-10-incidence-core run credited seven findings and shipped none of the seven fixes; four amendments failed at assembly with `no test target`, two passed only because a later worker retyped the gate's test and fix byte-identically during an unrelated recovery round, and one — `cargo test --test prefix_continuation completion_seal_is_not_a_resumable_prefix` — passed while running zero tests, because the filter matched nothing and the runner exited zero. The run ended `assembly-failed` with every authored criterion green.

The rejected alternative was to leave lineage untouched and re-materialize each amendment from its own witness/repair refs at every join and assembly. That certifies the tree the gate saw rather than the composed whole, which is the one thing assembly exists to test, so a green amendment would prove nothing by construction. The second rejected alternative was to treat a credited finding as new work for the package's worker, keeping gates purely inspectorial; it costs a worker round per finding and replaces a fix the driver has already watched flip red to green with an unproven one.

## Decision

A credited finding's repair commit becomes part of the package's lineage, so gate-authored code ships. The merge is an invariant checked immediately before any composition that includes a package — not an action taken once at credit time — so it is idempotent, and a journal whose findings were credited by an earlier binary is repaired on the way into the next composition without journal surgery. Every merge is recorded as a journal event.

Because a passing command is not by itself evidence that a check ran, an amendment is executed as a pair wherever it is executed: once against the composed tree, which must pass, and once against that tree with the repair commit's diff reverted, which must fail. An amendment whose reverted side also passes is recorded failed. When the revert cannot be constructed — a later package rewrote the hardened source — the driver charges that package, not the human: the package fails with the finding named, and its worker is dispatched to restore the finding's provability, exactly as a conflicted join hands its resolution to the dependent's worker.

## Consequences

What PCE delivers becomes what the workers built plus what the gates fixed on the way past, including source patches no authored criterion asked for and no worker reviewed. Amendment enforcement costs one extra execution per amendment per composition. A human is reached only by exhausting the ordinary recovery ladder, whose question — continue or repartition — is one the human can answer, unlike a replay-provability conflict.
