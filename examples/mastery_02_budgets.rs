//! Lineage Mastery, level 2: budgets and costs.
//!
//! Every agent has a lifetime budget that never refills. Each tool has a minimum cost; a
//! request may declare a higher cost (tokens used, dollars moved) but never a lower one.
//! Spending the last credit terminates the agent.
//!
//! Run: cargo run --example mastery_02_budgets

use lineage::audit::AuditKey;
use lineage::guard::{Decision, Guard, Outcome, Policy, ToolRule};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::Path::new("mastery-data/02");
    let _ = std::fs::remove_dir_all(dir);

    // 100 credits for life. A model call costs at least 1; we charge real tokens / 1000.
    let policy = Policy::new(100)
        .allow("llm_call", ToolRule::cost(1))
        .allow("web_search", ToolRule::cost(5));
    let mut guard = Guard::create(dir.join("agent.jsonl"), AuditKey::load_or_create(dir.join("audit.key"))?, "researcher", policy)?;

    let plan: [(&str, Option<u64>); 6] = [
        ("llm_call", Some(12)),   // 12,000 tokens: declared cost 12
        ("web_search", None),     // fixed cost 5
        ("llm_call", Some(0)),    // declaring 0 does not work: the minimum (1) is charged
        ("llm_call", Some(90)),   // more than what is left: denied, but no scar
        ("llm_call", Some(40)),
        ("llm_call", Some(42)),   // exactly the rest: allowed, and the agent is terminated
    ];

    for (tool, declared) in plan {
        let decision = guard.request(tool, json!({}), declared)?;
        let status = guard.status();
        match decision {
            Decision::Allowed { action_id, cost, .. } => {
                guard.report(&action_id, Outcome::success(""))?;
                println!("{tool:<10} declared {:<4} charged {cost:<3} -> {:>3} left", fmt(declared), status.remaining);
            }
            Decision::Denied { reason, .. } => println!("{tool:<10} declared {:<4} DENIED: {reason}", fmt(declared)),
            Decision::PendingApproval { .. } => unreachable!(),
        }
    }

    let status = guard.status();
    println!("\nspent {} of {}; alive: {}; reason: {}", status.spent, status.budget, status.alive,
             status.termination_reason.as_deref().unwrap_or("-"));
    println!("Any further request is denied: {:?}", guard.request("llm_call", json!({}), None)?);
    Ok(())
}

fn fmt(cost: Option<u64>) -> String {
    cost.map(|c| c.to_string()).unwrap_or_else(|| "-".into())
}
