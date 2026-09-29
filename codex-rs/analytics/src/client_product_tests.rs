//! Verify product attribution remains per thread across shared queues and HTTP batches.
//! Reconfiguration preserves buffered attribution; queue overflow falls back to unknown.

use super::AnalyticsEventsClient;
use super::AnalyticsEventsDestination;
use super::AnalyticsEventsQueue;
use super::sample_regular_track_event;
use super::sample_thread_start_response;
use super::sample_turn_start_response;
use super::send_track_events;
use crate::ThreadProductUpdate;
use crate::events::AppServerRpcTransport;
use crate::facts::PluginMeasurementRow;
use crate::facts::PluginMeasurementsInput;
use crate::product_attribution::ThreadProducts;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::CollabAgentTool;
use codex_app_server_protocol::CollabAgentToolCallStatus;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::RequestId;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::TurnStartedNotification;
use codex_login::AuthManager;
use codex_protocol::ThreadId;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

enum Responses {
    Immediate,
    PauseAfter(usize),
    Together,
}

type CapturedRequest = (Option<String>, serde_json::Value);

#[test]
fn product_registry_clears_unset_threads_and_bounds_retention() {
    let mut products = ThreadProducts::default();
    products.register("unset".to_string(), Some("aeon".to_string()));
    products.register("unset".to_string(), /*product*/ None);
    for index in 0..=4096 {
        products.register(format!("thread-{index}"), Some("tpp".to_string()));
    }
    let batches = crate::product_attribution::product_event_batches(
        vec![
            sample_regular_track_event("unset"),
            sample_regular_track_event("thread-0"),
            sample_regular_track_event("thread-4096"),
        ],
        &products,
    );
    assert_eq!(
        batches
            .into_iter()
            .map(|(product, events)| (product, events.len()))
            .collect::<Vec<_>>(),
        vec![(None, 2), (Some("tpp"), 1)]
    );
    for index in 0..17 {
        products.register(format!("thread-{index}"), Some(format!("product-{index}")));
    }
    let events = (0..17)
        .map(|index| sample_regular_track_event(&format!("thread-{index}")))
        .collect::<Vec<_>>();
    let expected = serde_json::json!([[null, &events]]);
    assert_eq!(
        serde_json::to_value(crate::product_attribution::product_event_batches(
            events, &products
        ))
        .unwrap(),
        expected
    );
}

async fn capture_requests(
    count: usize,
    responses: Responses,
) -> (
    AnalyticsEventsDestination,
    tokio::task::JoinHandle<Vec<CapturedRequest>>,
    tokio::sync::oneshot::Receiver<()>,
    tokio::sync::oneshot::Sender<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let destination = AnalyticsEventsDestination::Http {
        url: format!("http://{}/events", listener.local_addr().unwrap()),
    };
    let (paused_tx, paused_rx) = tokio::sync::oneshot::channel();
    let (resume_tx, resume_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut pause = Some((paused_tx, resume_rx));
        let mut requests = Vec::new();
        let mut pending = Vec::new();
        for _ in 0..count {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let (headers, body) = loop {
                let mut buffer = [0; 4096];
                let count = stream.read(&mut buffer).await.unwrap();
                assert!(count > 0);
                request.extend_from_slice(&buffer[..count]);
                if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break (
                            headers,
                            serde_json::from_slice(&request[end + 4..end + 4 + length]).unwrap(),
                        );
                    }
                }
            };
            let product = headers
                .lines()
                .find_map(|line| line.strip_prefix("x-openai-product-sku: "))
                .map(str::to_string);
            requests.push((product, body));
            if matches!(responses, Responses::PauseAfter(count) if count == requests.len()) {
                let (paused, resume) = pause.take().unwrap();
                paused.send(()).unwrap();
                resume.await.unwrap();
            }
            pending.push(stream);
            if matches!(responses, Responses::Together) && requests.len() < count {
                continue;
            }
            for mut stream in pending.drain(..) {
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await
                    .unwrap();
            }
        }
        requests
    });
    (destination, server, paused_rx, resume_tx)
}

#[tokio::test]
async fn product_streams_send_concurrently_and_preserve_event_order() {
    // Withhold every response until all 16 requests arrive: serial delivery would time out.
    let (destination, server, _, _) = capture_requests(/*count*/ 16, Responses::Together).await;
    let manager = AuthManager::from_auth_for_testing(
        codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    );
    let mut products = ThreadProducts::default();
    for index in 0..16 {
        products.register(format!("thread-{index}"), Some(format!("product-{index}")));
    }
    let event = |index| {
        let mut event = sample_regular_track_event(&format!("thread-{}", index % 16));
        if let crate::events::TrackEventRequest::SkillInvocation(skill) = &mut event {
            skill.event_params.turn_id = Some(format!("turn-{index}"));
        }
        event
    };
    let events = (0..32).map(event).collect();
    tokio::time::timeout(
        std::time::Duration::from_secs(15),
        send_track_events(&manager, &destination, events, &products),
    )
    .await
    .unwrap();
    let mut requests = server.await.unwrap();
    requests.sort_by(|a, b| a.0.cmp(&b.0));
    let mut expected = (0..16)
        .map(|index| {
            (
                Some(format!("product-{index}")),
                serde_json::json!({"events": [event(index), event(index + 16)]}),
            )
        })
        .collect::<Vec<_>>();
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(requests, expected);
}

#[tokio::test]
async fn buffered_tool_events_preserve_attribution_or_drop_it_on_queue_overflow() {
    for (new_product, overflow) in [
        (Some("tpp"), false),
        (None, false),
        (Some("tpp"), true),
        (None, true),
    ] {
        let (destination, server, paused, resume) = capture_requests(
            if overflow { 5 } else { 2 },
            if overflow {
                Responses::PauseAfter(2)
            } else {
                Responses::Immediate
            },
        )
        .await;
        let manager = AuthManager::from_auth_for_testing(
            codex_login::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        );
        let client = AnalyticsEventsClient {
            queue: Some(AnalyticsEventsQueue::new(manager, destination)),
        };
        let thread_id = ThreadId::new();
        client.update_thread_product_sku(thread_id, ThreadProductUpdate::Set("aeon".to_string()));
        client.track_initialize(
            /*connection_id*/ 1,
            InitializeParams {
                client_info: ClientInfo {
                    name: "test".to_string(),
                    title: None,
                    version: "1".to_string(),
                },
                capabilities: None,
            },
            "test_client".to_string(),
            AppServerRpcTransport::InProcess,
        );
        let mut response = sample_thread_start_response();
        let ClientResponsePayload::ThreadStart(started) = &mut response else {
            unreachable!()
        };
        started.thread.id = thread_id.to_string();
        client.track_response(/*connection_id*/ 1, RequestId::Integer(1), &response);
        let ClientResponsePayload::TurnStart(started) = sample_turn_start_response() else {
            unreachable!()
        };
        client.track_notification(&ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: thread_id.to_string(),
            turn: started.turn,
        }));
        // Collaborator completions wait in the reducer for subsequent sampling evidence.
        let completed = ItemCompletedNotification {
            thread_id: thread_id.to_string(),
            turn_id: "turn-1".to_string(),
            completed_at_ms: 2,
            item: ThreadItem::CollabAgentToolCall {
                id: "item".to_string(),
                tool: CollabAgentTool::SendMessage,
                status: CollabAgentToolCallStatus::Failed,
                sender_thread_id: thread_id.to_string(),
                receiver_thread_ids: Vec::new(),
                prompt: None,
                model: None,
                reasoning_effort: None,
                agents_states: Default::default(),
            },
        };
        client.track_notification(&ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: thread_id.to_string(),
            turn_id: "turn-1".to_string(),
            started_at_ms: 1,
            item: completed.item.clone(),
        }));
        client.track_notification(&ServerNotification::ItemCompleted(completed));
        let measurement = PluginMeasurementsInput {
            thread_id: thread_id.to_string(),
            turn_id: "turn-1".to_string(),
            item_id: "measurement".to_string(),
            originator: "test_client".to_string(),
            model_slug: None,
            reasoning_effort: None,
            plugin_id: "sites@test".to_string(),
            execution_id: "execution".to_string(),
            operation: "build".to_string(),
            rows: vec![PluginMeasurementRow {
                measurement_name: "duration_ms".to_string(),
                number_value: 12.0,
                dimensions: BTreeMap::new(),
            }],
        };
        if overflow {
            client.track_plugin_measurements(measurement.clone());
            paused.await.unwrap();
            // Hold HTTP delivery while stale registrations fill the shared queue.
            let capacity = client.queue.as_ref().unwrap().sender.capacity();
            for _ in 0..capacity {
                client.update_thread_product_sku(
                    thread_id,
                    ThreadProductUpdate::Set("aeon".to_string()),
                );
            }
        }
        client.update_thread_product_sku(
            thread_id,
            new_product.map_or(ThreadProductUpdate::Clear, |value| {
                ThreadProductUpdate::Set(value.to_string())
            }),
        );
        if overflow {
            resume.send(()).unwrap();
        }
        client.flush().await;
        if overflow {
            client.track_plugin_measurements(measurement.clone());
            client.flush().await;
            client
                .update_thread_product_sku(thread_id, ThreadProductUpdate::Set("tpp".to_string()));
            client.track_plugin_measurements(measurement);
            client.flush().await;
        }
        let requests = tokio::time::timeout(std::time::Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap();
        let expected = if overflow {
            vec![
                (Some("aeon"), "codex_thread_initialized"),
                (Some("aeon"), "codex_plugin_measurement_event"),
                (None, "codex_collab_agent_tool_call_event"),
                (None, "codex_plugin_measurement_event"),
                (Some("tpp"), "codex_plugin_measurement_event"),
            ]
        } else {
            vec![
                (Some("aeon"), "codex_thread_initialized"),
                (Some("aeon"), "codex_collab_agent_tool_call_event"),
            ]
        }
        .into_iter()
        .map(|(product, event)| (product.map(str::to_string), event.to_string()))
        .collect::<Vec<_>>();
        assert_eq!(
            requests
                .into_iter()
                .map(|(product, body)| (
                    product,
                    body["events"][0]["event_type"]
                        .as_str()
                        .unwrap()
                        .to_string(),
                ))
                .collect::<Vec<_>>(),
            expected
        );
    }
}
