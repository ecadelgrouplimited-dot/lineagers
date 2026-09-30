# Quickstart: guard server and Python

The guard server puts the guard behind an HTTP API, so agents in any language can use it. It comes with an operator console for approvals and audits, and a Python client that has no dependencies.

## 1. Start the server

```sh
guard-server
```

(Or from a source checkout: `cargo run --release --manifest-path apps/guard-server/Cargo.toml`.)

```text
guard-server listening on http://127.0.0.1:9200
  console     http://127.0.0.1:9200/
  data dir    guard-data
  public key  13f5fbff904c6f5306fbdeb2fbfc97f5d98df2e2716852bdf5d771715deacaef
  admin token from guard-data/keys/admin.token
  agents      0 loaded, 0 quarantined
```

On first start, the server creates `guard-data/` in the current directory:

| File | What it is |
|---|---|
| `keys/audit.key` | Ed25519 key that signs every log |
| `keys/token.key` | Secret used to derive per-agent tokens |
| `keys/admin.token` | Operator token, unless you set `GUARD_ADMIN_TOKEN` |
| `agents/<id>.jsonl` | One signed log per agent |

All three key files are created with mode 0600. Back up `guard-data/`: without `audit.key`, existing logs can still be verified but never appended to.

## 2. Write an agent

Put [`lineage_guard.py`](https://lineagrs.tech/downloads/) next to this script, `support_bot.py`, and run it from the directory where the server is running:

```python
from lineage_guard import ActionDenied, AgentClient, connect_admin

admin = connect_admin()   # finds guard-data/keys/admin.token
policy = {
    "budget": 100,
    "scar_limit": 10,
    "tools": {
        "search_kb": {"cost": 1},
        "issue_refund": {"cost": 10, "requires_approval": True},
    },
}
created = admin.create_agent("support-bot", policy)
agent = AgentClient(admin.url, "support-bot", created["token"])

@agent.tool("search_kb")
def search_kb(query):
    return [f"article about {query}"]

@agent.tool("issue_refund")
def issue_refund(order_id, amount):
    return f"refunded {amount} on {order_id}"

print(search_kb(query="late delivery"))

try:
    agent.authorize("delete_account", {"user_id": 42})
except ActionDenied as e:
    print("blocked:", e.reason)

print("waiting for approval at http://127.0.0.1:9200/ ...")
print(issue_refund(order_id="A-100", amount=40))
print(agent.status()["spent"], "credits spent")
```

```sh
python3 support_bot.py
```

The `@agent.tool` decorator asks the guard before the function runs. If a human needs to approve, it waits for them. Afterwards it reports success or failure back to the guard. A denied call raises `ActionDenied`, with the reason in `e.reason`.

> **Token discovery.** `connect_admin()` reads `GUARD_ADMIN_TOKEN` from the environment or a `.env` file. Failing that, it reads `keys/admin.token` in `$GUARD_DATA_DIR` or `./guard-data`. If the server runs elsewhere, set `GUARD_URL` and `GUARD_ADMIN_TOKEN`. If anything is missing, it raises `SetupError`, with a message that says how to fix it.

## 3. Approve the refund

The script stops at `issue_refund`. Open **http://127.0.0.1:9200/** and sign in with the admin token (`cat guard-data/keys/admin.token`). Pick `support-bot`, and under **Waiting for approval** you'll see the exact input, `{"order_id": "A-100", "amount": 40}`. Approve it, and the script finishes:

```text
['article about late delivery']
blocked: {'code': 'tool_not_allowed', 'tool': 'delete_account'}
waiting for approval at http://127.0.0.1:9200/ ...
refunded 40 on A-100
11 credits spent
```

The console also shows the budget, the scar from the `delete_account` attempt, and the full audit log. It has a **Verify log** button, and **Terminate**, which is permanent.

## 4. From any other language

The same flow in `curl`. Agents use their own token, which is returned when the agent is created:

```sh
ADMIN=$(cat guard-data/keys/admin.token)

# create an agent (operator)
curl -s localhost:9200/v1/agents -H "Authorization: Bearer $ADMIN" -H 'content-type: application/json' \
  -d '{"agent_id": "bot-1", "policy": {"budget": 10, "scar_limit": 10, "tools": {"search": {"cost": 1}}}}'
# -> {"agent": {...}, "token": "agt_bot-1.4f0c..."}

# ask before acting (agent)
curl -s localhost:9200/v1/agents/bot-1/actions -H "Authorization: Bearer $AGENT_TOKEN" \
  -H 'content-type: application/json' -d '{"tool": "search", "input": {"q": "status"}}'
# -> {"decision": "allowed", "action_id": "act-1", "cost": 1, "remaining": 9}

# report how it went (agent)
curl -s localhost:9200/v1/agents/bot-1/actions/act-1/outcome -H "Authorization: Bearer $AGENT_TOKEN" \
  -H 'content-type: application/json' -d '{"status": "success"}'
```

Every endpoint is listed in the [HTTP API reference](../reference/http-api.md).

## Next

- [Guarding an LLM agent](../guides/llm-agent.md): the pattern for Claude, DeepSeek, or any tool-calling model.
- [Deploying the guard server](../guides/deploying.md): TLS, systemd, Docker, and backups.
