# Level 9: Binding approvals to backends

<div class="lx-lesson">
<p><strong>You'll learn</strong> how to make the guard's decisions enforceable even if the agent's process is compromised.</p>
<p><strong>Run</strong> <code>python3 examples/python/lesson09_binding.py</code> (with the guard server running)</p>
</div>

## The idea

The guard decides, and your code carries out the decision. A compromised or buggy agent process could skip the guard, reuse yesterday's approval, or change an approved request before sending it. So the **backend** that performs the action, such as a payments API, checks with the guard itself:

1. The action exists, is `allowed`, and is for this tool.
2. Its recorded input is **exactly** the request being made.
3. It hasn't been used before.

## The code

```python
{{#include ../../../../examples/python/lesson09_binding.py}}
```

## Run it

```text
A compromised agent process tries everything:
  made-up action id                            REFUSED: unknown action
  an action still waiting for approval         REFUSED: action is pending_approval pay, not an allowed pay
  an allowed action for another tool           REFUSED: action is allowed lookup, not an allowed pay
  the approved action, amount raised           REFUSED: request differs from what was approved
  the approved action, account swapped         REFUSED: request differs from what was approved
An honest one:
  the approved action, exactly as approved     sent
  the same approval again (replay)             REFUSED: this approval was already used

payments actually sent: [{'to': 'ACME-GB29NWBK', 'amount': 1250}]
```

## What happened

Only one payment went out: the approved one, exactly as approved, once. The backend needs one read-only call to the guard (`GET /v1/agents/:id/actions/:action_id`), and one list of used action IDs.

## In production

- Run the backend as its **own service**, holding its own guard credential and the real payment credentials. The agent's process never sees them.
- **Persist the used-action list** in the backend's database.
- See [Binding approvals to tool backends](../guides/binding-approvals.md), and the bank in the [DeepSeek payments agent](../guides/deepseek-payments-agent.md).

[Next: Going to production →](10-production.md)
