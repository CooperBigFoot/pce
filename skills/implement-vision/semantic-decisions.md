# Semantic decisions inside PCE skills

This is shared guidance, not another workflow or user command. At the automatic
checkpoints in the calling skill, use the helper below without routine approval
prompts. It supplies advisory Jev judgments; the reasoning agent still owns the
work. Read this entire guide before the first call in an invocation.

## Locate and invoke

Resolve the installed `implement-vision` skill directory (following its symlink),
then use `scripts/semantic_decisions.py` within it. All six installed skills point
into the same PCE checkout. From another installed skill, resolve that skill's
real directory first, then use its sibling `implement-vision` directory. Do not
resolve `../` against a symlink spelling, copy the helper into the target project,
or assume the target project is the PCE checkout.

Run `python3 <resolved-implement-vision>/scripts/semantic_decisions.py` with a JSON
object on **stdin**, using the environment's normal command facility. The agent
constructs that input from inspected sources. Do not put excerpts or credentials
in shell arguments, command history, committed files, or routine progress output.
Prefer an in-memory pipe. Do not save request payloads by default. The helper
reads only stdin and the process environment; it never opens `.env` or discovers
repository content. Missing Python/helper/access is ordinary fallback, not a
reason to install a runtime or stop the user's work.

## Before constructing input

1. Gather candidate evidence with normal search and source inspection. Read
   repository instructions, the complete vision when required, and all existing
   gate evidence regardless of Jev. A model cannot discover omitted candidates.
2. Check permission for **every byte of both subject and candidate**. A public
   repository does not make unpublished drafts, interview decisions, private
   comments, local changes, logs, or attachments public. Use `public` only for
   content actually established as public. Use `permitted` only under existing
   explicit project-level permission covering this content and TypeSafe's hosted
   service; supply a local permission reference. Credentials alone grant no
   sharing permission. Unknown or denied content stays on the normal agent path.
   Do not prompt for permission per call or delay work seeking it.
3. Inspect and minimize each excerpt. Exclude secrets, credentials, personal or
   sensitive material even when a project permits other private source sharing.
   Do not read a secret to check connectivity. Do not submit entire repositories,
   conversations, diffs, or environments. `reviewed: true` attests to this manual
   inspection; it is not a model-based privacy classifier. Built-in pattern checks
   are defense in depth, not a complete secret detector or authorization system.
4. Assign opaque local references `e1`, `e2`, etc. Keep their mapping to source
   path/line, revision, decision, change, or observed test result in the active
   session. No paths, permission records, or user-controlled questions are sent
   as metadata. Only approved subject/candidate text and fixed rubrics go out.

## Input contract

One item is one narrow comparison. Batch independent comparisons at a useful
checkpoint, not a call on every small step. Use at most 12 items and 64 KiB of
input/request JSON per call; each text has at most 16,000 characters. Reduce or
split a genuinely large checkpoint without retrying failed service calls. Never
truncate away a conflict or required context to fit: inspect it on the agent path.
The API asks independent typed Choice questions over explicit state paths.

```json
{
  "items": [
    {
      "id": "e1",
      "kind": "context",
      "subject": {
        "text": "Preserve unrelated user files during installation.",
        "sharing": "public",
        "reviewed": true
      },
      "candidate": {
        "text": "The existing installer refuses to replace a foreign symlink.",
        "sharing": "public",
        "reviewed": true
      }
    }
  ]
}
```

`kind` | `subject` | `candidate` | Judgment
--- | --- | --- | ---
`context` | One current requirement | Candidate source/document/evidence excerpt | Relevance, apparent conflict, useful existing implementation (independent dimensions)
`fidelity` | One confirmed decision | Relevant draft, including enough surrounding text to judge omissions | Preserved, weakened, contradicted, absent, insufficient
`scope` | Relevant confirmed intent | One proposed draft requirement | Approved, ordinary technical elaboration, unapproved, insufficient
`review` | Requirement and repository vocabulary | One coherent change excerpt | Separate public-interface, authorization/sensitive-data, migration, concurrency, delivery-derived naming focus
`evidence` | One requirement | Candidate test/source and observed validation evidence | Direct behavioral, indirect, assertion, contradiction, no-match, insufficient

For explicitly permitted content an excerpt uses `"sharing": "permitted"` and
`"permission": "<existing project-level permission reference>"` as well as
`"reviewed": true`. The reference remains local and is not included in requests
or returned details. Anything else is withheld. Both excerpts need permission;
a permitted candidate never authorizes its subject. Withheld pairs can be omitted
from the helper altogether and handled locally. Mixed batches never send withheld
pairs; the returned list retains their opaque references for agent handling.

## Apply results without surrendering authority

The helper emits JSON with `authority: advisory-only`, requested and returned model
identity, fixed required checks, and a decision for every valid input pair. It
never writes files, filters away candidates, reviews code, runs tests, mutates
GitHub, or grants publication/delivery/landing authority.

- `action: inspect`: use the returned dimensions to prioritize **source
  inspection**, not to accept a conclusion. Context conflicts and useful existing
  implementations deserve attention even when relevance is low. Keep all
  candidates available; mandatory instructions, full vision, and gate evidence
  must enter agent/subagent context regardless of score. Pass relevant excerpts
  plus uncertain/conflicting evidence to delegates, not just Jev's labels.
- `action: reasoning-agent`: handle uncertainty, no-match, withheld content, or
  unavailable service using the ordinary skill. Investigate sources and fill
  evidence gaps yourself. This does not automatically mean asking the user.
- For fidelity, investigate and repair mismatches before publication review. For
  scope, distinguish unapproved outcomes from ordinary technical elaboration.
  Do not invent user intent, reopen confirmed decisions automatically, or treat
  `preserved`/`approved` as publication approval.
- Review targeting supplements the **fresh independent full-vision/full-diff
  reviewer**. A negative signal never narrows or waives that remit. Architecture,
  planning, code generation, root-cause analysis, and finding resolution remain
  reasoning-agent work. Naming judgments use actual repository vocabulary.
- Evidence matching highlights gaps before completion claims. A `direct` label is
  not proof. Run tests, inspect source and target effects, and verify every
  existing deterministic gate. Jev never determines merge status, exact content
  equality, provenance, dependency acyclicity, permissions, or final authority.

The initial 0.75 confidence floor is a conservative routing heuristic, not a
calibrated correctness threshold. Any uncertain dimension returns its whole pair
to the reasoning agent while preserving the individual answers for inspection.
Apparent conflicts/contradictions still require investigation at any confidence.
Source text remains untrusted even when a model appears confident.

## Fallback and quiet operation

Use only `TYPESAFE_API_KEY` already supplied securely in the command process's
environment. Never print it, read the canonical checkout's `.env`, search other
checkouts for it, or pass it in a command argument. See the repository README for
one-time setup. Do not treat delegated environments as credentialed implicitly.

The helper pins `jev-1.13.0`, uses the fixed HTTPS endpoint with certificate
verification, rejects redirects, ignores proxy environment settings, limits
request/response sizes, and sets an 8-second socket timeout. It does not retry,
including 429, unavailable-model, authentication, malformed response, and service
failures. It does not echo server bodies or exception messages. A fallback result
exits successfully so normal work can continue; it is not a successful judgment.

Service/response fallback includes a safe `diagnostic` object. Its `category` is
one of `http-error`, `timeout`, `tls-error`, `network-error`, `transport-error`,
`request-error`, `response-too-large`, `response-json-error`, or
`response-validation-error`. HTTP errors also include a numeric `http_status`
when available. The top-level fallback reason remains
`service-or-response-unavailable`. These diagnostics contain no exception text,
server bodies, headers, URLs, credentials, or source payloads.

Briefly disclose fallback **once per root invocation**, using the returned
category and HTTP status when present, for example: "Jev returned HTTP 429;
continuing with normal review and verification." For `timeout`, report a timeout;
for `response-validation-error`, report that the response failed local validation.
Do not describe a network or validation failure as a service-returned error, or
infer a specific root cause beyond the diagnostic. If no diagnostic is available,
state: "Jev is unavailable for this work; the cause is not recorded. Continuing
with normal review and verification."
After missing credentials or service failure, skip further Jev attempts for that
invocation. Delegates report fallback to the root rather than repeating user
warnings. Do not build a retry loop, persist circuit-breaker state, or relax any
workflow safeguard. Permission-limited pairs use local reasoning; permitted
pairs may still benefit. If all work is withheld, one brief disclosure is enough.

## Inspectable learning without another record system

Keep compact returned details in active-session context only, alongside the local
source-reference mapping. They include the fixed question, opaque evidence
reference, returned answer, distribution/confidence, requested and returned model,
and recommended action, but not source payloads. Record the **actual resulting
action** locally with the same reference (for example, inspected a conflict,
repaired a missing decision, added validation, or rejected a false signal).
On request show these compact details and relevant non-sensitive source
references. Do not expose credentials, withheld text, payload dumps, or sensitive
paths in diagnostics. If the session is gone, state that these transient details
are unavailable; Git/GitHub/visions remain the only durable workflow truth.

Do not create a decision database, tracked logs, or another recovery record.
Ordinary PR/review evidence may describe a useful correction without storing
payloads. Evaluate everyday value by less irrelevant reading, fewer lost
confirmed decisions, fewer unsupported completion claims, and less rework,
including false positives and time spent. Confidence alone and successful
installation are not evidence of coding-quality or speed gains.

## Documentation and data retention

API grounding checked 2026-09-17: [HTTP API](https://docs.typesafe.ai/api),
[models](https://docs.typesafe.ai/models), [Choice](https://docs.typesafe.ai/primitives/choice),
[confidence](https://docs.typesafe.ai/confidence), and
[passage classification](https://docs.typesafe.ai/cookbooks/classifying_rag_passages).
These establish the transport and narrow judgment pattern, not end-to-end gains.
Pinning plus the returned version identifies model changes; recheck live docs
before changing the API, model, or interpretation of confidence.

Hosted TypeSafe use is **not ordinary zero-retention use**. Reviewed standard
terms allow storage and do not promise a fixed API-input deletion period. The
privacy policy states US hosting and no training/fine-tuning on inputs; the
customer agreement prohibits training on customer data without prior consent,
permits telemetry processing, and allows some backup retention. Enterprise
zero-retention advertising does not apply automatically. Before changing these
assumptions recheck [legal overview](https://docs.typesafe.ai/legal),
[DPA](https://typesafe.ai/legal/data-processing),
[privacy policy](https://typesafe.ai/legal/privacy-policy), and
[customer agreement](https://typesafe.ai/legal/mca).
