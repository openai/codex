//! Verifies request-local trace ancestry through real RMCP queues and HTTP headers.

use std::time::Duration;

use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::McpProtocolMode;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::InMemorySpanExporter;
use opentelemetry_sdk::trace::SdkTracerProvider;
use pretty_assertions::assert_eq;
use rmcp::model::ClientCapabilities;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::PaginatedRequestParams;
use rmcp::model::RequestMetaObject;
use serde_json::Value;
use serde_json::json;
use tracing::Instrument;
use tracing_subscriber::prelude::*;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;

#[tokio::test(flavor = "current_thread")]
async fn mcp_requests_preserve_trace_context_across_workers_and_continuations() -> anyhow::Result<()>
{
    let exported = InMemorySpanExporter::default();
    let telemetry = SdkTracerProvider::builder()
        .with_simple_exporter(exported.clone())
        .build();
    let _subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(telemetry.tracer("mcp-trace-test")))
        .set_default();

    for mode in [McpProtocolMode::Legacy, McpProtocolMode::V20260728] {
        let modern = mode == McpProtocolMode::V20260728;
        let server = MockServer::start().await;
        Mock::given(method("POST")).respond_with(move |request: &Request| {
            let body: Value = request.body_json().unwrap();
            let result = match body["method"].as_str().unwrap() {
                "initialize" => json!({"protocolVersion":"2025-06-18", "capabilities":{"tools":{}}, "serverInfo":{"name":"test","version":"1"}}),
                "notifications/initialized" => return ResponseTemplate::new(202),
                "server/discover" => json!({"resultType":"complete", "supportedVersions":["2026-07-28"], "capabilities":{"tools":{}}, "_meta":{"io.modelcontextprotocol/serverInfo":{"name":"test","version":"1"}}, "ttlMs":0, "cacheScope":"private"}),
                "tools/list" => json!({"resultType":"complete", "tools":[], "ttlMs":60000, "cacheScope":"private"}),
                "tools/call" if modern && body["params"].get("requestState").is_none() => json!({"resultType":"input_required", "requestState":"continue"}),
                "tools/call" => json!({"resultType":"complete", "content":[]}),
                "test/custom" => json!({"resultType":"complete", "ok":true}),
                other => panic!("unexpected method: {other}"),
            };
            ResponseTemplate::new(200).set_body_json(json!({"jsonrpc":"2.0", "id":body["id"], "result":result}))
        }).mount(&server).await;
        let client = RmcpClient::new_streamable_http_client_with_protocol_mode(
            "trace-test",
            &server.uri(),
            Some("test-bearer".to_string()),
            /*http_headers*/ None,
            /*env_http_headers*/ None,
            OAuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::default(),
            Environment::default_for_tests().get_http_client(),
            /*auth_provider*/ None,
            mode,
        )
        .await?
        .with_read_only_tools(/*requires_read_only_tools*/ true);
        client
            .initialize(
                InitializeRequestParams::new(
                    ClientCapabilities::default(),
                    Implementation::new("test", "1"),
                ),
                Some(Duration::from_secs(5)),
                Box::new(|_, _| {
                    async {
                        Ok(ElicitationResponse {
                            action: ElicitationAction::Accept,
                            content: None,
                            meta: None,
                        })
                    }
                    .boxed()
                }),
            )
            .await?;

        let first = tracing::info_span!(parent: None, "first_page");
        let second = tracing::info_span!(parent: None, "second_page");
        let first_trace = codex_otel::span_w3c_trace_context(&first).unwrap();
        let second_trace = codex_otel::span_w3c_trace_context(&second).unwrap();
        let mut first_params = PaginatedRequestParams::default();
        first_params.meta = Some(RequestMetaObject::from(
            json!({"application":"preserved", "traceparent":"invalid", "tracestate":"stale=value"})
                .as_object()
                .unwrap()
                .clone(),
        ));
        let second_params =
            PaginatedRequestParams::default().with_cursor(Some("page2".to_string()));
        let (first_result, second_result) = tokio::join!(
            client
                .list_tools_with_connector_ids(Some(first_params), Some(Duration::from_secs(5)))
                .instrument(first),
            client
                .list_tools(Some(second_params), Some(Duration::from_secs(5)))
                .instrument(second),
        );
        first_result?;
        second_result?;

        let tool = tracing::info_span!(parent: None, "tool");
        let tool_trace = codex_otel::span_w3c_trace_context(&tool).unwrap();
        client
            .call_tool(
                "test".to_string(),
                /*arguments*/ None,
                Some(json!({"application":"tool-meta"})),
                Some(Duration::from_secs(5)),
            )
            .instrument(tool)
            .await?;
        let custom = tracing::info_span!(parent: None, "custom");
        let custom_trace = codex_otel::span_w3c_trace_context(&custom).unwrap();
        client.send_custom_request("test/custom", Some(json!({"argument":"preserved", "_meta":{"application":"custom-meta", "traceparent":"invalid", "tracestate":"stale=value"}})))
            .instrument(custom).await?;
        client.shutdown().await;
        telemetry.force_flush()?;
        let spans = exported.get_finished_spans()?;
        let requests = server.received_requests().await.unwrap();
        let requests = requests
            .iter()
            .filter_map(|request| {
                let body: Value = request.body_json().unwrap();
                matches!(
                    body["method"].as_str(),
                    Some("tools/list" | "tools/call" | "test/custom")
                )
                .then_some((request, body))
            })
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), if modern { 5 } else { 4 });
        for (request, body) in requests {
            if matches!(body["method"].as_str(), Some("tools/list" | "tools/call")) {
                assert_eq!(body["params"]["_meta"]["openai/readOnly"], true);
            }
            let expected = if body["method"] == "test/custom" {
                assert_eq!(body["params"]["_meta"]["application"], "custom-meta");
                assert_eq!(body["params"]["argument"], "preserved");
                assert!(body["params"]["_meta"].get("tracestate").is_none());
                &custom_trace
            } else if body["method"] == "tools/call" {
                assert_eq!(body["params"]["_meta"]["application"], "tool-meta");
                &tool_trace
            } else if body["params"]["cursor"] == "page2" {
                &second_trace
            } else {
                assert_eq!(body["params"]["_meta"]["application"], "preserved");
                assert!(body["params"]["_meta"].get("tracestate").is_none());
                &first_trace
            };
            let parent = expected
                .traceparent
                .as_deref()
                .unwrap()
                .split('-')
                .collect::<Vec<_>>();
            let metadata_parent = body["params"]["_meta"]["traceparent"]
                .as_str()
                .unwrap()
                .split('-')
                .collect::<Vec<_>>();
            assert_eq!(metadata_parent[1], parent[1]);
            if metadata_parent[2] != parent[2] {
                let caller = spans
                    .iter()
                    .find(|span| span.span_context.span_id().to_string() == metadata_parent[2])
                    .unwrap();
                assert_eq!(caller.name, "list_tools_with_connector_ids");
                assert_eq!(caller.parent_span_id.to_string(), parent[2]);
            }
            let header = request.headers.get("traceparent").unwrap().to_str()?;
            let header = header.split('-').collect::<Vec<_>>();
            assert_eq!(header[1], parent[1]);
            let http = spans
                .iter()
                .find(|span| span.span_context.span_id().to_string() == header[2])
                .unwrap();
            assert_eq!(http.name, "codex.exec_server.http_request");
            let bridge = spans
                .iter()
                .find(|span| span.span_context.span_id() == http.parent_span_id)
                .unwrap();
            assert_eq!(bridge.name, "mcp.http.request");
            assert_eq!(bridge.parent_span_id.to_string(), metadata_parent[2]);
        }
    }
    Ok(())
}
