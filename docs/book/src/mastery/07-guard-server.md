# Level 7: The guard server

<div class="lx-lesson">
<p><strong>You'll learn</strong> how to guard agents written in any language, and how people approve from a browser.</p>
<p><strong>Run</strong> <code>python3 examples/python/lesson07_guard_server.py</code></p>
</div>

## The idea

{{#include ../diagrams/running.html}}

The guard server is the same guard, behind HTTP. Each agent gets its own token, which can only act as that agent. Operators use the admin token to approve, reject, scar and terminate, from the console or from code. The server keeps every agent's keys and signed logs in `guard-data/`.

## Start the server

In one terminal, from the repository root:

```sh
cargo run --release --manifest-path apps/guard-server/Cargo.toml
```

(Or `guard-server`, if you used the [install script](../getting-started/installation.md).)

## The code

```python
{{#include ../../../../examples/python/lesson07_guard_server.py}}
```

## Run it

In a second terminal:

```sh
python3 examples/python/lesson07_guard_server.py
```

When it reaches the refund, open **http://127.0.0.1:9200/**, sign in with the token from `cat guard-data/keys/admin.token`, pick the agent, and approve. Or run it with `--auto-approve` to let a script approve for you:

```text
created support-1790767461 with a budget of 50

1. a free-to-use tool
   {'order_id': 'A-100', 'status': 'delivered late', 'total': 40}

2. a tool the policy doesn't list
   denied: {'code': 'tool_not_allowed', 'tool': 'close_account'}

3. a tool that needs a human
   [approver] issue_refund {'amount': 40, 'order_id': 'A-100'} -> approve
   refunded $40 on A-100

spent 6/50, scars 3, 2 allowed, 1 denied
log verifies: True
```

## What happened

- **`@agent.tool`** turned each function into a guarded tool. It asks first, runs only if allowed, and reports success or failure.
- **The client found the server's admin token by itself**, in `guard-data/keys/admin.token`. It also looks at `GUARD_ADMIN_TOKEN` and `.env`.
- **Everything from levels 1–6** works the same way here: budgets, scars, approvals, replay, and verification.

## Try this

Open the console while the lesson waits, and use **Reject and scar** instead of Approve. What does the script print?

[Next: Guarding an LLM loop →](08-llm-loop.md)
