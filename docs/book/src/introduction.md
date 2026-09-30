# Lineage

**Lineage makes autonomous agents accountable.** It is a policy guard that an agent must pass before every action, and a signed, tamper-evident audit log of everything the guard decided.

```text
agent ── "may I call send_email?" ──▶ guard ──▶ allowed | denied | waiting for a human
                                        │
                                        └──▶ agents/<id>.jsonl   (hash-chained, Ed25519-signed)
```

With Lineage, an agent:

- **only uses the tools its policy lists.** Anything else is denied, and the attempt scars the agent.
- **spends from a budget that never refills.** Budgets can be credits, API spend, or real money.
- **waits for a human** on risky actions. Approvals are re-checked when given and bound to the exact input.
- **accumulates permanent scars** from violations and bad outcomes, and is **terminated for good** when it has too many.
- **leaves a history nobody can quietly rewrite.** Anyone with the public key can verify the log offline.

Restarting a process changes none of this. The guard rebuilds its state by replaying the verified log, so spent budget stays spent and a terminated agent stays terminated.

## Why

Agents now run shell commands, send email, move money, and deploy code. They can also be steered by text they read (prompt injection), get stuck in loops, and be confidently wrong. Better prompts reduce these failures; they don't control them. Lineage puts the control outside the model, where the model cannot talk its way past it, and keeps evidence that stands up when someone asks what happened.

## Where it came from

Lineage started as a library about software identity with real consequences:

- identity cannot be cloned;
- history is append-only;
- energy is finite;
- damage leaves permanent scars;
- death is final.

Those constraints turn out to be exactly what autonomous agents need, and the guard and audit log are built on them. The original model is still part of the library; see [Identity, memory, energy, scars](library/core-model.md).

## What's in the box

| Component | What it is |
|---|---|
| `lineage-rs` crate | The guard (`lineage::guard`) and audit log (`lineage::audit`), plus the original identity, governance, provenance, and finance modules |
| `lineage` CLI | Create keys, append to logs, and verify them |
| `guard-server` | The guard over HTTP, for agents in any language, with an operator console for approvals and audits |
| Python client | `lineage_guard.py`, dependency-free |
| Example apps | A Claude incident-response agent and a DeepSeek accounts-payable agent, each facing real attacks |

## Where to start

- **Building in Rust?** Start with the [Rust quickstart](getting-started/quickstart.md).
- **Your agent is in Python or another language?** Start with the [guard server quickstart](getting-started/guard-server.md).
- **Want to see it work first?** Run one of the example apps offline, with no API key: [Claude ops agent](guides/claude-ops-agent.md) or [DeepSeek payments agent](guides/deepseek-payments-agent.md).
- **Auditing someone else's agent?** See [Verifying logs](guides/verifying.md).
