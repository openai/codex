//! Use a process-wide subscriber so another test cannot disable shared core callsites
//! while the startup task and regular turn are running.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use codex_api::WS_REQUEST_HEADER_TRACEPARENT_CLIENT_METADATA_KEY;
use codex_api::WS_REQUEST_HEADER_TRACESTATE_CLIENT_METADATA_KEY;
use codex_protocol::protocol::W3cTraceContext;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::start_websocket_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::SdkTracerProvider;
use pretty_assertions::assert_eq;
use tracing::Instrument;
use tracing_subscriber::filter::dynamic_filter_fn;
use tracing_subscriber::layer::SubscriberExt;

#[tokio::test(flavor = "current_thread")]
async fn websocket_startup_prewarm_propagates_trace() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let include_prewarm_span = Arc::new(AtomicBool::new(false));
    let filter_setting = Arc::clone(&include_prewarm_span);
    let provider = SdkTracerProvider::builder().build();
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry()
            .with(
                tracing_opentelemetry::layer().with_tracer(provider.tracer("startup-prewarm-test")),
            )
            .with(dynamic_filter_fn(move |meta, _| {
                *meta.level() <= tracing::Level::INFO
                    && (meta.name() != "startup_prewarm" || filter_setting.load(Ordering::Relaxed))
            })),
    )?;

    for enabled in [false, true] {
        include_prewarm_span.store(enabled, Ordering::Relaxed);
        assert_prewarm_and_turn_trace().await?;
    }
    Ok(())
}

async fn assert_prewarm_and_turn_trace() -> anyhow::Result<()> {
    let request_span = |trace_id: &str, tracestate: &str| {
        let span = tracing::info_span!(parent: None, "request");
        assert!(codex_otel::set_parent_from_w3c_trace_context(
            &span,
            &W3cTraceContext {
                traceparent: Some(format!("00-{trace_id}-0000000000000001-01")),
                tracestate: Some(tracestate.to_string()),
            }
        ));
        span
    };
    let start_trace_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let turn_trace_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let server = start_websocket_server(vec![vec![
        vec![ev_response_created("warm-1"), ev_completed("warm-1")],
        vec![
            ev_response_created("resp-1"),
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ],
    ]])
    .await;
    let test = test_codex()
        .build_with_websocket_server(&server)
        .instrument(request_span(start_trace_id, "vendor=start"))
        .await?;
    let warmup = server
        .wait_for_request(/*connection_index*/ 0, /*request_index*/ 0)
        .await
        .body_json();
    assert_eq!(warmup["type"], "response.create");
    assert_eq!(warmup["generate"], false);

    test.submit_text_turn("hello")
        .instrument(request_span(turn_trace_id, "vendor=turn"))
        .await?;
    let handshake = server.single_handshake();
    let connection = server.single_connection();
    assert_eq!(connection.len(), 2);
    let turn = connection[1].body_json();
    assert_eq!(turn["type"], "response.create");
    assert_eq!(turn.get("generate"), None);

    let assert_trace =
        |parent: Option<&str>, state: Option<&str>, expected_id: &str, expected_state: &str| {
            // Each carrier can have its own span ID within the originating trace.
            assert_eq!(
                parent.and_then(|value| value.split('-').nth(1)),
                Some(expected_id)
            );
            assert_eq!(state, Some(expected_state));
        };
    assert_trace(
        handshake.header("traceparent").as_deref(),
        handshake.header("tracestate").as_deref(),
        start_trace_id,
        "vendor=start",
    );
    for (frame, expected_id, expected_state) in [
        (&warmup, start_trace_id, "vendor=start"),
        (&turn, turn_trace_id, "vendor=turn"),
    ] {
        let metadata = &frame["client_metadata"];
        assert_trace(
            metadata[WS_REQUEST_HEADER_TRACEPARENT_CLIENT_METADATA_KEY].as_str(),
            metadata[WS_REQUEST_HEADER_TRACESTATE_CLIENT_METADATA_KEY].as_str(),
            expected_id,
            expected_state,
        );
    }
    test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}
