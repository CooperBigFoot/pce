# Vision: RivRetrieve store authority

## Goal / Why

Make compiled stores the only authoritative copy without allowing a plausible but uncertified prefix.

## Acceptance criteria (vision-level "done")

```json
{"criteria":[{"name":"Certified stores replace artifacts","input":"Compile all declared bulk sources","observation":"Only verified stores replace publisher artifacts"},{"name":"Retrieval remains coherent","input":"Query each ported provider","observation":"Every provider reads through the shared store path"}]}
```
