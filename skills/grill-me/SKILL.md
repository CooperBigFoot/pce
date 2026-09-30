---
name: grill-me
description: Clarify an idea through focused questions. Use for grilling, stress-testing a plan, or reaching shared understanding before a vision.
---

# Grill Me

Reach shared understanding of the proposed outcome before implementation begins.

Build a **design tree** from the current conversation and available repository evidence. Each unresolved decision can branch into later decisions that depend on it. Work through that tree in **rounds**.

For each round, identify the **frontier**: every material decision whose prerequisites are already settled. Ask the whole frontier in one batch. Do not include a question if its answer depends on another unresolved question in the same round. After the human answers, update the design tree, recompute the frontier, and ask the next round.

Number every question so the human can answer by number. Use this format:

```markdown
❓ **Q1 - <question title>**

<Explain the decision and its material consequences in terms the human can evaluate. Include clear options when useful.>

➡️ **Recommendation:** <recommended answer and why>

---

❓ **Q2 - <question title>**

<question body>

➡️ **Recommendation:** <recommended answer and why>
```

For every question:

- state the consequence in terms the human can evaluate;
- give a recommended answer and the reason for it;
- distinguish intent, taste, priorities, and outcome trade-offs from implementation mechanics.

Investigate the repository before asking anything that code, documentation, tests, or established engineering practice can answer. Finding facts is the agent's responsibility. If research is still running, defer only the questions that depend on it and ask the rest of the current frontier.

Translate the human's goals into technical language. Do not ask the human to choose code structure, libraries, algorithms, file layouts, test mechanics, or other reversible implementation details merely because they are difficult. Choose those details from evidence.

Ask only when the answer depends on a real human preference, authority, or outcome-level trade-off. Continue until the frontier is empty and all material branches are resolved.

## Completion Gate

When the frontier is empty, do not ask a ceremonial question. Summarize the resulting understanding so it can stand alone for a fresh implementing agent. Include the intended outcome, material decisions, constraints, and any assumptions. Ask the human to confirm or correct that summary.

Confirmation completes `grill-me`; confirmation is not publication and does not authorize further action. The confirmed summary alone is not ready for `implement-vision`: it must first become a repository vision and satisfy the target-branch durability gate. Stop after confirmation. Do not implement, edit files, write a vision, or invoke another workflow unless the human explicitly requests that next action.
