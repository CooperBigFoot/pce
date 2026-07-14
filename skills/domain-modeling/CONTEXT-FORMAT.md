# `CONTEXT.md` format

`CONTEXT.md` is the committed, repository-root glossary for domain language shared by humans and cold agent sessions. Keep the following headings in this order and keep every entry understandable without conversation history.

## Required layout

```markdown
# Project domain context

## Canonical terms

| Term | Meaning |
|---|---|
| ... | ... |

## Aliases to avoid

| Avoid | Use instead | Why |
|---|---|---|
| ... | ... | ... |

## Relationships

| Concepts | Relationship |
|---|---|
| ... | ... |

## Ambiguities

| Topic | Current interpretation | Resolution condition |
|---|---|---|
| ... | ... | ... |
```

## Section rules

### Canonical terms

Define the project's preferred domain nouns and lifecycle verbs. State what each term means in this project, including boundaries that prevent likely misreadings. Do not duplicate implementation documentation.

### Aliases to avoid

List names that are plausible but misleading or less precise. Point each alias to its canonical replacement and explain the distinction. Do not list harmless grammatical variants.

### Relationships

Record how canonical concepts depend on, contain, produce, constrain, or transition into one another. Prefer one precise sentence per row.

### Ambiguities

Record unresolved meanings that could materially alter design or workflow. State the interpretation currently used and the observable decision or evidence that will resolve it. Remove the row after resolution and update the other sections with the settled meaning.
