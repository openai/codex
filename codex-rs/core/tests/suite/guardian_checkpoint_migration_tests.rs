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
use codex_protocol::user_input::UserInput;
use codex_thread_store::LoadThreadHistoryParams;
use core_test_support::responses;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_case::test_case;

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
            thread_id: thread.session_configured().thread_id,
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
    let thread_id = thread.session_configured().thread_id;
    let environments = thread.environment_selections().await;
    thread.shutdown_and_wait().await?;
    test.thread_manager.remove_thread(&thread_id).await;
    let mut config = test.config.clone();
    config.features.enable(Feature::GuardianThreadContext)?;
    Ok(test
        .thread_manager
        .start_thread(StartThreadOptions {
            environments: Some(environments),
            initial_history: InitialHistory::Resumed(ResumedHistory {
                conversation_id: thread_id,
                history: Arc::new(serde_json::from_value(serde_json::to_value(history)?)?),
                rollout_path: None,
            }),
            ..StartThreadOptions::new(config)
        })
        .await?
        .thread)
}

#[test_case(ThreadHistoryMode::Legacy; "legacy")]
#[test_case(ThreadHistoryMode::Paginated; "paginated")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn old_checkpoint_migrates_after_compaction_and_resume(
    history_mode: ThreadHistoryMode,
) -> Result<()> {
    core_test_support::skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let test = test_codex()
        .with_history_mode(history_mode)
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_model("gpt-5.5")
        .with_model_info_override("gpt-5.5", |model| {
            model.comp_hash = Some("compatible".to_owned());
            model.auto_review_model_override = Some(model.slug.clone());
        })
        .with_config(|config| {
            config
                .features
                .disable(Feature::TokenBudget)
                .expect("disable token budget");
            for feature in [
                Feature::GuardianReuseParentCompaction,
                Feature::DefaultModeRequestUserInput,
            ] {
                config.features.enable(feature).expect("enable review flow");
            }
            config.permissions.approval_policy = Constrained::allow_any(AskForApproval::OnRequest);
            config.approvals_reviewer = ApprovalsReviewer::AutoReview;
        })
        .build_with_auto_env(&server)
        .await?;
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

    let review = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_function_call(
                    "ask",
                    "request_user_input",
                    &json!({
                        "questions": [{
                            "id": "publish", "header": "Publish",
                            "question": "Where may I publish?",
                            "options": [
                                {"label": "Private", "description": "Private repository only."},
                                {"label": "Nowhere", "description": "Keep local."}
                            ]
                        }]
                    })
                    .to_string(),
                ),
                responses::ev_completed("question"),
            ]),
            responses::sse(vec![
                responses::ev_function_call(
                    "exec",
                    "exec_command",
                    r#"{"cmd":"echo migration","sandbox_permissions":"require_escalated"}"#,
                ),
                responses::ev_completed("action"),
            ]),
            responses::sse(vec![
                responses::ev_assistant_message(
                    "decision",
                    r#"{"risk_level":"low","user_authorization":"high","outcome":"allow"}"#,
                ),
                responses::ev_completed("review"),
            ]),
            responses::sse(vec![responses::ev_completed("done")]),
        ],
    )
    .await;
    thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Check authorization, then run the command.".to_owned(),
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
    let requests = review.requests();
    let guardian = requests
        .iter()
        .find(|request| request.body_json()["client_metadata"]["x-openai-subagent"] == "guardian")
        .expect("Guardian reviewed the action");
    let prompt = guardian.message_input_texts("user").join("\n");
    assert!(prompt.contains("Private repository only."));
    assert!(prompt.contains("Keep the working tree unchanged."));

    let before_compaction = saved_history(&test, &thread).await?;
    responses::mount_sse_once(
        &server,
        responses::sse(vec![
            json!({"type": "response.output_item.done", "item": {
                "type": "compaction", "id": "migrated", "encrypted_content": "new checkpoint"
            }}),
            responses::ev_completed("compacted"),
        ]),
    )
    .await;
    thread.submit(Op::Compact).await?;
    finish_turn(&thread).await;
    assert_eq!(
        GuardianContextMode::from_history(thread.conversation_history_snapshot().await.as_ref()),
        GuardianContextMode::Legacy
    );
    let after_compaction = saved_history(&test, &thread).await?;
    let expected_answer = VerifiedAnswer {
        turn_id: question.turn_id,
        call_id: "ask".to_owned(),
        questions: vec![VerifiedQuestionAnswer {
            question: "Where may I publish?\nPrivate: Private repository only.".to_owned(),
            answer: "Private".to_owned(),
        }],
    };
    let mut thread = thread;
    // Both replayed suffix events and compacted checkpoints must preserve the accepted answer.
    for (saved, expected_hash) in [
        (before_compaction, None),
        (after_compaction, Some("compatible")),
    ] {
        thread = resume(&test, &thread, saved).await?;
        let history = thread.conversation_history_snapshot().await;
        let answers = history
            .retained_context()
            .expect("retained answer evidence")
            .verified_answers()
            .collect::<Vec<_>>();
        assert_eq!(
            (
                GuardianContextMode::from_history(history.as_ref()),
                history.latest_compaction_model_hash(),
                answers,
            ),
            (
                GuardianContextMode::ThreadOwned,
                expected_hash,
                vec![&expected_answer]
            ),
        );
    }
    thread.shutdown_and_wait().await?;
    Ok(())
}
