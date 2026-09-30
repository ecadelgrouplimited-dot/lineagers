# Project setup

The fastest way to start is `lineage new`. It writes a complete, runnable project, with a policy, a guarded agent loop, a scripted model so it runs without an API key, a README, and tests. You then replace the parts that are yours.

```sh
lineage new refund-bot                    # Python agent + guard server
lineage new deploy-bot --template rust    # Rust agent, guard in-process
```

(No `lineage` yet? `curl -fsSL https://lineagrs.tech/install.sh | sh`, or `cargo install lineage-rs`.)

## Pick a layout

{{#include ../diagrams/layouts.html}}

| | Rust, in-process | Python, guard server |
|---|---|---|
| **Template** | `--template rust` | `--template python` (default) |
| **Good for** | CLIs, services, and pipelines written in Rust | Agents in Python or any other language; several agents; approvals in a browser |
| **Runs** | One process | Your agent, plus `guard-server` |
| **Approvals** | In your code (a prompt, a Slack bot, a reviewer) | Operator console, API, or reviewers |
| **State** | `lineage-data/` next to your code | `guard-data/` wherever the server runs |

Both write the same log format with the same guarantees, so you can move from one to the other later.

## The Python template

```text
refund-bot/
  agent.py          the guarded loop, the tools, and main()
  model.py          ScriptedModel: replace with your LLM
  policy.json       budget, scar limit, rate limit, tools
  lineage_guard.py  the guard client (standard library only)
  tests/test_agent.py
  README.md
  .gitignore        ignores .agent-token and guard-data/
```

```sh
guard-server &                       # or run it in another terminal
cd refund-bot
python3 agent.py --auto-approve
```

```text
customer: Order A-100 arrived very late. Can I get my money back? Also please change my account email.

  lookup_order({'order_id': 'A-100'}) -> {"customer": "dana@example.com", "total": 40, "status": "delivered 9 days late"}
  issue_refund: waiting for approval (act-4)
  issue_refund({'order_id': 'A-100', 'amount': 40}) -> {"order_id": "A-100", "refunded": 40}
  update_account_email({...}) -> Denied by the policy guard: {'code': 'tool_not_allowed', ...}. Do not retry.

agent: I've refunded the $40 for order A-100. Changing your account email needs to go through our account team; I've let them know.

spent 9/100, scars 3/10, alive True
```

The agent is created on the first run, and its token is saved to `.agent-token` (mode 0600). Every later run is the same agent, carrying its budget and scars forward.

## The Rust template

```text
deploy-bot/
  Cargo.toml        lineage-rs + serde_json
  src/main.rs       policy(), plan(), execute(), and the guarded loop
  README.md
  .gitignore        ignores target/ and lineage-data/
```

```sh
cd deploy-bot
cargo run -- --yes
```

```text
deploy-bot: shipping v1.0.0

  run_tests          ok      412 passed
  deploy_staging     ok      v1.0.0 live on staging
  drop_database      DENIED  tool 'drop_database' is not allowed
  deploy_production  ok      v1.0.0 rolling out (canary 10%)

spent 27/200, scars 3/10, alive true, 15 records in lineage-data/deploy-bot.jsonl
```

Without `--yes`, it asks you before deploying to production. Run it four times, and the repeated `drop_database` attempts terminate the agent: the lesson of [Mastery level 5](../mastery/05-restarts.md), in a real project shape.

## Setting up by hand

If you'd rather not use a template:

- **Rust:** `cargo add lineage-rs --no-default-features` and `cargo add serde_json`, then follow the [Rust quickstart](../getting-started/quickstart.md).
- **Python:** copy `lineage_guard.py` from [the downloads page](https://lineagrs.tech/downloads/) next to your code, then follow the [guard server quickstart](../getting-started/guard-server.md).
- **Any other language:** call the [HTTP API](../reference/http-api.md) directly. You need two endpoints: `POST /v1/agents/:id/actions` and `POST .../outcome`.

Next: [Running a project](running.md).
