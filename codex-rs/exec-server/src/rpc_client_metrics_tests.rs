//! Exercise caller-side counters through the RPC transport, including failures and cancellation.

use std::collections::BTreeMap;
use std::time::Duration;

use codex_otel::MetricsClient;
use codex_otel::MetricsConfig;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::MetricData;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::sync::watch;

use super::RpcCallError;
use super::RpcClient;
use super::RpcClientEvent;
use crate::connection::JsonRpcConnection;
use crate::connection::JsonRpcConnectionEvent;
use crate::connection::JsonRpcTransport;
use crate::protocol::FS_READ_FILE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::JSONRPCMessage;
use crate::protocol::JSONRPCResponse;
use crate::protocol::RequestId;

struct Harness {
    client: RpcClient,
    metrics: MetricsClient,
    outgoing: mpsc::Receiver<JSONRPCMessage>,
    incoming: mpsc::Sender<JsonRpcConnectionEvent>,
    _events: mpsc::Receiver<RpcClientEvent>,
    _disconnected: watch::Sender<bool>,
}

impl Harness {
    fn new() -> Self {
        let metrics = MetricsClient::new(
            MetricsConfig::in_memory(
                "test",
                "exec-server-client-test",
                env!("CARGO_PKG_VERSION"),
                InMemoryMetricExporter::default(),
            )
            .with_runtime_reader(),
        )
        .expect("metrics client");
        let (outgoing_tx, outgoing) = mpsc::channel(/*buffer*/ 8);
        let (incoming, incoming_rx) = mpsc::channel(/*buffer*/ 8);
        let (disconnected, disconnected_rx) = watch::channel(/*init*/ false);
        let (mut client, events) = RpcClient::new(JsonRpcConnection {
            outgoing_tx,
            incoming_rx,
            disconnected_rx,
            task_handles: Vec::new(),
            transport: JsonRpcTransport::Plain,
        });
        client.metrics = Some(metrics.clone());
        Self {
            client,
            metrics,
            outgoing,
            incoming,
            _events: events,
            _disconnected: disconnected,
        }
    }

    fn counts(&self) -> BTreeMap<String, u64> {
        let snapshot = self.metrics.snapshot().expect("metrics snapshot");
        let mut counts = BTreeMap::new();
        for metric in snapshot
            .scope_metrics()
            .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
            .filter(|metric| metric.name() == "exec_server_client_requests_total")
        {
            let AggregatedMetrics::U64(MetricData::Sum(sum)) = metric.data() else {
                panic!("client request count should be a u64 sum");
            };
            for point in sum.data_points() {
                let attributes = point
                    .attributes()
                    .map(|attribute| {
                        (
                            attribute.key.as_str(),
                            attribute.value.as_str().into_owned(),
                        )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(attributes.len(), 1, "only the method is labeled");
                assert_eq!(attributes[0].0, "method");
                *counts.entry(attributes[0].1.clone()).or_default() += point.value();
            }
        }
        counts
    }
}

#[derive(Clone, Copy)]
enum CallKind {
    Regular,
    Untraced,
    WithTimeout,
    Cleanup,
}

#[tokio::test]
async fn each_request_entry_point_counts_once_and_preserves_response() {
    for kind in [
        CallKind::Regular,
        CallKind::Untraced,
        CallKind::WithTimeout,
        CallKind::Cleanup,
    ] {
        let mut harness = Harness::new();
        let params = serde_json::json!({"path": "/sensitive-test-path"});
        let request = async {
            match kind {
                CallKind::Regular => {
                    harness
                        .client
                        .call::<_, Value>(FS_READ_FILE_METHOD, &params)
                        .await
                }
                CallKind::Untraced => {
                    harness
                        .client
                        .call_untraced::<_, Value>(FS_READ_FILE_METHOD, &params)
                        .await
                }
                CallKind::WithTimeout => {
                    harness
                        .client
                        .call_with_timeout::<_, Value>(
                            FS_READ_FILE_METHOD,
                            &params,
                            Duration::from_secs(1),
                        )
                        .await
                }
                CallKind::Cleanup => {
                    harness
                        .client
                        .call_for_cleanup::<_, Value>(FS_READ_FILE_METHOD, &params)
                        .await
                }
            }
        };
        let server = async {
            let Some(JSONRPCMessage::Request(request)) = harness.outgoing.recv().await else {
                panic!("expected request");
            };
            assert_eq!(request.method, FS_READ_FILE_METHOD);
            assert_eq!(request.params, Some(params.clone()));
            harness
                .incoming
                .send(JsonRpcConnectionEvent::Message(JSONRPCMessage::Response(
                    JSONRPCResponse {
                        id: request.id,
                        result: serde_json::json!({"ok": true}),
                    },
                )))
                .await
                .expect("response accepted");
        };
        let (response, ()) = tokio::join!(request, server);
        assert_eq!(
            response.expect("RPC response"),
            serde_json::json!({"ok": true})
        );
        assert_eq!(
            harness.counts(),
            BTreeMap::from([(FS_READ_FILE_METHOD.to_string(), 1)])
        );
    }
}

#[tokio::test]
async fn local_rejections_and_closed_transport_count_as_attempts() {
    let harness = Harness::new();
    let slots = harness
        .client
        .shared_call_slots
        .acquire_many(super::MAX_IN_FLIGHT_REGULAR_CALLS as u32)
        .await
        .expect("occupy slots");
    let rejected = harness
        .client
        .call::<_, Value>(FS_READ_FILE_METHOD, &())
        .await;
    assert!(matches!(
        rejected,
        Err(RpcCallError::PendingRequestLimitExceeded { .. })
    ));
    drop(slots);
    harness.client.close_transport().await;
    let closed = harness
        .client
        .call::<_, Value>(FS_READ_FILE_METHOD, &())
        .await;
    assert!(matches!(closed, Err(RpcCallError::Closed)));
    assert_eq!(
        harness.counts(),
        BTreeMap::from([(FS_READ_FILE_METHOD.to_string(), 2)])
    );
}

#[tokio::test(start_paused = true)]
async fn timeout_and_cancellation_count_as_attempts() {
    let mut harness = Harness::new();
    let timed_out = harness
        .client
        .call_with_timeout::<_, Value>(FS_READ_FILE_METHOD, &(), Duration::from_secs(1))
        .await;
    assert!(matches!(timed_out, Err(RpcCallError::TimedOut { .. })));
    harness.outgoing.recv().await.expect("timed out request");
    let mut cancelled = Box::pin(harness.client.call::<_, Value>(FS_READ_FILE_METHOD, &()));
    assert!(futures::poll!(cancelled.as_mut()).is_pending());
    harness.outgoing.recv().await.expect("cancelled request");
    drop(cancelled);
    assert_eq!(
        harness.counts(),
        BTreeMap::from([(FS_READ_FILE_METHOD.to_string(), 2)])
    );
}

#[tokio::test]
async fn notifications_responses_and_disabled_metrics_do_not_record_attempts() {
    let mut harness = Harness::new();
    harness
        .client
        .notify(INITIALIZED_METHOD, &())
        .await
        .expect("notification");
    harness
        .client
        .respond(RequestId::Integer(1), &())
        .await
        .expect("response");
    assert_eq!(harness.counts(), BTreeMap::new());
    harness.client.metrics = None;
    harness.client.close_transport().await;
    assert!(matches!(
        harness
            .client
            .call::<_, Value>(FS_READ_FILE_METHOD, &())
            .await,
        Err(RpcCallError::Closed)
    ));
    assert_eq!(harness.counts(), BTreeMap::new());
}
