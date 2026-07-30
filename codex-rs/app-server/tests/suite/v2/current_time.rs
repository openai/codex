use anyhow::Result;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::create_final_assistant_message_sse_response;
use chrono::DateTime;
use chrono::Local;
use chrono::Utc;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::CurrentTimeReadResponse;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadStartParams;
use codex_app_server_protocol::ThreadStartResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStartResponse;
use codex_app_server_protocol::UserInput;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::time::Duration;
use tokio::time::timeout;

#[cfg(windows)]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(25);
#[cfg(not(windows))]
const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(10);
const CURRENT_TIME_AT: i64 = 1_781_717_655;
const CURRENT_TIME_REMINDER: &str = "It is 2026-06-17 17:34:15 UTC.";

#[tokio::test]
async fn current_time_read_round_trip_adds_reminder_to_model_input() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        create_final_assistant_message_sse_response("Done")?,
    )
    .await;
    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&server.uri())
        .with_extra_config(
            r#"[features.current_time_reminder]
enabled = true
reminder_interval_seconds = 1
clock_source = "external"
"#,
        )
        .write(codex_home.path())?;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .build_initialized()
        .await?;

    let ThreadStartResponse { thread, .. } = app_server
        .start_thread(ThreadStartParams::default())
        .await?;

    let _: TurnStartResponse = app_server
        .request(|request_id| ClientRequest::TurnStart {
            request_id,
            params: TurnStartParams {
                thread_id: thread.id.clone(),
                input: vec![UserInput::Text {
                    text: "What time is it?".to_string(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;

    let mut current_time_reads = 0;
    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            match app_server.read_next_message().await? {
                JSONRPCMessage::Request(request) => {
                    let server_request = ServerRequest::try_from(request)?;
                    let ServerRequest::CurrentTimeRead { request_id, params } = server_request
                    else {
                        panic!("expected CurrentTimeRead request, got: {server_request:?}");
                    };
                    assert_eq!(params.thread_id, thread.id);
                    current_time_reads += 1;
                    app_server
                        .send_response(
                            request_id,
                            serde_json::to_value(CurrentTimeReadResponse {
                                current_time_at: CURRENT_TIME_AT,
                            })?,
                        )
                        .await?;
                }
                JSONRPCMessage::Notification(notification)
                    if notification.method == "turn/completed" =>
                {
                    break Ok::<_, anyhow::Error>(());
                }
                _ => {}
            }
        }
    })
    .await??;
    assert!(current_time_reads >= 2);

    let request = response_mock.single_request();
    assert!(
        request
            .message_input_texts("developer")
            .iter()
            .any(|text| text == CURRENT_TIME_REMINDER)
    );
    let current_date = DateTime::<Utc>::from_timestamp(CURRENT_TIME_AT, 0)
        .expect("test timestamp should be valid")
        .with_timezone(&Local)
        .format("%Y-%m-%d")
        .to_string();
    assert!(request.message_input_texts("user").iter().any(|text| {
        text.contains("<environment_context>")
            && text.contains(&format!("<current_date>{current_date}</current_date>"))
    }));
    Ok(())
}
