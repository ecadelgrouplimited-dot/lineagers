# Level 4: Humans in the loop

<div class="lx-lesson">
<p><strong>You'll learn</strong> how risky actions wait for a person, and how approvals are recorded and re-checked.</p>
<p><strong>Run</strong> <code>cargo run --example mastery_04_approvals</code></p>
</div>

## The idea

{{#include ../diagrams/life.html}}

A tool marked `with_approval()` puts every call on hold. Nothing is charged while it waits. When a person approves, **every check runs again**, because the agent may have been terminated or run low on budget in the meantime. The approval, the approver, and their note all go into the signed log.

## The code

```rust
{{#include ../../../../examples/mastery_04_approvals.rs}}
```

## Run it

```text
send_refund A-100 $40   -> PendingApproval { action_id: "act-1" }
  spent while waiting: 0
  approver sees: send_refund {"amount":40,"order":"A-100"}
  alice approves         -> Allowed { action_id: "act-1", cost: 10, remaining: 90 }
send_refund A-101 $4000 -> Denied { action_id: "act-2", reason: Rejected { by: "bob", reason: "amount exceeds order value" } }
send_refund A-102 $25   -> approved after termination: false

spent 10 (only the approved refund), scars 3, alive false

The signed record of who decided what:
  action_allowed   {"action_id":"act-1","approved_by":"alice","note":"order shipped late; policy REF-3"}
  action_rejected  {"action_id":"act-2","by":"bob","reason":"amount exceeds order value"}
```

## What happened

- **The $40 refund waited.** Alice saw the exact input, approved it with a note, and only then was it charged.
- **The $4,000 refund was rejected** with a moderate scar, because asking was itself a red flag.
- **The $25 refund was approved** after the agent had been terminated, and that approval turned into a denial.

In real systems, approvals come from the [guard server's console](../getting-started/guard-server.md), a script, or an automated reviewer. See [level 7](07-guard-server.md).

## Try this

Reject the first refund *without* a scar (`None`), and compare the scar score.

[Next: Consequences survive restarts →](05-restarts.md)
