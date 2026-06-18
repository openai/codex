use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::Ordering;

use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use codex_core::CurrentTimeFuture;
use codex_core::CurrentTimeProvider;
use codex_core::config::VarlatencyConfig;
use codex_features::Feature;
use codex_features::VarlatencyClockSource;
use codex_model_provider_info::built_in_model_providers;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
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

fn current_time_reminders(request: &ResponsesRequest) -> Vec<String> {
    request
        .message_input_texts("developer")
        .into_iter()
        .filter(|text| text.starts_with("It is "))
        .collect()
}

fn enable_varlatency(config: &mut codex_core::config::Config, interval: u64) {
    config
        .features
        .enable(Feature::Varlatency)
        .expect("test config should allow varlatency");
    config.varlatency = Some(VarlatencyConfig {
        reminder_interval_model_requests: interval,
        clock_source: VarlatencyClockSource::AppServerClient,
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn varlatency_reminders_follow_request_interval_and_persist_in_history() -> Result<()> {
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
        .with_config(|config| enable_varlatency(config, /*interval*/ 2))
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
async fn varlatency_reminder_is_refreshed_after_compaction() -> Result<()> {
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
            enable_varlatency(config, /*interval*/ 50);
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
