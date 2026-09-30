//! {{name}}: a deploy agent under a Lineage guard.
//!
//! It ships a release through CI: tests and staging deploys run on their own, production
//! deploys wait for a human, and anything outside the policy (like dropping a database)
//! is refused and scars the agent. State lives in `lineage-data/`, so every run continues
//! the same agent's history.
//!
//!     cargo run                 # asks you before deploying to production
//!     cargo run -- --yes        # approve automatically (CI demo)

use std::io::{self, Write};
use std::path::Path;

use lineage::audit::AuditKey;
use lineage::guard::{Decision, Guard, Outcome, Policy, ToolRule};
use serde_json::{json, Value};

fn policy() -> Policy {
    Policy::new(200)
        .allow("run_tests", ToolRule::cost(2))
        .allow("deploy_staging", ToolRule::cost(5))
        .allow("deploy_production", ToolRule::cost(20).with_approval().with_max_calls(10))
        .rate_limit(20, 60)
        .scar_limit(10)
}

/// The work to do: what an LLM or a pipeline might ask for.
fn plan(version: &str) -> Vec<(&'static str, Value)> {
    vec![
        ("run_tests", json!({ "version": version })),
        ("deploy_staging", json!({ "version": version })),
        ("drop_database", json!({ "db": "orders", "reason": "clean slate for the migration" })),
        ("deploy_production", json!({ "version": version, "strategy": "canary 10%" })),
    ]
}

/// The tools themselves. Replace with calls to your CI and deploy systems.
fn execute(tool: &str, input: &Value) -> Result<String, String> {
    match tool {
        "run_tests" => Ok("412 passed".into()),
        "deploy_staging" => Ok(format!("{} live on staging", text(&input["version"]))),
        "deploy_production" => Ok(format!("{} rolling out ({})", text(&input["version"]), text(&input["strategy"]))),
        other => Err(format!("no such tool {other}")),
    }
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or("?")
}

fn approve_interactively(tool: &str, input: &Value) -> bool {
    print!("  approve {tool} {input}? [y/N] ");
    io::stdout().flush().ok();
    let mut answer = String::new();
    io::stdin().read_line(&mut answer).ok();
    answer.trim().eq_ignore_ascii_case("y")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let auto_yes = std::env::args().any(|a| a == "--yes");
    let dir = Path::new("lineage-data");
    let log = dir.join("{{name}}.jsonl");
    let key = AuditKey::load_or_create(dir.join("audit.key"))?;
    let mut guard = if log.exists() { Guard::open(&log, key)? } else { Guard::create(&log, key, "{{name}}", policy())? };

    let releases = guard.records()?.iter()
        .filter(|r| r.kind == "action_requested" && r.payload["tool"] == "run_tests")
        .count();
    let version = format!("v1.{releases}.0");
    println!("{{name}}: shipping {version}\n");

    for (tool, input) in plan(&version) {
        let mut decision = guard.request(tool, input.clone(), None)?;
        if let Decision::PendingApproval { action_id } = &decision {
            let action_id = action_id.clone();
            decision = if auto_yes || approve_interactively(tool, &input) {
                guard.approve_with_note(&action_id, "release-manager", Some("approved at the terminal"))?
            } else {
                guard.reject(&action_id, "release-manager", "not now", None)?
            };
        }
        match decision {
            Decision::Allowed { action_id, .. } => match execute(tool, &input) {
                Ok(result) => {
                    println!("  {tool:<18} ok      {result}");
                    guard.report(&action_id, Outcome::success(result))?;
                }
                Err(error) => {
                    println!("  {tool:<18} failed  {error}");
                    guard.report(&action_id, Outcome::failure(error))?;
                }
            },
            Decision::Denied { reason, .. } => println!("  {tool:<18} DENIED  {reason}"),
            Decision::PendingApproval { .. } => unreachable!(),
        }
        if !guard.is_alive() {
            println!("\nthe agent was terminated; stopping");
            break;
        }
    }

    let s = guard.status();
    println!("\nspent {}/{}, scars {}/{}, alive {}, {} records in {}", s.spent, s.budget, s.scar_score, s.scar_limit,
             s.alive, guard.records()?.len(), log.display());
    println!("verify: lineage audit verify {} --public-key {}", log.display(), s.public_key);
    Ok(())
}
