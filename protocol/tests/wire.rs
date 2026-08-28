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
fn reviewer_cannot_approve_with_findings_or_unreviewed_scope() {
    let invalid: ReviewRunResult = serde_json::from_str(
        r##"{"source_coverage":[],"reviewed_scope":["workspace"],"checks":[],"findings":[{"requirement":"typed core","fault":"Value leaked","evidence":["src/main.rs:1"],"suggested_next_objective":null}],"unreviewed_required_scope":[],"verdict":"approved"}"##,
    )
    .unwrap();
    assert!(invalid.validate().is_err());

    let valid: ReviewRunResult = serde_json::from_str(
        r##"{"source_coverage":[],"reviewed_scope":["complete project"],"checks":["cargo test"],"findings":[],"unreviewed_required_scope":[],"verdict":"approved"}"##,
    )
    .unwrap();
    assert_eq!(valid.validate(), Ok(()));
}

#[test]
fn public_event_does_not_require_raw_json() {
    let event = ClientEvent {
        protocol_version: PROTOCOL_VERSION,
        id: None,
        payload: ClientPayload::Error {
            code: "offline".into(),
            message: "Spoke is offline".into(),
        },
    };
    let value = serde_json::to_value(event).unwrap();
    assert_eq!(value["type"], "error");
    assert_eq!(value["protocol_version"], PROTOCOL_VERSION);
}
