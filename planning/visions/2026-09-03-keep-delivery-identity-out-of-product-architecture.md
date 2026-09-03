# Keep delivery identity out of product architecture

## Outcome

PCE must prevent Program and Effort tracking identities from becoming product architecture when `implement-vision` executes a vision. Implementation agents should name production components from the repository's established domain vocabulary and the stable responsibility of each component, rather than from the ticket, vision, or delivery phase that caused the component to be created.

This matters because names such as `et21_evaluation.py`, `ET21Evaluator`, or `et21-run` preserve temporary project-management structure in the maintained product. They make production structure harder to understand after the Effort is complete and encourage later work to extend a ticket-shaped boundary.

## Naming boundary

Keep delivery identity out of product architecture.

Production modules and internal identifiers must use established codebase and domain vocabulary. Public APIs must describe stable domain capabilities or genuinely reusable contracts. “Generic” does not mean vague names such as `Manager`, `Runner`, or `Handler`; domain-specific names remain appropriate when the capability is domain-specific.

Removing a ticket number alone is not sufficient. A cosmetic rename still fails when the resulting component remains a container organized around an Effort, ticket title, vision, or implementation phase instead of a stable product responsibility.

Ticket or Effort identity must not determine the names or boundaries of:

- production modules and packages;
- types and functions;
- commands and routes;
- services;
- runtime schemas;
- user-facing configuration keys;
- other product architecture or public interfaces.

Ticket identity remains valid delivery metadata and traceability evidence. It may appear in:

- issues and vision provenance;
- pull request descriptions;
- delivery records;
- branch and worktree names;
- commit messages;
- historical evidence;
- test names, fixtures, examples, and study-specific data configuration when necessary for traceability.

Maintained tests, fixtures, and examples should still prefer behavioral or domain names where practical. When an Effort reference is necessary, it may be carried as explicit metadata or a comment rather than used as the maintained artifact's organizing name.

## Implementation and review behavior

`implement-vision` is the primary enforcement point. Every implementation owner must receive the naming boundary as part of its delegated context and must inspect the repository's existing vocabulary before choosing production names or architectural boundaries.

Independent review must treat this as a delivery requirement. A review should reject:

- newly introduced ticket-derived production identifiers;
- existing ticket-shaped architecture that the implementation extends;
- cosmetic renames that remove the literal identifier but retain an Effort-shaped abstraction;
- vague supposedly generic APIs that conceal rather than express the stable capability.

The rule applies to inherited noise as follows:

1. Do not introduce or expand ticket-shaped product architecture.
2. Repair existing ticket-shaped components that the current implementation directly modifies or depends on.
3. Report unrelated occurrences without turning the current vision into repository-wide cleanup.
4. Preserve compatibility for existing public interfaces, or use an explicit migration when a repair changes such an interface.

The implementation and its independent reviewer should evaluate the resulting names in the context of the complete repository and vision, not merely search for a particular ticket-number pattern.

## Evidence of success

A fresh Prime Agent following `implement-vision`:

- clearly separates Effort traceability from product naming;
- gives implementation agents the naming constraint before they design or edit production code;
- uses domain responsibilities and existing repository vocabulary when evaluating names;
- catches violations during independent review before merge;
- repairs relevant inherited ticket-shaped architecture without performing unrelated cleanup;
- retains ticket identity in approved delivery and evidence locations;
- handles public compatibility deliberately rather than silently breaking an existing interface.

PCE's skill contract tests should make this behavior durable so a later rewrite of `implement-vision` does not silently remove the implementation and review requirements.

## Scope

This vision changes PCE's implementation guidance and the tests that preserve that guidance. It does not require a repository-wide naming linter, impose one universal naming convention on downstream repositories, rename PCE's Program or Effort workflow artifacts, or clean existing ticket-shaped architecture in unrelated product repositories. Those repositories are repaired when their own implementation work encounters the affected architecture.
