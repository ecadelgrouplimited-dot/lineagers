# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `apps/deepseek-payments-agent`: an accounts-payable agent on DeepSeek V4.x. A `deepseek-flash` clerk and a
  `deepseek-v4-pro` fraud reviewer work under the guard, whose budget is the agent's spending authority. Layered
  approval runs hard rules, then the model, then humans above a limit. The bank enforces guard approvals itself
  (exact input match, single use). Includes offline scenarios and 19 tests.
- Guard actions keep their requested `input` (`GET /v1/agents/:id/actions/:action_id` returns it), so tool
  backends can check that what they execute is exactly what was approved.
- `guard-server` no longer requires `GUARD_ADMIN_TOKEN`: without it, the server uses `keys/admin.token` in its
  data directory, creating it on first start. The Python client finds the token (`find_admin_token`,
  `connect_admin`) and explains setup problems: server not running, token mismatch, or missing key.
- Approvals take an optional `note`, signed into the log (`Guard::approve_with_note`, `{"approver", "note"}`).
- `apps/guarded-agent`: an incident-response agent on Claude Opus 5.5 where every model turn and tool
  call passes through the guard. Includes a prompt-injection scenario, secret redaction, an exfiltration
  monitor that reports harm, human approvals, a scripted model for offline runs, and integration tests.
- Operator console in `guard-server` at `/`: agents, approval queue, audit log, log verification, and
  terminate. It is served with a strict Content Security Policy and renders agent data as text only.

## [0.3.0] - 2026-09-30

### Added
- `audit` module: persistent, SHA-256 hash-chained, Ed25519-signed audit logs (JSON Lines),
  with offline verification, checkpoints to detect truncation, and a single-writer file lock.
- `guard` module: a policy gate for AI agents. Tool allowlists, a finite budget, rate limits,
  per-tool call caps, human approval, scars, and permanent termination. State is rebuilt by
  replaying the verified audit log, so restarts cannot refund budget or revive an agent.
- `lineage audit` CLI: `keygen`, `pubkey`, `append`, `show`, `verify`.
- `apps/guard-server`: HTTP API for the guard with per-agent tokens, an approval queue,
  quarantine of tampered logs, a Docker image, and a dependency-free Python client.
- `guarded_agent` example.

### Changed
- **Breaking:** the finance module is behind the `finance` feature and the `lineage` binary behind
  `cli`. Both are on by default. Use `default-features = false` for the lean core.
- **Breaking:** minimum Rust version is 1.89.
- Dependencies only used by examples (ratatui, crossterm, plotters, hyper, ...) are now
  dev-dependencies; unused `image` and `governor` were removed.
- `reqwest` uses rustls, so OpenSSL is no longer needed to build.
- The `lineage` binary uses the library instead of recompiling the core modules.
- Status reports moved to `docs/archive/`, reference docs to `docs/`, and scripts to `scripts/`.

### Fixed
- Graveyard signing keys were derived from a timestamp and could be guessed; they now come from the OS CSPRNG.
- Unit tests in `finance::data_providers`, `finance::visualization`, and `finance::ml::market_data` did not compile.
- The `colors`, `metrics_server`, and `phase3_training_with_evolution` examples did not compile.

### Security
- `.env` (containing an API key) and `.lineage/keys/tombstone.key` were committed in earlier versions.
  Both are now untracked and ignored. Rotate any key that was in them.

## [0.2.0] - 2026-02-01

### 🚀 Added

#### Lineage Finance Module (NEW!)
A complete evolutionary trading platform extending Lineage core with:

- **FinanceAgent** (`src/finance/agent.rs`): Trading agents with finite capital, trade history, and lifecycle management
- **Irreversible Trade Operations** (`src/finance/trade.rs`): Buy/sell execution with no rollback, P&L calculations, leverage support
- **Financial Scar Mechanics** (`src/finance/scars.rs`): Permanent damage from losses with cost multipliers, leverage restrictions
- **Spawning & Inheritance** (`src/finance/spawning.rs`): Successful agents spawn offspring inheriting optimized traits
- **Cryptographic Trust Scoring** (`src/finance/trust_scoring.rs`): Performance-based trust with tiered grants and permissions
- **Multi-Agent Arena** (`src/finance/arena.rs`): Competition simulations with market state evolution
- **Advanced Features** (`src/finance/advanced.rs`): Blockchain hooks, evolutionary AI framework, real-time adaptation, irreversible governance

#### Examples
- **decentralized_trading_agent** (`examples/decentralized_trading_agent.rs`): Full-featured demo showcasing all finance modules:
  - Agent lifecycle demo (capital depletion, trade recording)
  - Spawning mechanics (inheritance, cost calculation)
  - Trust scoring (performance-based, permission grants)
  - Arena competition (market simulation, agent ranking)
  - Advanced features (blockchain integration, evolutionary strategies, governance)

#### Documentation
- [FINANCE_GETTING_STARTED.md](FINANCE_GETTING_STARTED.md): Quick start guide for finance module
- [FINANCE_IMPLEMENTATION_ROADMAP.md](FINANCE_IMPLEMENTATION_ROADMAP.md): Complete feature tracking, vision, roadmap
- Updated [README.md](README.md) with finance section and quick start

### Key Features

✅ **Irreversible State**: Trades execute once with permanent consequences  
✅ **Finite Resources**: Agents operate under capital constraints  
✅ **Permanent Scars**: Losses permanently increase transaction costs  
✅ **Evolutionary Dynamics**: Successful lineages spawn optimized descendants  
✅ **Trust-Based Access**: Cryptographic trust scores determine resource availability  
✅ **Multi-Agent Competition**: Arena simulations with emergent behavior  
✅ **Auditability**: Sealed graveyard archives for regulatory compliance  
✅ **Extensibility**: Trait-based design for custom strategies

### Architecture

- 8 new modules under `src/finance/`
- Integration with existing Lineage core (Identity, Metabolism, ScarTissue, Trust)
- ~1800 lines of production-ready Rust
- Comprehensive example demonstrating all features
- Zero compiler warnings

### Testing

- All 120 existing Lineage tests pass
- New finance modules compile cleanly
- Example executable runs without errors

### Roadmap (Phase 2)

- Evolutionary AI integration (PyTorch via tch-rs)
- Blockchain deployment (Solana/Ethereum)
- Real-time market adaptation (Chainlink oracles)
- Community governance DAOs
- Permadeath economy mechanics

---

## [0.1.0] - 2026-01-30

### Initial Release

Core Lineage framework with:
- Unique, immutable agent identities
- Append-only tamper-proof history
- Finite energy system
- Permanent scar mechanics
- Trust scoring
- Genealogical spawning
- 12 interactive examples
- 120 comprehensive tests

---

## Release Notes

### v0.2.0 Highlights

**Position**: This release establishes Lineage as a foundational framework for **evolutionary finance**. By combining irreversible state transitions with trading mechanics, we've created a platform where:

1. **AI agents must account for consequences** — No reset buttons forces evolutionary pressure
2. **Trust is cryptographically proven** — Not assumed; earned through verifiable history
3. **Success breeds success** — Spawning mechanisms create lineages of increasingly optimized traders
4. **Failure teaches permanently** — Scars compound, forcing strategic adaptation

**Narrative**: *"We built trading bots that actually die—and their descendants learn from it."* This positions Lineage to disrupt traditional algorithmic trading by introducing Darwinian evolution to DeFi.

---

## Contribution Notes

For contributors interested in Phase 2 (evolutionary AI, blockchain integration):

1. Check [FINANCE_IMPLEMENTATION_ROADMAP.md](FINANCE_IMPLEMENTATION_ROADMAP.md) for feature status
2. Open GitHub issues for feature requests
3. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines
4. Phase 2 features are marked with 🔄 (planned) or 📋 (design phase)

---

*Last Updated: February 1, 2026*
