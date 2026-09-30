# How Lineage works

Lineage has two parts. The **guard** decides what an agent may do. The **audit log** records every decision so that nobody can change the record later without being caught. They're built so that the log is the only source of truth: the guard's state is whatever the verified log says.

## The lifecycle of an action

```text
          request(tool, input, cost)
agent ───────────────────────────────▶ guard
                                         │ 1. record the request
                                         │ 2. terminated?             → denied
                                         │ 3. tool in policy?          → denied + scar
                                         │ 4. tool's call cap reached? → denied + scar
                                         │ 5. rate limit reached?      → denied + scar
                                         │ 6. enough budget?           → denied
                                         │ 7. needs approval?          → pending_approval
                                         ▼
                                      allowed (budget spent)
                                         │
agent runs the tool, then ── report(outcome) ──▶ guard   (failure: minor scar, harmful: severe scar)
```

Every numbered step that decides something appends a record to the agent's log: the request, the allowance or denial, the approval, the outcome, and any scar. When scars reach the policy's limit, or the budget reaches zero, a `terminated` record is appended. From then on, every request is denied.

## Agents

An agent is one log file, `agents/<agent-id>.jsonl`, with a fixed policy. There is no separate database. `Guard::create` writes a `genesis` record and a `policy` record. `Guard::open` verifies the whole log and replays it to rebuild:

- the budget spent;
- the scars and the scar score;
- every action and its status (requested, pending approval, allowed, denied, completed);
- per-tool call counts, and the recent actions the rate limit counts;
- whether the agent is alive.

Because state is derived from the log, there is nothing to reset. Deleting records breaks the hash chain; editing them breaks the signatures; starting over means creating a new agent, with a new identity and an empty history, which is exactly what should happen.

## Costs and budgets

The budget is a number of credits the agent may ever spend. What a credit means is up to you: API calls, tokens, dollars. The DeepSeek example uses whole US dollars, so the budget *is* the agent's spending authority.

Each tool has a minimum `cost`. A request may declare a higher cost, for example the number of tokens a model call used, or a payment's amount. It can never declare a lower one. Credits are charged when an action is allowed, and are never refunded, even if the tool then fails.

## Scars

Scars are permanent marks on an agent's record, each with a severity and a reason. Their weights add up to a scar score. When the score reaches the policy's `scar_limit`, the agent is terminated. See [Decisions, scars, and termination](decisions.md).

## The guard in-process or over HTTP

- **In-process** (Rust): `lineage::guard::Guard`. One process owns the log, locked against a second writer.
- **Over HTTP** (`guard-server`): the same guard, one log per agent. It adds per-agent tokens, an operator role for approvals, and a web console. Agents in any language use it, and it's the right choice whenever the people approving actions are not the agent's own process.

## What Lineage does not do

- **It does not sandbox tools.** The guard decides; your code carries out the decision. See [Security model](security-model.md) for how to make decisions binding on tool backends.
- **It does not judge content by itself.** Harm is reported by you: a reviewer, a monitor, a rule. The example apps show monitors that redact secrets and a fraud reviewer that runs rules and a model.
