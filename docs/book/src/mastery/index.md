# Lineage Mastery

Ten levels, from your first guarded action to an agent running in production. Every level is a program you run, with the output you should see and an exercise to try. Each one builds on the one before.

{{#include ../diagrams/path.html}}

| Level | You'll learn | Runs with |
|---|---|---|
| [1. Your first guarded action](01-first-action.md) | Policies, requests, decisions, and the signed log | Rust |
| [2. Budgets and costs](02-budgets.md) | Finite budgets, declared costs, exhaustion | Rust |
| [3. Scars and termination](03-scars.md) | Scar weights, limits, monitors, the kill switch | Rust |
| [4. Humans in the loop](04-approvals.md) | Approvals, notes, rejections, re-checks | Rust |
| [5. Consequences survive restarts](05-restarts.md) | Replay: why nothing resets | Rust |
| [6. Proving what happened](06-audit.md) | Verification, tampering, checkpoints | Rust |
| [7. The guard server](07-guard-server.md) | Guarding agents in any language over HTTP | Python |
| [8. Guarding an LLM loop](08-llm-loop.md) | The loop every tool-calling agent needs | Python |
| [9. Binding approvals to backends](09-binding.md) | Making decisions enforceable | Python |
| [10. Going to production](10-production.md) | Deploying, monitoring, keys, checkpoints | Ops |

## Setup

Clone the repository once. Every level runs from its root:

```sh
git clone https://github.com/ecadelgrouplimited-dot/lineagers
cd lineagers
cargo run --example mastery_01_first_action
```

You need Rust 1.89+ for levels 1–6, and Python 3.8+ plus a running guard server for levels 7–9. The Rust levels write their files to `mastery-data/<level>/`, so you can open and inspect every log.

On Linux, the first build compiles the examples' dependencies, which needs the fontconfig headers: `sudo apt-get install libfontconfig1-dev pkg-config`.
