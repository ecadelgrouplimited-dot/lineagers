# Security model

Lineage is a control for **what an agent decides to do**. Read this page to know precisely what it protects against and what you still have to protect.

## What Lineage enforces

- **The model cannot exceed its policy.** Tool calls go through the guard, and the policy is fixed at creation, signed, and replayed on every start.
- **Agents cannot vouch for themselves.** On the guard server, agent tokens are scoped to one agent. They can't approve, reject, report harm, add scars, terminate, or read other agents.
- **History is evidence.** Records are hash-chained and signed; logs that fail verification are refused and quarantined.
- **Consequences persist.** Spent budget, scars, and termination survive restarts, because they are replayed from the verified log.

## What you must still do

**Make the guard the only way to act.** The guard decides; your code carries out the decision. If the process running an agent's tools is compromised, it can skip the guard entirely. For actions that matter, make the *tool backend* check the decision itself: before a payment rail, deploy system, or email relay acts, it confirms with the guard that the action was allowed with exactly this input, and that it hasn't already been used. See [Binding approvals to tool backends](../guides/binding-approvals.md). Keep tool credentials in those backends, not in the agent's process.

**Protect the keys.**

| Key | If stolen |
|---|---|
| `audit.key` | The thief can write validly signed records, and rewrite whole logs. Checkpoints published elsewhere still expose rewrites and truncation |
| `admin.token` / `GUARD_ADMIN_TOKEN` | Full operator power: approve, terminate, create agents |
| `token.key` | The thief can mint any agent's token |
| An agent token | Act as that one agent, within its policy |

**Publish checkpoints.** Without them, whoever holds the signing key, or can delete a log's tail, can make history shorter without detection. The guard does defend against the simplest version: if a replayed log shows the scar limit reached or the budget spent, but no `terminated` record, the guard terminates the agent on open, with the reason `termination record missing from the log`. Cutting further back than that is only detectable against a checkpoint.

**Put TLS in front of the guard server.** It speaks plain HTTP, and tokens are bearer credentials. Bind it to localhost and use a reverse proxy; see [Deploying](../guides/deploying.md).

**Redact before you log.** Tool inputs are recorded verbatim and kept forever. Don't send secrets or unnecessary personal data as tool inputs.

## Threats and where they're handled

| Threat | Handled by |
|---|---|
| Prompt injection asks for a tool outside scope | Guard: `tool_not_allowed`, plus a scar |
| Prompt injection asks for an in-scope tool with bad arguments | Your review and approval layer (rules, reviewers, humans); the input is recorded for them |
| Runaway loop | `rate_limit`, `max_calls`, a cost on model turns, budget |
| Agent leaks data through an allowed tool | Monitors that report `harmful`, as in the Claude example's data-loss monitor |
| Agent claims its own harmful action succeeded | Harm is reported by operators, not agents |
| Edited or replayed approval | Tool backend checks the action's recorded input and uses it once |
| Log edited after the fact | Hash chain, signatures, checkpoints |
| Guard server host compromised | Out of scope for the guard. Checkpoints published elsewhere bound the damage to history |

## Reporting a vulnerability

Please report security issues privately through [GitHub security advisories](https://github.com/ecadelgrouplimited-dot/lineagers/security/advisories/new) rather than in a public issue.
