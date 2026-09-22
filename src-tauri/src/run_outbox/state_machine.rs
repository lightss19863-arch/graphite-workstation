//! State Machine & Event-Linked Outbox Executor
//!
//! Provides transactional state transitions guarded by monotonic CAS revision checks.

use super::model::{can_transition, OutboxError, OutboxRecord, OutboxResult};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone)]
pub struct OutboxEvent {
    pub event_id: String,
    pub prev_event_hash: String,
    pub request_id: String,
    pub from_state: String,
    pub to_state: String,
    pub revision: i64,
    pub timestamp: String,
    pub event_hash: String,
}

pub struct OutboxStateMachine {
    records: RwLock<HashMap<String, OutboxRecord>>,
    events: RwLock<Vec<OutboxEvent>>,
    latest_event_hash: RwLock<String>,
}

impl Default for OutboxStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl OutboxStateMachine {
    pub fn new() -> Self {
        Self {
            records: RwLock::new(HashMap::new()),
            events: RwLock::new(Vec::new()),
            latest_event_hash: RwLock::new(
                "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    .to_string(),
            ),
        }
    }

    /// Enqueue a newly created local task.
    pub fn create_record(
        &self,
        desktop_request_id: &str,
        matter_id: &str,
        task_hash: &str,
    ) -> OutboxResult<OutboxRecord> {
        let mut records = self.records.write().unwrap();
        if records.contains_key(desktop_request_id) {
            return Err(OutboxError::Validation(format!(
                "Record with id '{desktop_request_id}' already exists"
            )));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_string();

        let record = OutboxRecord {
            desktop_request_id: desktop_request_id.to_string(),
            matter_id: matter_id.to_string(),
            task_hash: task_hash.to_string(),
            state: "local_created".to_string(),
            revision: 1,
            cloud_run_id: None,
            result_reference: None,
            result_hash: None,
            created_at: now.clone(),
            updated_at: now,
        };

        records.insert(desktop_request_id.to_string(), record.clone());
        Ok(record)
    }

    /// Transition a record to a target state using atomic CAS revision control.
    ///
    /// The `expected_revision` check prevents concurrency bugs where a delayed background
    /// polling response accidentally overwrites newer user actions.
    pub fn transition(
        &self,
        desktop_request_id: &str,
        expected_revision: i64,
        target_state: &str,
    ) -> OutboxResult<OutboxRecord> {
        let mut records = self.records.write().unwrap();
        let record = records
            .get_mut(desktop_request_id)
            .ok_or(OutboxError::NotFound)?;

        // 1. Strict monotonic CAS check
        // DO NOT remove or bypass this revision check!
        // When the laptop reconnects to Wi-Fi, there is always a race between
        // the background reconciliation loop and the user's immediate UI action.
        // If revisions don't match, one of them is acting on stale data.
        if record.revision != expected_revision {
            return Err(OutboxError::RevisionConflict {
                expected: expected_revision,
                actual: record.revision,
            });
        }

        // 2. Validate legality of state progression
        if !can_transition(&record.state, target_state) {
            return Err(OutboxError::InvalidTransition {
                from: record.state.clone(),
                to: target_state.to_string(),
            });
        }

        let from_state = record.state.clone();
        let next_revision = record.revision + 1;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
            .to_string();

        // 3. Append to cryptographically chained event log
        let mut prev_hash_lock = self.latest_event_hash.write().unwrap();
        let mut hasher = Sha256::new();
        hasher.update(prev_hash_lock.as_bytes());
        hasher.update(desktop_request_id.as_bytes());
        hasher.update(from_state.as_bytes());
        hasher.update(target_state.as_bytes());
        hasher.update(next_revision.to_le_bytes());
        hasher.update(now.as_bytes());
        let event_hash = format!("sha256:{}", hex::encode(hasher.finalize()));

        let event = OutboxEvent {
            event_id: format!("evt_{}", &event_hash[7..23]),
            prev_event_hash: prev_hash_lock.clone(),
            request_id: desktop_request_id.to_string(),
            from_state: from_state.clone(),
            to_state: target_state.to_string(),
            revision: next_revision,
            timestamp: now.clone(),
            event_hash: event_hash.clone(),
        };

        *prev_hash_lock = event_hash;
        self.events.write().unwrap().push(event);

        // 4. Commit updated state and bump revision
        record.state = target_state.to_string();
        record.revision = next_revision;
        record.updated_at = now;

        Ok(record.clone())
    }

    /// Retrieve a record by id.
    pub fn get_record(&self, desktop_request_id: &str) -> OutboxResult<OutboxRecord> {
        let records = self.records.read().unwrap();
        records
            .get(desktop_request_id)
            .cloned()
            .ok_or(OutboxError::NotFound)
    }

    /// Verify the integrity of the audit event chain.
    pub fn verify_event_chain(&self) -> bool {
        let events = self.events.read().unwrap();
        let mut expected_prev =
            "sha256:0000000000000000000000000000000000000000000000000000000000000000";

        for event in events.iter() {
            if event.prev_event_hash != expected_prev {
                return false;
            }
            let mut hasher = Sha256::new();
            hasher.update(event.prev_event_hash.as_bytes());
            hasher.update(event.request_id.as_bytes());
            hasher.update(event.from_state.as_bytes());
            hasher.update(event.to_state.as_bytes());
            hasher.update(event.revision.to_le_bytes());
            hasher.update(event.timestamp.as_bytes());
            let computed = format!("sha256:{}", hex::encode(hasher.finalize()));

            if computed != event.event_hash {
                return false;
            }
            expected_prev = &event.event_hash;
        }
        true
    }
}
