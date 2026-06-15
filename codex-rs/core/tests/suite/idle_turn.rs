use anyhow::Result;
use codex_core::IdleTurnInput;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::UserSubmission;
use codex_protocol::user_input::UserInput;
use core_test_support::responses;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn idle_user_submission_starts_a_model_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let request =
        responses::mount_sse_once(&server, responses::sse_completed("queued turn complete")).await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build(&server)
        .await?;
    let thread = test
        .thread_manager
        .get_thread(test.session_configured.thread_id)
        .await?;

    thread
        .try_start_turn_if_idle(IdleTurnInput::UserSubmission(UserSubmission {
            items: vec![UserInput::Text {
                text: "durable follow-up".to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
        }))
        .await
        .expect("idle user submission should start");

    wait_for_event_match(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;
    assert_eq!(
        request.single_request().message_input_texts("user"),
        vec!["durable follow-up"]
    );
    Ok(())
}
