//! Task completion preserves a queued notification's budget alongside real user input.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_finish_preserves_notification_budget_and_queued_user_input() {
    let (session, turn, _rx) = make_session_and_context_with_rx().await;
    session
        .spawn_task(
            Arc::clone(&turn),
            Vec::new(),
            NeverEndingTask {
                kind: TaskKind::Regular,
                listen_to_cancellation_token: false,
            },
        )
        .await;
    let text = "notification diagnostic line\n".repeat(500);
    let notification = |text| ResponseItem::CustomToolCallOutput {
        id: None,
        call_id: "call-a".to_string(),
        name: Some("exec".to_string()),
        output: FunctionCallOutputPayload::from_text(text),
        internal_chat_message_metadata_passthrough: None,
    };
    session
        .inject_if_running(vec![ResponseItemEnvelope {
            item: notification(text.clone()),
            metadata: Some(CodexHarnessMetadata {
                history_truncation_token_limit: Some(120),
                ..Default::default()
            }),
        }])
        .await
        .unwrap();
    let user_input = vec![UserInput::Text {
        text: "keep this queued user input".to_string(),
        text_elements: Vec::new(),
    }];
    assert!(matches!(
        submit_steer_only(&session, user_input.clone(), &turn.sub_id).await,
        TurnInputSubmission::Steered { .. }
    ));
    let mut current = turn.initial_settings.as_ref().clone();
    Arc::make_mut(&mut current.model_info).truncation_policy =
        codex_protocol::openai_models::TruncationPolicyConfig::tokens(/*limit*/ 400);
    turn.current_settings.store(Arc::new(current));

    session
        .on_task_finished(Arc::clone(&turn), /*task_result*/ Ok(None))
        .await;

    let expected = vec![
        notification(codex_utils_output_truncation::truncate_text(
            &text,
            codex_utils_output_truncation::TruncationPolicy::Tokens(120),
        )),
        session.response_item_from_user_input(user_input),
    ];
    let history = session.clone_history().await;
    assert_eq!(
        strip_response_item_ids(&strip_metadata_from_items(&raw_history_items(&history))),
        strip_response_item_ids(&strip_metadata_from_items(&expected)),
    );
}
