use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::Utc;
use codex_core::CurrentTimeFuture;
use codex_core::CurrentTimeProvider;
use codex_core::config::CurrentTimeReminderConfig;
use codex_features::CurrentTimeSource;
use codex_features::Feature;
use codex_model_provider_info::built_in_model_providers;
use codex_protocol::ThreadId;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;

const FIRST_REMINDER: &str = "It is 2026-06-17 17:34:15 UTC.";
const SECOND_REMINDER: &str = "It is 2026-06-17 17:35:15 UTC.";
const FIRST_TIME_UNIX_SECONDS: i64 = 1_781_717_655;

struct TestCurrentTimeProvider(AtomicI64);

impl Default for TestCurrentTimeProvider {
    fn default() -> Self {
        Self(AtomicI64::new(FIRST_TIME_UNIX_SECONDS))
    }
}

impl CurrentTimeProvider for TestCurrentTimeProvider {
    fn current_time(&self, _thread_id: ThreadId) -> CurrentTimeFuture<'_> {
        let timestamp = self.0.fetch_add(60, Ordering::Relaxed);
        Box::pin(async move {
            Ok(DateTime::<Utc>::from_timestamp(timestamp, 0)
                .expect("test timestamp should be valid"))
        })
    }
}

struct FailingCurrentTimeProvider;

impl CurrentTimeProvider for FailingCurrentTimeProvider {
    fn current_time(&self, _thread_id: ThreadId) -> CurrentTimeFuture<'_> {
        Box::pin(async { Err(anyhow!("test clock unavailable")) })
    }
}

fn current_time_reminders(request: &ResponsesRequest) -> Vec<String> {
    request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("It is "))
        .collect()
}

fn enable_current_time_reminder(config: &mut codex_core::config::Config, interval: u64) {
    config
        .features
        .enable(Feature::CurrentTimeReminder)
        .expect("test config should allow current-time reminders");
    config.current_time_reminder = Some(CurrentTimeReminderConfig {
        reminder_interval_model_requests: interval,
        clock_source: CurrentTimeSource::External,
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn current_time_reminders_follow_request_interval_and_persist_in_history() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
            sse(vec![ev_response_created("resp-3"), ev_completed("resp-3")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| enable_current_time_reminder(config, /*interval*/ 2))
        .with_current_time_provider(Arc::new(TestCurrentTimeProvider::default()))
        .build(&server)
        .await?;

    test.submit_turn("first turn").await?;
    test.submit_turn("second turn").await?;
    test.submit_turn("third turn").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(current_time_reminders(&requests[0]), vec![FIRST_REMINDER]);
    assert_eq!(current_time_reminders(&requests[1]), vec![FIRST_REMINDER]);
    assert_eq!(
        current_time_reminders(&requests[2]),
        vec![FIRST_REMINDER, SECOND_REMINDER]
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn current_time_reminder_is_refreshed_after_compaction() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
            sse(vec![
                ev_response_created("resp-compact"),
                ev_assistant_message("msg-compact", "compact summary"),
                ev_completed("resp-compact"),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let mut model_provider = built_in_model_providers(/*openai_base_url*/ None)["openai"].clone();
    model_provider.name = "OpenAI-compatible test provider".to_string();
    model_provider.base_url = Some(format!("{}/v1", server.uri()));
    model_provider.supports_websockets = false;
    let test = test_codex()
        .with_config(move |config| {
            config.model_provider = model_provider;
            enable_current_time_reminder(config, /*interval*/ 50);
        })
        .with_current_time_provider(Arc::new(TestCurrentTimeProvider::default()))
        .build(&server)
        .await?;

    test.submit_turn("before compact").await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_turn("after compact").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        current_time_reminders(&requests[2]),
        vec![SECOND_REMINDER],
        "a new context window should force a fresh reminder before the next model request"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn current_time_provider_failure_stops_before_inference() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("unused-response"),
            ev_completed("unused-response"),
        ]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| enable_current_time_reminder(config, /*interval*/ 1))
        .with_current_time_provider(Arc::new(FailingCurrentTimeProvider))
        .build(&server)
        .await?;

    test.codex
        .submit(Op::UserInput {
            items: vec![UserInput::Text {
                text: "fail before inference".into(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            responsesapi_client_metadata: None,
            additional_context: Default::default(),
            thread_settings: Default::default(),
        })
        .await?;

    let EventMsg::Error(error) =
        wait_for_event(&test.codex, |event| matches!(event, EventMsg::Error(_))).await
    else {
        unreachable!();
    };
    assert_eq!(
        error.message,
        "Fatal error: failed to read current time: test clock unavailable"
    );
    assert_eq!(error.codex_error_info, Some(CodexErrorInfo::Other));

    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    assert!(responses.requests().is_empty());

    Ok(())
}
