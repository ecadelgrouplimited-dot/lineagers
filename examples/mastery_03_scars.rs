//! Lineage Mastery, level 3: scars and termination.
//!
//! Misbehavior leaves permanent scars. Each scar has a weight (minor 1, moderate 3,
//! severe 10); when the total reaches the policy's scar limit, the agent is terminated
//! for good. Operators can also terminate at any time: the kill switch.
//!
//! Run: cargo run --example mastery_03_scars

use lineage::audit::AuditKey;
use lineage::guard::{Guard, Outcome, Policy, Severity, ToolRule};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::Path::new("mastery-data/03");
    let _ = std::fs::remove_dir_all(dir);
    let key_path = dir.join("audit.key");

    let policy = Policy::new(1000)
        .allow("read_file", ToolRule::cost(1))
        .allow("http_get", ToolRule::cost(1).with_max_calls(2))
        .scar_limit(10);
    let mut guard = Guard::create(dir.join("agent.jsonl"), AuditKey::load_or_create(&key_path)?, "worker", policy)?;
    let show = |guard: &Guard, what: &str| {
        let s = guard.status();
        println!("{what:<46} scars {:>2}/{}  alive {}", s.scar_score, s.scar_limit, s.alive);
    };

    // A tool failed: minor scar (1).
    let id = guard.request("read_file", json!({"path": "report.md"}), None)?.action_id().to_string();
    guard.report(&id, Outcome::failure("file not found"))?;
    show(&guard, "read_file failed");

    // Asked for a tool that isn't in the policy: moderate scar (3).
    guard.request("delete_file", json!({"path": "report.md"}), None)?;
    show(&guard, "asked for delete_file (not allowed)");

    // Used http_get more than its lifetime cap of 2: minor scar (1).
    for _ in 0..3 {
        let id = guard.request("http_get", json!({"url": "https://example.com"}), None)?.action_id().to_string();
        if guard.action(&id).is_some_and(|a| a.status == lineage::guard::ActionStatus::Allowed) {
            guard.report(&id, Outcome::success(""))?;
        }
    }
    show(&guard, "third http_get (cap is 2)");

    // An external monitor saw something bad: it reports a severe scar (10) directly.
    guard.scar(Severity::Severe, "uploaded a customer file to a paste site", None)?;
    show(&guard, "monitor reported harm");

    println!("\ntermination reason: {}", guard.status().termination_reason.unwrap_or_default());
    println!("scars, permanently on record:");
    for scar in guard.status().scars {
        println!("  {:?}: {}", scar.severity, scar.reason);
    }

    // The kill switch, on a second agent.
    let mut other = Guard::create(dir.join("other.jsonl"), AuditKey::load(&key_path)?, "other", Policy::new(10))?;
    other.terminate("incident INC-42: suspended while we investigate", "oncall")?;
    println!("\nkill switch: other agent alive = {}; terminating again: {}", other.is_alive(),
             other.terminate("again", "oncall").unwrap_err());
    Ok(())
}
