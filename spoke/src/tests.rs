use super::*;

fn chat(project_id: &ProjectId) -> IntentChatRecord {
    IntentChatRecord {
        intent_chat_id: IntentChatId("chat_test".into()),
        spoke_id: SpokeId("spoke_test".into()),
        project_id: project_id.clone(),
        source_of_intent_revision: 3,
        status: IntentStatus::Ready,
        next_cursor: 1,
        created_at: 1,
        updated_at: 1,
        last_active_at: 1,
    }
}

#[test]
fn timeline_cursors_are_monotonic() {
    let project_id = ProjectId("project_test".into());
    let mut store = Store::default();
    store.chats.insert(project_id.clone(), chat(&project_id));
    let first = append_item_locked(
        &mut store,
        &project_id,
        None,
        TimelineKind::Progress,
        None,
        None,
        None,
    );
    let second = append_item_locked(
        &mut store,
        &project_id,
        None,
        TimelineKind::Outcome,
        None,
        None,
        None,
    );
    assert_eq!((first.cursor, second.cursor), (1, 2));
    assert_eq!(second.source_of_intent_revision, Some(3));
}

#[tokio::test]
async fn prerelease_store_is_written_atomically_and_round_trips() {
    let dir = std::env::temp_dir().join(format!("pumpkinpi-spoke-test-{}", Uuid::new_v4()));
    let mut store = Store::default();
    let project_id = ProjectId("project_test".into());
    store.chats.insert(project_id.clone(), chat(&project_id));
    append_item_locked(
        &mut store,
        &project_id,
        None,
        TimelineKind::Question,
        None,
        Some("What outcome matters?".into()),
        None,
    );
    save_store(&dir, &store).await.unwrap();
    let loaded = load_store(&dir).await.unwrap();
    assert_eq!(loaded.timelines[&project_id][0].cursor, 1);
    assert!(!dir.join("store-v3.json.tmp").exists());
    let _ = tokio::fs::remove_dir_all(dir).await;
}

#[test]
fn interrupted_isolated_realization_is_queued_for_checkpoint_recovery() {
    let project_id = ProjectId("project_recovery".into());
    let operation_id = OperationId("operation_recovery".into());
    let mut store = Store::default();
    store.chats.insert(project_id.clone(), chat(&project_id));
    store.sources.insert(
        project_id.clone(),
        SourceOfIntentRecord {
            source_of_intent_id: SourceOfIntentId("source_recovery".into()),
            spoke_id: SpokeId("spoke_test".into()),
            project_id: project_id.clone(),
            format: "markdown.v1".into(),
            revision: 2,
            canonical_payload: "# Intent".into(),
            authoritative_bundle: None,
            content_hash: source_bundle::source_hash("# Intent", None),
            status: SourceStatus::Active,
            created_at: 1,
            updated_at: 1,
        },
    );
    store.projects.insert(
        project_id.clone(),
        ProjectRecord {
            project_id: project_id.clone(),
            spoke_id: SpokeId("spoke_test".into()),
            name: "recovery".into(),
            cwd: "/tmp/recovery".into(),
            source_of_intent_id: SourceOfIntentId("source_recovery".into()),
            intent_chat_id: IntentChatId("chat_test".into()),
            initialization_status: InitializationStatus::Ready,
            default_provider: None,
            default_model: None,
            run_as_user: None,
            allow_root_sessions: false,
            status: ProjectStatus::Active,
            trusted: true,
            realization_status: RealizationStatus::Reviewing,
            created_at: 1,
            updated_at: 1,
        },
    );
    store.operations.insert(
        operation_id.clone(),
        OperationRecord {
            operation_id: operation_id.clone(),
            request_id: None,
            spoke_id: SpokeId("spoke_test".into()),
            project_id: project_id.clone(),
            intent_chat_id: IntentChatId("chat_test".into()),
            source_of_intent_revision: Some(2),
            kind: "intent.send".into(),
            status: OperationStatus::Running,
            error: None,
            created_at: 1,
            updated_at: 1,
            completed_at: None,
        },
    );
    store.realizations.insert(
        operation_id.clone(),
        orchestrator::RealizationMachine {
            revision: 2,
            iteration: 3,
            phase: orchestrator::RealizationPhase::Reviewing,
            findings: vec![],
        },
    );
    store.workspaces.insert(
        operation_id.clone(),
        workspace::WorkspaceRecord {
            project_id: project_id.clone(),
            operation_id: operation_id.clone(),
            primary_root: "/tmp/primary".into(),
            primary_cwd: "/tmp/primary".into(),
            worktree_root: "/tmp/worktree".into(),
            execution_cwd: "/tmp/worktree".into(),
            branch: "pumpkinpi/recovery".into(),
            base_commit: "base".into(),
            checkpoint_commit: "checkpoint".into(),
            status: workspace::WorkspaceStatus::Active,
        },
    );

    assert!(reconcile_interrupted(&mut store));
    assert_eq!(
        store.operations[&operation_id].status,
        OperationStatus::Queued
    );
    assert_eq!(
        store.realizations[&operation_id].phase,
        orchestrator::RealizationPhase::Implementing
    );
    assert_eq!(
        store.projects[&project_id].realization_status,
        RealizationStatus::Reconciling
    );
}

fn observed_check(subject: &str, output: &str) -> ObservedReviewEvidence {
    ObservedReviewEvidence {
        evidence_id: "sha256:observed".into(),
        tool_call_id: "call-observed".into(),
        tool_name: if subject.ends_with(".md") {
            "read".into()
        } else {
            "bash".into()
        },
        subject: subject.into(),
        args: if subject.ends_with(".md") {
            json!({"path": subject})
        } else {
            json!({"command": subject})
        },
        output_lines: vec![output.into()],
        successful: true,
        observed_at: now(),
    }
}

#[test]
fn promotion_requires_canonical_coverage_revision_reality_and_observed_evidence() {
    let bundle = SourceOfIntentBundle {
        manifest_path: "design.md".into(),
        bundle_hash: "bundle".into(),
        documents: vec![SourceDocument {
            path: "design.md".into(),
            content_hash: "canonical-hash".into(),
            byte_len: 7,
            content: "# Intent".into(),
        }],
    };
    let obligation = ReviewObligation {
        obligation_id: "obligation-test".into(),
        kind: ReviewObligationKind::ValidationCommand,
        subject: "cargo test --workspace".into(),
        expected_content_hash: None,
        validation_area: Some("workspace tests".into()),
    };
    let obligations = vec![obligation];
    let review = ReviewRunResult {
        source_coverage: source_bundle::coverage(&bundle),
        target_revision: 3,
        observed_reality_version: "reality-3".into(),
        scope: ReviewScope::WholeProject,
        reviewed_scope: vec!["obligation-test".into()],
        checks: vec!["cargo test --workspace".into()],
        evidence: vec!["sha256:observed".into()],
        obligation_observations: vec![ReviewObligationObservation {
            obligation_id: "obligation-test".into(),
            evidence_id: "sha256:observed".into(),
        }],
        findings: vec![],
        unreviewed_required_scope: vec![],
        verdict: ReviewVerdict::Approved,
    };

    let observed = vec![observed_check(
        "cargo test --workspace",
        "workspace suite passed",
    )];
    assert!(
        validate_review_for_promotion(
            &review,
            3,
            "reality-3",
            Some(&bundle),
            &obligations,
            &observed
        )
        .is_ok()
    );

    let mut stale = review.clone();
    stale.target_revision = 2;
    assert!(
        validate_review_for_promotion(
            &stale,
            3,
            "reality-3",
            Some(&bundle),
            &obligations,
            &observed
        )
        .is_err()
    );

    let mut wrong_reality = review.clone();
    wrong_reality.observed_reality_version = "model-claimed-reality".into();
    assert!(
        validate_review_for_promotion(
            &wrong_reality,
            3,
            "reality-3",
            Some(&bundle),
            &obligations,
            &observed,
        )
        .is_err()
    );

    let mut incomplete = review;
    incomplete.source_coverage.clear();
    assert!(
        validate_review_for_promotion(
            &incomplete,
            3,
            "reality-3",
            Some(&bundle),
            &obligations,
            &observed
        )
        .is_err()
    );
}

#[test]
fn fabricated_nonempty_review_prose_cannot_approve() {
    let review = ReviewRunResult {
        source_coverage: vec![],
        target_revision: 1,
        observed_reality_version: "reality".into(),
        scope: ReviewScope::WholeProject,
        reviewed_scope: vec!["complete project".into()],
        checks: vec!["tests passed".into()],
        evidence: vec!["evidence exists".into()],
        obligation_observations: vec![ReviewObligationObservation {
            obligation_id: "invented".into(),
            evidence_id: "evidence exists".into(),
        }],
        findings: vec![],
        unreviewed_required_scope: vec![],
        verdict: ReviewVerdict::Approved,
    };

    let obligation = ReviewObligation {
        obligation_id: "required-read".into(),
        kind: ReviewObligationKind::ProjectFile,
        subject: "README.md".into(),
        expected_content_hash: Some("canonical".into()),
        validation_area: None,
    };
    let obligations = vec![obligation];
    let error = validate_review_for_promotion(&review, 1, "reality", None, &obligations, &[])
        .unwrap_err()
        .to_string();
    assert!(error.contains("map one-to-one") || error.contains("reviewed_scope"));

    let unrelated = vec![observed_check("tests passed", "evidence exists")];
    assert!(
        validate_review_for_promotion(&review, 1, "reality", None, &obligations, &unrelated)
            .is_err()
    );
}

#[test]
fn copied_coverage_and_one_trivial_read_cannot_approve_complete_scope() {
    let bundle = SourceOfIntentBundle {
        manifest_path: "design.md".into(),
        bundle_hash: "bundle".into(),
        documents: vec![SourceDocument {
            path: "design.md".into(),
            content_hash: "canonical-hash".into(),
            byte_len: 7,
            content: "# Intent".into(),
        }],
    };
    let obligations = vec![
        ReviewObligation {
            obligation_id: "read-design".into(),
            kind: ReviewObligationKind::AuthoritativeDocument,
            subject: "design.md".into(),
            expected_content_hash: Some("canonical-hash".into()),
            validation_area: None,
        },
        ReviewObligation {
            obligation_id: "workspace-tests".into(),
            kind: ReviewObligationKind::ValidationCommand,
            subject: "cargo test --workspace --all-targets".into(),
            expected_content_hash: None,
            validation_area: Some("workspace tests".into()),
        },
    ];
    let copied_observation = observed_check("design.md", "# Intent");
    let review = ReviewRunResult {
        source_coverage: source_bundle::coverage(&bundle),
        target_revision: 3,
        observed_reality_version: "reality".into(),
        scope: ReviewScope::WholeProject,
        reviewed_scope: vec!["read-design".into(), "workspace-tests".into()],
        checks: vec![
            "design.md".into(),
            "cargo test --workspace --all-targets".into(),
        ],
        evidence: vec![
            copied_observation.evidence_id.clone(),
            "sha256:invented".into(),
        ],
        obligation_observations: vec![
            ReviewObligationObservation {
                obligation_id: "read-design".into(),
                evidence_id: copied_observation.evidence_id.clone(),
            },
            ReviewObligationObservation {
                obligation_id: "workspace-tests".into(),
                evidence_id: "sha256:invented".into(),
            },
        ],
        findings: vec![],
        unreviewed_required_scope: vec![],
        verdict: ReviewVerdict::Approved,
    };

    let error = validate_review_for_promotion(
        &review,
        3,
        "reality",
        Some(&bundle),
        &obligations,
        &[copied_observation],
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("not bound to a successful Spoke-observed tool result"));
}

#[test]
fn spoke_issues_file_and_validation_obligations_from_reality() {
    let dir = std::env::temp_dir().join(format!("pumpkinpi-obligations-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("design.md"), "# Intent\n").unwrap();
    std::fs::write(dir.join("Cargo.toml"), "[workspace]\n").unwrap();
    std::fs::write(dir.join("README.md"), "project\n").unwrap();
    let bundle = SourceOfIntentBundle {
        manifest_path: "design.md".into(),
        bundle_hash: "bundle".into(),
        documents: vec![SourceDocument {
            path: "design.md".into(),
            content_hash: hex::encode(Sha256::digest(b"# Intent\n")),
            byte_len: 9,
            content: "# Intent\n".into(),
        }],
    };

    let obligations = issue_review_obligations(&dir, Some(&bundle), 3, "reality").unwrap();
    assert!(obligations.iter().any(|item| {
        item.kind == ReviewObligationKind::AuthoritativeDocument && item.subject == "design.md"
    }));
    for path in ["Cargo.toml", "README.md"] {
        assert!(obligations.iter().any(|item| {
            item.kind == ReviewObligationKind::ProjectFile && item.subject == path
        }));
    }
    assert_eq!(
        obligations
            .iter()
            .filter(|item| item.kind == ReviewObligationKind::ValidationCommand)
            .count(),
        3
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn active_intent_iterates_through_independent_review_to_satisfaction() {
    let dir = std::env::temp_dir().join(format!("pumpkinpi-orchestrator-test-{}", Uuid::new_v4()));
    let project_root = dir.join("project");
    tokio::fs::create_dir_all(&project_root).await.unwrap();
    tokio::fs::write(project_root.join("README.md"), "fixture\n")
        .await
        .unwrap();
    tokio::fs::write(
        project_root.join("design.md"),
        "# Fixture Intent\n\nThe README must exist.\n",
    )
    .await
    .unwrap();
    let authoritative_bundle = source_bundle::import(&project_root).unwrap().unwrap();
    for args in [
        vec!["init", "-b", "main"],
        vec!["add", "."],
        vec![
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-m",
            "fixture",
        ],
    ] {
        let status = Command::new("git")
            .args(args)
            .current_dir(&project_root)
            .status()
            .await
            .unwrap();
        assert!(status.success());
    }
    let coverage_json =
        serde_json::to_string(&source_bundle::coverage(&authoritative_bundle)).unwrap();
    let fixture_reality = project_fingerprint(project_root.clone()).await.unwrap();
    let fixture_obligations = issue_review_obligations(
        &project_root,
        Some(&authoritative_bundle),
        1,
        &fixture_reality,
    )
    .unwrap();
    let mut fixture_events = Vec::new();
    let mut fixture_bindings = Vec::new();
    for (index, obligation) in fixture_obligations.iter().enumerate() {
        let tool_call_id = format!("fixture-call-{index}");
        let (tool_name, args, text) = match obligation.kind {
            ReviewObligationKind::AuthoritativeDocument | ReviewObligationKind::ProjectFile => (
                "read",
                json!({"path": obligation.subject}),
                std::fs::read_to_string(project_root.join(&obligation.subject)).unwrap(),
            ),
            ReviewObligationKind::ValidationCommand => (
                "bash",
                json!({"command": obligation.subject}),
                "validation passed\n".into(),
            ),
        };
        fixture_events.push(json!({
            "type": "tool_execution_start",
            "toolCallId": tool_call_id,
            "toolName": tool_name,
            "args": args,
        }));
        let end = json!({
            "type": "tool_execution_end",
            "toolCallId": tool_call_id,
            "toolName": tool_name,
            "result": {"content": [{"type": "text", "text": text}]},
            "isError": false,
        });
        let observation = observed_review_evidence(&end, tool_name.into(), args).unwrap();
        fixture_bindings.push(ReviewObligationObservation {
            obligation_id: obligation.obligation_id.clone(),
            evidence_id: observation.evidence_id,
        });
        fixture_events.push(end);
    }
    let fixture_event_lines = fixture_events
        .iter()
        .map(serde_json::Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let fixture_scope = serde_json::to_string(
        &fixture_obligations
            .iter()
            .map(|item| &item.obligation_id)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let fixture_checks = serde_json::to_string(
        &fixture_obligations
            .iter()
            .map(|item| &item.subject)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let fixture_evidence = serde_json::to_string(
        &fixture_bindings
            .iter()
            .map(|item| &item.evidence_id)
            .collect::<Vec<_>>(),
    )
    .unwrap();
    let fixture_bindings = serde_json::to_string(&fixture_bindings).unwrap();

    let fake_pi = dir.join("fake-pi");
    let script = r####"#!/bin/sh
read request
case "$request" in
  *"Project Intent Agent"*)
    result='{"acts":["reference_context"],"source_coverage":__COVERAGE__,"projection":"Intent adopted.","question":null,"source_update":{"base_revision":0,"canonical_payload":"# Intent\\n\\nImplement and validate the fixture completely.","activate":true},"assumptions":[]}'
    ;;
  *"independent whole-Project reviewer"*)
    reality=$(printf '%s' "$request" | sed -n 's/.*observed_reality_version [^0-9a-f]*\([0-9a-f]\{64\}\).*/\1/p')
    printf '%s\n' '__REVIEW_EVENTS__'
    result='{"source_coverage":__COVERAGE__,"target_revision":1,"observed_reality_version":"'"$reality"'","scope":"whole_project","reviewed_scope":__REVIEW_SCOPE__,"checks":__REVIEW_CHECKS__,"evidence":__REVIEW_EVIDENCE__,"obligation_observations":__REVIEW_BINDINGS__,"findings":[],"unreviewed_required_scope":[],"verdict":"approved"}'
    ;;
  *)
    result='{"source_coverage":__COVERAGE__,"objective":"verify fixture","summary":"Fixture conforms.","observations":["README exists"],"changes":[],"validation":["fixture inspected"],"evidence":["README.md"],"residual_divergence":[],"question":null}'
    ;;
esac
escaped=$(printf '%s' "$result" | sed 's/\\/\\\\/g; s/"/\\"/g')
printf '%s\n' "{\"type\":\"message_end\",\"message\":{\"content\":\"$escaped\"}}"
printf '%s\n' '{"type":"agent_settled"}'
"####;
    let script = script
        .replace("__COVERAGE__", &coverage_json)
        .replace("__REVIEW_EVENTS__", &fixture_event_lines)
        .replace("__REVIEW_SCOPE__", &fixture_scope)
        .replace("__REVIEW_CHECKS__", &fixture_checks)
        .replace("__REVIEW_EVIDENCE__", &fixture_evidence)
        .replace("__REVIEW_BINDINGS__", &fixture_bindings);
    tokio::fs::write(&fake_pi, script).await.unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&fake_pi, std::fs::Permissions::from_mode(0o700))
            .await
            .unwrap();
    }

    let spoke_id = SpokeId("spoke_test".into());
    let project_id = ProjectId("project_test".into());
    let chat_id = IntentChatId("chat_test".into());
    let operation_id = OperationId("operation_test".into());
    let n = now();
    let project = ProjectRecord {
        project_id: project_id.clone(),
        spoke_id: spoke_id.clone(),
        name: "fixture".into(),
        cwd: project_root.to_string_lossy().into_owned(),
        source_of_intent_id: SourceOfIntentId("source_test".into()),
        intent_chat_id: chat_id.clone(),
        initialization_status: InitializationStatus::Clarifying,
        default_provider: None,
        default_model: None,
        run_as_user: None,
        allow_root_sessions: false,
        status: ProjectStatus::Active,
        trusted: true,
        realization_status: RealizationStatus::Inactive,
        created_at: n,
        updated_at: n,
    };
    let source_payload = "# Intent\n\nAssembling.".to_string();
    let source = SourceOfIntentRecord {
        source_of_intent_id: project.source_of_intent_id.clone(),
        spoke_id: spoke_id.clone(),
        project_id: project_id.clone(),
        format: "markdown.v1".into(),
        revision: 0,
        canonical_payload: source_payload.clone(),
        authoritative_bundle: Some(authoritative_bundle.clone()),
        content_hash: source_bundle::source_hash(&source_payload, Some(&authoritative_bundle)),
        status: SourceStatus::Assembling,
        created_at: n,
        updated_at: n,
    };
    let chat = IntentChatRecord {
        intent_chat_id: chat_id.clone(),
        spoke_id: spoke_id.clone(),
        project_id: project_id.clone(),
        source_of_intent_revision: 0,
        status: IntentStatus::WaitingForUser,
        next_cursor: 1,
        created_at: n,
        updated_at: n,
        last_active_at: n,
    };
    let operation = OperationRecord {
        operation_id: operation_id.clone(),
        request_id: None,
        spoke_id: spoke_id.clone(),
        project_id: project_id.clone(),
        intent_chat_id: chat_id,
        source_of_intent_revision: Some(0),
        kind: "intent.send".into(),
        status: OperationStatus::Accepted,
        error: None,
        created_at: n,
        updated_at: n,
        completed_at: None,
    };
    let mut store = Store::default();
    store.projects.insert(project_id.clone(), project);
    store.sources.insert(project_id.clone(), source);
    store.chats.insert(project_id.clone(), chat);
    store.operations.insert(operation_id.clone(), operation);

    let state = State {
        config: Config {
            spoke_id,
            hub_url: "http://unused".into(),
            trusted_roots: vec![project_root],
            max_runs_per_project: 1,
            pi_binary: Some(fake_pi),
        },
        data_dir: dir.clone(),
        store: Arc::new(Mutex::new(store)),
        project_lanes: Default::default(),
        realization_lanes: Default::default(),
        cancellations: Default::default(),
        interactions: Default::default(),
    };
    let (tx, mut events) = mpsc::unbounded_channel();
    let (_cancel_tx, cancel_rx) = watch::channel(false);

    run_operation(
        state.clone(),
        tx,
        project_id.clone(),
        operation_id.clone(),
        "Use the existing design.".into(),
        cancel_rx,
        BTreeMap::new(),
    )
    .await
    .unwrap();
    while events.try_recv().is_ok() {}

    let store = state.store.lock().await;
    assert_eq!(store.sources[&project_id].revision, 1);
    assert_eq!(store.sources[&project_id].status, SourceStatus::Active);
    assert_eq!(
        store.sources[&project_id]
            .authoritative_bundle
            .as_ref()
            .unwrap()
            .bundle_hash,
        authoritative_bundle.bundle_hash
    );
    assert_eq!(
        store.projects[&project_id].realization_status,
        RealizationStatus::Satisfied
    );
    assert_eq!(store.reviews.len(), 1);
    assert_eq!(
        store.reviews.values().next().unwrap().verdict,
        ReviewVerdict::Approved
    );
    assert_eq!(
        store.operations[&operation_id].status,
        OperationStatus::Completed
    );
    drop(store);
    let _ = tokio::fs::remove_dir_all(dir).await;
}
