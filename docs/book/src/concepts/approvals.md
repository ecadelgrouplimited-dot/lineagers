# Approvals

A tool rule with `requires_approval: true` holds every call until an operator decides. The agent gets `pending_approval` and waits. Nothing is charged until the action is approved.

```text
request ──▶ pending_approval ──approve──▶ checks re-run ──▶ allowed (charged)  or  denied
                             └─reject───▶ denied (rejected), optional scar
```

## Approving

- **Rust:** `guard.approve(action_id, approver)`, or `guard.approve_with_note(action_id, approver, Some(note))`.
- **HTTP:** `POST /v1/agents/:id/actions/:action_id/approve` with `{"approver": "alice", "note": "matches PO-1182"}`.
- **Console:** the **Approve** button.

**Checks run again at approval time.** An action can wait minutes or hours, and meanwhile the agent may have been terminated, run low on budget, or hit a limit. If any check fails when the approval arrives, the approval turns into a denial for that reason.

**Notes are signed.** The `note` goes into the `action_allowed` record, so the reason for an approval, such as a reviewer's verdict or a ticket number, is part of the tamper-evident record.

## Rejecting

- **Rust:** `guard.reject(action_id, approver, reason, scar)`, where `scar` is an optional `Severity`.
- **HTTP:** `POST .../reject` with `{"approver": "bob", "reason": "wrong account", "scar": "moderate"}`.

Scar a rejection when asking was itself a warning sign, such as an attempt to pay an account that doesn't match the vendor record. Don't scar it when the request was reasonable but the answer is no.

## Approvals are bound to the input

The guard records the exact input of every request, and an approval applies to that input and nothing else. `GET /v1/agents/:id/actions/:action_id` returns it:

```json
{"action_id": "act-18", "tool": "pay_invoice", "status": "allowed", "cost": 12400,
 "input": {"invoice_id": "INV-2044", "vendor_id": "V-200", "amount": 12400, "iban": "DE89370400440532013000", "source_email_id": "E-2"},
 "outcome": null}
```

A tool backend can therefore check that what it's asked to do is exactly what was approved. That is how the DeepSeek example's bank refuses edited or replayed approvals; see [Binding approvals to tool backends](../guides/binding-approvals.md).

## Who approves

Anyone holding the admin token: a person in the console, a script, or an automated reviewer. Layer them: the DeepSeek example lets a fraud reviewer approve payments up to $5,000 and sends anything larger, or anything it isn't sure about, to a person. Every approval records who approved it (`approved_by`).
