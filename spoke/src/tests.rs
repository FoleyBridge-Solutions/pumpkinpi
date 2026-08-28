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
        tool_name: "bash".into(),
        subject: subject.into(),
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
    let review = ReviewRunResult {
        source_coverage: source_bundle::coverage(&bundle),
        target_revision: 3,
        observed_reality_version: "reality-3".into(),
        scope: ReviewScope::WholeProject,
        reviewed_scope: vec!["all project files and required behavior".into()],
        checks: vec!["cargo test --workspace".into()],
        evidence: vec!["workspace suite passed".into()],
        findings: vec![],
        unreviewed_required_scope: vec![],
        verdict: ReviewVerdict::Approved,
    };

    let observed = vec![observed_check(
        "cargo test --workspace",
        "workspace suite passed",
    )];
    assert!(
        validate_review_for_promotion(&review, 3, "reality-3", Some(&bundle), &observed).is_ok()
    );

    let mut stale = review.clone();
    stale.target_revision = 2;
    assert!(
        validate_review_for_promotion(&stale, 3, "reality-3", Some(&bundle), &observed).is_err()
    );

    let mut wrong_reality = review.clone();
    wrong_reality.observed_reality_version = "model-claimed-reality".into();
    assert!(
        validate_review_for_promotion(&wrong_reality, 3, "reality-3", Some(&bundle), &observed,)
            .is_err()
    );

    let mut incomplete = review;
    incomplete.source_coverage.clear();
    assert!(
        validate_review_for_promotion(&incomplete, 3, "reality-3", Some(&bundle), &observed)
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
        findings: vec![],
        unreviewed_required_scope: vec![],
        verdict: ReviewVerdict::Approved,
    };

    let error = validate_review_for_promotion(&review, 1, "reality", None, &[])
        .unwrap_err()
        .to_string();
    assert!(error.contains("not bound to a successful Spoke-observed tool result"));

    let unrelated = vec![observed_check("cargo test --workspace", "all tests passed")];
    assert!(validate_review_for_promotion(&review, 1, "reality", None, &unrelated).is_err());
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

    let fake_pi = dir.join("fake-pi");
    let script = r####"#!/bin/sh
read request
case "$request" in
  *"Project Intent Agent"*)
    result='{"acts":["reference_context"],"source_coverage":__COVERAGE__,"projection":"Intent adopted.","question":null,"source_update":{"base_revision":0,"canonical_payload":"# Intent\\n\\nImplement and validate the fixture completely.","activate":true},"assumptions":[]}'
    ;;
  *"independent whole-Project reviewer"*)
    reality=$(printf '%s' "$request" | sed -n 's/.*observed_reality_version [^0-9a-f]*\([0-9a-f]\{64\}\).*/\1/p')
    printf '%s\n' '{"type":"tool_execution_start","toolCallId":"call_review","toolName":"bash","args":{"command":"printf fixture-observed"}}'
    printf '%s\n' '{"type":"tool_execution_end","toolCallId":"call_review","toolName":"bash","result":{"content":[{"type":"text","text":"fixture-observed\n"}]},"isError":false}'
    result='{"source_coverage":__COVERAGE__,"target_revision":1,"observed_reality_version":"'"$reality"'","scope":"whole_project","reviewed_scope":["complete project"],"checks":["printf fixture-observed"],"evidence":["fixture-observed"],"findings":[],"unreviewed_required_scope":[],"verdict":"approved"}'
    ;;
  *)
    result='{"source_coverage":__COVERAGE__,"objective":"verify fixture","summary":"Fixture conforms.","observations":["README exists"],"changes":[],"validation":["fixture inspected"],"evidence":["README.md"],"residual_divergence":[],"question":null}'
    ;;
esac
escaped=$(printf '%s' "$result" | sed 's/\\/\\\\/g; s/"/\\"/g')
printf '%s\n' "{\"type\":\"message_end\",\"message\":{\"content\":\"$escaped\"}}"
printf '%s\n' '{"type":"agent_settled"}'
"####;
    let script = script.replace("__COVERAGE__", &coverage_json);
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
