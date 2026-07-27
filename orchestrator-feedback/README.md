# PCE orchestrator feedback

Use this directory to collect evidence-based feedback from an orchestrator after
it runs, pauses, or aborts the PCE workflow.

## Prompt for the orchestrator

Point the orchestrator to this file and give it this instruction:

> Read `orchestrator-feedback/README.md`. Review the PCE workflow you just ran,
> then create a feedback report in `orchestrator-feedback/` using
> `TEMPLATE.md`. Base every finding on evidence from the run. Do not change the
> workflow itself.

The report name must be `<YYYY-MM-DD>-<vision-slug>.md`. If that name already
exists, append `-2`, `-3`, and so on rather than overwriting an earlier report.

## Review scope

Evaluate the workflow as an orchestrator, not the product change delivered by
the run. Cover only observations supported by the run's transcript or durable
artifacts, including:

- where orchestration instructions were ambiguous, contradictory, or missing;
- unnecessary agent calls, retries, waiting, context use, or repeated work;
- failures in handoffs, grounding, state persistence, recovery, or escalation;
- gates that caught defects and gates that failed to catch defects;
- manual improvisations that should become explicit workflow rules;
- rules that added ceremony without improving the result.

For each finding, cite the relevant phase, command, artifact path, state entry,
or verdict. Separate observed facts from inferences. Do not invent timings,
token counts, or causal explanations that the available evidence cannot prove.

## Recommendation standard

Recommendations must identify the smallest concrete change and the file or
workflow section it would affect. Preserve useful constraints unless the report
shows that the constraint caused a specific failure or measurable waste. Mark
speculative ideas as experiments rather than required changes.

If the run produced no actionable findings, create the report anyway and state
that conclusion together with the evidence reviewed.
