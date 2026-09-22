use super::*;

#[test]
fn test_successful_state_progression_and_chain_integrity() {
    let sm = OutboxStateMachine::new();
    let id = "req_test_001";
    let matter = "matter_abc";
    let task_hash = "sha256:1111111111111111111111111111111111111111111111111111111111111111";

    let initial = sm.create_record(id, matter, task_hash).unwrap();
    assert_eq!(initial.state, "local_created");
    assert_eq!(initial.revision, 1);

    let step1 = sm.transition(id, 1, "admission_pending").unwrap();
    assert_eq!(step1.state, "admission_pending");
    assert_eq!(step1.revision, 2);

    let step2 = sm.transition(id, 2, "admitted").unwrap();
    assert_eq!(step2.state, "admitted");
    assert_eq!(step2.revision, 3);

    let step3 = sm.transition(id, 3, "staging").unwrap();
    assert_eq!(step3.revision, 4);

    let step4 = sm.transition(id, 4, "submitted").unwrap();
    assert_eq!(step4.revision, 5);

    let step5 = sm.transition(id, 5, "cloud_running").unwrap();
    assert_eq!(step5.revision, 6);

    let step6 = sm.transition(id, 6, "result_ready").unwrap();
    assert_eq!(step6.revision, 7);

    let step7 = sm.transition(id, 7, "downloaded").unwrap();
    assert_eq!(step7.revision, 8);

    let step8 = sm.transition(id, 8, "acknowledged").unwrap();
    assert_eq!(step8.revision, 9);

    // Verify cryptographic audit event chain
    assert!(sm.verify_event_chain());
}

#[test]
fn test_cas_revision_conflict_rejection() {
    let sm = OutboxStateMachine::new();
    let id = "req_test_cas";
    sm.create_record(id, "matter_1", "sha256:...").unwrap();

    // Passing stale revision 0 when actual is 1
    let err = sm.transition(id, 0, "admission_pending").unwrap_err();
    assert!(matches!(
        err,
        OutboxError::RevisionConflict {
            expected: 0,
            actual: 1
        }
    ));
}

#[test]
fn test_terminal_states_cannot_transition() {
    let sm = OutboxStateMachine::new();
    let id = "req_test_terminal";
    sm.create_record(id, "matter_1", "sha256:...").unwrap();

    // Cancel the task
    sm.transition(id, 1, "cancelled").unwrap();

    // Attempting to move from cancelled -> submitted must fail
    let err = sm.transition(id, 2, "submitted").unwrap_err();
    assert!(matches!(err, OutboxError::InvalidTransition { .. }));
}
