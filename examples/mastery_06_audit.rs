//! Lineage Mastery, level 6: proving what happened.
//!
//! Every record is hash-chained to the one before and signed with Ed25519. With the public
//! key alone, anyone can verify a log. With a checkpoint published earlier, they can also
//! prove that nothing was cut off the end. This lesson tampers with a log three ways and
//! shows each one being caught.
//!
//! Run: cargo run --example mastery_06_audit

use std::fs;

use lineage::audit::{self, AuditKey, VerifyOptions};
use lineage::guard::{Guard, Outcome, Policy, ToolRule};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::Path::new("mastery-data/06");
    let _ = fs::remove_dir_all(dir);
    let log = dir.join("agent.jsonl");

    let mut guard = Guard::create(&log, AuditKey::load_or_create(dir.join("audit.key"))?, "payer",
                                  Policy::new(1000).allow("pay", ToolRule::cost(1)))?;
    for amount in [120, 75, 310] {
        let id = guard.request("pay", json!({"to": "ACME", "amount": amount}), Some(amount))?.action_id().to_string();
        guard.report(&id, Outcome::success("sent"))?;
    }
    let public_key = guard.status().public_key;
    let checkpoint = guard.head(); // publish this somewhere the writer can't reach
    drop(guard);
    println!("public key  {public_key}\ncheckpoint  {}:{}\n", checkpoint.seq, checkpoint.hash);

    let trusted = VerifyOptions { public_key: Some(public_key.clone()), checkpoint: Some(checkpoint.clone()) };
    let original = fs::read_to_string(&log)?;
    check("original log", &log, &trusted);

    // 1. Change a payment amount.
    fs::write(&log, original.replacen("\"amount\":310", "\"amount\":31", 1))?;
    check("amount 310 edited to 31", &log, &trusted);

    // 2. Delete a record from the middle.
    let lines: Vec<&str> = original.lines().collect();
    let without: Vec<&str> = lines.iter().enumerate().filter(|(i, _)| *i != 5).map(|(_, l)| *l).collect();
    fs::write(&log, without.join("\n") + "\n")?;
    check("record 5 deleted", &log, &trusted);

    // 3. Cut the last records off. The chain is still internally valid...
    fs::write(&log, lines[..lines.len() - 3].join("\n") + "\n")?;
    check("last 3 records cut (key only)", &log, &VerifyOptions { public_key: Some(public_key), checkpoint: None });
    // ...which is exactly why checkpoints exist.
    check("last 3 records cut (with checkpoint)", &log, &trusted);

    fs::write(&log, original)?;
    println!("\nTry it on the command line:\n  lineage audit verify {} --public-key <key> --checkpoint {}:{}", log.display(), checkpoint.seq, checkpoint.hash);
    Ok(())
}

fn check(label: &str, log: &std::path::Path, options: &VerifyOptions) {
    let report = audit::verify_file(log, options);
    match report.failure {
        None => println!("{label:<38} OK      ({} records)", report.records),
        Some(failure) => println!("{label:<38} FAILED  {failure}"),
    }
}
