use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use anyhow::Result;
use anyhow::anyhow;
use chrono::DateTime;
use chrono::Utc;
use codex_core::TimeFuture;
use codex_core::TimeProvider;
use codex_core::config::CurrentTimeReminderConfig;
use codex_features::CurrentTimeSource;
use codex_features::Feature;
use codex_model_provider_info::built_in_model_providers;
use codex_protocol::ThreadId;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::user_input::UserInput;
use core_test_support::assert_regex_match;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use serde_json::json;

const FIRST_REMINDER: &str = "It is 2026-06-17 17:34:15 UTC.";
const SECOND_REMINDER: &str = "It is 2026-06-17 17:35:15 UTC.";
const THIRD_REMINDER: &str = "It is 2026-06-17 17:36:15 UTC.";
const FIRST_TIME_UNIX_SECONDS: i64 = 1_781_717_655;

struct TestTimeProvider(AtomicI64);

impl Default for TestTimeProvider {
    fn default() -> Self {
        Self(AtomicI64::new(FIRST_TIME_UNIX_SECONDS))
    }
}

impl TimeProvider for TestTimeProvider {
    fn current_time(&self, _thread_id: ThreadId) -> TimeFuture<'_> {
        let timestamp = self.0.fetch_add(60, Ordering::Relaxed);
        Box::pin(async move {
            Ok(DateTime::<Utc>::from_timestamp(timestamp, 0)
                .expect("test timestamp should be valid"))
        })
    }
}

struct FailingTimeProvider;

impl TimeProvider for FailingTimeProvider {
    fn current_time(&self, _thread_id: ThreadId) -> TimeFuture<'_> {
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

fn enable_current_time_reminder(
    config: &mut codex_core::config::Config,
    interval: u64,
    clock_source: CurrentTimeSource,
) {
    config
        .features
        .enable(Feature::CurrentTimeReminder)
        .expect("test config should allow current-time reminders");
    config.current_time_reminder = Some(CurrentTimeReminderConfig {
        reminder_interval_seconds: interval,
        clock_source,
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn current_time_reminders_follow_time_interval_and_persist_in_history() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let tool_args = json!({
        "command": "echo current time",
        "timeout_ms": 1_000,
    });
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    "current-time-tool-call",
                    "shell_command",
                    &serde_json::to_string(&tool_args)?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-2", "done"),
                ev_completed("resp-2"),
            ]),
            sse(vec![ev_response_created("resp-3"), ev_completed("resp-3")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            enable_current_time_reminder(config, /*interval*/ 120, CurrentTimeSource::External)
        })
        .with_external_time_provider(Arc::new(TestTimeProvider::default()))
        .build(&server)
        .await?;

    test.submit_turn_with_permission_profile("first turn", PermissionProfile::Disabled)
        .await?;
    test.submit_turn("second turn").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(current_time_reminders(&requests[0]), vec![FIRST_REMINDER]);
    assert_eq!(current_time_reminders(&requests[1]), vec![FIRST_REMINDER]);
    assert_eq!(
        current_time_reminders(&requests[2]),
        vec![FIRST_REMINDER, THIRD_REMINDER]
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn system_time_source_adds_current_time_reminder() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let responses = mount_sse_once(
        &server,
        sse(vec![ev_response_created("resp-1"), ev_completed("resp-1")]),
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            enable_current_time_reminder(config, /*interval*/ 1, CurrentTimeSource::System)
        })
        .build(&server)
        .await?;

    test.submit_turn("what time is it?").await?;

    let reminders = current_time_reminders(&responses.single_request());
    assert_eq!(reminders.len(), 1);
    assert_regex_match(
        r"^It is \d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} UTC\.$",
        &reminders[0],
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
            enable_current_time_reminder(
                config,
                /*interval*/ 3_000,
                CurrentTimeSource::External,
            );
        })
        .with_external_time_provider(Arc::new(TestTimeProvider::default()))
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
async fn time_provider_failure_stops_before_inference() -> Result<()> {
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
        .with_config(|config| {
            enable_current_time_reminder(config, /*interval*/ 1, CurrentTimeSource::External)
        })
        .with_external_time_provider(Arc::new(FailingTimeProvider))
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn current_time_tool_returns_the_latest_time() -> Result<()> {
    skip_if_no_network!(Ok(()));

    const CALL_ID: &str = "current-time";

    let server = start_mock_server().await;
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call_with_namespace(CALL_ID, "clock", "curr_time", "{}"),
                ev_completed("resp-1"),
            ]),
            sse(vec![ev_response_created("resp-2"), ev_completed("resp-2")]),
        ],
    )
    .await;
    let test = test_codex()
        .with_config(|config| {
            enable_current_time_reminder(
                config,
                /*interval*/ 3_000,
                CurrentTimeSource::External,
            )
        })
        .with_external_time_provider(Arc::new(TestTimeProvider::default()))
        .build(&server)
        .await?;

    test.submit_turn("check the current time").await?;

    let requests = responses.requests();
    assert!(
        requests[0].tool_by_name("clock", "curr_time").is_some(),
        "clock.curr_time should be exposed when current-time reminders are enabled"
    );
    assert_eq!(
        requests[1].function_call_output_text(CALL_ID),
        Some(SECOND_REMINDER.to_string())
    );

    Ok(())
}
