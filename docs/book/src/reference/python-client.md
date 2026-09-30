# Python client

`lineage_guard.py` is a single file with no dependencies beyond the Python standard library. Get it from [the downloads page](https://lineagrs.tech/downloads/) or `apps/guard-server/clients/python/`.

```python
from lineage_guard import AgentClient, AdminClient, ActionDenied, GuardError, SetupError, connect_admin
```

## Setup helpers

**`connect_admin(url=None, token=None) -> AdminClient`** checks that the server is running and the admin token is accepted. On a problem, it raises `SetupError`, whose message explains the fix. `url` defaults to `$GUARD_URL` or `http://127.0.0.1:9200`.

**`find_admin_token() -> str`** looks for the token in this order:
1. `GUARD_ADMIN_TOKEN` in the environment;
2. `GUARD_ADMIN_TOKEN` in `.env` at the repository root;
3. `keys/admin.token` in `$GUARD_DATA_DIR`, the repository's `guard-data/`, or `./guard-data/`.

**`load_dotenv(path=None)`** loads `KEY=value` lines into `os.environ` without overriding existing variables.

## AgentClient

Used by the agent, with its own token.

```python
agent = AgentClient("http://127.0.0.1:9200", "support-bot", token, timeout=30)
```

| Method | Returns | |
|---|---|---|
| `request(tool, input=None, cost=None)` | decision dict | Ask permission |
| `authorize(tool, input=None, cost=None, wait_for_approval=True, approval_timeout=300)` | action ID | Ask, wait out an approval, and raise `ActionDenied` if refused |
| `report(action_id, status, detail="")` | status dict | `status`: `"success"` or `"failure"` |
| `action(action_id)` | action dict | |
| `wait_for_approval(action_id, timeout=300, poll=2.0)` | final status | Raises `TimeoutError` |
| `status()` | status dict | |
| `tool(name, cost=None, wait_for_approval=True)` | decorator | Guards a function; see below |

```python
@agent.tool("send_email")
def send_email(to, body):
    ...

send_email(to="ops@example.com", body="...")
```

The decorator authorizes with the call's arguments as input (`{"args": [...], "kwargs": {...}}`), runs the function only if allowed, reports `success`, or reports `failure` and re-raises if the function raises. It raises `ActionDenied` if refused.

## AdminClient

Used by operators, dashboards, monitors, and tool backends.

```python
admin = AdminClient("http://127.0.0.1:9200", admin_token)
```

| Method | |
|---|---|
| `url` | The server's base URL |
| `create_agent(agent_id, policy)` | Returns `{"agent": status, "token": ...}` |
| `agents()` | All agents, including quarantined ones |
| `status(agent_id)` | |
| `action(agent_id, action_id)` | Includes the recorded `input`; for [binding approvals](../guides/binding-approvals.md) |
| `pending(agent_id)` | The approval queue |
| `approve(agent_id, action_id, approver, note=None)` | |
| `reject(agent_id, action_id, approver, reason, scar=None)` | |
| `report_harm(agent_id, action_id, detail)` | A `harmful` outcome: a severe scar |
| `scar(agent_id, severity, reason, action_id=None)` | |
| `terminate(agent_id, reason, by)` | Permanent |
| `log(agent_id, after=None, limit=None)` | `{"records": [...], "head": ...}` |
| `verify(agent_id)` | |
| `public_key()` | |

## Exceptions

| Exception | When |
|---|---|
| `ActionDenied(action_id, reason)` | `authorize` or a decorated tool was refused. `reason` is the server's deny reason dict |
| `GuardError(status, code, message)` | The server returned an HTTP error (see [HTTP API](http-api.md#errors)) |
| `SetupError` | Raised by `connect_admin` and `find_admin_token`; the message says how to fix it |
