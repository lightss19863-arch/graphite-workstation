//! Graphite Workstation CLI / Smoke Entrypoint
//!
//! A minimal smoke entrypoint to verify integrity and state machine initialization.

use graphite_workstation_core::{can_transition, verify_runtime_integrity};

fn main() {
    println!("Graphite Workstation Core Subsystem");
    println!("===================================");

    // Run-time integrity check
    if let Err(e) = verify_runtime_integrity() {
        eprintln!("Integrity verification failed: {}", e);
        std::process::exit(1);
    }

    println!("Runtime integrity verified: anti-tamper guard active.");
    println!("Outbox transition sample check:");
    println!(
        "  local_created -> admission_pending: {}",
        can_transition("local_created", "admission_pending")
    );
    println!(
        "  acknowledged -> submitted (invalid): {}",
        can_transition("acknowledged", "submitted")
    );
    println!("System ready.");
}
