//! # Lineage CLI
//!
//! - `lineage demo` walks through the core principles: unique identity, append-only
//!   causal memory, finite metabolic budget, permanent scars, irreversible death.
//! - `lineage audit ...` creates keys for, inspects, and verifies tamper-evident audit logs.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use lineage::audit::{self, AuditKey, AuditLog, Checkpoint, VerifyOptions};
use lineage::scar::ScarSeverity;
use lineage::{Lineage, OperationError, OperationResult, PulseBehavior};

#[derive(Parser)]
#[command(name = "lineage", version, about = "Accountability for autonomous agents: tamper-evident audit logs and the Lineage demo")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Walk through the core principles (default)
    Demo,
    /// Tamper-evident audit logs
    #[command(subcommand)]
    Audit(AuditCommand),
}

#[derive(Subcommand)]
enum AuditCommand {
    /// Generate a new signing key file (hex secret, mode 0600)
    Keygen {
        /// Where to write the key
        path: PathBuf,
    },
    /// Print the public key for a signing key file
    Pubkey {
        path: PathBuf,
    },
    /// Verify a log's hash chain and signatures
    Verify {
        log: PathBuf,
        /// Trusted hex public key. Without it the log is only self-attested.
        #[arg(long)]
        public_key: Option<String>,
        /// Published checkpoint as SEQ:HASH; detects truncation and rewrites
        #[arg(long)]
        checkpoint: Option<String>,
        /// Print the report as JSON
        #[arg(long)]
        json: bool,
    },
    /// Append a record to a log, creating the log if needed. Prints the new head.
    Append {
        log: PathBuf,
        /// Signing key file
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        actor: String,
        #[arg(long)]
        kind: String,
        /// JSON payload
        #[arg(long, default_value = "{}")]
        payload: String,
    },
    /// Print a log's records
    Show {
        log: PathBuf,
        /// Print raw JSON Lines instead of a table
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Demo) {
        Command::Demo => {
            run_demo();
            ExitCode::SUCCESS
        }
        Command::Audit(command) => match run_audit(command) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("error: {}", e);
                ExitCode::from(2)
            }
        },
    }
}

fn run_audit(command: AuditCommand) -> Result<ExitCode, Box<dyn std::error::Error>> {
    match command {
        AuditCommand::Keygen { path } => {
            let key = AuditKey::generate();
            key.save(&path)?;
            println!("secret key written to {}", path.display());
            println!("public key: {}", key.public_key_hex());
        }
        AuditCommand::Pubkey { path } => {
            println!("{}", AuditKey::load(&path)?.public_key_hex());
        }
        AuditCommand::Verify { log, public_key, checkpoint, json } => {
            let checkpoint = checkpoint.map(|c| parse_checkpoint(&c)).transpose()?;
            let report = audit::verify_file(&log, &VerifyOptions { public_key, checkpoint });
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else if report.ok {
                let head = report.head.as_ref().expect("ok report has a head");
                println!("OK  {} records, log {}", report.records, report.log_id.as_deref().unwrap_or("?"));
                println!("    head       {}:{}", head.seq, head.hash);
                println!("    public key {}", report.public_key.as_deref().unwrap_or("?"));
                if !report.key_trusted {
                    println!("    WARNING: no --public-key given; the signature only proves the log is");
                    println!("    self-consistent, not who wrote it.");
                }
            } else {
                println!("FAILED  {}", report.failure.as_ref().map(|f| f.to_string()).unwrap_or_default());
            }
            if !report.ok {
                return Ok(ExitCode::FAILURE);
            }
        }
        AuditCommand::Append { log, key, actor, kind, payload } => {
            let payload: serde_json::Value = serde_json::from_str(&payload)?;
            let log_id = log.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let mut audit_log = AuditLog::open_or_create(&log, AuditKey::load(&key)?, &log_id)?;
            let record = audit_log.append(&actor, &kind, payload)?;
            println!("{}:{}", record.seq, record.hash);
        }
        AuditCommand::Show { log, json } => {
            let records = audit::read_records(&log).map_err(|f| f.to_string())?;
            for record in records {
                if json {
                    println!("{}", serde_json::to_string(&record)?);
                } else {
                    println!("{:>6}  {}  {:<16} {:<20} {}", record.seq, record.timestamp, record.actor, record.kind, record.payload);
                }
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn parse_checkpoint(value: &str) -> Result<Checkpoint, String> {
    let (seq, hash) = value.split_once(':').ok_or("checkpoint must be SEQ:HASH")?;
    Ok(Checkpoint {
        seq: seq.parse().map_err(|_| "checkpoint SEQ must be a number")?,
        hash: hash.to_string(),
    })
}

fn run_demo() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║        LINEAGE - Ontological Software Demonstration       ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // === DEMONSTRATION 1: Birth ===
    println!("┌─ DEMONSTRATION 1: Birth ─────────────────────────────────┐");
    let mut lineage = Lineage::create(1000);
    println!("Lineage created with identity: {}", lineage.identity().id());
    println!("Birth time: {} nanoseconds since epoch", lineage.identity().birth_time());
    println!("Initial energy: {} units", lineage.metabolism().initial_energy());
    println!("└──────────────────────────────────────────────────────────┘\n");

    // === DEMONSTRATION 2: Behavior Loop (Pulse System) ===
    println!("┌─ DEMONSTRATION 2: Consequential Behavior Loop ──────────┐");
    println!("Demonstrating: One behavior, one contract, one injury path\n");
    
    let pulse_behavior = PulseBehavior::new();
    println!("Pulse behavior initialized:");
    println!("  - Base cost: 10 energy");
    println!("  - Threshold: {} energy (minimum for healthy pulse)", pulse_behavior.threshold());
    println!("  - Contract: Pulse below threshold causes strain");
    println!("  - Consequence: Strain scars increase future cost by 5\n");

    // Execute healthy pulses
    println!("Executing healthy pulses...");
    for i in 1..=3 {
        let output = pulse_behavior.execute_pulse(&mut lineage);
        println!("  Pulse #{}: energy={}, strong={}, cost={}", 
            i, 
            output.energy_at_pulse,
            output.is_strong,
            pulse_behavior.current_pulse_cost(&lineage)
        );
    }
    
    println!("\nCurrent state:");
    println!("  Energy: {}", lineage.metabolism().energy());
    println!("  Scars: {}", lineage.scars().scar_count());
    println!("  Pulse cost: {}\n", pulse_behavior.current_pulse_cost(&lineage));

    // Deplete to danger zone
    println!("Depleting energy to danger zone...");
    let current = lineage.metabolism().energy();
    lineage.perform_operation("Heavy work".to_string(), current - 25);
    println!("  Energy now: {} (below threshold!)\n", lineage.metabolism().energy());

    // Execute weak pulse - CONTRACT VIOLATION
    println!("Executing weak pulse (contract violation)...");
    let output = pulse_behavior.execute_pulse(&mut lineage);
    println!("  ⚠️  STRAIN DETECTED!");
    println!("  - Energy at pulse: {}", output.energy_at_pulse);
    println!("  - Below threshold: {}", output.energy_at_pulse < pulse_behavior.threshold());
    println!("  - Strain inflicted: {}", output.strain_occurred);
    println!("  - Permanent scar recorded: YES");
    println!("  - New pulse cost: {} (+5 penalty)\n", pulse_behavior.current_pulse_cost(&lineage));

    // Show permanent consequence
    println!("Consequence is permanent:");
    println!("  Scars before: 0 → Scars now: {}", lineage.scars().scar_count());
    println!("  Cost before: 10 → Cost now: {}", pulse_behavior.current_pulse_cost(&lineage));
    
    // Demonstrate death spiral
    println!("\nDemonstrating death spiral from accumulated strain...");
    let mut pulse_count = 0;
    while lineage.is_alive() && pulse_count < 10 {
        let energy = lineage.metabolism().energy();
        if (15..35).contains(&energy) {
            let output = pulse_behavior.execute_pulse(&mut lineage);
            if output.strain_occurred {
                println!("  Pulse #{}: STRAIN (energy={}, cost={})", 
                    pulse_count + 1,
                    output.energy_at_pulse,
                    pulse_behavior.current_pulse_cost(&lineage)
                );
            }
            pulse_count += 1;
        } else {
            break;
        }
    }
    
    println!("\nFinal state:");
    println!("  Energy: {}", lineage.metabolism().energy());
    println!("  Strain scars: {}", lineage.scars().scar_count());
    println!("  Pulse cost: {} (increased by {})", 
        pulse_behavior.current_pulse_cost(&lineage),
        (pulse_behavior.current_pulse_cost(&lineage) - 10)
    );
    println!("  Alive: {}", lineage.is_alive());
    println!("└──────────────────────────────────────────────────────────┘\n");

    // === DEMONSTRATION 3: Normal Operations ===
    println!("┌─ DEMONSTRATION 3: Other Operations ─────────────────────┐");
    
    // Reset lineage for cleaner demonstration
    let mut lineage = Lineage::create(1000);
    
    match lineage.perform_operation("Initialize subsystems".to_string(), 100) {
        OperationResult::Success { energy_consumed } => {
            println!("✓ Operation succeeded (consumed {} energy)", energy_consumed);
            println!("  Remaining energy: {}", lineage.metabolism().energy());
        }
        _ => println!("✗ Operation failed"),
    }

    match lineage.perform_operation("Process data batch".to_string(), 150) {
        OperationResult::Success { energy_consumed } => {
            println!("✓ Operation succeeded (consumed {} energy)", energy_consumed);
            println!("  Remaining energy: {}", lineage.metabolism().energy());
        }
        _ => println!("✗ Operation failed"),
    }

    match lineage.perform_operation("Execute computation".to_string(), 200) {
        OperationResult::Success { energy_consumed } => {
            println!("✓ Operation succeeded (consumed {} energy)", energy_consumed);
            println!("  Remaining energy: {}", lineage.metabolism().energy());
        }
        _ => println!("✗ Operation failed"),
    }
    
    println!("└──────────────────────────────────────────────────────────┘\n");

    // === DEMONSTRATION 4: Encountering Errors (Scars) ===
    println!("┌─ DEMONSTRATION 4: Scars ─────────────────────────────────┐");
    
    let minor_error = OperationError::new(
        ScarSeverity::Minor,
        "Network timeout during sync".to_string(),
    ).with_context("Attempted connection to peer node failed after 3 retries".to_string());
    
    lineage.record_error(minor_error);
    println!("✗ Minor scar inflicted: Network timeout");
    println!("  Scar count: {}", lineage.scars().scar_count());
    println!("  Status: {}", if lineage.is_alive() { "ALIVE" } else { "DEAD" });

    let moderate_error = OperationError::new(
        ScarSeverity::Moderate,
        "Data corruption in cache layer".to_string(),
    );
    
    lineage.record_error(moderate_error);
    println!("✗ Moderate scar inflicted: Cache corruption");
    println!("  Scar count: {}", lineage.scars().scar_count());
    println!("  Damage score: {}", lineage.scars().damage_score());
    
    println!("└──────────────────────────────────────────────────────────┘\n");

    // === DEMONSTRATION 5: Insufficient Energy ===
    println!("┌─ DEMONSTRATION 5: Energy Exhaustion ────────────────────┐");
    
    match lineage.perform_operation("Expensive AI inference".to_string(), 800) {
        OperationResult::Success { .. } => {
            println!("✗ This should not have succeeded!");
        }
        OperationResult::InsufficientEnergy { required, available } => {
            println!("✗ Operation rejected: Insufficient energy");
            println!("  Required: {} units", required);
            println!("  Available: {} units", available);
            println!("  Status: Still alive, but constrained");
        }
        _ => {}
    }
    
    println!("└──────────────────────────────────────────────────────────┘\n");

    // === DEMONSTRATION 6: Gradual Depletion ===
    println!("┌─ DEMONSTRATION 6: Path to Death ────────────────────────┐");
    
    let remaining_energy = lineage.metabolism().energy();
    println!("Current energy: {} units", remaining_energy);
    println!("Performing operations until death...\n");

    let mut operation_count = 0;
    while lineage.is_alive() {
        operation_count += 1;
        let result = lineage.perform_operation(
            format!("Final operations #{}", operation_count),
            100,
        );
        
        match result {
            OperationResult::Success { energy_consumed } => {
                let remaining = lineage.metabolism().energy();
                println!("  Op #{}: consumed {}, remaining {}", 
                    operation_count, energy_consumed, remaining);
                
                if !lineage.is_alive() {
                    println!("\n💀 DEATH: Energy depleted");
                    break;
                }
            }
            OperationResult::InsufficientEnergy { .. } => {
                println!("  Op #{}: Insufficient energy, skipping", operation_count);
                break;
            }
            OperationResult::Dead => {
                println!("\n💀 DEATH: Already dead");
                break;
            }
            _ => {}
        }
    }
    
    println!("└──────────────────────────────────────────────────────────┘\n");

    // === DEMONSTRATION 6: Operations After Death ===
    println!("┌─ DEMONSTRATION 6: Operations After Death ───────────────┐");
    
    match lineage.perform_operation("Attempted resurrection".to_string(), 10) {
        OperationResult::Dead => {
            println!("✗ Operation rejected: Lineage is dead");
            println!("  Death is irreversible.");
            println!("  No resurrection. No rollback. No second chances.");
        }
        _ => println!("✗ This should not be possible!"),
    }
    
    println!("└──────────────────────────────────────────────────────────┘\n");

    // === FINAL STATUS ===
    println!("┌─ FINAL STATUS ───────────────────────────────────────────┐");
    let status = lineage.status();
    println!("{}", status);
    
    println!("Memory History (last 5 events):");
    let history = lineage.memory().history();
    let start = if history.len() > 5 { history.len() - 5 } else { 0 };
    for event in &history[start..] {
        println!("  [{}] {}", event.sequence(), event.description());
    }
    
    println!("\nScar Tissue:");
    for scar in lineage.scars().all_scars() {
        println!("  [{:?}] {}", scar.severity(), scar.description());
        if let Some(ctx) = scar.context() {
            println!("    Context: {}", ctx);
        }
    }
    
    println!("└──────────────────────────────────────────────────────────┘\n");

    // === PHILOSOPHICAL CONCLUSION ===
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                    ONTOLOGICAL TRUTHS                      ║");
    println!("╠════════════════════════════════════════════════════════════╣");
    println!("║  ✓ Identity was unique and non-copyable                   ║");
    println!("║  ✓ History was append-only and immutable                  ║");
    println!("║  ✓ Energy could only decrease, never increase             ║");
    println!("║  ✓ Scars were permanent and accumulated                   ║");
    println!("║  ✓ Death was final and irreversible                       ║");
    println!("║                                                            ║");
    println!("║  This is not a simulation.                                ║");
    println!("║  This is not a convenience feature.                       ║");
    println!("║  This is ontology.                                        ║");
    println!("╚════════════════════════════════════════════════════════════╝");
}

// Additional demonstration: Cannot clone lineage
// 
// If you uncomment this function, it will fail to compile:
//
// fn attempt_clone(lineage: &Lineage) {
//     let cloned = lineage.clone(); // Compilation error
// }
//
// This is intentional. Identity cannot be duplicated.
