---
name: land-ticket
description: Land one delivered Effort ticket in the active Program — prove delivery from the driver's own journal, record the decision on the Map, and close the ticket. Use for `/land-ticket <n>`. Asks the human exactly one question, and never asks them to confirm mechanics.
---

# Land a ticket

Landing records that a question was answered. Everything about *whether* it was answered is already
on disk, because the driver executed every criterion and replayed every gate finding before it
called anything complete.

So this skill reads rather than interrogates. **It asks the human one question**, and that question
is the only thing in the whole procedure a human is better placed to answer than the binary.

Run every command from the repository the vision belongs to.

## The question budget

This is a hard constraint on the skill, not a style preference.

**One question by default.** Two only when the Program is genuinely finished. Three only when a
decision meets every ADR condition.

Every other fact is derived. If you find yourself wanting to ask something else, the answer is
somewhere in the journal, the graph, git, or `gh` — go and read it. A ratification the human cannot
evaluate launders a machine decision into a human one, and this corpus records that happening twice.
Asking a human to supply an observation, select a ref, interpret an exit status, or confirm a
reconciliation is that failure, not diligence.

## 1. Establish the ticket, the Program, and the vision

`$1` is the Effort ticket number. If absent, ask which ticket and stop.

```bash
gh issue list --label pce:program --state open
```

Exactly one open Program is required. Zero or several: report and stop.

Read the ticket and the Map in full with `gh issue view`, including comments. The ticket must carry
`Program: #N` matching the Map. From the ticket, find the vision directory it was delivered through;
if it names none, read the Map's index. Never ask the human for a path.

## 2. Prove delivery from the driver's journal

The driver already holds the proof. Read it:

```bash
pce package driver-status --graph <vision-dir>/graph.v<N>.json --journal <vision-dir>/driver-events.jsonl
```

Delivery is proved when, and only when, all of these hold:

1. Every package in the frozen graph is complete. A parked, failed or never-dispatched package means
   the ticket has not been delivered.
2. Every criterion of every package has a recorded execution that exited zero — including criterion
   amendments accepted from gate findings, which are criteria like any other.
3. Every gate finding was replayed and decided. An accepted finding carries a witness at which its
   command failed and a repair at which it passed.
4. The work merged. Verify with fresh `git` and `gh` observation, never by assertion.

Any of these failing is a stop. Report which one, with the recorded output of the failing criterion
if that is the cause. **Do not ask the human to interpret it, override it, or accept it anyway.** A
landing that can be talked into completing is not a proof of delivery.

Render the run and publish it as an artifact, so the evidence is something the human can look at
rather than a claim they have to take:

```bash
pce package render --graph <vision-dir>/graph.v<N>.json --journal <vision-dir>/driver-events.jsonl --output <vision-dir>/landed.html
```

## 3. Ask the one question

Everything above is mechanical. This is not.

Show the human, in plain language and with no identifiers:

- What the vision said would be true when this was done.
- What is now true, drawn from the criteria that ran — the world, not the test names.
- What changed along the way that they did not ask for, if anything.
- What the graph deliberately did not cover.

Then ask exactly one thing, in your own words but to this effect:

> Does the world now look like the one this vision described?

That is the question only they can answer. The machine can prove that every command it was given
exited zero; it cannot know whether the commands described the world the human wanted. The most
expensive failure in this corpus was work that was correct against every criterion and was still the
wrong build, and it surfaced only when a human saw a concrete description and flinched.

**No** is not a failure of this skill. It means the vision was wrong, which is a finding: record it
on the ticket, leave the ticket open, and stop. Do not attempt to re-plan here.

**Yes** proceeds. Do not follow it with a confirmation.

## 4. Write the decision, derived not asked

Compose the Map's `Decisions so far` entry yourself, from the ticket's question, the frozen graph,
the criteria that ran, and any ADRs the vision produced. One line, present tense, naming what is now
true rather than what was done. Match the voice of the existing entries.

You are not asking the human to author or approve this sentence. They answered the question in step
3; this is the record of that answer.

Append it inside the existing `## Decisions so far` section, immediately before the next `## `
header. Preserve every other section byte-for-byte. If the ticket already has an entry, reconcile it
rather than duplicating.

**The ADR question, and only if it earns itself.** Offer to write an ADR only when all three hold:
the decision is hard to reverse; it would be surprising to a future reader without context; and it
came from a real trade-off between genuine alternatives. If any one fails, do not ask. When you do
ask, propose the ADR's content — do not ask the human what it should say.

## 5. Apply, in an order that makes partial failure auditable

No approval step. There is no new territory here: Fog graduation and ticket minting belong to
`/chart-program`, which surveys the whole Program with the human's attention on it. A landing that
also re-surveys the Program buries a survey inside a question about one ticket.

1. Update the Map body with the decision entry and the refreshed ticket index.
2. Close the Effort ticket, with a comment carrying the artifact link and the one-line decision.

After each write, retain the result. If a write fails partway, stop, report exactly what landed and
what did not, and compensate for nothing silently.

## 6. Program completion — the second question, rarely

The Program is finished only when both hold at once: no open `pce:ticket` issue references this Map,
and `Not yet specified` is empty.

If either fails, say what remains and stop. **Do not ask about closing.**

Only when both hold, compose a summary from the Destination, the complete decision index, and the
ADRs, then ask whether to close the Program. Nothing earlier authorizes that closure.

## 7. Report

Open with what the Program now knows that it did not know before, and what is worth taking next — in
plain language, no issue numbers.

Then, marked as skippable: the ticket closed, the decision recorded, the artifact link, any ADR, the
remaining Frontier and Fog, and anything that did not apply cleanly.

If the human cannot tell from your first paragraph what was settled and what they would do next, the
report has failed regardless of its accuracy.
