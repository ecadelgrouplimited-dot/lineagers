# Examples gallery

Everything here runs offline. Where a real model is involved, a scripted one stands in unless you add an API key. Commands run from the repository root.

## Complete apps

| App | What it shows | Try it |
|---|---|---|
| **[DeepSeek payments agent](../guides/deepseek-payments-agent.md)** | Accounts payable with a $25,000 spending authority. A `deepseek-flash` clerk and a `deepseek-v4-pro` fraud reviewer face bank-detail fraud, a duplicate invoice, and a prompt injection; the bank honors only exact, unused approvals | `cd apps/deepseek-payments-agent && .venv/bin/python run.py --mock fooled --human approve` |
| **[Claude ops agent](../guides/claude-ops-agent.md)** | Incident response on Claude Opus 5.5. A prompt-injected shell command is denied, restarts need approval, and leaking a password terminates the agent | `cd apps/guarded-agent && .venv/bin/python run.py --mock exfil --approvals auto` (export `GUARD_ADMIN_TOKEN` from `guard-data/keys/admin.token` first) |
| **Governance ops console** | Councils voting with finite energy, on a live web console | `cargo run --manifest-path apps/governance-ops/Cargo.toml` |

(Create each app's `.venv` first: `python3 -m venv .venv && .venv/bin/pip install -r requirements.txt`.)

## Project templates

| Template | What it is | Create it |
|---|---|---|
| **Refund support agent** (Python) | Looks up orders, refunds with approval, can't touch accounts | `lineage new refund-bot` |
| **Deploy agent** (Rust) | Tests and staging on its own, production with approval, no `drop_database` | `lineage new deploy-bot --template rust` |

## Lineage Mastery

Ten levels, from a first guarded action to production: [start here](../mastery/index.md).

| | |
|---|---|
| `cargo run --example mastery_01_first_action` | Policies, decisions, the signed log |
| `cargo run --example mastery_02_budgets` | Budgets that never refill |
| `cargo run --example mastery_03_scars` | Scars, limits, the kill switch |
| `cargo run --example mastery_04_approvals` | People in the loop |
| `cargo run --example mastery_05_persistence` | Nothing resets on restart |
| `cargo run --example mastery_06_audit` | Catching tampering |
| `python3 examples/python/lesson07_guard_server.py --auto-approve` | The guard server |
| `python3 examples/python/lesson08_llm_loop.py` | A prompt injection that goes nowhere |
| `python3 examples/python/lesson09_binding.py` | A backend that refuses forged approvals |

## More Rust examples

| | |
|---|---|
| `cargo run --example guarded_agent` | The guard and audit log in one run |
| `cargo run --example lifecycle_demo` | The original model: identity, energy, scars, death |
| `cargo run --example provenance_chain_demo` | Chain of custody |
| `cargo run --example governance_ws_broadcast` | Governance with a web dashboard |
| `cargo run --example arena_with_live_market --release` | Trading agents with finite capital |
