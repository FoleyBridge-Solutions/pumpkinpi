use pumpkinpi_protocol::*;

#[test]
fn client_commands_are_typed_and_round_trip() {
    let request = ClientRequest {
        protocol_version: PROTOCOL_VERSION,
        id: RequestId("request-1".into()),
        command: ClientCommand::IntentSend {
            spoke_id: SpokeId("spoke-1".into()),
            project_id: ProjectId("project-1".into()),
            message: "Make the tests pass".into(),
            expected_revision: Some(7),
        },
    };
    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("intent_send"));
    let decoded: ClientRequest = serde_json::from_str(&json).unwrap();
    match decoded.command {
        ClientCommand::IntentSend {
            expected_revision, ..
        } => assert_eq!(expected_revision, Some(7)),
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn intent_agent_contract_rejects_prose_and_the_old_execute_shortcut() {
    assert!(serde_json::from_str::<IntentTurnProposal>("I think we should proceed").is_err());
    assert!(serde_json::from_str::<IntentTurnProposal>(
        r##"{"response":"Intent is clear","question":null,"updated_source":"# Intent","execute":true,"work_request":"Run the tests"}"##,
    )
    .is_err());

    let proposal: IntentTurnProposal = serde_json::from_str(
        r##"{"acts":["reference_context"],"source_coverage":[],"projection":"I adopted the design.","question":null,"source_update":{"base_revision":0,"canonical_payload":"# Intent","activate":true},"assumptions":[]}"##,
    )
    .unwrap();
    assert!(proposal.source_update.unwrap().activate);
}

#[test]
fn reviewer_approval_requires_complete_explicit_review_contract() {
    let empty_approval: ReviewRunResult = serde_json::from_str(
        r##"{"source_coverage":[],"target_revision":3,"observed_reality_version":"sha256:reality","scope":"whole_project","reviewed_scope":[],"checks":[],"evidence":[],"findings":[],"unreviewed_required_scope":[],"verdict":"approved"}"##,
    )
    .unwrap();
    assert!(empty_approval.validate().is_err());

    let bounded_approval: ReviewRunResult = serde_json::from_str(
        r##"{"source_coverage":[],"target_revision":3,"observed_reality_version":"sha256:reality","scope":"bounded_objective","reviewed_scope":["src"],"checks":["cargo test"],"evidence":["test output"],"findings":[],"unreviewed_required_scope":[],"verdict":"approved"}"##,
    )
    .unwrap();
    assert!(bounded_approval.validate().is_err());

    let valid: ReviewRunResult = serde_json::from_str(
        r##"{"source_coverage":[],"target_revision":3,"observed_reality_version":"sha256:reality","scope":"whole_project","reviewed_scope":["complete project"],"checks":["cargo test"],"evidence":["all workspace tests passed"],"findings":[],"unreviewed_required_scope":[],"verdict":"approved"}"##,
    )
    .unwrap();
    assert_eq!(valid.validate(), Ok(()));
}

#[test]
fn reviewer_contract_requires_revision_reality_and_scope_fields() {
    let legacy = r##"{"source_coverage":[],"reviewed_scope":["complete project"],"checks":["cargo test"],"findings":[],"unreviewed_required_scope":[],"verdict":"approved"}"##;
    assert!(serde_json::from_str::<ReviewRunResult>(legacy).is_err());
}

#[test]
fn every_client_event_carries_its_authoritative_creation_timestamp() {
    let event = ClientEvent {
        protocol_version: PROTOCOL_VERSION,
        id: None,
        created_at: 1_704_164_645,
        payload: ClientPayload::Interaction {
            spoke_id: SpokeId("spoke-1".into()),
            project_id: ProjectId("project-1".into()),
            operation_id: OperationId("operation-1".into()),
            request_id: "interaction-1".into(),
            method: "confirm".into(),
            payload: serde_json::json!({"message": "Proceed?"}),
        },
    };

    let value = serde_json::to_value(event).unwrap();
    assert_eq!(value["type"], "interaction");
    assert_eq!(value["created_at"], 1_704_164_645_u64);
}

#[test]
fn client_events_without_authoritative_time_are_rejected() {
    let json = r#"{"protocol_version":3,"id":null,"type":"error","code":"offline","message":"Spoke is offline"}"#;
    assert!(serde_json::from_str::<ClientEvent>(json).is_err());
}

#[test]
fn public_event_does_not_require_raw_json() {
    let event = ClientEvent {
        protocol_version: PROTOCOL_VERSION,
        id: None,
        created_at: 1_704_164_646,
        payload: ClientPayload::Error {
            code: "offline".into(),
            message: "Spoke is offline".into(),
        },
    };
    let value = serde_json::to_value(event).unwrap();
    assert_eq!(value["type"], "error");
    assert_eq!(value["protocol_version"], PROTOCOL_VERSION);
    assert_eq!(value["created_at"], 1_704_164_646_u64);
}
