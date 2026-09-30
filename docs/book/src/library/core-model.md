# Identity, memory, energy, scars

The guard is built on Lineage's original model of software with real consequences. The model is still available directly, for simulations, games, research, and any system that should age rather than reset.

| Principle | Module | Type |
|---|---|---|
| Identity cannot be cloned | `lineage::identity` | `Identity`, deliberately neither `Clone` nor `Copy` |
| History is append-only | `lineage::memory` | `Memory`, `Event` |
| Energy is finite and never recharges | `lineage::metabolism` | `Metabolism` |
| Damage leaves permanent scars | `lineage::scar` | `ScarTissue`, `Scar`, `ScarSeverity` |
| Death is final | `lineage::lineage` | `Lineage`, which ties the four together |

```rust
use lineage::{Lineage, OperationError, OperationResult};
use lineage::scar::ScarSeverity;

let mut lineage = Lineage::create(1000);                 // 1000 energy, a unique identity
match lineage.perform_operation("Initialize".to_string(), 100) {
    OperationResult::Success { energy_consumed } => println!("used {energy_consumed}"),
    other => println!("{other:?}"),                        // InsufficientEnergy, Dead, …
}
let _ = lineage.record_error(OperationError::new(ScarSeverity::Minor, "timeout".to_string()));
println!("{}", lineage.status());
```

```text
=== Lineage Status ===
Identity: 3c11becc8cd2fb99f1c7d25deb518f8aa2e1f0549e89fc61e69a5d13d4518f78
Status: ALIVE
Energy: 900/1000 (10.0% consumed)
Events: 4
Scars: 1 (damage score: 1)
```

Operations cost energy. Errors leave scars that raise future costs. When energy runs out, the lineage dies, and its memory is sealed with a termination event. There's no API to heal, recharge, or revive.

**Task agents** (`lineage::agent`) wrap the model for task execution:

```rust
use lineage::{TaskAgent, Task, TaskOutcome};

let mut agent = TaskAgent::create(500);
let result = agent.execute_task(Task::new("Execute governance vote".to_string(), 20), TaskOutcome::Success);
// Completed { energy_consumed: 20 }
```

## How it relates to the guard

| Original model | Guard |
|---|---|
| `Identity` | An agent ID and its log's genesis record |
| `Memory` (in process) | The signed, persistent audit log |
| `Metabolism` energy | The policy budget |
| `ScarTissue` | Guard scars and `scar_limit` |
| Death | Termination |

The guard adds what the in-process model can't give you: persistence across restarts, cryptographic evidence, and control from outside the agent's process.

Try `lineage demo`, or `cargo run --example lifecycle_demo` in the repository. The design is described in [`docs/DOCTRINE.md`](https://github.com/ecadelgrouplimited-dot/lineagers/blob/main/docs/DOCTRINE.md) and [`docs/MANIFESTO.md`](https://github.com/ecadelgrouplimited-dot/lineagers/blob/main/docs/MANIFESTO.md).
