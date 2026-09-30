# HTTP API

The guard server speaks JSON over HTTP. Base URL: `http://127.0.0.1:9200` by default.

## Authentication

Every `/v1` endpoint except `/v1/public-key` needs `Authorization: Bearer <token>`.

| Token | Where it comes from | Can |
|---|---|---|
| **Admin** | `GUARD_ADMIN_TOKEN`, or `keys/admin.token` in the data directory | Everything |
| **Agent** | Returned when the agent is created, or re-issued by `GET /v1/agents/:id/token`. Format `agt_<agent_id>.<hmac>` | Its own agent only: status, request, poll, and report success or failure |

Agent tokens are derived from `keys/token.key`, so the server stores no token list; rotating that key invalidates every agent token.

## Errors

Errors return a status code and `{"error": "<message>", "code": "<code>"}`.

| Status | `code` | |
|---|---|---|
| 400 | `bad_request` | Invalid input (agent ID format, tool name length, …) |
| 401 | `unauthorized` | Missing or invalid token |
| 403 | `forbidden` | Valid token, not allowed here (for example, an agent token on another agent, or reporting harm) |
| 404 | `not_found`, `unknown_action` | No such agent or action |
| 409 | `agent_exists`, `invalid_state`, `already_terminated` | Conflicts with the current state |
| 503 | `quarantined` | The agent's log failed verification at startup |
| 500 | `internal` | Server-side failure |

A denied *action* is not an error: `POST .../actions` returns 200 with `"decision": "denied"`.

## Objects

**Decision**
```json
{"decision": "allowed", "action_id": "act-3", "cost": 2, "remaining": 43}
{"decision": "denied", "action_id": "act-4", "reason": {"code": "tool_not_allowed", "tool": "run_shell"}}
{"decision": "pending_approval", "action_id": "act-5"}
```

**Action**
```json
{"action_id": "act-5", "tool": "send_email", "input": {"to": "ops@example.com"}, "cost": 5,
 "status": "pending_approval", "outcome": null}
```
`status` is one of `requested`, `pending_approval`, `allowed`, `denied`, `completed`. `outcome` is `null`, `success`, `failure`, or `harmful`.

**Agent status**
```json
{
  "agent_id": "support-bot", "alive": true, "termination_reason": null,
  "budget": 100, "spent": 11, "remaining": 89,
  "scar_score": 3, "scar_limit": 10,
  "scars": [{"seq": 9, "severity": "moderate", "reason": "tool 'delete_account' is not allowed", "action_id": "act-2"}],
  "actions_allowed": 2, "actions_denied": 1, "actions_pending": 0,
  "head": {"seq": 16, "hash": "9e0d…"},
  "public_key": "13f5fb…"
}
```

## Endpoints

### Server

| | |
|---|---|
| `GET /healthz` | `{"ok": true}`. No authentication |
| `GET /v1/public-key` | `{"public_key": "<hex>", "algorithm": "ed25519"}`. No authentication |
| `GET /` | The operator console (static page; it calls the API with the admin token you enter) |

### Agents (admin)

**`POST /v1/agents`**: create an agent.
```json
{"agent_id": "support-bot", "policy": { ...see Policy schema... }}
```
`agent_id` must be 1–64 characters of `[A-Za-z0-9_-]`. Returns `{"agent": <status>, "token": "agt_…"}`. Returns 409 `agent_exists` if the ID is taken.

**`GET /v1/agents`**: all agents.
```json
{"agents": [
  {"agent_id": "support-bot", "state": "active", "status": { ... }},
  {"agent_id": "old-bot", "state": "quarantined", "error": "log is corrupt: line 2 (seq 1): hash does not match record content"}
]}
```

**`GET /v1/agents/:id/token`**: re-issue an agent's token. `{"token": "agt_…"}`

### Agent status (agent or admin)

**`GET /v1/agents/:id`**: agent status.

### Actions

**`POST /v1/agents/:id/actions`** (agent or admin): request permission.
```json
{"tool": "send_email", "input": {"to": "ops@example.com"}, "cost": 5}
```
`input` is optional (defaults to `null`) and is recorded verbatim. `cost` is optional; the charge is the larger of `cost` and the tool's minimum. `tool` must be 1–128 characters. Returns a decision.

**`GET /v1/agents/:id/actions/:action_id`** (agent or admin): one action. Poll this while an action is `pending_approval`.

**`POST /v1/agents/:id/actions/:action_id/outcome`** (agent or admin): report how an allowed action went.
```json
{"status": "success", "detail": "3 results"}
```
`status` is `success`, `failure`, or `harmful`. Only the admin token may report `harmful` (403 otherwise). The action must be `allowed` (409 `invalid_state` otherwise). Returns the agent status.

**`GET /v1/agents/:id/actions/pending`** (admin): the approval queue, with each request's input.
```json
{"pending": [{"requested_at": "2026-09-30T08:41:12.508772Z", "seq": 14,
              "action": {"action_id": "act-5", "tool": "send_email", "input": {...}, "cost": 5}}]}
```

**`POST /v1/agents/:id/actions/:action_id/approve`** (admin)
```json
{"approver": "alice", "note": "matches ticket OPS-1182"}
```
`note` is optional and signed into the log. The guard's checks run again. Returns a decision: `allowed`, or `denied` if a check now fails.

**`POST /v1/agents/:id/actions/:action_id/reject`** (admin)
```json
{"approver": "bob", "reason": "wrong account", "scar": "moderate"}
```
`scar` is optional: `minor`, `moderate`, `severe`, or `fatal`. Returns a `denied` decision with reason `rejected`.

### Scars and termination (admin)

**`POST /v1/agents/:id/scars`**
```json
{"severity": "severe", "reason": "leaked a customer's address", "action_id": "act-9"}
```
`action_id` is optional; if given, it must exist. Returns the agent status. The agent is terminated if its scar score reaches the limit.

**`POST /v1/agents/:id/terminate`**
```json
{"reason": "incident INC-221", "by": "oncall"}
```
Permanent. Returns the agent status; 409 `already_terminated` if it already was.

### Audit (admin)

**`GET /v1/agents/:id/log?after=<seq>&limit=<n>`**: signed records with `seq > after`. `limit` defaults to 500, maximum 5000.
```json
{"records": [ {"seq": 0, "kind": "genesis", ...}, ... ], "head": {"seq": 69, "hash": "986e…"}}
```

**`GET /v1/agents/:id/verify`**: re-verify the chain and signatures against the server's public key.
```json
{"ok": true, "records": 70, "public_key": "13f5fb…", "head": {"seq": 69, "hash": "986e…"}}
{"ok": false, "records": 70, "failure": {"line": 12, "seq": 11, "reason": "hash does not match record content"}}
```
