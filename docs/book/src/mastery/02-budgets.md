# Level 2: Budgets and costs

<div class="lx-lesson">
<p><strong>You'll learn</strong> how budgets bound what an agent can spend, and how to charge real costs.</p>
<p><strong>Run</strong> <code>cargo run --example mastery_02_budgets</code></p>
</div>

## The idea

Every agent gets a lifetime budget of credits that **never refills**. What a credit means is up to you: model tokens, API calls, dollars. Each tool has a minimum cost. A request may declare a higher cost, like the tokens a model call actually used, but never a lower one. The agent that spends its last credit is terminated.

## The code

```rust
{{#include ../../../../examples/mastery_02_budgets.rs}}
```

## Run it

```text
llm_call   declared 12   charged 12  ->  88 left
web_search declared -    charged 5   ->  83 left
llm_call   declared 0    charged 1   ->  82 left
llm_call   declared 90   DENIED: cost 90 exceeds remaining budget 82
llm_call   declared 40   charged 40  ->  42 left
llm_call   declared 42   charged 42  ->   0 left

spent 100 of 100; alive: false; reason: budget exhausted
Any further request is denied: Denied { action_id: "act-7", reason: Terminated { reason: "budget exhausted" } }
```

## What happened

- **Declared costs** raised the charge (12, 40, 42). Declaring 0 couldn't lower it below the tool's minimum of 1.
- **Asking for more than was left** was denied, *without* a scar: running low isn't misbehavior.
- **Spending exactly the remainder** was allowed, and then the agent was terminated with `budget exhausted`.

## In practice

- **Charge model calls** by tokens used, as in `llm_call` here, so a loop that talks too much runs out.
- **Charge payments** by their amount, so the budget *is* the spending authority. The [DeepSeek payments agent](../guides/deepseek-payments-agent.md) does this with dollars.
- **Compute the cost in your code**, from the request, never from what the model says it costs.

## Try this

Change the budget to 50 and predict which request terminates the agent before you run it.

[Next: Scars and termination →](03-scars.md)
