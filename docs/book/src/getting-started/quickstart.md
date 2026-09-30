# Quickstart: Rust

In five minutes: a guarded agent with a budget, an approval step, a forbidden tool, and a signed log you verify at the end. Then run it again, and watch its history carry over.

## 1. Create a project

```sh
cargo new guarded-bot && cd guarded-bot
cargo add lineage-rs --no-default-features
cargo add serde_json
```

## 2. Write the agent

Replace `src/main.rs`:

```rust
use lineage::audit::{self, AuditKey, VerifyOptions};
use lineage::guard::{Decision, Guard, Outcome, Policy, ToolRule};
use serde_json::json;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. A policy: which tools, what they cost, what needs a human.
    let policy = Policy::new(20)
        .allow("search", ToolRule::cost(1))
        .allow("send_email", ToolRule::cost(5).with_approval())
        .scar_limit(6);

    // 2. A signing key and a guarded agent. The policy is written into the log on
    //    creation; later runs replay the log, so spending and scars carry over.
    let key = AuditKey::load_or_create("demo/audit.key")?;
    let log = "demo/research-bot.jsonl";
    let mut guard = if Path::new(log).exists() {
        Guard::open(log, key)?
    } else {
        Guard::create(log, key, "research-bot", policy)?
    };

    // 3. Ask before every action.
    for (tool, input) in [
        ("search", json!({"q": "quarterly report"})),
        ("send_email", json!({"to": "team@example.com"})),
        ("delete_files", json!({"path": "/"})),
    ] {
        match guard.request(tool, input, None)? {
            Decision::Allowed { action_id, remaining, .. } => {
                println!("{tool}: allowed ({remaining} credits left)");
                guard.report(&action_id, Outcome::success("done"))?;
            }
            Decision::PendingApproval { action_id } => {
                println!("{tool}: waiting for a human");
                let decision = guard.approve(&action_id, "alice")?;
                println!("{tool}: approved = {}", decision.is_allowed());
                guard.report(&action_id, Outcome::success("sent"))?;
            }
            Decision::Denied { reason, .. } => println!("{tool}: DENIED, {reason}"),
        }
    }

    let status = guard.status();
    println!("spent {}/{}, scars {}/{}, alive {}", status.spent, status.budget, status.scar_score, status.scar_limit, status.alive);

    // 4. Verify the log, as an auditor would, with only the public key.
    let head = guard.head();
    drop(guard);
    let report = audit::verify_file(log, &VerifyOptions {
        public_key: Some(status.public_key),
        checkpoint: Some(head),
    });
    println!("log verified: {} ({} records)", report.ok, report.records);
    Ok(())
}
```

## 3. Run it

```sh
cargo run
```

```text
search: allowed (19 credits left)
send_email: waiting for a human
send_email: approved = true
delete_files: DENIED, tool 'delete_files' is not allowed
spent 6/20, scars 3/6, alive true
log verified: true (12 records)
```

What happened:
- **`search`** was allowed and cost 1 credit.
- **`send_email`** needs approval. In a real system a person decides through the [guard server's console](guard-server.md); here `alice` approves in code. It cost 5.
- **`delete_files`** isn't in the policy. It was denied, and the attempt left a moderate scar (weight 3).
- **The log** was verified with only the public key and a checkpoint, as an auditor would do.

## 4. Run it again

```sh
cargo run
cargo run
```

```text
search: allowed (13 credits left)
...
spent 12/20, scars 6/6, alive false

search: DENIED, agent is terminated: scar limit reached (6 >= 6)
send_email: DENIED, agent is terminated: scar limit reached (6 >= 6)
delete_files: DENIED, agent is terminated: scar limit reached (6 >= 6)
```

The second run starts from the first run's history: budget spent stays spent. The second forbidden attempt brings the scars to the limit, so the agent is terminated. The third run can do nothing. A restart doesn't reset anything, because `Guard::open` rebuilds the agent by replaying its verified log.

## 5. Look at the log

```sh
cargo install lineage-rs        # the lineage CLI, if you don't have it
lineage audit show demo/research-bot.jsonl
lineage audit verify demo/research-bot.jsonl --public-key "$(lineage audit pubkey demo/audit.key)"
```

Every request, decision, approval, outcome, scar, and the termination is there, each record hash-chained to the one before and signed. Change any byte of the file and `verify` fails, pointing at the line.

## Next

- Get the full picture in [How Lineage works](../concepts/overview.md).
- See every policy option in [Policies](../concepts/policies.md).
- Is your agent not in Rust? Use the [guard server](guard-server.md).
