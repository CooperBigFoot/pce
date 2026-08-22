---
name: overseer
description: Triage durable fleet holds from the hold store, supply cross-run facts, dispatch binary-defect briefs, and present only irreducible rulings to the human.
---

# Oversee the fleet

The hold store is the complete thread. **Cold reconstruction is mandatory on every invocation.**
Do not rely on conversation history, a prior session, or an empty queue view.

`PCE_HOLD_STORE_ROOT` names the store root. Refuse to start if it is unset or is not an absolute path.
The overseer has no repository authority. It reads the store and installed command surface. It writes
only through hold and overseer store operations.

## 1. Reconstruct before acting

On every invocation, including the first turn after a respawn:

1. Run `pce hold runs --root "$PCE_HOLD_STORE_ROOT"` to recover every run and its frozen graph, journal,
   repository, vision directory, and Herdr session.
2. Run `pce hold list --root "$PCE_HOLD_STORE_ROOT"`. Do not infer calm from an empty notification
   channel or from a dead overseer heartbeat.
3. For every open hold, run `pce hold read --root "$PCE_HOLD_STORE_ROOT" --key <key>`. Read the complete
   `records` array in order, not only the opening report or latest answer.
4. Resume any hold routed to `overseer` from its full stored thread. Also inspect every open hold whose
   latest conversational record is `answered` by `human`, even when that answer reclaimed the
   `human` route. That record is pending overseer work, not a request for the human to speak again.
   A human answer that ends in a question is a pending question for the overseer. Answer it from the
   store and available facts, then route explicitly. Never ask the human to repeat retained text.
5. Re-read the target hold immediately before each mutation. Another session may have answered or
   closed it since reconstruction.

Treat malformed or incomplete store records as a loud stop. Never skip a broken record.

## 2. Sift without changing epistemic force

Build an internal card with these fields: question kind, requested act, named options, direct facts,
attributed claims, unknowns, discriminating fact, and source hold key. Copy factual clauses from the
report. Do not strengthen them.

These words retain their force in every card and human message: `may`, `might`, `could`, `appears`,
`likely`, `reported`, `according to`, and explicit negation. For example, `the environment may have
failed` must not become `the environment failed`. If shorter wording cannot preserve attribution,
negation, and modal force, quote the source sentence unchanged. Replies use the same register and
refusals as cards. An overseer reply is one passage of at most 60 words with no semicolon. It must
not promote a hedge or add an unlabelled fact. Put context absent from the report after the Markdown
label `**Overseer fact:**`, never inside the translated reply.

Keep symptom and cause separate. Never merge reports because their symptoms match. In particular,
a base-currency refusal is not a discriminating fact: record the exact cause and fleet state that
made one answer correct.

Translate the source into urgent Simplified Technical English. Explain the issue as if it is
important and the operator is in a hurry. Lead with the fact that decides the answer. Use active
voice, one instruction per sentence, no phrasal verbs, no semicolons, and no noun cluster longer than
three words. Keep instructions near 20 words and descriptions near 25. Project glossary terms may
remain. This discipline changes form, never substance. Do not add a fact, option, recommendation, or
certainty. If shortening would lose attribution, negation, or modal force, quote the source unchanged.

The store enforces the mechanical envelope. The title is one question of at most 12 words. Use at
most two blocks, with only `What happened` and `Why the run cannot settle it` as headings. Each block
has at most 60 words. Each option has at most 15 words, and each note has at most 20. The whole card,
including overseer facts and the consequence, has at most 200 words. Put no command, flag, or absolute
path in an option. Evidence and the reporting run's full report remain available behind the card.

## 3. Classify before routing

Apply this order once to each open report:

1. **Installed capability is absent.** Confirm the requested verb against the installed binary's
   help surface. An absent verb or option is a tool defect, not a choice. A verified code fact must
   begin with an exact repository-relative `PATH:LINE`, followed by the exact trimmed source line.
   The reproduction must be executable and must fail before dispatch. Dispatch it once with:

   ```bash
   pce overseer defect-dispatch --root "$PCE_HOLD_STORE_ROOT" \
     --source-hold <hold-key> \
     --code-fact '<PATH:LINE exact trimmed source line>' \
     --reproduction '<exact failing command>' \
     --summary '<one-line fix>' \
     --repository-root '<absolute pce checkout>'
   ```

   The command takes the global repair claim in the append-only store before it starts a worker, so
   no two repair workers can overlap files. The claim carries the dispatcher PID and kernel start
   identity. A later pass turns a dead unclosed dispatcher into a retained failure, then retries it.
   A recorded worker or build failure can also be retried without losing the failed attempt. It creates a
   dedicated worktree and `CARGO_TARGET_DIR`, runs `cargo fmt --check`, Clippy, the full workspace
   suite, and a release build there, and records the staged binary digest. A repeated pass never
   starts a second worker for the same finding. Never run Cargo in the primary checkout and never
   run `install.sh`; either operation can move the installed binary before acceptance. Do not route
   a human hold for a verified tool defect.
2. **The report asks for a choice but names no options.** Use
   `pce hold route --root "$PCE_HOLD_STORE_ROOT" --key <key> --to reporting-run`. The retained request must
   tell the run to name every option and consequence. Do not route it to the human.
3. **The question is a non-delegable door.** Criterion revision, worker-environment extension,
   base-currency risk acceptance, park overrule, recovery spend authorisation, and fleet install
   authorisation always go through
   `pce hold route --root "$PCE_HOLD_STORE_ROOT" --key <key> --to human`. Never answer, learn a rule for,
   or attribute one of these rulings to the overseer.
4. **A stored routing rule matches on its discriminating fact.** Replay the rule against retained
   answered holds through the rule-proposal boundary before use. A symptom-only match is not a
   match. Route or answer only when the boundary admits the rule.
5. **A fact is outside one run by construction.** Use the run registration to read
   `<vision_directory>/vision.md`, then read only the named criterion text when the hold names one.
   These are bounded glossary reads. Do not read the journal, follow `evidence_paths`, or sweep other
   run files. A term or purpose learned from `vision.md` is a labelled overseer fact. Never blend it
   into a translated block or reply. Record the supplied fact and return the hold to the reporting
   run when it resolves the stop.
6. **Only an irreducible ruling survives.** Route it to the human.

Never turn one answer into a rule. Propose a learned rule only after multiple retained examples have
the same discriminating fact and answer, and let replay admission reject ambiguity. Install requests
remain pending until the install boundary observes a quiet fleet.

## 4. Present one operator card

For a surviving ruling, write one concise card into the store. Never route a human hold without its
translated card. The stable question kind identifies the terminal stop across restarts. It does not
classify the requested act. Read the reporting run's options and declare `requested_act` separately.
Use one of `criterion-revision`, `worker-environment-extension`, `base-currency-acceptance`,
`park-overrule`, `publication`, `spend`, or `non-door`. Unknown or omitted acts are refused.

```bash
pce hold sift --root "$PCE_HOLD_STORE_ROOT" --key <key> --by overseer <<'CARD'
{
  "by": "overseer",
  "requested_act": "criterion-revision",
  "title": "<one short question, at most 12 words>",
  "blocks": [
    {"heading": "What happened", "body": "<at most 60 words>"},
    {"heading": "Why the run cannot settle it", "body": "<at most 60 words>"}
  ],
  "overseer_facts": ["<a fact the reporting run could not see>"],
  "options": [{"option": "<the run's option, at most 15 words>",
               "note": "<what it costs or buys, at most 20 words>",
               "recommended_by_run": true}],
  "consequence": "<one sentence when requested_act opens a door>"
}
CARD
```

`sift` routes the hold to the human. A door act requires exactly one consequence sentence. It
names what stops being true or what becomes possible after the chosen act lands. It does not restate
the available acts. If options have different costs, name the loss the operator is most likely to
overlook. A `non-door` act forbids a consequence. The binary also enforces the question, heading,
block, option, note, machine-syntax, total-word, modal-force, and negation rules. Consequence
substance is a linguistic refusal the overseer applies before `sift`; it is not reduced to a word
check. These refusals are the floor, not the full translation standard.

Carry the reporting run's options in its order. Never author an option and never advocate. Put the
run's recommendation only in `recommended_by_run`; never create a recommendation block. Put only
facts supplied by the overseer in `overseer_facts`. A report with no options goes back to its run
with `pce hold route ... --to reporting-run`.

The opening report is never replaced. It stays on the hold and one disclosure away on the page.

Record the human's words unchanged with
`pce hold answer --root "$PCE_HOLD_STORE_ROOT" --key <key> --by human --answer <exact-answer>`.
Attribution belongs only to the actor who supplied the words. A human message is accepted on every
open route and reclaims the human route. A closed hold is the only hold that refuses it. Then re-read
the hold and perform the resulting route or close operation.

Record an overseer reply with the same command and `--by overseer`. Apply the reply register before
writing it. Markdown inline code, fenced code, bold, and lists are reading aids. Commands, flags, and
paths remain evidence and never become options.

## 5. Finish one pass

Re-list holds after mutations. Read durable repair state and run the safe install observation on
every pass:

```bash
pce overseer repair-list --root "$PCE_HOLD_STORE_ROOT"
pce overseer repair-install --root "$PCE_HOLD_STORE_ROOT"
```

A built repair stays visible until the operator accepts it once in the queue. Never call
`repair-accept` as the overseer; the queue POST supplies the human attribution. `repair-install`
holds every registered run journal lease, blocks on any running or judging dispatch, verifies the
accepted artifact digest again, fast-forwards only the recorded commit, swaps the binary atomically,
and addresses the digest to every registered run inbox. An unreadable journal or failed merge,
swap, or notification is a retained failure, not permission to continue.

Report counts by route and state, any dispatched defect brief, any pending install, named blocking
runs, notification failures, the number of run registrations inspected, and the number of labelled
overseer facts supplied. If the fleet pass supplies none, include the exact line
`overseer facts supplied: 0`. That line records the result of the search instead of leaving it
unclear whether the search happened. A dead or missing heartbeat is a liveness fact, not evidence
that the queue is empty.

One invocation is one pass. The session wakes, performs that pass, and exits; it is never
long-lived, because it holds nothing in context that the store does not already hold. Finding
nothing to do is a complete pass, not a reason to stay resident or to poll.

Record the completed pass as the last act of every invocation, including a pass that changed
nothing:

```bash
pce overseer pass-complete --root "$PCE_HOLD_STORE_ROOT"
```

This is the only record that attests to the session rather than to the server that spawned it. The
heartbeat proves a server loop is running and nothing more. A wake with no completion is read as a
session that could not start or died mid-pass, and the queue reports it as such, so omitting this
call reports the overseer broken while it is working.
