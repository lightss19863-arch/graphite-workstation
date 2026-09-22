//! Graphite Workstation — Sanitized Core Subsystems
//!
//! This crate contains the core state machine, hardware attestation, and anti-tamper
//! primitives extracted from the Graphite desktop workstation.
//!
//! Note: Proprietary IPC handlers, SQLCipher database encryption keys, and internal
//! cloud endpoints have been intentionally excluded from this public distribution.
//! What is preserved here is the actual verification logic, outbox state machine,
//! and cryptographic attestation primitives.

pub mod device_seal;
pub mod run_outbox;
pub mod tamper_guard;

pub use device_seal::{sign_request, DeviceSealHeaders};
pub use run_outbox::{can_transition, OutboxError, OutboxRecord, OutboxResult, OutboxStateMachine};
pub use tamper_guard::verify_runtime_integrity;
