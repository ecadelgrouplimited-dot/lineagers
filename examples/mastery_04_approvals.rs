//! Lineage Mastery, level 4: humans in the loop.
//!
//! Tools marked `with_approval()` wait for a person. Nothing is charged while an action
//! waits. Approvals re-run every check, carry a signed note, and bind to the exact input
//! that was requested. Rejections can scar the agent when asking was itself a red flag.
//!
//! Run: cargo run --example mastery_04_approvals

use lineage::audit::AuditKey;
use lineage::guard::{Decision, Guard, Outcome, Policy, Severity, ToolRule};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::Path::new("mastery-data/04");
    let _ = std::fs::remove_dir_all(dir);

    let policy = Policy::new(100)
        .allow("draft_reply", ToolRule::cost(1))
        .allow("send_refund", ToolRule::cost(10).with_approval());
    let mut guard = Guard::create(dir.join("agent.jsonl"), AuditKey::load_or_create(dir.join("audit.key"))?, "support", policy)?;

    // 1. The agent asks to refund an order. It has to wait.
    let first = guard.request("send_refund", json!({"order": "A-100", "amount": 40}), None)?;
    println!("send_refund A-100 $40   -> {first:?}");
    println!("  spent while waiting: {}", guard.status().spent);

    // 2. A person looks at the exact input and approves, with a note for the record.
    let action = guard.action(first.action_id()).unwrap();
    println!("  approver sees: {} {}", action.tool, action.input);
    let approved = guard.approve_with_note(first.action_id(), "alice", Some("order shipped late; policy REF-3"))?;
    println!("  alice approves         -> {approved:?}");
    guard.report(first.action_id(), Outcome::success("refund issued"))?;

    // 3. A suspicious request: a refund far above the order value. Rejected, with a scar.
    let second = guard.request("send_refund", json!({"order": "A-101", "amount": 4000}), None)?;
    let rejected = guard.reject(second.action_id(), "bob", "amount exceeds order value", Some(Severity::Moderate))?;
    println!("send_refund A-101 $4000 -> {rejected:?}");

    // 4. Checks run again at approval time: if the agent was terminated meanwhile, the
    //    approval turns into a denial.
    let third = guard.request("send_refund", json!({"order": "A-102", "amount": 25}), None)?;
    guard.terminate("support queue paused", "oncall")?;
    let late = guard.approve(third.action_id(), "alice")?;
    println!("send_refund A-102 $25   -> approved after termination: {}", matches!(late, Decision::Allowed { .. }));

    let s = guard.status();
    println!("\nspent {} (only the approved refund), scars {}, alive {}", s.spent, s.scar_score, s.alive);
    println!("\nThe signed record of who decided what:");
    for r in guard.records()?.iter().filter(|r| r.kind.starts_with("action_allowed") || r.kind == "action_rejected") {
        println!("  {:<16} {}", r.kind, r.payload);
    }
    Ok(())
}
