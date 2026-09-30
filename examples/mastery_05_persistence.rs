//! Lineage Mastery, level 5: consequences survive restarts.
//!
//! The guard keeps no state of its own. `Guard::open` verifies the agent's log and
//! replays it, so spent budget, scars, pending approvals, and termination all carry over.
//! Run this example several times in a row and watch the agent age.
//!
//! Run:   cargo run --example mastery_05_persistence          (again and again)
//! Reset: cargo run --example mastery_05_persistence -- --reset

use std::path::Path;

use lineage::audit::AuditKey;
use lineage::guard::{Decision, Guard, Outcome, Policy, ToolRule};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = Path::new("mastery-data/05");
    if std::env::args().any(|a| a == "--reset") {
        let _ = std::fs::remove_dir_all(dir);
        println!("reset: the agent is gone. Its history is not 'undone': it is simply a new agent next time.");
        return Ok(());
    }
    let log = dir.join("agent.jsonl");
    let key = AuditKey::load_or_create(dir.join("audit.key"))?;

    // Open the agent if it exists (replaying its history), otherwise create it.
    let mut guard = if log.exists() {
        Guard::open(&log, key)?
    } else {
        println!("(first run: creating the agent)");
        Guard::create(&log, key, "night-shift", Policy::new(20).allow("work", ToolRule::cost(3)).scar_limit(6))?
    };

    let before = guard.status();
    println!("start of run: spent {}/{}, scars {}/{}, alive {}", before.spent, before.budget,
             before.scar_score, before.scar_limit, before.alive);

    // Each run does some work and makes one mistake.
    for (tool, input) in [("work", json!({"task": "reconcile"})), ("wipe_disk", json!({"disk": "/dev/sda"}))] {
        match guard.request(tool, input, None)? {
            Decision::Allowed { action_id, .. } => {
                guard.report(&action_id, Outcome::success(""))?;
                println!("  {tool:<9} allowed");
            }
            Decision::Denied { reason, .. } => println!("  {tool:<9} denied: {reason}"),
            Decision::PendingApproval { .. } => unreachable!(),
        }
    }

    let after = guard.status();
    println!("end of run:   spent {}/{}, scars {}/{}, alive {}  ({} records in the log)", after.spent, after.budget,
             after.scar_score, after.scar_limit, after.alive, guard.records()?.len());
    if !after.alive {
        println!("\nThe agent is terminated: {}. Restarting won't change that.", after.termination_reason.unwrap_or_default());
        println!("Start over with --reset (which creates a new agent, not a revived one).");
    }
    Ok(())
}
