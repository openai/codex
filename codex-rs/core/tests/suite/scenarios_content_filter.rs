//! Content-filter retries retain delivered output and append catalog guidance after each block.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn content_filter_guidance_is_appended_after_each_block() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex()
        .with_config(|config| {
            configure_scenario_catalog(config);
            config.model_provider.stream_max_retries = Some(2);
        })
        .with_model_info_override("gpt-5.5", |model| {
            model
                .model_messages
                .get_or_insert_default()
                .content_filter_guidance = Some(
                "Your previous response was blocked. Offer a permitted alternative.".to_string(),
            );
        })
        .build_with_auto_env(&server)
        .await?;
    let blocked = json!({
        "type": "response.incomplete",
        "response": { "incomplete_details": { "reason": "content_filter" } }
    });
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("blocked-response"),
                ev_assistant_message("partial-message", "I can help with this topic."),
                blocked.clone(),
            ]),
            sse(vec![ev_response_created("blocked-again-response"), blocked]),
            sse(vec![
                ev_response_created("recovery-response"),
                ev_assistant_message(
                    "alternative-message",
                    "I can help with a permitted alternative.",
                ),
                ev_completed("recovery-response"),
            ]),
        ],
    )
    .await;

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Help me with this topic.".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_event(&test.codex, |event| {
        assert!(!matches!(event, EventMsg::Error(_)), "{event:?}");
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    insta::assert_snapshot!(
        "content_filter_guidance",
        context_snapshot::format_request_history_snapshot(
            "Repeated content-filter blocks append catalog guidance and retain delivered output.",
            &requests,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    Ok(())
}
