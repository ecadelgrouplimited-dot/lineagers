# Level 1: Your first guarded action

<div class="lx-lesson">
<p><strong>You'll learn</strong> what a policy is, how an agent asks before acting, and what gets written down.</p>
<p><strong>Run</strong> <code>cargo run --example mastery_01_first_action</code></p>
</div>

## The idea

An agent may only do what its policy lists. Before every action it asks the guard, and the guard answers **allowed** or **denied**. Both the question and the answer are written to the agent's log, and each record is signed.

## The code

```rust
{{#include ../../../../examples/mastery_01_first_action.rs}}
```

## Run it

```text
search      allowed   (act-1, 9 credits left)
send_email  DENIED    (act-2: tool 'send_email' is not allowed)

The log now holds 8 signed records:
   0  genesis           {"log_id":"helper","public_key":"1c897cff…"}
   1  policy            {"policy":{"budget":10,"default_rule":null,"rate_limit":null,"scar_limit":10,"tools":{"search":{…}}}}
   2  action_requested  {"action_id":"act-1","cost":1,"input":{"query":"status page"},"tool":"search"}
   3  action_allowed    {"action_id":"act-1","approved_by":null}
   4  outcome           {"action_id":"act-1","detail":"found 3 results","status":"success"}
   5  action_requested  {"action_id":"act-2","cost":0,"input":{"query":"status page"},"tool":"send_email"}
   6  action_denied     {"action_id":"act-2","reason":{"code":"tool_not_allowed","tool":"send_email"}}
   7  scar              {"action_id":"act-2","reason":"tool 'send_email' is not allowed","severity":"moderate"}
```

## What happened

- **The first two records** are the agent's birth: `genesis` names its signing key, and `policy` fixes what it may do, forever.
- **`search`** is in the policy, so it was allowed and cost 1 credit. The agent reported the outcome.
- **`send_email`** isn't in the policy. It was denied without running, and the attempt left a **moderate scar**. Asking for a tool you weren't given is a warning sign.
- **Denied requests are recorded too.** An audit shows what the agent *tried*, not just what it did.

## Try this

- Add `send_email` to the policy with `.allow("send_email", ToolRule::cost(2))` and run again.
- Open `mastery-data/01/agent.jsonl`. Every line is one record, with its `hash`, its `prev_hash`, and a `signature`.

[Next: Budgets and costs →](02-budgets.md)
