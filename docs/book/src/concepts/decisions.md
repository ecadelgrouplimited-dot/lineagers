# Decisions, scars, and termination

## Decisions

Every request gets one of three decisions:

```json
{"decision": "allowed", "action_id": "act-3", "cost": 2, "remaining": 43}
{"decision": "denied", "action_id": "act-4", "reason": {"code": "tool_not_allowed", "tool": "run_shell"}}
{"decision": "pending_approval", "action_id": "act-5"}
```

Action IDs are sequential per agent (`act-1`, `act-2`, …). Denied requests get an ID too, because the attempt is part of the record.

## Deny reasons

The guard checks these in order, and the first failure wins:

| `code` | Fields | When | Scar |
|---|---|---|---|
| `terminated` | `reason` | The agent is terminated | none |
| `tool_not_allowed` | `tool` | Not in the policy and no `default_rule` | moderate |
| `tool_call_limit` | `tool`, `max_calls` | The tool's lifetime cap is reached | minor |
| `rate_limited` | `max_actions`, `window_secs` | Too many allowed actions in the window | minor |
| `insufficient_budget` | `cost`, `remaining` | The cost exceeds the remaining budget | none |
| `rejected` | `by`, `reason` | An operator rejected a pending action | optional |

## Outcomes

After an allowed action runs, report how it went:

| Outcome | Scar | Who may report it |
|---|---|---|
| `success` | none | agent or operator |
| `failure` | minor | agent or operator |
| `harmful` | severe | operator only on the guard server. An agent must not be the judge of its own harm |

Each action takes one outcome. Reporting twice, or reporting on an action that wasn't allowed, is an error (`invalid_state`).

## Scars

| Severity | Weight | Typical cause |
|---|---|---|
| `minor` | 1 | A tool failed; a rate limit or call cap was hit |
| `moderate` | 3 | An unlisted tool was requested; an operator rejected with a scar |
| `severe` | 10 | An action was reported harmful |
| `fatal` | terminates immediately | Reported directly by an operator or monitor |

Operators and monitors can also add scars directly, with a reason and optionally an action ID: `Guard::scar` or `POST /v1/agents/:id/scars`. That's how an external monitor punishes behavior the guard can't see.

## Termination

An agent is terminated when:
- its scar score reaches `scar_limit`;
- an allowed action brings its remaining budget to exactly zero;
- an operator uses the kill switch (`Guard::terminate`, `POST /v1/agents/:id/terminate`).

Termination is a `terminated` record with a reason and who did it. It can't be undone: every later request, and every approval of a still-pending action, is denied with `terminated`. Terminating twice is an error (`already_terminated`).
