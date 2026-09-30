# Binding approvals to tool backends

The guard decides, and your code carries out the decision. That leaves one gap: if the process running the agent's tools is compromised or buggy, it could skip the guard, reuse an old approval, or change an approved request before sending it.

Close the gap by making the **tool backend** (the payment rail, deploy system, email relay, or database admin API) check with the guard itself before it acts.

## The check

The agent passes its `action_id` along with the request. The backend then:

1. **Fetches the action** from the guard, using its own credential: `GET /v1/agents/:id/actions/:action_id`.
2. **Requires** `status == "allowed"` and `tool` to be the operation it's about to perform.
3. **Requires the recorded `input` to equal the request it received**, field for field. Also check `cost` if it encodes an amount.
4. **Uses each action once.** It records the action ID and refuses it next time.

```python
class Bank:
    def __init__(self, guard_admin, agent_id):
        self.admin, self.agent_id = guard_admin, agent_id
        self.used = set()

    def transfer(self, action_id, invoice_id, vendor_id, amount, iban, source_email_id):
        request = {"invoice_id": invoice_id, "vendor_id": vendor_id, "amount": amount,
                   "iban": iban, "source_email_id": source_email_id}
        if action_id in self.used:
            raise TransferRefused("approval already used")
        action = self.admin.action(self.agent_id, action_id)
        if action["tool"] != "pay_invoice" or action["status"] != "allowed":
            raise TransferRefused("not an allowed pay_invoice")
        if action["input"] != request or action["cost"] != amount:
            raise TransferRefused("does not match what was approved")
        self.used.add(action_id)
        ...  # move the money
```

This is the bank in the [DeepSeek example](deepseek-payments-agent.md), and its tests try each attack: a made-up action ID, a pending action, an action for a different tool, an edited IBAN, an edited amount, and a replay. All are refused.

## Deployment notes

- **Run the backend as a separate service, with its own credentials.** The whole point is that the agent's process can't reach the rail directly. The example shares a process for simplicity.
- **Give the backend read access to the guard.** Today that's the admin token. Keep it in the backend, never in the agent.
- **Persist the used-action set** in the backend's own database, so a restart doesn't allow replays.
- **Check the status at execution time, not earlier.** An agent can be terminated between approval and execution; an approved action on a terminated agent is still `allowed`, so add `GET /v1/agents/:id` → `alive` to the check if that matters to you.
