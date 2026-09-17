# Automatic Jev decisions in PCE skills

## Outcome

Build TypeSafe AI's Jev into PCE's existing skills so normal use automatically benefits from focused semantic decisions. The user wants greater confidence, fewer errors, and faster useful progress, starting with vision creation after a grill and continuing through implementation and delivery verification. This is an everyday workflow integration to use for a while and then keep, adjust, or remove based on experience. It is not a standalone experiment, benchmark project, or new workflow the user must invoke.

The user continues to use the same commands, handoffs, and approval boundaries. After initial credential setup, the workflow should feel the same. Jev calls happen inside the skills at useful points, without routine approval prompts, extra manual steps, or a separate Jev command. Quiet, outcome-focused progress remains the default.

## Repository context and boundaries

PCE distributes six skills across Claude Code, Codex, and Prime Agent. Its standalone path is `grill-me → to-vision → implement-vision`; its Program path is `chart-program → grill-ticket → implement-vision → land-ticket`. Keep both paths and the existing publication, input-validation, independent-review, delivery, and landing contracts intact.

Implement a lightweight shared capability within the existing skill distribution. Choose reversible implementation details from repository evidence. Do not introduce a seventh workflow skill, compiled application, package manager, CI workflow, orchestration runtime, replacement durable records, or tracked generated runtime state. Git, GitHub, and repository visions remain authoritative. Preserve unrelated installation and user state. Do not tie the integration to a particular agent harness.

## Decisions delegated to Jev

### Context selection and conflicts

During vision authoring, implementation investigation, delegation, and resumed work, use Jev to rank or classify candidate source excerpts, documentation, and evidence against the current requirement. Identify direct relevance, supporting background, apparent conflicts, and useful existing implementations. Use these results to focus reading and prepare relevant subagent context.

Search tools and the reasoning agent still gather candidate evidence. Jev cannot discover omitted candidates or substitute for source inspection. Never omit repository instructions, the complete vision where required, or evidence required by an existing workflow gate because of a Jev relevance score. Keep uncertain and potentially conflicting evidence available for investigation rather than silently discarding it.

### Vision fidelity

In `to-vision` and vision authoring through `grill-ticket`, compare individual confirmed decisions with the draft. Identify decisions that appear preserved, weakened, contradicted, absent, or impossible to assess from the supplied context. Check proposed requirements for unapproved scope as distinct from ordinary technical elaboration.

The author investigates and repairs mismatches before publication review. Jev does not invent missing user intent, reopen settled decisions automatically, or approve publication. The same authoring mechanism applies when `chart-program` converges on a standalone vision.

### Implementation review targeting

In `implement-vision`, classify coherent changes for additional review focus, including public-interface changes, authorization or sensitive-data handling, persistent-data migrations, concurrency, and delivery-derived naming inconsistent with repository vocabulary.

Use those judgments to focus or supplement existing delegation and review. They do not replace the fresh independent reviewer, narrow its required full-vision/full-diff remit, or allow a negative risk signal to waive review. Architecture, planning, code generation, root-cause analysis, and resolution of review findings remain reasoning-agent responsibilities.

### Requirement-to-evidence matching

During implementation validation, delivery-record preparation, and `land-ticket` verification, match individual requirements to candidate tests, source excerpts, and observed validation evidence. Distinguish direct behavioral evidence, indirect evidence, unsupported assertions, and apparent contradictions. Direct the agent toward missing validation before it claims completion.

Semantic matching is not proof of correctness. Run tests and inspect source and target-branch effects as required. Jev never determines merge status, exact content equality, provenance, dependency acyclicity, permissions, or final delivery/landing authority in place of deterministic or explicitly verified facts.

## Runtime behavior and learning

Use narrow, explicit questions over the relevant state. Batch independent judgments when useful instead of adding a serial model call to every small step. Include an insufficient-evidence or no-match outcome where applicable. Avoid open-ended instructions such as deciding whether an entire implementation is correct.

When Jev is uncertain or its response is unusable, return the work to the reasoning agent, not automatically to the user. On unavailable credentials, service failure, or unavailable Jev access, continue through the ordinary PCE workflow without relaxing any safeguard. Briefly disclose fallback without repeated warnings or a retry loop that stalls progress. Missing permission to share content is not permission to transmit it; perform that work without Jev instead.

The user also wants to learn how Jev helps. Make compact decision details inspectable on request: the question, relevant evidence references, returned answer and uncertainty, and resulting action. Do not flood normal progress output or create another recovery database. Avoid retaining sensitive payloads by default. Never expose credentials in diagnostics. Record enough model identity to distinguish changes in model behavior without prescribing a storage mechanism.

## Credentials and data sharing

Jev is accessed through TypeSafe's hosted API. A local `.env` containing `TYPESAFE_API_KEY` was created in the canonical PCE checkout during discovery, with owner-only permissions and a local Git exclusion. Its presence is local setup context, not a portable dependency, permission to read or display the secret, or proof that authentication and billing work. No inference request was made during discovery. Never commit credentials or assume that downstream repositories and delegated environments can read this checkout's `.env`. Provide a secure, documented setup compatible with supported environments; routine use after setup should be automatic.

The user has not approved unrestricted transmission of private or confidential project material. Limit initial transmission to public or explicitly permitted content. For other content, use the normal agent path unless appropriate project-level permission has been established. Do not add repeated per-call permission prompts. Exclude secrets, credentials, and sensitive material from judgment payloads and routine records.

TypeSafe's reviewed standard terms allow storage and do not specify a fixed API-input deletion period. The privacy policy states US hosting and no training or fine-tuning on inputs; the customer agreement prohibits training on customer data without prior consent, allows telemetry processing, and permits some backup retention. Zero data retention is advertised for enterprise customers. Do not describe ordinary API use as zero-retention or treat saving an API key as consent to transmit confidential data. Recheck current terms before changing these assumptions.

## Scope exclusions

Do not modify `grill-me`'s interview process or add question screening. Context gathering during a grill may reuse the shared capability later, but is not required for this delivery. Failure triage, autonomous root-cause diagnosis, model-based merge approval, and replacement of the reasoning agent are also outside this initial integration.

## Evidence of completion

A fresh agent using either PCE path should encounter the relevant Jev behavior automatically through the skills, with the same user-facing invocations and safeguards. Demonstrate coherent coverage from vision fidelity through implementation context, review targeting, and delivery evidence matching, including the Program equivalents.

Validate the actual shared call path and integration behavior, not only the presence of wording. Cover useful responses, uncertainty, malformed or failed responses, missing credentials, and content not permitted for transmission. Demonstrate that fallback preserves the ordinary workflow and that mandatory context, review, and verification cannot be bypassed by a model score. Keep validation proportional; do not build a benchmark platform or make extended comparative research a prerequisite for everyday use. Never send private repository data merely to validate connectivity.

Run `python3 -m unittest discover -s tests -v` from the repository root. Installer changes must preserve unrelated state and be tested with an isolated temporary `HOME`. For bugs exposed during implementation, prove the failing path before fixing it as required by repository policy.

Actual speed and reliability improvements remain an empirical question for normal use. Successful installation is not evidence that Jev improves coding quality. The desired observable effects are less irrelevant context, fewer lost decisions, fewer unsupported completion claims, and less correction or rework. Jev confidence scores alone are not success measures.

## Documentation grounding

Consult current live documentation before implementation; API and model details can change. The following sources informed this design:

- [TypeSafe documentation index](https://docs.typesafe.ai/llms.txt)
- [Building with System One](https://docs.typesafe.ai/concepts/how-to-build-with-system-one)
- [Passage classification](https://docs.typesafe.ai/cookbooks/classifying_rag_passages)
- [Function calling](https://docs.typesafe.ai/cookbooks/function_calling)
- [Verification cascade](https://docs.typesafe.ai/cookbooks/sde_cascade)
- [Parallel questions](https://docs.typesafe.ai/patterns/fan-out)
- [Confidence](https://docs.typesafe.ai/confidence) and [Jev limitations](https://docs.typesafe.ai/model-jaggedness/jev-1.13)
- [HTTP API](https://docs.typesafe.ai/api) and [models](https://docs.typesafe.ai/models)
- [Legal overview](https://docs.typesafe.ai/legal), [DPA](https://typesafe.ai/legal/data-processing), [privacy policy](https://typesafe.ai/legal/privacy-policy), and [customer agreement](https://typesafe.ai/legal/mca)

These sources establish supported decision patterns, not demonstrated end-to-end gains on PCE. In particular, current documented weaknesses include irrelevant context, multi-step indirection, numeric precision, and adversarial input. Treat returned probabilities as judgments over supplied evidence, never authorization or guaranteed truth.
