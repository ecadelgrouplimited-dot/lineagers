# Policy schema

```json
{
  "budget": 80,
  "scar_limit": 10,
  "rate_limit": { "max_actions": 30, "window_secs": 60 },
  "tools": {
    "<tool name>": { "cost": 1, "requires_approval": false, "max_calls": null }
  },
  "default_rule": null
}
```

| Field | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `budget` | integer ≥ 0 | yes | | Lifetime credits. Never refilled |
| `scar_limit` | integer ≥ 0 | yes | (`Policy::new`: 10) | Scar score at which the agent is terminated |
| `tools` | object | yes | | Tool name → tool rule. The allowlist |
| `default_rule` | tool rule or `null` | no | `null` | Rule for unlisted tools. `null` denies them with a moderate scar |
| `rate_limit` | object or `null` | no | `null` | Sliding-window limit on allowed actions |
| `rate_limit.max_actions` | integer | yes, if set | | Allowed actions per window |
| `rate_limit.window_secs` | integer | yes, if set | | Window length in seconds |

**Tool rule**

| Field | Type | Required | Default | Meaning |
|---|---|---|---|---|
| `cost` | integer ≥ 0 | yes | | Minimum charge per call |
| `requires_approval` | boolean | no | `false` | Hold each call for an operator |
| `max_calls` | integer or `null` | no | `null` | Lifetime cap on allowed calls |

**Scar weights:** `minor` 1, `moderate` 3, `severe` 10, `fatal` terminates immediately.

The policy is stored as the log's `policy` record and can't change. Tool names are case-sensitive strings of 1–128 characters on the guard server.
