use codex_otel::RuntimeMetricTotals;
use codex_otel::RuntimeMetricsSummary;
use codex_otel::SessionTelemetry;
use codex_otel::TelemetryAuthMode;
use codex_otel::WellKnownApiRequestError;
use codex_otel::metrics::MetricsClient;
use codex_otel::metrics::MetricsConfig;
use codex_otel::metrics::Result;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use eventsource_stream::Event as StreamEvent;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use pretty_assertions::assert_eq;
use std::time::Duration;
use tokio_tungstenite::tungstenite::Message;

#[test]
fn runtime_metrics_summary_collects_tool_api_and_streaming_metrics() -> Result<()> {
    let exporter = InMemoryMetricExporter::default();
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory("test", "codex-cli", env!("CARGO_PKG_VERSION"), exporter)
            .with_runtime_reader(),
    )?;
    let manager = SessionTelemetry::new(
        ThreadId::new(),
        "gpt-5.1",
        "gpt-5.1",
        Some("account-id".to_string()),
        /*account_email*/ None,
        Some(TelemetryAuthMode::ApiKey),
        "test_originator".to_string(),
        /*log_user_prompts*/ true,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics(metrics);

    manager.reset_runtime_metrics();

    manager.tool_result_with_tags(
        "shell",
        "call-1",
        "{\"cmd\":\"echo\"}",
        Duration::from_millis(250),
        /*success*/ true,
        "ok",
        &[],
        /*mcp_server*/ None,
        /*mcp_server_origin*/ None,
    );
    manager.record_api_request(
        /*attempt*/ 1,
        Some(200),
        /*error*/ None,
        Duration::from_millis(300),
        /*auth_header_attached*/ false,
        /*auth_header_name*/ None,
        /*retry_after_unauthorized*/ false,
        /*recovery_mode*/ None,
        /*recovery_phase*/ None,
        "/responses",
        /*request_id*/ None,
        /*cf_ray*/ None,
        /*auth_error*/ None,
        /*auth_error_code*/ None,
        WellKnownApiRequestError::None,
    );
    manager.record_websocket_request(
        Duration::from_millis(400),
        /*error*/ None,
        /*connection_reused*/ false,
    );
    let sse_response: std::result::Result<
        Option<std::result::Result<StreamEvent, eventsource_stream::EventStreamError<&str>>>,
        tokio::time::error::Elapsed,
    > = Ok(Some(Ok(StreamEvent {
        event: "response.created".to_string(),
        data: "{}".to_string(),
        id: String::new(),
        retry: None,
    })));
    manager.log_sse_event(&sse_response, Duration::from_millis(120));
    let ws_response: std::result::Result<
        Option<std::result::Result<Message, tokio_tungstenite::tungstenite::Error>>,
        codex_api::ApiError,
    > = Ok(Some(Ok(Message::Text(
        r#"{"type":"response.created"}"#.into(),
    ))));
    manager.record_websocket_event(&ws_response, Duration::from_millis(80));
    let ws_timing_response: std::result::Result<
        Option<std::result::Result<Message, tokio_tungstenite::tungstenite::Error>>,
        codex_api::ApiError,
    > = Ok(Some(Ok(Message::Text(
        r#"{"type":"responsesapi.websocket_timing","timing_metrics":{"responses_duration_excl_engine_and_client_tool_time_ms":124,"engine_service_total_ms":457,"engine_iapi_ttft_total_ms":211,"engine_service_ttft_total_ms":233,"engine_iapi_tbt_across_engine_calls_ms":377,"engine_service_tbt_across_engine_calls_ms":399}}"#
            .into(),
    ))));
    manager.record_websocket_event(&ws_timing_response, Duration::from_millis(20));
    manager.record_duration(
        "codex.turn.ttft.duration_ms",
        Duration::from_millis(95),
        &[],
    );
    manager.record_duration(
        "codex.turn.ttfm.duration_ms",
        Duration::from_millis(180),
        &[],
    );

    let summary = manager
        .runtime_metrics_summary()
        .expect("runtime metrics summary should be available");
    let expected = RuntimeMetricsSummary {
        tool_calls: RuntimeMetricTotals {
            count: 1,
            duration_ms: 250,
        },
        api_calls: RuntimeMetricTotals {
            count: 1,
            duration_ms: 300,
        },
        streaming_events: RuntimeMetricTotals {
            count: 1,
            duration_ms: 120,
        },
        websocket_calls: RuntimeMetricTotals {
            count: 1,
            duration_ms: 400,
        },
        websocket_events: RuntimeMetricTotals {
            count: 2,
            duration_ms: 100,
        },
        responses_api_overhead_ms: 124,
        responses_api_inference_time_ms: 457,
        responses_api_engine_iapi_ttft_ms: 211,
        responses_api_engine_service_ttft_ms: 233,
        responses_api_engine_iapi_tbt_ms: 377,
        responses_api_engine_service_tbt_ms: 399,
        turn_ttft_ms: 95,
        turn_ttfm_ms: 180,
    };
    assert_eq!(summary, expected);

    Ok(())
}
