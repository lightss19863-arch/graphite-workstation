//! Run Outbox & State Machine Subsystem
//!
//! Handles crash-safe state progression for asynchronous legal tasks dispatched to the cloud.
//! Enforces monotonic CAS revisioning and cryptographically hashed event chains.

pub mod model;
pub mod state_machine;

#[cfg(test)]
mod tests;

pub use model::{can_transition, OutboxError, OutboxRecord, OutboxResult};
pub use state_machine::OutboxStateMachine;
