//! Old encrypted checkpoints keep legacy review while newly captured answers survive migration.

use std::sync::Arc;

use anyhow::Result;
use codex_core::CodexThread;
use codex_core::StartThreadOptions;
use codex_core::TurnInputRequest;
use codex_core::config::Constrained;
use codex_core::context::GuardianContextMode;
use codex_features::Feature;
use codex_history::InitialHistory;
use codex_history::ResumedHistory;
use codex_history::RolloutItem;
use codex_history::VerifiedAnswer;
use codex_history::VerifiedQuestionAnswer;
use codex_login::CodexAuth;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use codex_thread_store::LoadThreadHistoryParams;
use core_test_support::responses;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;

async fn finish_turn(thread: &CodexThread) {
    wait_for_event_match(thread, |event| match event {
        EventMsg::TurnComplete(_) => Some(()),
        EventMsg::Error(error) => panic!("unexpected turn error: {error:?}"),
        _ => None,
    })
    .await;
}

async fn saved_history(test: &TestCodex, thread: &CodexThread) -> Result<Vec<RolloutItem>> {
    thread.flush_rollout().await?;
    Ok(test
        .thread_store
        .load_latest_model_context(LoadThreadHistoryParams {
            thread_id: thread.startup_metadata().thread_id,
            include_archived: false,
        })
        .await?
        .items)
}

pub(super) async fn resume(
    test: &TestCodex,
    thread: &CodexThread,
    history: Vec<RolloutItem>,
) -> Result<Arc<CodexThread>> {
    let thread_id = thread.startup_metadata().thread_id;
    let environments = thread.environment_selections().await;
    let model = thread.config_snapshot().await.model;
    thread.shutdown_and_wait().await?;
    test.thread_manager.remove_thread(&thread_id).await;
    let mut config = test.config.clone();
    config.model = Some(model);
    config.features.enable(Feature::GuardianThreadContext)?;
    Ok(test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(environments),
            initial_history: InitialHistory::Resumed(ResumedHistory {
                conversation_id: thread_id,
                history: Arc::new(history),
                rollout_path: None,
            }),
            ..StartThreadOptions::new(config)
        })
        .await?
        .thread)
}

pub(super) async fn migration_scenario() -> Result<Vec<responses::ResponsesRequest>> {
    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_history_mode(ThreadHistoryMode::Paginated)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model_info_override("gpt-5.5", |model| {
            model.comp_hash = Some("previous-model".to_owned());
            model.auto_review_model_override = Some(model.slug.clone());
        })
        .with_model_info_override("gpt-5.6-luna", |model| {
            model.comp_hash = Some("current-model".to_owned());
            model.auto_review_model_override = Some(model.slug.clone());
        })
        .with_model("gpt-5.5")
        .with_config(|config| {
            config
                .features
                .disable(Feature::TokenBudget)
                .expect("disable token budget");
            config
                .features
                .disable(Feature::GuardianReuseParentCompaction)
                .expect("use the selected reviewer");
            config
                .features
                .enable(Feature::DefaultModeRequestUserInput)
                .expect("enable user input");
            config.model_auto_compact_token_limit = Some(100_000);
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        })
        .build_with_auto_env(&server)
        .await?;
    let compact = |id: &str| {
        responses::sse(vec![
            json!({"type": "response.output_item.done", "item": {
                "type": "compaction", "id": id, "encrypted_content": format!("encrypted checkpoint {id}")
            }}),
            responses::ev_completed(id),
        ])
    };
    // Old wire format, including an ordinary instruction recorded after the checkpoint.
    let history: Vec<RolloutItem> = serde_json::from_value(json!([
        {"type": "compacted", "payload": {
            "message": "old checkpoint",
            "replacement_history": [{
                "type": "compaction", "id": "old", "encrypted_content": "opaque checkpoint"
            }]
        }},
        {"type": "response_item", "payload": {
            "type": "message", "id": "restriction", "role": "user",
            "content": [{"type": "input_text", "text": "Keep the working tree unchanged."}]
        }}
    ]))?;
    test.codex.ensure_rollout_materialized().await;
    test.codex.append_rollout_items(&history).await?;
    let thread = resume(&test, &test.codex, history).await?;
    assert_eq!(
        GuardianContextMode::from_history(thread.conversation_history_snapshot().await.as_ref()),
        GuardianContextMode::Legacy
    );

    let answer = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_assistant_message(
                    "ordinary-question",
                    "May I publish after these checks?",
                ),
                responses::ev_function_call(
                    "ask",
                    "request_user_input",
                    // Keep the transcript independent of serde_json's preserve_order feature.
                    concat!(
                        r#"{"questions":[{"header":"Publish","id":"publish","options":["#,
                        r#"{"description":"Private repository only.","label":"Private"},"#,
                        r#"{"description":"Keep local.","label":"Nowhere"}],"#,
                        r#""question":"Where may I publish?"}]}"#,
                    ),
                ),
                responses::ev_completed("question"),
            ]),
            responses::sse(vec![responses::ev_completed("done")]),
        ],
    )
    .await;
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Confirm where publishing is allowed.".to_owned(),
            text_elements: Vec::new(),
        }]))
        .await?;
    let question = wait_for_event_match(&thread, |event| match event {
        EventMsg::RequestUserInput(request) => Some(request.clone()),
        EventMsg::Error(error) => panic!("unexpected question error: {error:?}"),
        _ => None,
    })
    .await;
    thread
        .submit(Op::UserInputAnswer {
            id: question.turn_id.clone(),
            response: serde_json::from_value(json!({
                "answers": {"publish": {"answers": ["Private"]}}
            }))?,
        })
        .await?;
    finish_turn(&thread).await;
    let before_compaction = saved_history(&test, &thread).await?;
    let thread = resume(&test, &thread, before_compaction).await?;
    assert_eq!(
        GuardianContextMode::from_history(thread.conversation_history_snapshot().await.as_ref()),
        GuardianContextMode::Legacy
    );
    let mut all_requests = answer.requests();

    // Automatic compaction uses the previous model, whose checkpoint the new reviewer cannot read.
    let compaction = responses::mount_sse_sequence(
        &server,
        vec![
            compact("cmp_previous_model"),
            responses::sse(vec![responses::ev_completed("continued")]),
        ],
    )
    .await;
    thread
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Continue with the new model.".to_owned(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                model: Some("gpt-5.6-luna".to_owned()),
                ..Default::default()
            }),
        )
        .await?;
    finish_turn(&thread).await;
    all_requests.extend(compaction.requests());
    let after = thread.conversation_history_snapshot().await;
    assert_eq!(
        (
            GuardianContextMode::from_history(after.as_ref()),
            after
                .latest_compaction()
                .and_then(|checkpoint| checkpoint.model_hash)
        ),
        (GuardianContextMode::Legacy, Some("previous-model")),
    );

    // A real restart must preserve legacy review and the persisted answer for a mismatched hash.
    let incompatible_history = saved_history(&test, &thread).await?;
    let thread = resume(&test, &thread, incompatible_history).await?;
    for (call_id, checkpoint) in [("resumed", None), ("after", Some("cmp_migrated"))] {
        if let Some(checkpoint) = checkpoint {
            let compaction = responses::mount_sse_once(&server, compact(checkpoint)).await;
            thread.submit(Op::Compact).await?;
            finish_turn(&thread).await;
            all_requests.extend(compaction.requests());
        }
        assert_eq!(
            GuardianContextMode::from_history(
                thread.conversation_history_snapshot().await.as_ref()
            ),
            if checkpoint.is_some() {
                GuardianContextMode::ThreadOwned
            } else {
                GuardianContextMode::Legacy
            },
        );
        let followup = responses::mount_sse_sequence(
            &server,
            vec![
                responses::sse(vec![
                    responses::ev_function_call(
                        call_id,
                        "exec_command",
                        r#"{"cmd":"exit 0","sandbox_permissions":"require_escalated"}"#,
                    ),
                    responses::ev_completed(&format!("{call_id}-action")),
                ]),
                responses::sse(vec![
                    responses::ev_assistant_message(
                        &format!("{call_id}-decision"),
                        // Exercise review without starting processes on the shared executor.
                        r#"{"outcome":"deny"}"#,
                    ),
                    responses::ev_completed(&format!("{call_id}-review")),
                ]),
                responses::sse(vec![responses::ev_completed(&format!("{call_id}-done"))]),
            ],
        )
        .await;
        thread
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Run another check after compaction.".to_owned(),
                text_elements: Vec::new(),
            }]))
            .await?;
        finish_turn(&thread).await;
        all_requests.extend(followup.requests());
    }
    let after_compaction = saved_history(&test, &thread).await?;
    let expected_answer = VerifiedAnswer {
        turn_id: question.turn_id,
        call_id: "ask".to_owned(),
        questions: vec![VerifiedQuestionAnswer {
            question: "Where may I publish?\nPrivate: Private repository only.".to_owned(),
            answer: "Private".to_owned(),
        }],
    };
    // The answer has survived suffix replay, both compactions, and checkpoint replay.
    let thread = resume(&test, &thread, after_compaction).await?;
    let history = thread.conversation_history_snapshot().await;
    assert!(
        history
            .retained_context()
            .expect("retained context")
            .ordered_entries()
            .any(|(_, entry)| {
                matches!(entry, codex_history::RetainedContextEntry::AssistantMessage(message)
            if message.message_id.as_deref() == Some("ordinary-question")
                && message.text == "May I publish after these checks?")
            })
    );
    let answers = history
        .retained_context()
        .expect("retained answer evidence")
        .verified_answers()
        .collect::<Vec<_>>();
    assert_eq!(
        (
            GuardianContextMode::from_history(history.as_ref()),
            history
                .latest_compaction()
                .and_then(|checkpoint| checkpoint.model_hash),
            answers,
        ),
        (
            GuardianContextMode::ThreadOwned,
            Some("current-model"),
            vec![&expected_answer]
        ),
    );
    thread.shutdown_and_wait().await?;
    Ok(all_requests)
}
