use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;
use std::time::Duration;

use anyhow::Context;
use codex_core::NotSubmittedReason;
use codex_core::StartIfIdleSubmission;
use codex_core::StartThreadOptions;
use codex_core::TurnInput;
use codex_core::TurnInputRequest;
use codex_core::TurnInputSubmission;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionEventSink;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ExtensionWarning;
use codex_extension_api::NoopExtensionEventSink;
use codex_extension_api::ThreadIdleCause;
use codex_extension_api::ThreadIdleInput;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadResumeInput;
use codex_protocol::ThreadId;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ImageDetail;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use codex_protocol::user_input::UserInput;
use codex_queue_extension::QueueServiceError;
use codex_queue_extension::QueuedItemService;
use codex_state::SqliteConfig;
use codex_state::StateRuntime;
use codex_thread_store::LocalQueueStore;
use codex_thread_store::QueueStore;
use codex_thread_store::ThreadStoreError;
use codex_utils_absolute_path::test_support::PathExt;
use core_test_support::hooks::trust_discovered_hooks;
use core_test_support::responses;
use core_test_support::responses::start_mock_server;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use core_test_support::wait_for_event_with_timeout;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use tokio::sync::oneshot;

const TINY_PNG_BYTES: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0,
    0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2, 0, 0, 5, 0, 1,
    122, 94, 171, 63, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
const TINY_PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR4nGNgAAIAAAUAAXpeqz8AAAAASUVORK5CYII=";

#[derive(Default)]
struct RecordingEventSink {
    events: Mutex<Vec<Event>>,
}

impl ExtensionEventSink for RecordingEventSink {
    fn emit(&self, event: Event) {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event);
    }

    fn emit_warning(&self, _warning: ExtensionWarning) {}
}

#[derive(Default)]
struct InstalledQueue {
    service: OnceLock<Arc<QueuedItemService>>,
    skip_next_idle: Mutex<Option<ThreadId>>,
}

impl ThreadLifecycleContributor<codex_core::config::Config> for InstalledQueue {
    fn on_thread_resume<'a>(&'a self, input: ThreadResumeInput<'a>) -> ExtensionFuture<'a, ()> {
        match self.service.get() {
            Some(service) => <QueuedItemService as ThreadLifecycleContributor<
                codex_core::config::Config,
            >>::on_thread_resume(service.as_ref(), input),
            None => Box::pin(async {}),
        }
    }

    fn on_thread_idle<'a>(&'a self, input: ThreadIdleInput<'a>) -> ExtensionFuture<'a, ()> {
        if self
            .skip_next_idle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take_if(|thread_id| thread_id.to_string() == input.thread_store.level_id())
            .is_some()
        {
            return Box::pin(async {});
        }
        match self.service.get() {
            Some(service) => <QueuedItemService as ThreadLifecycleContributor<
                codex_core::config::Config,
            >>::on_thread_idle(service.as_ref(), input),
            None => Box::pin(async {}),
        }
    }
}

fn registered_queue_extensions() -> (
    Arc<InstalledQueue>,
    Arc<ExtensionRegistry<codex_core::config::Config>>,
) {
    let installed = Arc::new(InstalledQueue::default());
    let mut extensions = ExtensionRegistryBuilder::new();
    extensions.thread_lifecycle_contributor(installed.clone());
    (installed, Arc::new(extensions.build()))
}

fn install_registered_queue(
    test: &TestCodex,
    installed: &InstalledQueue,
) -> anyhow::Result<Arc<QueuedItemService>> {
    let service = Arc::new(QueuedItemService::new(
        loaded_thread_queue(test)?,
        Arc::downgrade(&test.thread_manager),
        Arc::new(NoopExtensionEventSink),
    ));
    assert!(installed.service.set(Arc::clone(&service)).is_ok());
    Ok(service)
}

fn write_rejecting_prompt_hook(home: &Path) {
    let script_path = home.join("queue_prompt_hook.py");
    let log_path = home.join("queue_prompt_hook.log");
    let script = format!(
        r#"import json
from pathlib import Path
import sys

payload = json.load(sys.stdin)
with Path(r"{log_path}").open("a", encoding="utf-8") as log:
    log.write(payload["prompt"] + "\n")
if payload["prompt"] == "blocked":
    print(json.dumps({{"decision": "block", "reason": "blocked by queue hook"}}))
"#,
        log_path = log_path.display(),
    );
    std::fs::write(&script_path, script)
        .unwrap_or_else(|error| panic!("write queue hook script: {error}"));
    let hooks = serde_json::json!({
        "hooks": {
            "UserPromptSubmit": [{
                "hooks": [{
                    "type": "command",
                    "command": format!("python3 {}", script_path.display()),
                }]
            }]
        }
    });
    std::fs::write(home.join("hooks.json"), hooks.to_string())
        .unwrap_or_else(|error| panic!("write queue hooks: {error}"));
}

async fn test_queue() -> anyhow::Result<(Arc<dyn QueueStore>, TempDir)> {
    let home = tempfile::tempdir()?;
    let sqlite = SqliteConfig::new_for_testing(home.path().abs());
    let runtime = StateRuntime::init(sqlite.clone(), "test-provider".to_string()).await?;
    let queue: Arc<dyn QueueStore> = Arc::new(LocalQueueStore::new(runtime));
    Ok((queue, home))
}

fn loaded_thread_queue(test: &TestCodex) -> anyhow::Result<Arc<dyn QueueStore>> {
    let runtime = test.codex.state_db().context("state runtime unavailable")?;
    Ok(Arc::new(LocalQueueStore::new(runtime)))
}

fn user_input(text: &str) -> TurnInput {
    TurnInput::UserInput {
        content: vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
        client_id: None,
    }
}

fn structured_user_input(text: &str) -> TurnInput {
    TurnInput::UserInput {
        content: vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
        client_id: Some("stable-client-message".to_string()),
    }
}

fn user_input_with_media(text: &str, image: UserInput, audio: UserInput) -> TurnInput {
    let mut input = structured_user_input(text);
    if let TurnInput::UserInput { content, .. } = &mut input {
        content.extend([image, audio]);
    }
    input
}

async fn emit_idle(service: &QueuedItemService, thread_id: ThreadId) {
    emit_idle_with_cause(service, thread_id, ThreadIdleCause::Completed).await;
}

async fn emit_idle_with_cause(
    service: &QueuedItemService,
    thread_id: ThreadId,
    cause: ThreadIdleCause,
) {
    let session_store = ExtensionData::new("session");
    let thread_store = ExtensionData::new(thread_id.to_string());
    <QueuedItemService as ThreadLifecycleContributor<()>>::on_thread_idle(
        service,
        ThreadIdleInput {
            cause,
            session_store: &session_store,
            thread_store: &thread_store,
        },
    )
    .await;
}

#[tokio::test]
async fn queued_input_and_unique_event_ids_round_trip() -> anyhow::Result<()> {
    let (queue, _home) = test_queue().await?;
    let sink = Arc::new(RecordingEventSink::default());
    let event_sink: Arc<dyn ExtensionEventSink> = sink.clone();
    let service = QueuedItemService::new(queue, Weak::new(), event_sink);
    let thread_id = ThreadId::new();
    let input = structured_user_input("structured message");

    let first = service.enqueue(thread_id, input.clone()).await?;
    let second = service.enqueue(thread_id, user_input("next")).await?;
    let TurnInput::UserInput {
        client_id: Some(generated_client_id),
        ..
    } = &second.input
    else {
        anyhow::bail!("queued message did not receive a client message id");
    };
    assert_eq!(
        7,
        uuid::Uuid::parse_str(generated_client_id)?.get_version_num()
    );
    assert_eq!(
        vec![first.clone(), second.clone()],
        service.list(thread_id).await?
    );
    assert_eq!(input, first.input);

    {
        let events = sink
            .events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(2, events.len());
        for event in events.iter() {
            let EventMsg::ThreadQueueChanged(change) = &event.msg else {
                anyhow::bail!("event is not a queue change");
            };
            assert_eq!(change.thread_id, thread_id);
        }
        assert_ne!(events[0].id, events[1].id);
    }
    Ok(())
}

#[tokio::test]
async fn editing_reordering_and_deleting_preserve_queue_identity() -> anyhow::Result<()> {
    let (queue, _home) = test_queue().await?;
    let service = QueuedItemService::new(queue, Weak::new(), Arc::new(NoopExtensionEventSink));
    let thread_id = ThreadId::new();
    let first = service
        .enqueue(thread_id, structured_user_input("first"))
        .await?;
    let second = service.enqueue(thread_id, user_input("second")).await?;
    let edited = service
        .update(thread_id, first.id.clone(), user_input("edited"))
        .await?
        .context("queued item missing")?;
    assert_eq!(first.id, edited.id);
    let TurnInput::UserInput { client_id, .. } = &edited.input else {
        anyhow::bail!("edited queue item does not contain user input");
    };
    assert_eq!(Some("stable-client-message"), client_id.as_deref());
    assert!(matches!(
        service.reorder(thread_id, vec![first.id.clone()]).await,
        Err(QueueServiceError::Storage(
            ThreadStoreError::InvalidRequest { .. }
        ))
    ));
    service
        .reorder(thread_id, vec![second.id.clone(), first.id.clone()])
        .await?;
    assert_eq!(
        vec![second.clone(), edited.clone()],
        service.list(thread_id).await?
    );
    assert!(service.delete(thread_id, second.id).await?);
    assert_eq!(vec![edited], service.list(thread_id).await?);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn starting_a_selected_item_preserves_the_remaining_queue() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let response =
        responses::mount_sse_once(&server, responses::sse_completed("selected-turn")).await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let thread_id = test.session_configured.thread_id;
    let queue = loaded_thread_queue(&test)?;
    let service = QueuedItemService::new(queue, Weak::new(), Arc::new(NoopExtensionEventSink));
    let first = service.enqueue(thread_id, user_input("first")).await?;
    let second = service
        .enqueue(thread_id, structured_user_input("second"))
        .await?;

    let submission = service
        .start(
            test.codex.as_ref(),
            Some(second.id.clone()),
            /*trace*/ None,
        )
        .await?;

    assert!(matches!(
        submission,
        StartIfIdleSubmission::Started { turn_id } if !turn_id.is_empty()
    ));
    assert_eq!(vec![first], service.list(thread_id).await?);
    wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::TurnComplete(_) => Some(()),
        _ => None,
    })
    .await;
    assert_eq!(
        Some("second"),
        response
            .single_request()
            .message_input_texts("user")
            .last()
            .map(String::as_str)
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn starting_a_selected_item_while_active_leaves_it_queued() -> anyhow::Result<()> {
    let (release_response, response_gate) = oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![vec![
        StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![responses::ev_response_created("resp-1")]),
        },
        StreamingSseChunk {
            gate: Some(response_gate),
            body: responses::sse(vec![responses::ev_completed("resp-1")]),
        },
    ]])
    .await;
    let test = test_codex().build_with_streaming_server(&server).await?;
    let thread_id = test.session_configured.thread_id;
    let service = QueuedItemService::new(
        loaded_thread_queue(&test)?,
        Weak::new(),
        Arc::new(NoopExtensionEventSink),
    );
    let queued = service
        .enqueue(thread_id, user_input("stay queued"))
        .await?;

    let active_turn = test
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "active turn".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    assert!(matches!(active_turn, TurnInputSubmission::Started { .. }));
    tokio::time::timeout(
        Duration::from_secs(5),
        server.wait_for_request_count(/*count*/ 1),
    )
    .await?;

    let submission = service
        .start(
            test.codex.as_ref(),
            Some(queued.id.clone()),
            /*trace*/ None,
        )
        .await?;
    assert!(matches!(
        submission,
        StartIfIdleSubmission::NotSubmitted {
            reason: NotSubmittedReason::NotIdle
        }
    ));
    assert_eq!(vec![queued], service.list(thread_id).await?);

    release_response
        .send(())
        .expect("active response gate should remain open");
    wait_for_event_match(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupted_turns_pause_queued_messages_but_failed_turns_drain_them() -> anyhow::Result<()>
{
    let server = start_mock_server().await;
    let response =
        responses::mount_sse_once(&server, responses::sse_completed("failed-follow-up")).await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let thread_id = test.session_configured.thread_id;
    let queue = loaded_thread_queue(&test)?;
    let service = QueuedItemService::new(
        queue,
        Arc::downgrade(&test.thread_manager),
        Arc::new(NoopExtensionEventSink),
    );
    let queued = service
        .enqueue(thread_id, user_input("continue after failure"))
        .await?;

    emit_idle_with_cause(&service, thread_id, ThreadIdleCause::Interrupted).await;
    assert_eq!(vec![queued], service.list(thread_id).await?);

    emit_idle_with_cause(&service, thread_id, ThreadIdleCause::Failed).await;
    wait_for_event_match(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;
    assert!(service.list(thread_id).await?.is_empty());
    assert_eq!(
        Some("continue after failure"),
        response
            .single_request()
            .message_input_texts("user")
            .last()
            .map(String::as_str)
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn registered_queue_lifecycle_starts_messages_in_fifo_order() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let responses = responses::mount_sse_sequence(
        &server,
        ["turn-a", "turn-b", "turn-c"]
            .into_iter()
            .map(responses::sse_completed)
            .collect(),
    )
    .await;
    let (installed, extensions) = registered_queue_extensions();
    let test = test_codex()
        .with_extensions(extensions)
        .with_config(|config| config.include_environment_context = false)
        .build_with_auto_env(&server)
        .await?;
    let thread_id = test.session_configured.thread_id;
    let staging = QueuedItemService::new(
        loaded_thread_queue(&test)?,
        Weak::new(),
        Arc::new(NoopExtensionEventSink),
    );
    for prompt in ["B", "C"] {
        staging.enqueue(thread_id, user_input(prompt)).await?;
    }
    let queue = install_registered_queue(&test, installed.as_ref())?;

    tokio::time::timeout(Duration::from_secs(10), async {
        test.submit_text_turn("A").await?;
        for _ in 0..2 {
            wait_for_event_match(test.codex.as_ref(), |event| {
                matches!(event, EventMsg::TurnComplete(_)).then_some(())
            })
            .await;
        }
        anyhow::Ok(())
    })
    .await??;

    let prompts = responses
        .requests()
        .into_iter()
        .map(|request| {
            request
                .message_input_texts("user")
                .pop()
                .context("model request has no user input")
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(vec!["A", "B", "C"], prompts);
    assert!(queue.list(thread_id).await?.is_empty());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn externally_changed_queues_dispatch_independently_and_retry_failed_wakes()
-> anyhow::Result<()> {
    let server = start_mock_server().await;
    let model_responses = responses::mount_sse_sequence(
        &server,
        [
            "independent-queued-turn",
            "external-queued-turn",
            "resumed-queued-turn",
        ]
        .into_iter()
        .map(responses::sse_completed)
        .collect(),
    )
    .await;
    let (installed, extensions) = registered_queue_extensions();
    let test = test_codex()
        .with_extensions(extensions)
        .with_config(|config| config.include_environment_context = false)
        .build_with_auto_env(&server)
        .await?;
    let thread_id = test.session_configured.thread_id;
    let queue = install_registered_queue(&test, installed.as_ref())?;
    let independent_thread = test
        .thread_manager
        .start_thread(StartThreadOptions::new(test.config.clone()))
        .await?;
    let external_runtime = StateRuntime::init(
        test.codex
            .state_db()
            .context("state runtime unavailable")?
            .sqlite()
            .clone(),
        "test-provider".to_string(),
    )
    .await?;
    let external_queue = QueuedItemService::new(
        Arc::new(LocalQueueStore::new(external_runtime)),
        Weak::new(),
        Arc::new(NoopExtensionEventSink),
    );
    let mut watcher_extensions = ExtensionRegistryBuilder::<codex_core::config::Config>::new();
    codex_queue_extension::install(&mut watcher_extensions, Arc::clone(&queue));

    tokio::time::sleep(Duration::from_secs(/*secs*/ 1)).await;
    assert!(model_responses.requests().is_empty());
    *installed
        .skip_next_idle
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(thread_id);
    let first = external_queue
        .enqueue(thread_id, user_input("written by another process"))
        .await?;
    let updated = queue
        .update(
            thread_id,
            first.id,
            user_input("locally edited external message"),
        )
        .await?
        .context("external queue item disappeared")?;
    external_queue
        .enqueue(
            independent_thread.thread_id,
            user_input("independent thread"),
        )
        .await?;

    wait_for_event_with_timeout(
        independent_thread.thread.as_ref(),
        |event| matches!(event, EventMsg::TurnComplete(_)),
        Duration::from_secs(/*secs*/ 25),
    )
    .await;
    assert_eq!(1, model_responses.requests().len());
    assert_eq!(vec![updated], queue.list(thread_id).await?);

    wait_for_event_with_timeout(
        test.codex.as_ref(),
        |event| matches!(event, EventMsg::TurnComplete(_)),
        Duration::from_secs(/*secs*/ 25),
    )
    .await;
    tokio::time::sleep(Duration::from_secs(/*secs*/ 11)).await;

    assert!(queue.list(thread_id).await?.is_empty());
    assert!(queue.list(independent_thread.thread_id).await?.is_empty());

    let rollout_path = test.codex.rollout_path().context("rollout path missing")?;
    test.codex.shutdown_and_wait().await?;
    test.thread_manager.remove_thread(&thread_id).await;
    external_queue
        .enqueue(thread_id, user_input("queued before ordinary resume"))
        .await?;
    tokio::time::sleep(Duration::from_secs(/*secs*/ 11)).await;
    let resumed = test
        .thread_manager
        .resume_thread_from_rollout(
            test.config.clone(),
            rollout_path,
            test.thread_manager.auth_manager(),
            /*parent_trace*/ None,
            Default::default(),
        )
        .await?;
    wait_for_event_with_timeout(
        resumed.thread.as_ref(),
        |event| matches!(event, EventMsg::TurnComplete(_)),
        Duration::from_secs(/*secs*/ 25),
    )
    .await;
    assert!(queue.list(thread_id).await?.is_empty());

    let prompts = model_responses
        .requests()
        .into_iter()
        .filter_map(|request| request.message_input_texts("user").pop())
        .collect::<Vec<_>>();
    assert_eq!(
        vec![
            "independent thread",
            "locally edited external message",
            "queued before ordinary resume",
        ],
        prompts
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_queue_messages_are_consumed_without_retrying_or_blocking_followups()
-> anyhow::Result<()> {
    let server = start_mock_server().await;
    let responses = responses::mount_sse_sequence(
        &server,
        ["initial-turn", "following-turn"]
            .into_iter()
            .map(responses::sse_completed)
            .collect(),
    )
    .await;
    let (installed, extensions) = registered_queue_extensions();
    let test = test_codex()
        .with_extensions(extensions)
        .with_pre_build_hook(write_rejecting_prompt_hook)
        .with_config(trust_discovered_hooks)
        .with_config(|config| config.include_environment_context = false)
        .build_with_auto_env(&server)
        .await?;
    let thread_id = test.session_configured.thread_id;
    let staging = QueuedItemService::new(
        loaded_thread_queue(&test)?,
        Weak::new(),
        Arc::new(NoopExtensionEventSink),
    );
    for prompt in ["blocked", "C"] {
        staging.enqueue(thread_id, user_input(prompt)).await?;
    }
    let queue = install_registered_queue(&test, installed.as_ref())?;

    tokio::time::timeout(Duration::from_secs(10), async {
        test.submit_text_turn("A").await?;
        for _ in 0..2 {
            wait_for_event_match(test.codex.as_ref(), |event| {
                matches!(event, EventMsg::TurnComplete(_)).then_some(())
            })
            .await;
        }
        anyhow::Ok(())
    })
    .await??;

    let prompts = responses
        .requests()
        .into_iter()
        .map(|request| {
            request
                .message_input_texts("user")
                .pop()
                .context("model request has no user input")
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    assert_eq!(vec!["A", "C"], prompts);
    let hook_log = std::fs::read_to_string(test.codex_home_path().join("queue_prompt_hook.log"))?;
    assert_eq!(
        vec!["A", "blocked", "C"],
        hook_log.lines().collect::<Vec<_>>()
    );
    assert!(queue.list(thread_id).await?.is_empty());
    assert_eq!(2, responses.requests().len());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicitly_started_rejected_queue_messages_are_consumed() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let responses =
        responses::mount_sse_once(&server, responses::sse_completed("unexpected-turn")).await;
    let test = test_codex()
        .with_pre_build_hook(write_rejecting_prompt_hook)
        .with_config(trust_discovered_hooks)
        .with_config(|config| config.include_environment_context = false)
        .build_with_auto_env(&server)
        .await?;
    let thread_id = test.session_configured.thread_id;
    let queue = QueuedItemService::new(
        loaded_thread_queue(&test)?,
        Weak::new(),
        Arc::new(NoopExtensionEventSink),
    );

    let rejected = queue.enqueue(thread_id, user_input("blocked")).await?;
    let submission = tokio::time::timeout(
        Duration::from_secs(10),
        queue.start(test.codex.as_ref(), Some(rejected.id), /*trace*/ None),
    )
    .await?
    .expect("explicitly started input should be submitted");
    assert!(matches!(submission, StartIfIdleSubmission::Started { .. }));
    wait_for_event_match(test.codex.as_ref(), |event| {
        matches!(event, EventMsg::TurnComplete(_)).then_some(())
    })
    .await;
    assert!(queue.list(thread_id).await?.is_empty());
    let hook_log = std::fs::read_to_string(test.codex_home_path().join("queue_prompt_hook.log"))?;
    assert_eq!(vec!["blocked"], hook_log.lines().collect::<Vec<_>>());
    assert!(responses.requests().is_empty());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_attachments_are_snapshotted_before_enqueue_and_update() -> anyhow::Result<()> {
    let (queue, home) = test_queue().await?;
    let service = QueuedItemService::new(
        Arc::clone(&queue),
        Weak::new(),
        Arc::new(NoopExtensionEventSink),
    );
    let thread_id = ThreadId::new();
    let image_path = home.path().join("queued.png");
    let audio_path = home.path().join("queued.mp3");
    std::fs::write(&image_path, TINY_PNG_BYTES)?;
    std::fs::write(&audio_path, b"audio")?;
    let queued_input = user_input_with_media(
        "queued attachments",
        UserInput::LocalImage {
            path: image_path.clone(),
            detail: Some(ImageDetail::Original),
        },
        UserInput::LocalAudio {
            path: audio_path.clone(),
        },
    );
    let expected_queued_input = user_input_with_media(
        "queued attachments",
        UserInput::Image {
            image_url: TINY_PNG_DATA_URL.to_string(),
            detail: Some(ImageDetail::Original),
        },
        UserInput::Audio {
            audio_url: "data:audio/mpeg;base64,YXVkaW8=".to_string(),
        },
    );

    let queued = service.enqueue(thread_id, queued_input).await?;
    std::fs::remove_file(&image_path)?;
    std::fs::remove_file(&audio_path)?;

    assert_eq!(expected_queued_input, queued.input);
    assert_eq!(vec![queued.clone()], service.list(thread_id).await?);
    let queued_record = queue
        .list_page(thread_id, /*offset*/ 0, /*limit*/ 1)
        .await?
        .into_iter()
        .next()
        .context("snapshotted queued attachments were not persisted")?;
    let persisted_queued_input: TurnInput = serde_json::from_str(&queued_record.payload)?;
    assert_eq!(expected_queued_input, persisted_queued_input);

    let edited_image_path = home.path().join("edited.png");
    let edited_audio_path = home.path().join("edited.m4a");
    std::fs::write(&edited_image_path, TINY_PNG_BYTES)?;
    std::fs::write(&edited_audio_path, b"edited audio")?;
    let edited_input = user_input_with_media(
        "edited attachments",
        UserInput::LocalImage {
            path: edited_image_path.clone(),
            detail: Some(ImageDetail::High),
        },
        UserInput::LocalAudio {
            path: edited_audio_path.clone(),
        },
    );
    let expected_edited_input = user_input_with_media(
        "edited attachments",
        UserInput::Image {
            image_url: TINY_PNG_DATA_URL.to_string(),
            detail: Some(ImageDetail::High),
        },
        UserInput::Audio {
            audio_url: "data:audio/mp4;base64,ZWRpdGVkIGF1ZGlv".to_string(),
        },
    );

    let edited = service
        .update(thread_id, queued.id.clone(), edited_input)
        .await?
        .context("snapshotted queued attachments were not updated")?;
    std::fs::remove_file(&edited_image_path)?;
    std::fs::remove_file(&edited_audio_path)?;

    assert_eq!(expected_edited_input, edited.input);
    assert_eq!(vec![edited.clone()], service.list(thread_id).await?);
    let edited_record = queue
        .list_page(thread_id, /*offset*/ 0, /*limit*/ 1)
        .await?
        .into_iter()
        .next()
        .context("snapshotted edited attachments were not persisted")?;
    let persisted_edited_input: TurnInput = serde_json::from_str(&edited_record.payload)?;
    assert_eq!(expected_edited_input, persisted_edited_input);

    Ok(())
}

#[tokio::test]
async fn invalid_local_attachments_do_not_mutate_queue() -> anyhow::Result<()> {
    let (queue, home) = test_queue().await?;
    let service = QueuedItemService::new(queue, Weak::new(), Arc::new(NoopExtensionEventSink));
    let thread_id = ThreadId::new();
    let existing = service.enqueue(thread_id, user_input("existing")).await?;
    let missing_path = home.path().join("missing.png");
    let invalid_input = user_input_with_media(
        "invalid attachments",
        UserInput::LocalImage {
            path: missing_path,
            detail: Some(ImageDetail::Original),
        },
        UserInput::Audio {
            audio_url: "data:audio/mpeg;base64,YXVkaW8=".to_string(),
        },
    );

    assert!(matches!(
        service
            .enqueue(thread_id, invalid_input.clone())
            .await,
        Err(QueueServiceError::InvalidAttachment(error))
            if error.kind() == std::io::ErrorKind::NotFound
    ));
    assert!(matches!(
        service
            .update(
                thread_id,
                existing.id.clone(),
                invalid_input,
            )
            .await,
        Err(QueueServiceError::InvalidAttachment(error))
            if error.kind() == std::io::ErrorKind::NotFound
    ));
    assert_eq!(vec![existing], service.list(thread_id).await?);

    Ok(())
}

#[tokio::test]
async fn non_user_input_cannot_enter_the_user_message_queue() -> anyhow::Result<()> {
    let (queue, _home) = test_queue().await?;
    let service = QueuedItemService::new(queue, Weak::new(), Arc::new(NoopExtensionEventSink));
    let error = service
        .enqueue(
            ThreadId::new(),
            TurnInput::ResponseItem(ResponseItem::Other),
        )
        .await
        .expect_err("response item should not enter the user queue");
    assert!(matches!(error, QueueServiceError::InvalidInput));
    Ok(())
}

#[tokio::test]
async fn queued_text_limit_is_enforced_across_all_input_items() -> anyhow::Result<()> {
    let (queue, _home) = test_queue().await?;
    let service = QueuedItemService::new(queue, Weak::new(), Arc::new(NoopExtensionEventSink));
    let thread_id = ThreadId::new();
    let existing = service.enqueue(thread_id, user_input("existing")).await?;
    let mut oversized = user_input(&"x".repeat(MAX_USER_INPUT_TEXT_CHARS / 2));
    if let TurnInput::UserInput { content, .. } = &mut oversized {
        content.push(UserInput::Text {
            text: "y".repeat(MAX_USER_INPUT_TEXT_CHARS / 2 + 1),
            text_elements: Vec::new(),
        });
    }

    assert!(matches!(
        service
            .enqueue(thread_id, oversized.clone())
            .await,
        Err(QueueServiceError::InputTooLarge { actual_chars })
            if actual_chars == MAX_USER_INPUT_TEXT_CHARS + 1
    ));
    assert!(matches!(
        service
            .update(
                thread_id,
                existing.id.clone(),
                oversized,
            )
            .await,
        Err(QueueServiceError::InputTooLarge { actual_chars })
            if actual_chars == MAX_USER_INPUT_TEXT_CHARS + 1
    ));
    assert_eq!(vec![existing], service.list(thread_id).await?);

    Ok(())
}

#[tokio::test]
async fn queued_text_limit_counts_characters_and_ignores_non_text_items() -> anyhow::Result<()> {
    let (queue, _home) = test_queue().await?;
    let service = QueuedItemService::new(queue, Weak::new(), Arc::new(NoopExtensionEventSink));
    let mut input = structured_user_input(&"é".repeat(MAX_USER_INPUT_TEXT_CHARS));
    if let TurnInput::UserInput { content, .. } = &mut input {
        content.push(UserInput::Mention {
            name: "Demo App".to_string(),
            path: "app://demo-app".to_string(),
        });
    }

    let queued = service.enqueue(ThreadId::new(), input.clone()).await?;
    assert_eq!(input, queued.input);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_head_is_skipped_and_a_live_user_turn_is_accepted() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let response =
        responses::mount_sse_once(&server, responses::sse_completed("queued-turn")).await;
    let test = test_codex()
        .with_config(|config| config.include_environment_context = false)
        .build_with_auto_env(&server)
        .await?;
    let thread_id = test.session_configured.thread_id;
    let queue = loaded_thread_queue(&test)?;
    queue
        .enqueue(thread_id, r#"{"unsupported":true}"#.to_string())
        .await?;
    let service = QueuedItemService::new(
        queue,
        Arc::downgrade(&test.thread_manager),
        Arc::new(NoopExtensionEventSink),
    );

    service
        .enqueue(thread_id, structured_user_input("durable follow-up"))
        .await?;
    emit_idle(&service, thread_id).await;
    let client_id = wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::ItemCompleted(event) => match &event.item {
            TurnItem::UserMessage(item) => Some(item.client_id.clone()),
            _ => None,
        },
        _ => None,
    })
    .await;
    assert_eq!(Some("stable-client-message".to_string()), client_id);
    wait_for_event_match(test.codex.as_ref(), |event| match event {
        EventMsg::TurnComplete(_) => Some(()),
        _ => None,
    })
    .await;
    assert!(service.list(thread_id).await?.is_empty());
    let request = response.single_request();
    assert_eq!(
        vec!["durable follow-up"],
        request.message_input_texts("user")
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn resumed_idle_dispatches_input_without_a_loaded_manager() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    responses::mount_sse_once(&server, responses::sse_completed("resumed-turn")).await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let thread_id = test.session_configured.thread_id;
    let queue = loaded_thread_queue(&test)?;
    QueuedItemService::new(
        Arc::clone(&queue),
        Weak::new(),
        Arc::new(NoopExtensionEventSink),
    )
    .enqueue(thread_id, user_input("queued while unloaded"))
    .await?;
    let service = QueuedItemService::new(
        queue,
        Arc::downgrade(&test.thread_manager),
        Arc::new(NoopExtensionEventSink),
    );
    emit_idle(&service, thread_id).await;
    assert!(service.list(thread_id).await?.is_empty());
    Ok(())
}
