# Level 10: Going to production

<div class="lx-lesson">
<p><strong>You'll learn</strong> what changes between a laptop and a real deployment.</p>
</div>

Everything from levels 1–9 works the same in production. What changes is where things run, who holds which key, and what you watch.

## The checklist

| | On your laptop | In production |
|---|---|---|
| **Guard server** | `cargo run` in a terminal | systemd or Docker, bound to `127.0.0.1`, behind TLS ([Deploying](../guides/deploying.md)) |
| **Admin token** | `guard-data/keys/admin.token` | `GUARD_ADMIN_TOKEN` from a root-only env file or your secret manager |
| **Signing key** | `guard-data/keys/audit.key` | Same file, backed up encrypted, readable only by the service |
| **Approvals** | The console on localhost | The console behind SSO or a VPN; automated reviewers for low-risk cases |
| **Tool backends** | The agent calls tools itself | Backends check approvals themselves ([level 9](09-binding.md)) |
| **Checkpoints** | Printed at the end of a run | Published on a schedule, to a system the guard host can't modify |
| **Monitoring** | Reading the transcript | `/healthz`, quarantined agents, and scar and termination alerts |

## Publish checkpoints

A few lines of cron make truncation and rewrites detectable forever:

```sh
#!/bin/sh
# /etc/cron.hourly/lineage-checkpoints: record every agent's head somewhere else
ADMIN="Authorization: Bearer $GUARD_ADMIN_TOKEN"
for id in $(curl -s -H "$ADMIN" https://guard.example.com/v1/agents | jq -r '.agents[].agent_id'); do
  curl -s -H "$ADMIN" "https://guard.example.com/v1/agents/$id/verify" \
    | jq -c --arg id "$id" '{agent: $id, ok, head, at: now|todate}'
done >> /mnt/audit-archive/checkpoints.jsonl     # a different machine, or append-only storage
```

Alert when `ok` is ever `false`, or when an agent shows up as `quarantined`.

## Watch for

| Signal | Where | Meaning |
|---|---|---|
| Quarantined agent | Startup output, `GET /v1/agents` | Its log failed verification. Treat it as an incident |
| Termination | `terminated` records, agent status | An agent crossed its limits, or someone hit the kill switch |
| Rising scars | `scar_score` in the agent status | An agent that's struggling, or being attacked |
| Pending approvals piling up | `actions_pending` | People are the bottleneck; consider an automated reviewer for low-risk cases |

## You've finished

You can now design a policy, guard any agent, put people in the loop, prove what happened, and run it for real. Next:

- Start a real project: [Project setup](../building/new-project.md).
- See complete apps: [Examples gallery](../building/gallery.md).
- Keep the [Security model](../concepts/security-model.md) close.
