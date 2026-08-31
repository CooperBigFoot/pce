---
name: to-vision
description: Materialize the current conversation's shared understanding as a durable standalone vision. Use when the user asks to capture, write, or convert the discussion into an implementation vision.
---

# To Vision

Turn the completed discussion into one standalone handoff for a fresh Prime Agent that cannot see the conversation.

## Name and path

Use `$ARGUMENTS` as the human-readable vision name. If it is empty, ask only for the name. From the target repository root, run:

```bash
python3 <path-to-this-skill>/scripts/create_vision.py "$ARGUMENTS"
```

The helper prints `planning/visions/YYYY-MM-DD-<slug>.md`. It creates an empty file only when the path does not exist. A repeated call returns the existing path without changing its content.

Read an existing file before editing it. Never silently replace prior content. Reconcile the current shared understanding with it and preserve still-valid information.

## Authoring contract

Choose the structure that best communicates this specific work. Do not impose fixed headings, a JSON schema, command-level acceptance syntax, or a universal template.

Write enough context that the fresh agent can implement without the prior conversation. Where relevant, communicate:

- the desired outcome and why it matters;
- externally observable evidence of success;
- scope boundaries and explicit exclusions;
- constraints and settled decisions;
- important repository facts learned during the grill;
- material risks or genuine uncertainty that remains.

Translate intent into useful technical context, but leave reversible mechanisms to the implementing agent. Do not invent unresolved questions to fill a section. Do not reopen decisions settled during the grill.

After writing, report the path and a concise summary of what the document captures. Do not invoke implementation automatically.
