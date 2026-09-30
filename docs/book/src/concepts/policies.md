# Policies

A policy says what an agent may do, what it costs, and how much damage it can take. It's written into the agent's log as the second record, right after genesis, and can never be changed. To change a policy, create a new agent.

```json
{
  "budget": 25000,
  "scar_limit": 10,
  "rate_limit": { "max_actions": 60, "window_secs": 60 },
  "tools": {
    "llm_turn":      { "cost": 0, "max_calls": 60 },
    "read_invoice":  { "cost": 0 },
    "pay_invoice":   { "cost": 1, "requires_approval": true, "max_calls": 10 }
  },
  "default_rule": null
}
```

The same policy in Rust:

```rust
use lineage::guard::{Policy, ToolRule};

let policy = Policy::new(25_000)
    .allow("llm_turn", ToolRule::cost(0).with_max_calls(60))
    .allow("read_invoice", ToolRule::cost(0))
    .allow("pay_invoice", ToolRule::cost(1).with_approval().with_max_calls(10))
    .rate_limit(60, 60)
    .scar_limit(10);
```

## Fields

**`budget`** is the total number of credits the agent may ever spend. When an allowed action brings the remainder to exactly zero, the agent is terminated with the reason `budget exhausted`. A request costing more than what remains is denied with `insufficient_budget`, but doesn't scar: running low isn't misbehavior.

**`tools`** is the allowlist. Each entry is a tool rule:

| Field | Default | Meaning |
|---|---|---|
| `cost` | required | Minimum credits per call. A request can declare more, never less |
| `requires_approval` | `false` | Hold every call until an operator approves or rejects it |
| `max_calls` | none | Lifetime cap on allowed calls of this tool |

**`default_rule`** is the rule for tools not listed. `null` (the default) denies them, with a moderate scar. Setting a default rule turns the allowlist into a price list; use it only when you have another control on which tools exist.

**`rate_limit`** means at most `max_actions` allowed actions in any sliding window of `window_secs` seconds, across all tools. A request over the limit is denied with a minor scar. This is what stops runaway loops.

**`scar_limit`** is the scar score at which the agent is terminated. `Policy::new` defaults to 10.

## Designing a policy

- **List only what the agent needs.** Every tool you leave out is one it can't be tricked into using. It's also a tripwire: an agent asking for `run_shell` or `update_vendor_bank_details` has been confused or compromised, and the scar records that.
- **Gate the model too.** Guard each model call as a tool, as the examples do with `llm_turn`, with `max_calls` or a cost. Then a looping agent runs into a limit even when it never calls a real tool.
- **Price by consequence.** Reads can be free. Actions with side effects should cost something, and irreversible ones should cost what they're worth.
- **Approve the irreversible.** Payments, customer-facing messages, deletions, and deploys should have `requires_approval`. Cap one-shot actions with `max_calls`.
- **Pick a scar limit you'd accept.** With the default weights, 10 means one severe incident, or about three unlisted-tool attempts, before termination.

Full field reference: [Policy schema](../reference/policy.md).
