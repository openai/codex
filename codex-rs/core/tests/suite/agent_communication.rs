use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use codex_core::SleepFuture;
use codex_core::TimeFuture;
use codex_core::TimeProvider;
use codex_features::Feature;
use codex_protocol::ThreadId;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tracing::Level;
use tracing_test::internal::MockWriter;
use wiremock::Request;

const ROOT_SIMCLOCK_TIME: i64 = 1_000;
const CHILD_SIMCLOCK_TIME: i64 = 2_000;

#[derive(Default)]
struct ThreadSimClock {
    root_thread_id: Mutex<Option<ThreadId>>,
}

impl ThreadSimClock {
    fn set_root_thread_id(&self, thread_id: ThreadId) {
        *self.root_thread_id.lock().expect("simclock lock") = Some(thread_id);
    }
}

impl TimeProvider for ThreadSimClock {
    fn current_time(&self, thread_id: ThreadId) -> TimeFuture<'_> {
        let root_thread_id = self
            .root_thread_id
            .lock()
            .expect("simclock lock")
            .expect("root thread ID should be installed before reading the clock");
        let timestamp = if thread_id == root_thread_id {
            ROOT_SIMCLOCK_TIME
        } else {
            CHILD_SIMCLOCK_TIME
        };
        Box::pin(async move {
            Ok(DateTime::<Utc>::from_timestamp(timestamp, 0)
                .expect("test simclock timestamp should be valid"))
        })
    }

    fn sleep(&self, _thread_id: ThreadId, _duration: Duration) -> SleepFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn v2_spawn_logs_correlated_send_and_receive_with_simclock_time() -> Result<()> {
    let output: &'static Mutex<Vec<u8>> = Box::leak(Box::new(Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_max_level(Level::TRACE)
        .with_writer(MockWriter::new(output))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let server = start_mock_server().await;
    let spawn_message = "inspect the repository";
    let spawn_args = serde_json::to_string(&serde_json::json!({
        "message": spawn_message,
        "task_name": "worker",
    }))?;
    mount_sse_once_match(
        &server,
        |request: &Request| body_contains(request, "start"),
        sse(vec![
            ev_response_created("resp-parent-1"),
            ev_function_call_with_namespace(
                "spawn-call-1",
                "collaboration",
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed("resp-parent-1"),
        ]),
    )
    .await;
    let child_request = mount_sse_once_match(
        &server,
        |request: &Request| request_has_input_type(request, "agent_message"),
        sse(vec![
            ev_response_created("resp-child-1"),
            ev_completed("resp-child-1"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &Request| {
            body_contains(request, "spawn-call-1")
                && !request_has_input_type(request, "agent_message")
        },
        sse(vec![
            ev_response_created("resp-parent-2"),
            ev_assistant_message("msg-parent-2", "done"),
            ev_completed("resp-parent-2"),
        ]),
    )
    .await;

    let simclock = Arc::new(ThreadSimClock::default());
    let mut builder = test_codex()
        .with_external_time_provider(simclock.clone())
        .with_config(|config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("test config should allow feature update");
            config
                .features
                .disable(Feature::EnableRequestCompression)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;
    let root_thread_id = test.session_configured.thread_id;
    simclock.set_root_thread_id(root_thread_id);
    test.submit_turn("start").await?;
    assert!(!child_request.requests().is_empty());

    let child_thread_id = test
        .thread_manager
        .list_thread_ids()
        .await
        .into_iter()
        .find(|thread_id| *thread_id != root_thread_id)
        .expect("child thread ID");
    let logs = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let logs = String::from_utf8(output.lock().expect("buffer lock").clone())
                .expect("logs should be UTF-8");
            if logs.contains("kind=\"spawn\"") && logs.contains("state=\"receive\"") {
                break logs;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("spawn communication logs should be emitted");

    let send = logs
        .lines()
        .find(|line| line.contains("kind=\"spawn\"") && line.contains("state=\"send\""))
        .expect("spawn send event");
    assert!(send.contains(&format!("sender_thread_id={root_thread_id}")));
    assert!(send.contains(&format!("receiver_thread_id={child_thread_id}")));
    assert!(send.contains(&format!("content=\"{spawn_message}\"")));
    assert!(send.contains(&format!("simclock_time={ROOT_SIMCLOCK_TIME}")));

    let communication_id = log_field(send, "communication_id").expect("communication ID");
    let receive = logs
        .lines()
        .find(|line| {
            line.contains("state=\"receive\"")
                && log_field(line, "communication_id") == Some(communication_id)
        })
        .expect("correlated receive event");
    assert!(receive.contains(&format!("simclock_time={CHILD_SIMCLOCK_TIME}")));

    Ok(())
}

fn body_contains(request: &Request, text: &str) -> bool {
    String::from_utf8_lossy(&request.body).contains(text)
}

fn request_has_input_type(request: &Request, ty: &str) -> bool {
    serde_json::from_slice::<serde_json::Value>(&request.body)
        .ok()
        .and_then(|body| {
            body.get("input")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| item.get("type").and_then(serde_json::Value::as_str) == Some(ty))
        })
}

fn log_field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name}=");
    line.split_ascii_whitespace()
        .find_map(|field| field.strip_prefix(&prefix))
        .map(|value| value.trim_matches('"'))
}
