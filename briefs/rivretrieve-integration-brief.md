# Integrate the work-package branches in RivRetrieve

Eight work packages built real code against RivRetrieve, each on its own branch, each green on its
own criteria, and none of them built on top of the others. The union does not import. Your job is to
make it one coherent, green tree.

This is a repair of a delivered run, not new feature work. Prefer the smallest change that makes the
composed whole correct.

Decide your own method. Everything you need is below; nothing is assumed.

---

## Where to work

Repository: `/Users/nicolaslazaro/Desktop/work/RivRetrieve`. Its default branch is `main` at
`b8d6deb3b75ff889eac048e23b20359412d53fcb`.

Work in a **new git worktree** off `b8d6deb` with a fresh integration branch. Do not work in the main
checkout.

## Boundaries

1. **Do not touch `/Users/nicolaslazaro/Desktop/work/pce`.** A separate build is live there and six
   other runs read it. Nothing about this task requires it.
2. Do not disturb `/tmp/pce-work-package-worktrees` or
   `/Users/nicolaslazaro/Desktop/work/rivretrieve-worktrees` — other work owns them.
3. Do not delete, rename, force-move or rewrite any `pce/2026-08-11-the-store-is-the-only-copy*`
   branch. They are the only record of what the run produced.
4. **Do not merge into `main`, do not push, do not tag, and do not open a pull request.** Leave your
   integration branch local and report. Promotion is a human act.
5. A scratch worktree from a trial integration exists at `/tmp/rr-integration-trial` on branch
   `trial/integration`. Ignore it or delete it; do not build on it.

## What already happened

A work-package graph drove eight packages. Each got its own worktree branched from `b8d6deb`, and
because of a defect since fixed in the tooling, **no package was built on top of its dependencies**.
Each package's acceptance criteria were executed and passed in isolation. Gates then attacked each
built artifact and authored repairs on separate branches.

## The branch map, already worked out — do not re-derive it

Most package branches are already contained in others. The minimal covering set is three package
branches plus six gate branches:

```
pce/2026-08-11-the-store-is-the-only-copy/RR1
pce/2026-08-11-the-store-is-the-only-copy/RR5     (contains RR2, RR3, RR4, RR6)
pce/2026-08-11-the-store-is-the-only-copy/RR8     (contains RR2, RR3, RR4, RR7)

pce/2026-08-11-the-store-is-the-only-copy-gate-3/RR1
pce/2026-08-11-the-store-is-the-only-copy-gate-4/RR2
pce/2026-08-11-the-store-is-the-only-copy-gate-5/RR3
pce/2026-08-11-the-store-is-the-only-copy-gate-6/RR4
pce/2026-08-11-the-store-is-the-only-copy-gate-7/RR6
pce/2026-08-11-the-store-is-the-only-copy-gate-8/RR8
```

**The gate branches are not contained in the package branches.** Merging only the package branches
silently drops every repair a gate found and proved — including a `ca_eccc` fix matching HYDAT
`DOUBLE` declarations and a `pl_imgw` fix preserving missing-value sentinel states. Those are real
bug fixes in real providers. Verify for yourself that each gate branch's work is present in your
final tree, and say how you checked.

Ignore `…/RR1-attempt-1`. It is a failed first attempt kept only for inspection.

## What a trial merge already found — start from here

Merging the nine branches in the order above produces exactly two conflicted files, both additive
export lists:

- `src/rivretrieve/_internal/store/__init__.py` — when merging `RR5`
- `src/rivretrieve/_internal/registry.py` — when merging `RR8`

Resolve those properly rather than by blindly keeping both sides.

**Then the tree does not import:**

```
ImportError: cannot import name 'RawMode' from 'rivretrieve._internal.observations'
```

The cause is measured and is not a merge artifact:

| branch | `registry.py` imports `RawMode` | `observations.py` defines it |
|---|---|---|
| `RR1` | yes | yes |
| `RR5` | yes | yes |
| `RR8` | **no** | **no** |

Every branch is internally consistent. `RR8` refactored `RawMode` away across both files and is
green. `RR1` and `RR5` still depend on it and are green. The merge takes `RR8`'s `observations.py`
and `RR5`'s `registry.py`, and the package breaks.

Expect more of this shape than the one instance already found. Two packages that never met can
disagree about any symbol, not only this one.

## The one judgement call

`RawMode` has to be resolved in one of two directions:

- **Complete `RR8`'s refactor** — carry it through `RR1`'s and `RR5`'s call sites so the symbol stays
  gone.
- **Restore `RawMode`** — keep it and reconcile `RR8` with it.

Read `RR8`'s commits and its gate's findings first and work out what it was actually doing. If its
refactor is coherent and the remaining call sites can follow it **without changing behaviour**,
complete it — that is the expected answer. If completing it would change what the library does for a
user, stop and report instead; that is a decision for the operator, not for you.

Say which you chose and why, either way.

## What must be true when you are done

1. One integration branch containing all three package branches and all six gate branches.
2. The package imports. `python -c "import rivretrieve"` succeeds.
3. `uv run pytest` is green. The tip of milestone 2 measured **1651 passed, 2 skipped**, both skips
   being `folium` imports; your tree should be at least that, and the two skips are expected.
4. Every gate repair is present in the final tree, verified by inspection rather than assumed.
5. No behaviour change beyond what integration required. You are reconciling delivered work, not
   improving it. If you find a bug that none of this forces you to fix, report it and leave it.
6. Every original branch still resolves to the commit it did before you started.
7. Nothing merged to `main`, nothing pushed, no pull request.

## Report back with

- The merge order, and every conflict with how you resolved it.
- Which direction you took on `RawMode`, and why.
- Every other cross-package disagreement you hit, in the same shape as the `RawMode` table — those
  are the valuable findings here, not the merge mechanics.
- How you verified each gate repair survived.
- The final `uv run pytest` summary.
- The integration branch name and its head commit.
- Anything you found that contradicts what this brief assumes.
