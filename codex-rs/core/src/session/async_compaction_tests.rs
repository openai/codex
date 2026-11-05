//! Background compaction eligibility and cancellation.

use super::*;
use codex_analytics::CompactionReason;
use codex_analytics::CompactionTrigger;
use codex_login::CodexAuth;
use codex_protocol::models::AudioReference;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::InputModality;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::TokenUsage;
use core_test_support::responses;
use pretty_assertions::assert_eq;

#[test_case::test_case(CompactionTrigger::Auto; "optional ready adoption")]
#[test_case::test_case(CompactionTrigger::Manual; "manual ready reuse")]
#[tokio::test]
async fn cancelled_turn_skips_launch_but_preserves_existing_work(trigger: CompactionTrigger) {
    let phase = CompactionPhase::MidTurn;
    let server = responses::start_mock_server().await;
    let mut provider =
        codex_model_provider_info::built_in_model_providers(/*openai_base_url*/ None)["openai"]
            .clone();
    provider.base_url = Some(format!("{}/v1", server.uri()));
    provider.supports_websockets = false;
    let server_uri = server.uri();
    let (sess, mut turn, _events) =
        crate::session::tests::make_session_and_context_with_auth_and_config_and_rx(
            CodexAuth::create_dummy_chatgpt_auth_for_testing(),
            Vec::new(),
            move |config| {
                config.model = Some("gpt-5.2".to_string());
                config.model_context_window = Some(100_000);
                config.model_auto_compact_token_limit = Some(90_000);
                config.async_compaction_threshold_percent = 50;
                config.model_provider = provider;
                config.chatgpt_base_url = server_uri;
                config.features.enable(Feature::AsyncCompaction).unwrap();
                config.features.disable(Feature::TokenBudget).unwrap();
            },
        )
        .await;
    crate::session::tests::update_turn_settings_for_test(
        Arc::get_mut(&mut turn).expect("unshared turn context"),
        |settings| {
            Arc::make_mut(&mut settings.model_info)
                .input_modalities
                .push(InputModality::Audio);
        },
    );
    let request = responses::mount_response_sequence(
        &server,
        vec![
            wiremock::ResponseTemplate::new(404).set_body_json(serde_json::json!({
                "error": {"message": "Invalid input: Unknown `file_id` in input content."}
            })),
            responses::sse_response(responses::sse(vec![
                serde_json::json!({"type":"response.output_item.done", "item": {
                    "type":"compaction", "encrypted_content":"prepared-summary"
                }}),
                responses::ev_completed("prepared"),
            ])),
        ],
    )
    .await;
    let turn_cancellation = CancellationToken::new();
    let step = sess
        .capture_step_context(Arc::clone(&turn), &turn_cancellation)
        .await
        .expect("capture step");
    let rejected_file_id = "file_rejected_background_compaction";
    let original = ResponseItem::Message {
        encrypted_content: None,
        status: None,
        id: None,
        role: "user".to_string(),
        content: vec![
            ContentItem::InputText {
                text: "original turn".to_string(),
            },
            ContentItem::InputAudio {
                audio: AudioReference::File {
                    file_id: rejected_file_id.to_string(),
                },
            },
            ContentItem::InputAudio {
                audio: AudioReference::File {
                    file_id: "file_expired_since_last_turn".to_string(),
                },
            },
        ],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    sess.replace_history(vec![original], /*reference_context_item*/ None)
        .await;
    sess.state
        .lock()
        .await
        .history
        .restore_rejected_audio_file_ids(std::collections::BTreeSet::from([
            rejected_file_id.to_string()
        ]));
    sess.update_token_usage_info(
        &turn,
        Some(&TokenUsage {
            input_tokens: 60_000,
            total_tokens: 60_000,
            ..Default::default()
        }),
        /*includes_reasoning*/ false,
    )
    .await
    .expect("record usage");
    let status = super::super::context_window::context_window_token_status(&sess, &turn).await;
    assert!(!status.token_limit_reached);
    let world_state = Arc::new(WorldState::default());
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let auth = sess.services.model_client.auth_owner_generation();
    maybe_start(
        &sess,
        &step,
        &world_state,
        phase,
        &cancelled,
        &status,
        (Some("routing".to_string()), auth),
    )
    .await;
    assert!(
        sess.state
            .lock()
            .await
            .async_compaction
            .last_attempt
            .is_none()
    );
    assert_eq!(request.requests().len(), 0);

    // The same eligible work can launch on a live turn, then outlive its cancellation.
    maybe_start(
        &sess,
        &step,
        &world_state,
        phase,
        &turn_cancellation,
        &status,
        (Some("routing".to_string()), auth),
    )
    .await;
    turn_cancellation.cancel();
    maybe_start(
        &sess,
        &step,
        &world_state,
        phase,
        &turn_cancellation,
        &status,
        (Some("routing".to_string()), auth),
    )
    .await;
    // Wait for client publication before exercising either ready-result consumer.
    wait_for_background_compaction_for_test(&sess).await;
    assert_eq!(request.requests().len(), 2, "background retry completed");
    let next_step = sess
        .capture_step_context(Arc::clone(&turn), &CancellationToken::new())
        .await
        .expect("capture next step");
    match trigger {
        CompactionTrigger::Auto => {
            let candidate = take_ready(&sess, &turn).await.expect("published candidate");
            let mut client_session = sess.services.model_client.new_session();
            assert!(
                crate::compact_remote_v2::run_inline_remote_auto_compact_task(
                    Arc::clone(&sess),
                    Arc::clone(&next_step),
                    next_step,
                    &mut client_session,
                    world_state,
                    CompactionReason::ContextLimit,
                    phase,
                    Some(candidate),
                    crate::compact_remote_v2::CompactionFallback::Skip,
                )
                .await
                .expect("optional adoption")
            );
        }
        CompactionTrigger::Manual => {
            crate::compact_remote_v2::run_remote_compact_task(
                Arc::clone(&sess),
                next_step,
                world_state,
            )
            .await
            .expect("manual compaction reuses the completed request");
        }
    }
    let history = sess.clone_history().await;
    let summaries = history
        .raw_items()
        .filter_map(|item| match item {
            ResponseItem::Compaction {
                encrypted_content, ..
            } => Some(encrypted_content.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(summaries, ["prepared-summary"]);
    let requests = request.requests();
    assert_eq!(requests.len(), 2);
    for (index, request) in requests.into_iter().enumerate() {
        let body = request.body_json().to_string();
        assert!(body.contains("original turn"));
        assert!(!body.contains(rejected_file_id));
        assert_eq!(body.contains("file_expired_since_last_turn"), index == 0);
    }
    shutdown(&sess).await;
}

#[test_case::test_case(false; "all agents")]
#[test_case::test_case(true; "root only")]
#[tokio::test]
async fn root_only_controls_child_eligibility(root_only: bool) {
    let (_, mut turn) = crate::session::tests::make_session_and_context().await;
    let features = &mut Arc::make_mut(&mut turn.config).features;
    features.enable(Feature::AsyncCompaction).unwrap();
    features
        .set_enabled(Feature::AsyncCompactionRootOnly, root_only)
        .unwrap();

    assert!(enabled(&turn));
    for source in [
        SessionSource::SubAgent(SubAgentSource::Review),
        SessionSource::Internal(InternalSessionSource::Guardian),
    ] {
        turn.session_source = source;
        assert_eq!(enabled(&turn), !root_only);
    }
}
