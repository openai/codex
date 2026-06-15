use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Weak;

use codex_extension_api::ExtensionData;
use codex_extension_api::NoopExtensionEventSink;
use codex_extension_api::ThreadIdleInput;
use codex_extension_api::ThreadLifecycleContributor;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::UserSubmission;
use codex_protocol::user_input::UserInput;
use codex_queue_extension::QueuedItemProvenance;
use codex_queue_extension::QueuedItemService;
use core_test_support::responses;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;

async fn service() -> anyhow::Result<QueuedItemService> {
    let runtime =
        codex_state::StateRuntime::init(tempfile::tempdir()?.keep(), "test-provider".to_string())
            .await?;
    Ok(QueuedItemService::new(
        runtime,
        Weak::new(),
        Arc::new(NoopExtensionEventSink),
    ))
}

fn submission(text: &str) -> UserSubmission {
    UserSubmission {
        items: vec![UserInput::Text {
            text: text.to_string(),
            text_elements: Vec::new(),
        }],
        ..Default::default()
    }
}

#[tokio::test]
async fn service_round_trips_external_provenance_metadata() -> anyhow::Result<()> {
    let service = service().await?;
    let thread_id = ThreadId::new();
    let provenance = QueuedItemProvenance::ExternalEvent {
        source: "slack".to_string(),
        metadata: HashMap::from([("channel".to_string(), json!("C123"))]),
    };

    let added = service
        .enqueue(thread_id, submission("hello"), provenance.clone())
        .await?;
    let listed = service.list(thread_id, /*offset*/ 0, /*limit*/ 10).await?;

    assert_eq!(added, listed[0]);
    assert_eq!(provenance, listed[0].provenance);
    Ok(())
}

#[tokio::test]
async fn delete_and_reorder_preserve_visible_order() -> anyhow::Result<()> {
    let service = service().await?;
    let thread_id = ThreadId::new();
    let first = service
        .enqueue(thread_id, submission("first"), QueuedItemProvenance::User)
        .await?;
    let second = service
        .enqueue(thread_id, submission("second"), QueuedItemProvenance::User)
        .await?;

    service
        .reorder(thread_id, &[second.id.clone(), first.id.clone()])
        .await?;
    let reordered = service.list(thread_id, /*offset*/ 0, /*limit*/ 10).await?;
    assert_eq!(
        vec![second.id.clone(), first.id.clone()],
        reordered
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>()
    );
    assert!(service.delete(thread_id, &second.id).await?);
    assert_eq!(
        vec![first],
        service.list(thread_id, /*offset*/ 0, /*limit*/ 10).await?
    );
    Ok(())
}

#[test]
fn dispatch_starts_a_live_core_turn_and_completes_the_claim() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_stack_size(16 * 1024 * 1024)
        .enable_all()
        .build()?
        .block_on(async {
            let server = start_mock_server().await;
            let response =
                responses::mount_sse_once(&server, responses::sse_completed("queued-turn")).await;
            let test = test_codex()
                .with_config(|config| config.include_environment_context = false)
                .build(&server)
                .await?;
            let thread_id = test.session_configured.thread_id;
            let state_db = test.codex.state_db().expect("state db");
            let service = QueuedItemService::new(
                state_db,
                Arc::downgrade(&test.thread_manager),
                Arc::new(NoopExtensionEventSink),
            );

            service
                .enqueue(
                    thread_id,
                    submission("durable follow-up"),
                    QueuedItemProvenance::User,
                )
                .await?;
            let session_store = ExtensionData::new("session");
            let thread_store = ExtensionData::new(thread_id.to_string());
            <QueuedItemService as ThreadLifecycleContributor<()>>::on_thread_idle(
                &service,
                ThreadIdleInput {
                    session_store: &session_store,
                    thread_store: &thread_store,
                },
            )
            .await;
            assert!(
                service
                    .list(thread_id, /*offset*/ 0, /*limit*/ 10)
                    .await?
                    .is_empty()
            );

            wait_for_event_match(test.codex.as_ref(), |event| match event {
                EventMsg::TurnComplete(event) => Some(event.turn_id.clone()),
                _ => None,
            })
            .await;
            assert_eq!(
                response.single_request().message_input_texts("user"),
                vec!["durable follow-up"]
            );
            Ok::<(), anyhow::Error>(())
        })
}

#[test]
fn post_resume_idle_dispatches_an_item_queued_while_the_thread_was_unloaded() -> anyhow::Result<()>
{
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_stack_size(16 * 1024 * 1024)
        .enable_all()
        .build()?
        .block_on(async {
            let server = start_mock_server().await;
            responses::mount_sse_once(&server, responses::sse_completed("resumed-turn")).await;
            let test = test_codex().build(&server).await?;
            let thread_id = test.session_configured.thread_id;
            let state_db = test.codex.state_db().expect("state db");
            QueuedItemService::new(
                state_db.clone(),
                Weak::new(),
                Arc::new(NoopExtensionEventSink),
            )
            .enqueue(
                thread_id,
                submission("queued while unloaded"),
                QueuedItemProvenance::User,
            )
            .await?;
            let service = QueuedItemService::new(
                state_db,
                Arc::downgrade(&test.thread_manager),
                Arc::new(NoopExtensionEventSink),
            );
            let session_store = ExtensionData::new("session");
            let thread_store = ExtensionData::new(thread_id.to_string());

            <QueuedItemService as ThreadLifecycleContributor<()>>::on_thread_idle(
                &service,
                ThreadIdleInput {
                    session_store: &session_store,
                    thread_store: &thread_store,
                },
            )
            .await;

            assert!(
                service
                    .list(thread_id, /*offset*/ 0, /*limit*/ 10)
                    .await?
                    .is_empty()
            );
            Ok::<(), anyhow::Error>(())
        })
}

#[test]
fn invalid_payload_is_discarded_instead_of_blocking_the_queue() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_stack_size(16 * 1024 * 1024)
        .enable_all()
        .build()?
        .block_on(async {
            let server = start_mock_server().await;
            let response =
                responses::mount_sse_once(&server, responses::sse_completed("valid-turn")).await;
            let test = test_codex()
                .with_config(|config| config.include_environment_context = false)
                .build(&server)
                .await?;
            let thread_id = test.session_configured.thread_id;
            let state_db = test.codex.state_db().expect("state db");
            state_db
                .thread_queue()
                .enqueue(thread_id, br#"{"unsupported":true}"#)
                .await?;
            let service = QueuedItemService::new(
                state_db.clone(),
                Arc::downgrade(&test.thread_manager),
                Arc::new(NoopExtensionEventSink),
            );
            service
                .enqueue(
                    thread_id,
                    submission("valid follow-up"),
                    QueuedItemProvenance::User,
                )
                .await?;
            let session_store = ExtensionData::new("session");
            let thread_store = ExtensionData::new(thread_id.to_string());

            <QueuedItemService as ThreadLifecycleContributor<()>>::on_thread_idle(
                &service,
                ThreadIdleInput {
                    session_store: &session_store,
                    thread_store: &thread_store,
                },
            )
            .await;

            assert!(
                state_db
                    .thread_queue()
                    .list_page(thread_id, /*offset*/ 0, /*limit*/ 1)
                    .await?
                    .is_empty()
            );
            wait_for_event_match(test.codex.as_ref(), |event| match event {
                EventMsg::TurnComplete(event) => Some(event.turn_id.clone()),
                _ => None,
            })
            .await;
            assert_eq!(
                response.single_request().message_input_texts("user"),
                vec!["valid follow-up"]
            );
            Ok::<(), anyhow::Error>(())
        })
}
