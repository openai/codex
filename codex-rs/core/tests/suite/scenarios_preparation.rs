//! Verifies prepared context reaches both the first model request and a follow-up.

use super::*;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::protocol::ThreadHistoryMode;
use codex_protocol::turn_input::AnnotatedResponseItem;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn raw_injection_rejects_uploaded_audio_before_modifying_history() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let before = test.codex.conversation_history_snapshot().await;
    let audio: ResponseItem = serde_json::from_value(json!({
        "type": "message", "role": "developer",
        "content": [{"type": "input_audio", "file_id": "file_injected"}]
    }))?;
    let batch = vec![
        core_test_support::responses::user_message_item("must not be partially written"),
        audio.clone(),
    ];

    let history_error = test
        .codex
        .inject_response_items(batch.clone())
        .await
        .unwrap_err();
    let preparation_error = test
        .codex
        .inject_response_items_for_turn(batch)
        .await
        .unwrap_err();
    for error in [history_error, preparation_error] {
        assert!(matches!(
            error.details(),
            CodexErrorDetails::InvalidRequest(_)
        ));
    }
    for input in [
        TurnInput::ResponseItem(audio.clone()),
        TurnInput::AnnotatedResponseItem(AnnotatedResponseItem {
            item: audio,
            annotations: Default::default(),
        }),
    ] {
        let error = test
            .codex
            .start_or_steer_turn(TurnInputRequest::new(input))
            .await
            .unwrap_err();
        assert!(matches!(
            error.details(),
            CodexErrorDetails::InvalidRequest(_)
        ));
    }
    let after = test.codex.conversation_history_snapshot().await;
    assert_eq!(after.history_version(), before.history_version());
    assert_eq!(
        after.items().collect::<Vec<_>>(),
        before.items().collect::<Vec<_>>()
    );
    test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[test_case(ThreadHistoryMode::Legacy; "legacy")]
#[test_case(ThreadHistoryMode::Paginated; "paginated")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prepared_context_reaches_first_request_and_survives_follow_up(
    history_mode: ThreadHistoryMode,
) -> Result<()> {
    const PREPARED_CONTEXT: &str = "Prepared project context: the launch owner is Mira.";

    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let test = test_codex()
        .with_history_mode(history_mode)
        .build_with_auto_env(&server)
        .await?;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("first-response"),
                ev_assistant_message("first-message", "The launch owner is Mira."),
                ev_completed("first-response"),
            ]),
            sse(vec![
                ev_response_created("follow-up-response"),
                ev_assistant_message("follow-up-message", "Mira will send the launch update."),
                ev_completed("follow-up-response"),
            ]),
        ],
    )
    .await;

    // A hosted caller can inject preparation before the user starts the first turn.
    test.codex
        .inject_response_items(vec![serde_json::from_value(json!({
            "type": "message",
            "role": "developer",
            "content": [{
                "type": "input_text",
                "text": PREPARED_CONTEXT
            }],
        }))?])
        .await?;
    assert!(mock.requests().is_empty());

    test.submit_turn("Who owns the launch?").await?;
    test.submit_turn("Who will send the launch update?").await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    for request in requests {
        let prepared_context = request
            .message_input_texts("developer")
            .into_iter()
            .filter(|text| text == PREPARED_CONTEXT)
            .collect::<Vec<_>>();
        assert_eq!(prepared_context, vec![PREPARED_CONTEXT]);
    }
    test.codex.shutdown_and_wait().await?;
    Ok(())
}
