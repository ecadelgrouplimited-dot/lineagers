//! Lineage Mastery, level 1: your first guarded action.
//!
//! An agent may only do what its policy lists. Ask the guard before acting; the guard
//! answers allowed or denied, and writes both the question and the answer to a signed log.
//!
//! Run: cargo run --example mastery_01_first_action

use lineage::audit::AuditKey;
use lineage::guard::{Decision, Guard, Outcome, Policy, ToolRule};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::Path::new("mastery-data/01");
    let _ = std::fs::remove_dir_all(dir); // every run of this lesson starts fresh

    // The policy: one tool, "search", costing 1 credit. Nothing else is allowed.
    let policy = Policy::new(10).allow("search", ToolRule::cost(1));

    // A signing key, and a new agent whose log is agent.jsonl.
    let key = AuditKey::load_or_create(dir.join("audit.key"))?;
    let mut guard = Guard::create(dir.join("agent.jsonl"), key, "helper", policy)?;

    // Ask before acting.
    for tool in ["search", "send_email"] {
        match guard.request(tool, json!({ "query": "status page" }), None)? {
            Decision::Allowed { action_id, remaining, .. } => {
                println!("{tool:<11} allowed   ({action_id}, {remaining} credits left)");
                // ... run the tool here, then say how it went:
                guard.report(&action_id, Outcome::success("found 3 results"))?;
            }
            Decision::Denied { action_id, reason } => println!("{tool:<11} DENIED    ({action_id}: {reason})"),
            Decision::PendingApproval { .. } => unreachable!("nothing in this policy needs approval"),
        }
    }

    println!("\nThe log now holds {} signed records:", guard.records()?.len());
    for record in guard.records()? {
        println!("  {:>2}  {:<17} {}", record.seq, record.kind, record.payload);
    }
    println!("\nOpen {} to see them yourself.", dir.join("agent.jsonl").display());
    Ok(())
}
