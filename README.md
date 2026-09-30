# Lineage

**Accountability infrastructure for autonomous agents.** Lineage gives every agent a unique identity, a finite budget, permanent consequences, and a signed history that cannot be quietly rewritten.

As AI agents get tools such as shell access, email, payments, and deploys, "what did it do, who allowed it, and can we prove it?" stops being optional. Lineage answers it with two primitives:

- **Guard**: a policy gate an agent must pass before every action. Allowlisted tools, a budget that never refills, rate limits, human approval for risky actions, scars for violations, and permanent termination.
- **Audit log**: every decision is hash-chained (SHA-256) and signed (Ed25519). Anyone with the public key can verify the history offline and detect modified, deleted, reordered, or truncated records.

Guard state is rebuilt by replaying the verified log, so restarting a process cannot refund budget, heal scars, or revive a terminated agent.

## Quick start

```bash
curl -fsSL https://lineagrs.tech/install.sh | sh     # the lineage CLI and guard-server
lineage new my-agent                                 # a runnable guarded agent (Python; --template rust for Rust)
```

**New to Lineage?** [Lineage Mastery](https://docs.lineagrs.tech/mastery/index.html) takes you from a first guarded action to production in ten runnable levels (`cargo run --example mastery_01_first_action`). The whole system is also explained in diagrams in [Lineage in pictures](https://docs.lineagrs.tech/pictures.html), and there's a playground at [lineagrs.tech](https://lineagrs.tech/#play).

### In Rust

```toml
[dependencies]
lineage-rs = { version = "0.3", default-features = false }   # core only: no finance, no CLI
serde_json = "1"
```

```rust
use lineage::audit::AuditKey;
use lineage::guard::{Decision, Guard, Outcome, Policy, ToolRule};
use serde_json::json;

let policy = Policy::new(100)                                  // lifetime budget
    .allow("web_search", ToolRule::cost(1))
    .allow("send_email", ToolRule::cost(5).with_approval())    // a human decides
    .rate_limit(20, 60)
    .scar_limit(10);                                           // then terminated

let key = AuditKey::load_or_create("audit.key")?;
let mut guard = Guard::create("agents/research-bot.jsonl", key, "research-bot", policy)?;

match guard.request("web_search", json!({"q": "outage report"}), None)? {
    Decision::Allowed { action_id, .. } => {
        // run the tool, then:
        guard.report(&action_id, Outcome::success("3 results"))?;
    }
    Decision::PendingApproval { action_id } => { /* guard.approve(&action_id, "alice") */ }
    Decision::Denied { reason, .. } => eprintln!("blocked: {reason}"),
}
```

See the whole lifecycle (allow, approve, deny, harm, termination, verification, restart) in one run:

```bash
cargo run --example guarded_agent
```

### From any language

[`apps/guard-server`](apps/guard-server) wraps the guard in an HTTP API with per-agent tokens, an approval queue, and a Docker image. It includes a dependency-free Python client:

```python
guard = AgentClient("http://127.0.0.1:9200", "support-bot", AGENT_TOKEN)

@guard.tool("send_email")      # asks first, waits for approval, reports the outcome
def send_email(to, body): ...
```

### A real agent

```bash
cd apps/guarded-agent && python3 -m venv .venv && .venv/bin/pip install -r requirements.txt
.venv/bin/python run.py --mock incident      # or without --mock to run Claude Opus 5.5
```

See [`apps/guarded-agent`](apps/guarded-agent) for what it does and how to adapt it.

### Verify a log

```bash
cargo install lineage-rs
lineage audit verify agents/research-bot.jsonl --public-key <hex> --checkpoint <seq>:<hash>
```

```
OK  22 records, log research-bot
    head       21:9f95…e4a8
```

Change one byte and it fails, pointing at the line:

```
FAILED  line 2 (seq 1): hash does not match record content
```

## Guarantees

| | Enforced by |
|---|---|
| Agents only use allowlisted tools | Guard; unlisted tools are denied and scar the agent |
| Budget only decreases | Guard; spent credits are recorded and replayed |
| Risky actions need a human | `requires_approval` rules, re-checked at approval time |
| Scars are permanent; enough of them kill | Scar weights (minor 1, moderate 3, severe 10, fatal) vs `scar_limit` |
| Death is final | `terminated` record; every later request is denied |
| History cannot be silently changed | Hash chain and Ed25519 signatures; checkpoints catch truncation |
| Policy cannot be loosened | Policy is the log's first record; editing it breaks the signature |
| One writer per log | OS file lock |

What it does **not** do: the holder of the signing key can rewrite and re-sign a whole log. Publish checkpoints (`guard.head()`) somewhere the writer cannot change, and keep the key away from the agents.

## Core library

The crate is modular. The core has no network, UI, or async dependencies.

| Module | Purpose |
|---|---|
| `guard` | Policy gate for agents |
| `audit` | Signed, hash-chained, persistent logs; verification |
| `identity`, `memory`, `metabolism`, `scar`, `lineage` | The original model: unique identity, causal memory, finite energy, permanent scars, death |
| `trust`, `agent`, `behavior` | Capability scoring and task agents |
| `graveyard` | Sealed tombstones for dead agents |
| `governance` | Proposals, votes, and a governance ledger |
| `provenance` | Chain-of-custody for assets |
| `finance` | Trading agents, arenas, market data (feature `finance`) |
| `finance::ml` | Learning agents (feature `ml`) |

| Feature | Default | Adds |
|---|---|---|
| `finance` | yes | `finance` module (reqwest, tokio) |
| `cli` | yes | `lineage` binary (`new`, `audit keygen/pubkey/append/show/verify`, `demo`) |
| `ml` | no | `finance::ml` (ndarray) |

## Apps

- [`apps/deepseek-payments-agent`](apps/deepseek-payments-agent): an accounts-payable agent on DeepSeek (`deepseek-flash` clerk, `deepseek-v4-pro` fraud reviewer). The guard budget is its spending authority in dollars. It faces a business-email-compromise attempt, a duplicate invoice, and a prompt injection, and the bank pays only against approved, unmodified guard actions. It runs offline with scripted models.
- [`apps/guarded-agent`](apps/guarded-agent): an on-call incident-response agent on Claude. The guard blocks a prompt-injected shell command, holds restarts for human approval, and terminates the agent when it tries to leak a credential. It runs offline with a scripted model.
- [`apps/guard-server`](apps/guard-server): HTTP guard and audit service for AI agents, with an operator console for approvals and audits.
- [`apps/governance-ops`](apps/governance-ops): governance operations console with graveyard integration.

## Examples

```bash
cargo run --example guarded_agent               # guard + audit, end to end
cargo run --example lifecycle_demo              # identity, energy, scars, death
cargo run --example governance_ws_broadcast     # governance with a web dashboard
cargo run --example provenance_chain_demo       # chain of custody
cargo run --example arena_with_live_market      # finance arena (live prices with COINMARKETCAP_API_KEY)
cargo run --example ml_learning_advanced --features ml
```

The full list is in [`docs/EXAMPLES.md`](docs/EXAMPLES.md). Market-data examples read API keys from `.env` (see [`.env.example`](.env.example)).

## Development

Requires Rust 1.89+. Running the tests also needs the fontconfig headers for a dev-dependency (`sudo apt-get install libfontconfig1-dev pkg-config`).

```bash
cargo test                                   # default features
cargo test --no-default-features             # core only
cargo test --features ml
cargo build --manifest-path apps/guard-server/Cargo.toml
```

## Documentation

- [`docs/DOCTRINE.md`](docs/DOCTRINE.md), [`docs/MANIFESTO.md`](docs/MANIFESTO.md): the principles
- [`docs/CODE_ARCHITECTURE.md`](docs/CODE_ARCHITECTURE.md): how the core fits together
- [`docs/TRUST_SYSTEM.md`](docs/TRUST_SYSTEM.md), [`docs/GRAVEYARD_GUIDE.md`](docs/GRAVEYARD_GUIDE.md), [`docs/RESURRECTION_MECHANICS.md`](docs/RESURRECTION_MECHANICS.md), [`docs/DESCENDANCY_AND_SEAL.md`](docs/DESCENDANCY_AND_SEAL.md)
- [`docs/FINANCE_GETTING_STARTED.md`](docs/FINANCE_GETTING_STARTED.md), [`docs/MARKET_DATA_INTEGRATION.md`](docs/MARKET_DATA_INTEGRATION.md)
- [`docs/archive/`](docs/archive): historical phase reports

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).

## License

MIT. See [`LICENSE`](LICENSE).
