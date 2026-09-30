# {{name}}

A deploy agent guarded by [Lineage](https://lineagrs.tech), running in-process with no server needed. It runs tests and deploys to staging on its own. It waits for you before production, and refuses anything outside its policy.

```text
{{name}}/
  Cargo.toml
  src/main.rs        the policy, the plan, the tools, and the guarded loop
  lineage-data/      created on first run: the signing key and the agent's signed log
```

## Run

```sh
cargo run              # asks before deploying to production
cargo run -- --yes     # approves automatically
```

Run it several times. Each run is the *same* agent: the budget keeps going down, and the scar from the `drop_database` attempt stays. After enough runs the agent is terminated, and stays terminated. To start over, delete `lineage-data/`.

Check the history:

```sh
lineage audit show lineage-data/{{name}}.jsonl
lineage audit verify lineage-data/{{name}}.jsonl --public-key "$(lineage audit pubkey lineage-data/audit.key)"
```

## Make it yours

- `policy()`: tools, costs, approvals, limits.
- `execute()`: call your real CI and deploy systems.
- `plan()`: where the work comes from. Replace it with an LLM's tool calls, or your pipeline's steps.

Keep `lineage-data/audit.key` secret, and back up `lineage-data/`.

Docs: https://docs.lineagrs.tech
