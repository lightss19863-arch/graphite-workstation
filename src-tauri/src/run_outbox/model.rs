use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", content = "detail", rename_all = "snake_case")]
pub enum OutboxError {
    Validation(String),
    NotFound,
    RevisionConflict { expected: i64, actual: i64 },
    InvalidTransition { from: String, to: String },
    IdempotencyConflict,
    Persistence(String),
}

impl Display for OutboxError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validation(msg) => write!(formatter, "Validation error: {msg}"),
            Self::NotFound => write!(formatter, "Outbox record not found"),
            Self::RevisionConflict { expected, actual } => {
                write!(
                    formatter,
                    "Revision conflict: expected {expected}, got {actual}"
                )
            }
            Self::InvalidTransition { from, to } => {
                write!(
                    formatter,
                    "Illegal state transition from '{from}' to '{to}'"
                )
            }
            Self::IdempotencyConflict => {
                write!(formatter, "Idempotency key reused with different payload")
            }
            Self::Persistence(msg) => write!(formatter, "Storage persistence error: {msg}"),
        }
    }
}

impl std::error::Error for OutboxError {}

pub type OutboxResult<T> = Result<T, OutboxError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxRecord {
    pub desktop_request_id: String,
    pub matter_id: String,
    pub task_hash: String,
    pub state: String,
    pub revision: i64,
    pub cloud_run_id: Option<String>,
    pub result_reference: Option<String>,
    pub result_hash: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Validates whether a state transition is legal according to the outbox protocol.
///
/// Notice that terminal states are one-way streets. Once a task reaches 'acknowledged',
/// 'failed', 'cancelled', or 'expired', it can never be transitioned again.
/// If you need to re-run, you must create a new desktop_request_id.
pub fn can_transition(from: &str, to: &str) -> bool {
    if matches!(from, "acknowledged" | "failed" | "cancelled" | "expired") {
        return false;
    }
    matches!(
        (from, to),
        ("local_created", "admission_pending")
            | ("admission_pending", "admitted")
            | ("admitted", "staging")
            | ("staging", "submitted")
            | ("submitted", "cloud_running")
            | ("submitted", "awaiting_approval")
            | ("cloud_running", "awaiting_approval")
            | ("awaiting_approval", "submitted")
            | ("awaiting_approval", "cloud_running")
            | ("awaiting_approval", "result_ready")
            | ("cloud_running", "result_ready")
            | ("submitted", "paused")
            | ("cloud_running", "paused")
            | ("paused", "submitted")
            | ("result_ready", "downloaded")
            | ("downloaded", "acknowledged")
            | (
                "reconciliation_required",
                "admission_pending"
                    | "admitted"
                    | "staging"
                    | "submitted"
                    | "cloud_running"
                    | "awaiting_approval"
                    | "result_ready"
                    | "downloaded"
            )
            | (
                _,
                "reconciliation_required" | "failed" | "cancelled" | "expired"
            )
    )
}
