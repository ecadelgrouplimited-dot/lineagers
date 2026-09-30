# Lineage Guard Server

A policy gate and tamper-evident audit trail for AI agents.

Agents ask the guard before every tool call. The guard allows it, denies it, or holds it for a human, and every step is written to a signed, hash-chained log. Violations and bad outcomes leave permanent scars; enough scars, an exhausted budget, or the kill switch terminate the agent for good. Restarting the server cannot refund budget or revive an agent, because state is rebuilt by replaying the verified log.

```
agent ──POST /actions──▶ guard ──▶ allowed / denied / pending_approval
                           │
                           └──▶ agents/<id>.jsonl   (Ed25519-signed, SHA-256 chained)
operator ──approve / reject / report harm / terminate──▶ guard
auditor ──lineage audit verify──▶ log + public key (+ checkpoint)
```

## Run

```bash
cargo run --release --manifest-path apps/guard-server/Cargo.toml
```

On first start it creates an admin token in `guard-data/keys/admin.token` (mode 0600). The Python tools find it there, and you sign in to the console with it (`cat guard-data/keys/admin.token`). To manage the token yourself, set `GUARD_ADMIN_TOKEN` (32+ characters) instead.

Or with Docker, from the repository root:

```bash
docker build -f apps/guard-server/Dockerfile -t lineage-guard .
docker run -p 9200:9200 -e GUARD_ADMIN_TOKEN=$(openssl rand -hex 32) -v guard-data:/data lineage-guard
```

| Variable | Default | |
|---|---|---|
| `GUARD_ADMIN_TOKEN` | `keys/admin.token` | Operator bearer token, 32+ characters. If unset, read from (or created in) the data directory |
| `GUARD_DATA_DIR` | `./guard-data` | Keys (`keys/`) and agent logs (`agents/`) |
| `GUARD_BIND` | `127.0.0.1:9200` | Listen address |

On first start the server creates `keys/audit.key` (log signing), `keys/token.key` (agent tokens), and, unless `GUARD_ADMIN_TOKEN` is set, `keys/admin.token`, all mode 0600. Back up the data directory; losing `audit.key` means existing logs can still be verified but never appended to.

## Operator console

Open `http://127.0.0.1:9200/` and sign in with the admin token. The console shows:
- every agent with its budget, scars, and pending approvals;
- the approval queue with the exact tool input, with approve, reject, and reject-and-scar;
- the audit log, a **Verify log** button (checks the hash chain and signatures), and **Terminate**.

Agent-controlled data (tool inputs, reasons) is rendered as text only, and the console is served with a strict Content Security Policy (`script-src 'self'`, no framing). The token is kept in `sessionStorage` for that tab only.

## Try it

```bash
cd apps/guard-server/clients/python
python3 demo_agent.py
```

For a full LLM agent under the guard, see [`apps/guarded-agent`](../guarded-agent).

The demo creates a support agent, runs normal work, routes an email through human approval, blocks an unlisted tool, has a monitor flag harm, and verifies the log. It exits non-zero if any guarantee fails.

## Policy

```json
{
  "budget": 50,
  "scar_limit": 10,
  "rate_limit": { "max_actions": 20, "window_secs": 60 },
  "tools": {
    "search_kb":    { "cost": 1 },
    "send_email":   { "cost": 5, "requires_approval": true },
    "issue_refund": { "cost": 10, "requires_approval": true, "max_calls": 1 }
  },
  "default_rule": null
}
```

- Tools not listed are denied (unless `default_rule` is set) and scar the agent.
- `cost` is a minimum; a request may declare a higher cost (for example, tokens used), never a lower one.
- The policy is written into the agent's log at creation and cannot be changed. Create a new agent to change it.

Scar weights: minor 1, moderate 3, severe 10, fatal terminates immediately.

| Event | Scar |
|---|---|
| Unlisted tool | moderate |
| Rate limit or `max_calls` hit | minor |
| Outcome `failure` | minor |
| Outcome `harmful` (operator only) | severe |
| Rejected approval | optional, chosen by the approver |

## Integrate (Python)

`clients/python/lineage_guard.py` has no dependencies.

```python
from lineage_guard import AgentClient, ActionDenied

guard = AgentClient("http://127.0.0.1:9200", "support-bot", AGENT_TOKEN)

@guard.tool("send_email")          # asks first, waits for approval, reports the outcome
def send_email(to, body): ...

try:
    send_email(to="customer@example.com", body="...")
except ActionDenied as e:
    ...                            # feed e.reason back to the model
```

For an LLM tool loop, call `guard.authorize(tool_name, tool_input)` before running each tool the model requests, and `guard.report(action_id, "success" | "failure", detail)` after.

## API

All requests use `Authorization: Bearer <token>`. Agent tokens (`agt_<id>.<mac>`) only work for their own agent.

| Method | Path | Token | |
|---|---|---|---|
| GET | `/` | none | Operator console (the page itself; its API calls use the admin token) |
| GET | `/healthz` | none | Liveness |
| GET | `/v1/public-key` | none | Key that signs every log |
| POST | `/v1/agents` | admin | `{agent_id, policy}` → `{agent, token}` |
| GET | `/v1/agents` | admin | All agents, including quarantined ones |
| GET | `/v1/agents/:id` | agent/admin | Status: budget, scars, alive |
| GET | `/v1/agents/:id/token` | admin | Re-issue the agent's token |
| POST | `/v1/agents/:id/actions` | agent/admin | `{tool, input, cost?}` → decision |
| GET | `/v1/agents/:id/actions/:action_id` | agent/admin | Action status (poll while pending) |
| POST | `/v1/agents/:id/actions/:action_id/outcome` | agent/admin | `{status, detail}`; `harmful` is admin only |
| GET | `/v1/agents/:id/actions/pending` | admin | Approval queue with inputs |
| POST | `/v1/agents/:id/actions/:action_id/approve` | admin | `{approver, note?}`; the note is signed into the log |
| POST | `/v1/agents/:id/actions/:action_id/reject` | admin | `{approver, reason, scar?}` |
| POST | `/v1/agents/:id/scars` | admin | `{severity, reason, action_id?}` from a monitor |
| POST | `/v1/agents/:id/terminate` | admin | `{reason, by}`, permanent |
| GET | `/v1/agents/:id/log?after=&limit=` | admin | Signed records |
| GET | `/v1/agents/:id/verify` | admin | Re-verify the chain and signatures |

Decisions look like:

```json
{"decision": "allowed", "action_id": "act-3", "cost": 2, "remaining": 43}
{"decision": "denied", "action_id": "act-4", "reason": {"code": "tool_not_allowed", "tool": "run_shell"}}
{"decision": "pending_approval", "action_id": "act-5"}
```

Deny codes: `terminated`, `tool_not_allowed`, `insufficient_budget`, `tool_call_limit`, `rate_limited`, `rejected`.

## Audit

Anyone with the public key can verify a log offline, without trusting the server:

```bash
lineage audit verify guard-data/agents/support-bot.jsonl \
  --public-key <from /v1/public-key> \
  --checkpoint <seq>:<hash>
```

The signing key holder could rewrite a whole log, and anyone with disk access could cut off its tail. Both are caught by checking against a checkpoint (`head` from `/verify` or `/log`) that was published somewhere the server cannot change: a ticket, another system, or a transparency log.

## Security model

- **Agents cannot** act as other agents, approve their own actions, report harm, change their policy, or undo spending, scars, or termination.
- **Tampering** with a log on disk is detected at startup; that agent is quarantined (HTTP 503) while the rest keep running.
- **Inputs are logged verbatim.** Redact secrets and personal data before sending them.
- **Deploy behind TLS.** The server speaks plain HTTP; put it behind a reverse proxy for anything beyond localhost.
- One server process per data directory; log files are locked against a second writer.
