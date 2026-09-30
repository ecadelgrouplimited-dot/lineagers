# Building an app

A template gets you a running agent. This page covers turning it into something you'd put in front of customers or money. Both example apps, [Claude ops](../guides/claude-ops-agent.md) and [DeepSeek payments](../guides/deepseek-payments-agent.md), follow this structure; read them next to this page.

{{#include ../diagrams/anatomy.html}}

## 1. Write the policy first

Start with what the agent must be able to do, and nothing else.

```json
{
  "budget": 25000,
  "scar_limit": 10,
  "rate_limit": { "max_actions": 60, "window_secs": 60 },
  "tools": {
    "llm_turn":     { "cost": 0, "max_calls": 60 },
    "read_invoice": { "cost": 0 },
    "pay_invoice":  { "cost": 1, "requires_approval": true, "max_calls": 10 }
  }
}
```

Ask these questions of every tool:
- **Could it cause harm if the model is tricked?** Then require approval, or leave it out.
- **Is it irreversible?** Cap it with `max_calls`.
- **Does it cost money?** Make its cost the amount.

Then guard **model turns** too (`llm_turn`), so the loop itself is bounded.

## 2. Guard the loop

Wrap every model call and every tool call; the [LLM agent guide](../guides/llm-agent.md) has the complete pattern. The rules:

- **Denials become tool errors**, returned to the model with "do not retry".
- **Costs come from your code**, computed from the request, never from the model.
- **Report every outcome**, `success` or `failure`.
- **Stop as soon as the agent is terminated.**

## 3. Put reviewers and people where the policy holds back

Everything with `requires_approval` lands in the approval queue. Decide who answers it:

| Layer | Example | Can |
|---|---|---|
| **Rules** | IBAN matches the vendor master; the invoice isn't already paid | Reject. Never overridden by a model |
| **Model reviewer** | A reasoning model reads the email and the invoice | Reject or escalate; approve only low-risk cases |
| **People** | The console, a Slack bot, a ticket | Everything else |

The DeepSeek app's [`approvals.py`](https://github.com/ecadelgrouplimited-dot/lineagers/blob/main/apps/deepseek-payments-agent/approvals.py) is a complete router in about 100 lines. Record each reviewer's verdict as the approval's signed `note`.

## 4. Watch outputs, report harm

A **monitor** sees what tools return and what the agent sends out:
- **Redact secrets** from tool output before the model sees them.
- **Block outbound calls** (tickets, emails, status pages) that would leak something.
- **Report harm** with the operator token: `report_harm`, a severe scar.

The Claude app's [`monitor.py`](https://github.com/ecadelgrouplimited-dot/lineagers/blob/main/apps/guarded-agent/monitor.py) does all three.

## 5. Make backends check approvals

For anything that moves money, changes production, or talks to customers, the backend should confirm with the guard that the action was allowed, with exactly this input, and not used before: [Mastery level 9](../mastery/09-binding.md).

## 6. Test with a scripted model

Every example app has a scripted model that returns real SDK response objects, including scenarios where the model is **fooled**. Test that the guard, reviewers, monitors and backends stop it:

```python
def test_fooled_clerk_cannot_pay_the_attacker(self):
    ...
    self.assertNotIn("GB94BARC10201530093459", [t["iban"] for t in bank.transfers])
    self.assertEqual(denied, ["update_vendor_bank_details", "pay_invoice", "pay_invoice"])
```

Run these tests in CI against a real guard server; the example apps' tests start one on a free port.

## 7. Ship

Deploy the guard server, publish checkpoints, and watch scars and quarantines: [Mastery level 10](../mastery/10-production.md) and [Deploying](../guides/deploying.md).

## Checklist

- [ ] Every tool the agent has is in the policy, on purpose
- [ ] Model turns are guarded and bounded
- [ ] Irreversible or costly actions need approval, or are capped
- [ ] Costs are computed by code
- [ ] Denials go back to the model; termination stops the loop
- [ ] A monitor redacts secrets and reports harm
- [ ] Backends check approvals for high-stakes actions
- [ ] Tests include a model that gets fooled
- [ ] Checkpoints are published outside the guard host
