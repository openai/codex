//! Verifies prepared context reaches both the first model request and a follow-up.

use super::*;
use codex_protocol::protocol::ThreadHistoryMode;
use pretty_assertions::assert_eq;
use test_case::test_case;

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
