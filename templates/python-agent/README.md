# {{name}}

A customer-support agent guarded by [Lineage](https://lineagrs.tech). It looks orders up freely, issues refunds only after a human approves, and can't change account settings, however it's asked.

```text
{{name}}/
  agent.py          the guarded agent loop and its tools
  model.py          the model driving it (scripted; swap in your LLM)
  policy.json       what the agent may do, what it costs, what needs approval
  lineage_guard.py  the Lineage guard client (no dependencies)
  tests/            an end-to-end test
```

## Run

1. Start a guard server, and leave it running:
   ```sh
   guard-server        # or: curl -fsSL https://lineagrs.tech/install.sh | sh
   ```
2. In this directory:
   ```sh
   python3 agent.py --auto-approve     # demo: approvals happen automatically
   python3 agent.py                    # approve refunds yourself in the console
   ```

The console is at http://127.0.0.1:9200/. Sign in with the token in `guard-data/keys/admin.token`, which is in the directory where the server runs. If the server runs somewhere else, set `GUARD_URL` and `GUARD_ADMIN_TOKEN`.

The agent is created on the first run, and its token is saved in `.agent-token`. Later runs are the *same* agent: spent budget and scars carry over. To start over, delete `.agent-token` and set a new `AGENT_ID`.

## Make it yours

- **Tools:** add functions to `TOOLS` in `agent.py`, and add them to `policy.json`. A tool that isn't in the policy is denied, and scars the agent.
- **Model:** replace `ScriptedModel` with your LLM (see `model.py`).
- **Policy:** raise or lower the budget, add `requires_approval` to anything irreversible, and cap one-shot actions with `max_calls`.

## Test

```sh
python3 -m unittest discover -s tests -v
```

Docs: https://docs.lineagrs.tech
