use super::*;
use crate::codex::make_session_and_context;
use crate::config::ApprovalsReviewer;
use crate::config::ConfigToml;
use crate::config::types::AppConfig;
use crate::config::types::AppToolConfig;
use crate::config::types::AppToolsConfig;
use crate::config::types::AppsConfigToml;
use codex_config::CONFIG_TOML_FILE;
use codex_otel::SessionTelemetry;
use codex_otel::metrics::MetricsClient;
use codex_otel::metrics::MetricsConfig;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::InMemoryMetricExporter;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::data::Metric;
use opentelemetry_sdk::metrics::data::MetricData;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::tempdir;
use tracing::Instrument;
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_test::internal::MockWriter;

fn annotations(
    read_only: Option<bool>,
    destructive: Option<bool>,
    open_world: Option<bool>,
) -> ToolAnnotations {
    ToolAnnotations {
        destructive_hint: destructive,
        idempotent_hint: None,
        open_world_hint: open_world,
        read_only_hint: read_only,
        title: None,
    }
}

fn approval_metadata(
    connector_id: Option<&str>,
    connector_name: Option<&str>,
    connector_description: Option<&str>,
    tool_title: Option<&str>,
    tool_description: Option<&str>,
) -> McpToolApprovalMetadata {
    McpToolApprovalMetadata {
        annotations: None,
        connector_id: connector_id.map(str::to_string),
        connector_name: connector_name.map(str::to_string),
        connector_description: connector_description.map(str::to_string),
        tool_title: tool_title.map(str::to_string),
        tool_description: tool_description.map(str::to_string),
        codex_apps_meta: None,
    }
}

fn prompt_options(
    allow_session_remember: bool,
    allow_persistent_approval: bool,
) -> McpToolApprovalPromptOptions {
    McpToolApprovalPromptOptions {
        allow_session_remember,
        allow_persistent_approval,
    }
}

fn test_session_telemetry() -> SessionTelemetry {
    let exporter = InMemoryMetricExporter::default();
    let metrics = MetricsClient::new(
        MetricsConfig::in_memory("test", "codex-core", env!("CARGO_PKG_VERSION"), exporter)
            .with_runtime_reader(),
    )
    .expect("in-memory metrics client");
    SessionTelemetry::new(
        ThreadId::new(),
        "gpt-5.1",
        "gpt-5.1",
        None,
        None,
        None,
        "test_originator".to_string(),
        false,
        "tty".to_string(),
        SessionSource::Cli,
    )
    .with_metrics_without_metadata_tags(metrics)
}

fn find_metric<'a>(resource_metrics: &'a ResourceMetrics, name: &str) -> &'a Metric {
    for scope_metrics in resource_metrics.scope_metrics() {
        for metric in scope_metrics.metrics() {
            if metric.name() == name {
                return metric;
            }
        }
    }
    panic!("metric {name} missing");
}

fn attributes_to_map<'a>(
    attributes: impl Iterator<Item = &'a KeyValue>,
) -> BTreeMap<String, String> {
    attributes
        .map(|kv| (kv.key.as_str().to_string(), kv.value.as_str().to_string()))
        .collect()
}

fn duration_metric_point(
    resource_metrics: &ResourceMetrics,
    name: &str,
) -> (BTreeMap<String, String>, f64, u64) {
    let metric = find_metric(resource_metrics, name);
    match metric.data() {
        AggregatedMetrics::F64(data) => match data {
            MetricData::Histogram(histogram) => {
                let points: Vec<_> = histogram.data_points().collect();
                assert_eq!(points.len(), 1);
                let point = points[0];
                (
                    attributes_to_map(point.attributes()),
                    point.sum(),
                    point.count(),
                )
            }
            _ => panic!("unexpected histogram aggregation"),
        },
        _ => panic!("unexpected metric data type"),
    }
}

#[test]
fn approval_required_when_read_only_false_and_destructive() {
    let annotations = annotations(Some(false), Some(true), None);
    assert_eq!(requires_mcp_tool_approval(Some(&annotations)), true);
}

#[test]
fn approval_required_when_read_only_false_and_open_world() {
    let annotations = annotations(Some(false), None, Some(true));
    assert_eq!(requires_mcp_tool_approval(Some(&annotations)), true);
}

#[test]
fn approval_required_when_destructive_even_if_read_only_true() {
    let annotations = annotations(Some(true), Some(true), Some(true));
    assert_eq!(requires_mcp_tool_approval(Some(&annotations)), true);
}

#[test]
fn approval_required_when_annotations_are_absent() {
    assert_eq!(requires_mcp_tool_approval(None), true);
}

#[test]
fn approval_not_required_when_read_only_and_other_hints_are_absent() {
    let annotations = annotations(Some(true), None, None);
    assert_eq!(requires_mcp_tool_approval(Some(&annotations)), false);
}

#[test]
fn prompt_mode_does_not_allow_persistent_remember() {
    assert_eq!(
        normalize_approval_decision_for_mode(
            McpToolApprovalDecision::AcceptForSession,
            AppToolApproval::Prompt,
        ),
        McpToolApprovalDecision::Accept
    );
    assert_eq!(
        normalize_approval_decision_for_mode(
            McpToolApprovalDecision::AcceptAndRemember,
            AppToolApproval::Prompt,
        ),
        McpToolApprovalDecision::Accept
    );
}

#[test]
fn approval_question_text_prepends_safety_reason() {
    assert_eq!(
        mcp_tool_approval_question_text(
            "Allow this action?".to_string(),
            Some("This tool may contact an external system."),
        ),
        "Tool call needs your approval. Reason: This tool may contact an external system."
    );
}

#[test]
fn mcp_call_metric_tags_include_connector_metadata_when_available() {
    let tags = mcp_call_metric_tags(
        "ok",
        "calendar_create_event",
        Some("connector_calendar"),
        Some("Google Calendar"),
    );

    assert_eq!(
        tags,
        vec![
            ("status", "ok".to_string()),
            ("tool_name", "calendar_create_event".to_string()),
            ("connector_id", "connector_calendar".to_string()),
            ("connector_name", "Google_Calendar".to_string()),
        ]
    );
}

#[test]
fn mcp_call_metric_tags_skip_missing_connector_metadata() {
    let tags = mcp_call_metric_tags("error", "echo", None, Some(""));

    assert_eq!(
        tags,
        vec![
            ("status", "error".to_string()),
            ("tool_name", "echo".to_string()),
        ]
    );
}

#[test]
fn mcp_call_duration_metric_records_connector_metadata_when_available() {
    let session_telemetry = test_session_telemetry();
    let tags = mcp_call_metric_tags(
        "ok",
        "calendar_create_event",
        Some("connector_calendar"),
        Some("Google Calendar"),
    );
    let tag_refs: Vec<(&str, &str)> = tags
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect();

    session_telemetry.record_duration(
        MCP_CALL_DURATION_METRIC,
        Duration::from_millis(125),
        &tag_refs,
    );

    let snapshot = session_telemetry
        .snapshot_metrics()
        .expect("runtime metrics snapshot");
    let (attrs, sum, count) = duration_metric_point(&snapshot, MCP_CALL_DURATION_METRIC);

    assert_eq!(count, 1);
    assert_eq!(sum, 125.0);
    assert_eq!(
        attrs,
        BTreeMap::from([
            ("connector_id".to_string(), "connector_calendar".to_string()),
            ("connector_name".to_string(), "Google_Calendar".to_string()),
            ("status".to_string(), "ok".to_string()),
            ("tool_name".to_string(), "calendar_create_event".to_string()),
        ])
    );
}

#[tokio::test]
async fn mcp_tool_call_span_records_expected_fields() {
    let buffer: &'static std::sync::Mutex<Vec<u8>> =
        Box::leak(Box::new(std::sync::Mutex::new(Vec::new())));
    let subscriber = tracing_subscriber::fmt()
        .with_level(true)
        .with_ansi(false)
        .with_max_level(Level::TRACE)
        .with_span_events(FmtSpan::FULL)
        .with_writer(MockWriter::new(buffer))
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let (session, turn_context) = make_session_and_context().await;

    async {}
        .instrument(mcp_tool_call_span(
            &session,
            &turn_context,
            McpToolCallSpanFields {
                server_name: "rmcp",
                tool_name: "echo",
                call_id: "call-123",
                server_origin: Some("https://example.com:8443/mcp"),
                connector_id: Some("calendar"),
                connector_name: Some("Calendar"),
            },
        ))
        .await;

    let logs = String::from_utf8(buffer.lock().expect("buffer lock").clone()).expect("utf8 logs");
    assert!(
        logs.contains("mcp.tools.call{otel.kind=\"client\"")
            && logs.contains("rpc.system=\"jsonrpc\"")
            && logs.contains("rpc.method=\"tools/call\"")
            && logs.contains("mcp.server.name=\"rmcp\"")
            && logs.contains("mcp.server.origin=\"https://example.com:8443/mcp\"")
            && logs.contains("mcp.transport=\"streamable_http\"")
            && logs.contains("mcp.connector.id=\"calendar\"")
            && logs.contains("mcp.connector.name=\"Calendar\"")
            && logs.contains("tool.name=\"echo\"")
            && logs.contains("tool.call_id=\"call-123\"")
            && logs.contains("server.address=\"example.com\"")
            && logs.contains("server.port=8443")
            && logs.contains("conversation.id=")
            && logs.contains("session.id=")
            && logs.contains("turn.id="),
        "missing MCP tool span fields\nlogs:\n{logs}"
    );
}

#[tokio::test]
async fn approval_elicitation_request_uses_message_override_and_preserves_tool_params_keys() {
    let (session, turn_context) = make_session_and_context().await;
    let question = build_mcp_tool_approval_question(
        "q".to_string(),
        CODEX_APPS_MCP_SERVER_NAME,
        "create_event",
        Some("Calendar"),
        prompt_options(true, true),
        Some("Allow Calendar to create an event?"),
    );

    let request = build_mcp_tool_approval_elicitation_request(
        &session,
        &turn_context,
        McpToolApprovalElicitationRequest {
            server: CODEX_APPS_MCP_SERVER_NAME,
            metadata: Some(&approval_metadata(
                Some("calendar"),
                Some("Calendar"),
                Some("Manage events and schedules."),
                Some("Create Event"),
                Some("Create a calendar event."),
            )),
            tool_params: Some(&serde_json::json!({
                "calendar_id": "primary",
                "title": "Roadmap review",
            })),
            tool_params_display: Some(&[
                RenderedMcpToolApprovalParam {
                    name: "calendar_id".to_string(),
                    value: serde_json::json!("primary"),
                    display_name: "Calendar".to_string(),
                },
                RenderedMcpToolApprovalParam {
                    name: "title".to_string(),
                    value: serde_json::json!("Roadmap review"),
                    display_name: "Title".to_string(),
                },
            ]),
            question,
            message_override: Some("Allow Calendar to create an event?"),
            prompt_options: prompt_options(true, true),
        },
    );

    assert_eq!(
        request,
        McpServerElicitationRequestParams {
            thread_id: session.conversation_id.to_string(),
            turn_id: Some(turn_context.sub_id),
            server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
            request: McpServerElicitationRequest::Form {
                meta: Some(serde_json::json!({
                    MCP_TOOL_APPROVAL_KIND_KEY: MCP_TOOL_APPROVAL_KIND_MCP_TOOL_CALL,
                    MCP_TOOL_APPROVAL_PERSIST_KEY: [
                        MCP_TOOL_APPROVAL_PERSIST_SESSION,
                        MCP_TOOL_APPROVAL_PERSIST_ALWAYS,
                    ],
                    MCP_TOOL_APPROVAL_SOURCE_KEY: MCP_TOOL_APPROVAL_SOURCE_CONNECTOR,
                    MCP_TOOL_APPROVAL_CONNECTOR_ID_KEY: "calendar",
                    MCP_TOOL_APPROVAL_CONNECTOR_NAME_KEY: "Calendar",
                    MCP_TOOL_APPROVAL_CONNECTOR_DESCRIPTION_KEY: "Manage events and schedules.",
                    MCP_TOOL_APPROVAL_TOOL_TITLE_KEY: "Create Event",
                    MCP_TOOL_APPROVAL_TOOL_DESCRIPTION_KEY: "Create a calendar event.",
                    MCP_TOOL_APPROVAL_TOOL_PARAMS_KEY: {
                        "calendar_id": "primary",
                        "title": "Roadmap review",
                    },
                    MCP_TOOL_APPROVAL_TOOL_PARAMS_DISPLAY_KEY: [
                        {
                            "name": "calendar_id",
                            "value": "primary",
                            "display_name": "Calendar",
                        },
                        {
                            "name": "title",
                            "value": "Roadmap review",
                            "display_name": "Title",
                        },
                    ],
                })),
                message: "Allow Calendar to create an event?".to_string(),
                requested_schema: McpElicitationSchema {
                    schema_uri: None,
                    type_: McpElicitationObjectType::Object,
                    properties: BTreeMap::new(),
                    required: None,
                },
            },
        }
    );
}

#[test]
fn custom_mcp_tool_question_mentions_server_name() {
    let question = build_mcp_tool_approval_question(
        "q".to_string(),
        "custom_server",
        "run_action",
        None,
        prompt_options(false, false),
        None,
    );

    assert_eq!(question.header, "Approve app tool call?");
    assert_eq!(
        question.question,
        "Allow the custom_server MCP server to run tool \"run_action\"?"
    );
    assert!(
        !question
            .options
            .expect("options")
            .into_iter()
            .map(|option| option.label)
            .any(|label| label == MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER)
    );
}

#[test]
fn codex_apps_tool_question_uses_fallback_app_label() {
    let question = build_mcp_tool_approval_question(
        "q".to_string(),
        CODEX_APPS_MCP_SERVER_NAME,
        "run_action",
        None,
        prompt_options(true, true),
        None,
    );

    assert_eq!(
        question.question,
        "Allow this app to run tool \"run_action\"?"
    );
}

#[test]
fn trusted_codex_apps_tool_question_offers_always_allow() {
    let question = build_mcp_tool_approval_question(
        "q".to_string(),
        CODEX_APPS_MCP_SERVER_NAME,
        "run_action",
        Some("Calendar"),
        prompt_options(true, true),
        None,
    );
    let options = question.options.expect("options");

    assert!(options.iter().any(|option| {
        option.label == MCP_TOOL_APPROVAL_ACCEPT_FOR_SESSION
            && option.description == "Run the tool and remember this choice for this session."
    }));
    assert!(options.iter().any(|option| {
        option.label == MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER
            && option.description == "Run the tool and remember this choice for future tool calls."
    }));
    assert_eq!(
        options
            .into_iter()
            .map(|option| option.label)
            .collect::<Vec<_>>(),
        vec![
            MCP_TOOL_APPROVAL_ACCEPT.to_string(),
            MCP_TOOL_APPROVAL_ACCEPT_FOR_SESSION.to_string(),
            MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER.to_string(),
            MCP_TOOL_APPROVAL_CANCEL.to_string(),
        ]
    );
}

#[test]
fn codex_apps_tool_question_without_elicitation_omits_always_allow() {
    let session_key = McpToolApprovalKey {
        server: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        connector_id: Some("calendar".to_string()),
        tool_name: "run_action".to_string(),
    };
    let persistent_key = session_key.clone();
    let question = build_mcp_tool_approval_question(
        "q".to_string(),
        CODEX_APPS_MCP_SERVER_NAME,
        "run_action",
        Some("Calendar"),
        mcp_tool_approval_prompt_options(Some(&session_key), Some(&persistent_key), false),
        None,
    );

    assert_eq!(
        question
            .options
            .expect("options")
            .into_iter()
            .map(|option| option.label)
            .collect::<Vec<_>>(),
        vec![
            MCP_TOOL_APPROVAL_ACCEPT.to_string(),
            MCP_TOOL_APPROVAL_ACCEPT_FOR_SESSION.to_string(),
            MCP_TOOL_APPROVAL_CANCEL.to_string(),
        ]
    );
}

#[test]
fn custom_mcp_tool_question_offers_session_remember_without_always_allow() {
    let question = build_mcp_tool_approval_question(
        "q".to_string(),
        "custom_server",
        "run_action",
        None,
        prompt_options(true, false),
        None,
    );

    assert_eq!(
        question
            .options
            .expect("options")
            .into_iter()
            .map(|option| option.label)
            .collect::<Vec<_>>(),
        vec![
            MCP_TOOL_APPROVAL_ACCEPT.to_string(),
            MCP_TOOL_APPROVAL_ACCEPT_FOR_SESSION.to_string(),
            MCP_TOOL_APPROVAL_CANCEL.to_string(),
        ]
    );
}

#[test]
fn custom_servers_keep_session_remember_without_persistent_approval() {
    let invocation = McpInvocation {
        server: "custom_server".to_string(),
        tool: "run_action".to_string(),
        arguments: None,
    };
    let expected = McpToolApprovalKey {
        server: "custom_server".to_string(),
        connector_id: None,
        tool_name: "run_action".to_string(),
    };

    assert_eq!(
        session_mcp_tool_approval_key(&invocation, None, AppToolApproval::Auto),
        Some(expected)
    );
    assert_eq!(
        persistent_mcp_tool_approval_key(&invocation, None, AppToolApproval::Auto),
        None
    );
}

#[test]
fn codex_apps_connectors_support_persistent_approval() {
    let invocation = McpInvocation {
        server: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        tool: "calendar/list_events".to_string(),
        arguments: None,
    };
    let metadata = approval_metadata(Some("calendar"), Some("Calendar"), None, None, None);
    let expected = McpToolApprovalKey {
        server: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        connector_id: Some("calendar".to_string()),
        tool_name: "calendar/list_events".to_string(),
    };

    assert_eq!(
        session_mcp_tool_approval_key(&invocation, Some(&metadata), AppToolApproval::Auto),
        Some(expected.clone())
    );
    assert_eq!(
        persistent_mcp_tool_approval_key(&invocation, Some(&metadata), AppToolApproval::Auto),
        Some(expected)
    );
}

#[test]
fn sanitize_mcp_tool_result_for_model_rewrites_image_content() {
    let result = Ok(CallToolResult {
        content: vec![
            serde_json::json!({
                "type": "image",
                "data": "Zm9v",
                "mimeType": "image/png",
            }),
            serde_json::json!({
                "type": "text",
                "text": "hello",
            }),
        ],
        structured_content: None,
        is_error: Some(false),
        meta: None,
    });

    let got = sanitize_mcp_tool_result_for_model(false, result).expect("sanitized result");

    assert_eq!(
        got.content,
        vec![
            serde_json::json!({
                "type": "text",
                "text": "<image content omitted because you do not support image input>",
            }),
            serde_json::json!({
                "type": "text",
                "text": "hello",
            }),
        ]
    );
}

#[test]
fn sanitize_mcp_tool_result_for_model_preserves_image_when_supported() {
    let original = CallToolResult {
        content: vec![serde_json::json!({
            "type": "image",
            "data": "Zm9v",
            "mimeType": "image/png",
        })],
        structured_content: Some(serde_json::json!({"x": 1})),
        is_error: Some(false),
        meta: Some(serde_json::json!({"k": "v"})),
    };

    let got =
        sanitize_mcp_tool_result_for_model(true, Ok(original.clone())).expect("unsanitized result");

    assert_eq!(got, original);
}

#[tokio::test]
async fn mcp_tool_call_request_meta_includes_turn_metadata_for_custom_server() {
    let (_, turn_context) = make_session_and_context().await;
    let expected_turn_metadata = serde_json::from_str::<serde_json::Value>(
        &turn_context
            .turn_metadata_state
            .current_header_value()
            .expect("turn metadata header"),
    )
    .expect("turn metadata json");

    let meta =
        build_mcp_tool_call_request_meta(&turn_context, "custom_server", /*metadata*/ None)
            .expect("custom servers should receive turn metadata");

    assert_eq!(
        meta,
        serde_json::json!({
            crate::X_CODEX_TURN_METADATA_HEADER: expected_turn_metadata,
        })
    );
}

#[tokio::test]
async fn codex_apps_tool_call_request_meta_includes_turn_metadata_and_codex_apps_meta() {
    let (_, turn_context) = make_session_and_context().await;
    let expected_turn_metadata = serde_json::from_str::<serde_json::Value>(
        &turn_context
            .turn_metadata_state
            .current_header_value()
            .expect("turn metadata header"),
    )
    .expect("turn metadata json");
    let metadata = McpToolApprovalMetadata {
        annotations: None,
        connector_id: Some("calendar".to_string()),
        connector_name: Some("Calendar".to_string()),
        connector_description: Some("Manage events".to_string()),
        tool_title: Some("Create Event".to_string()),
        tool_description: Some("Create a calendar event.".to_string()),
        codex_apps_meta: Some(
            serde_json::json!({
                "resource_uri": "connector://calendar/tools/calendar_create_event",
                "contains_mcp_source": true,
                "connector_id": "calendar",
            })
            .as_object()
            .cloned()
            .expect("_codex_apps metadata should be an object"),
        ),
    };

    assert_eq!(
        build_mcp_tool_call_request_meta(
            &turn_context,
            CODEX_APPS_MCP_SERVER_NAME,
            Some(&metadata),
        ),
        Some(serde_json::json!({
            crate::X_CODEX_TURN_METADATA_HEADER: expected_turn_metadata,
            MCP_TOOL_CODEX_APPS_META_KEY: {
                "resource_uri": "connector://calendar/tools/calendar_create_event",
                "contains_mcp_source": true,
                "connector_id": "calendar",
            },
        }))
    );
}

#[test]
fn accepted_elicitation_content_converts_to_request_user_input_response() {
    let response = request_user_input_response_from_elicitation_content(Some(serde_json::json!(
        {
            "approval": MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER,
        }
    )));

    assert_eq!(
        response,
        Some(RequestUserInputResponse {
            answers: std::collections::HashMap::from([(
                "approval".to_string(),
                RequestUserInputAnswer {
                    answers: vec![MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER.to_string()],
                },
            )]),
        })
    );
}

#[test]
fn approval_elicitation_meta_marks_tool_approvals() {
    assert_eq!(
        build_mcp_tool_approval_elicitation_meta(
            "custom_server",
            None,
            None,
            None,
            prompt_options(false, false),
        ),
        Some(serde_json::json!({
            MCP_TOOL_APPROVAL_KIND_KEY: MCP_TOOL_APPROVAL_KIND_MCP_TOOL_CALL,
        }))
    );
}

#[test]
fn approval_elicitation_meta_keeps_session_persist_behavior_for_custom_servers() {
    assert_eq!(
        build_mcp_tool_approval_elicitation_meta(
            "custom_server",
            Some(&approval_metadata(
                None,
                None,
                None,
                Some("Run Action"),
                Some("Runs the selected action."),
            )),
            Some(&serde_json::json!({"id": 1})),
            None,
            prompt_options(true, false),
        ),
        Some(serde_json::json!({
            MCP_TOOL_APPROVAL_KIND_KEY: MCP_TOOL_APPROVAL_KIND_MCP_TOOL_CALL,
            MCP_TOOL_APPROVAL_PERSIST_KEY: MCP_TOOL_APPROVAL_PERSIST_SESSION,
            MCP_TOOL_APPROVAL_TOOL_TITLE_KEY: "Run Action",
            MCP_TOOL_APPROVAL_TOOL_DESCRIPTION_KEY: "Runs the selected action.",
            MCP_TOOL_APPROVAL_TOOL_PARAMS_KEY: {
                "id": 1,
            },
        }))
    );
}

#[test]
fn guardian_mcp_review_request_includes_invocation_metadata() {
    let invocation = McpInvocation {
        server: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        tool: "browser_navigate".to_string(),
        arguments: Some(serde_json::json!({
            "url": "https://example.com",
        })),
    };

    let request = build_guardian_mcp_tool_review_request(
        "call-1",
        &invocation,
        Some(&approval_metadata(
            Some("playwright"),
            Some("Playwright"),
            Some("Browser automation"),
            Some("Navigate"),
            Some("Open a page"),
        )),
    );

    assert_eq!(
        request,
        GuardianApprovalRequest::McpToolCall {
            id: "call-1".to_string(),
            server: CODEX_APPS_MCP_SERVER_NAME.to_string(),
            tool_name: "browser_navigate".to_string(),
            arguments: Some(serde_json::json!({
                "url": "https://example.com",
            })),
            connector_id: Some("playwright".to_string()),
            connector_name: Some("Playwright".to_string()),
            connector_description: Some("Browser automation".to_string()),
            tool_title: Some("Navigate".to_string()),
            tool_description: Some("Open a page".to_string()),
            annotations: None,
        }
    );
}

#[test]
fn guardian_mcp_review_request_includes_annotations_when_present() {
    let invocation = McpInvocation {
        server: "custom_server".to_string(),
        tool: "dangerous_tool".to_string(),
        arguments: None,
    };
    let metadata = McpToolApprovalMetadata {
        annotations: Some(annotations(Some(false), Some(true), Some(true))),
        connector_id: None,
        connector_name: None,
        connector_description: None,
        tool_title: None,
        tool_description: None,
        codex_apps_meta: None,
    };

    let request = build_guardian_mcp_tool_review_request("call-1", &invocation, Some(&metadata));

    assert_eq!(
        request,
        GuardianApprovalRequest::McpToolCall {
            id: "call-1".to_string(),
            server: "custom_server".to_string(),
            tool_name: "dangerous_tool".to_string(),
            arguments: None,
            connector_id: None,
            connector_name: None,
            connector_description: None,
            tool_title: None,
            tool_description: None,
            annotations: Some(GuardianMcpAnnotations {
                destructive_hint: Some(true),
                open_world_hint: Some(true),
                read_only_hint: Some(false),
            }),
        }
    );
}

#[test]
fn prepare_arc_request_action_serializes_mcp_tool_call_shape() {
    let invocation = McpInvocation {
        server: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        tool: "browser_navigate".to_string(),
        arguments: Some(serde_json::json!({
            "url": "https://example.com",
        })),
    };

    let action = prepare_arc_request_action(
        &invocation,
        Some(&approval_metadata(
            None,
            Some("Playwright"),
            None,
            Some("Navigate"),
            None,
        )),
    );

    assert_eq!(
        action,
        serde_json::json!({
            "tool": "mcp_tool_call",
            "server": CODEX_APPS_MCP_SERVER_NAME,
            "tool_name": "browser_navigate",
            "arguments": {
                "url": "https://example.com",
            },
            "connector_name": "Playwright",
            "tool_title": "Navigate",
        })
    );
}

#[test]
fn guardian_review_decision_maps_to_mcp_tool_decision() {
    assert_eq!(
        mcp_tool_approval_decision_from_guardian(ReviewDecision::Approved),
        McpToolApprovalDecision::Accept
    );
    assert_eq!(
        mcp_tool_approval_decision_from_guardian(ReviewDecision::Denied),
        McpToolApprovalDecision::Decline
    );
    assert_eq!(
        mcp_tool_approval_decision_from_guardian(ReviewDecision::Abort),
        McpToolApprovalDecision::Decline
    );
}

#[test]
fn approval_elicitation_meta_includes_connector_source_for_codex_apps() {
    assert_eq!(
        build_mcp_tool_approval_elicitation_meta(
            CODEX_APPS_MCP_SERVER_NAME,
            Some(&approval_metadata(
                Some("calendar"),
                Some("Calendar"),
                Some("Manage events and schedules."),
                Some("Run Action"),
                Some("Runs the selected action."),
            )),
            Some(&serde_json::json!({
                "calendar_id": "primary",
            })),
            None,
            prompt_options(false, false),
        ),
        Some(serde_json::json!({
            MCP_TOOL_APPROVAL_KIND_KEY: MCP_TOOL_APPROVAL_KIND_MCP_TOOL_CALL,
            MCP_TOOL_APPROVAL_SOURCE_KEY: MCP_TOOL_APPROVAL_SOURCE_CONNECTOR,
            MCP_TOOL_APPROVAL_CONNECTOR_ID_KEY: "calendar",
            MCP_TOOL_APPROVAL_CONNECTOR_NAME_KEY: "Calendar",
            MCP_TOOL_APPROVAL_CONNECTOR_DESCRIPTION_KEY: "Manage events and schedules.",
            MCP_TOOL_APPROVAL_TOOL_TITLE_KEY: "Run Action",
            MCP_TOOL_APPROVAL_TOOL_DESCRIPTION_KEY: "Runs the selected action.",
            MCP_TOOL_APPROVAL_TOOL_PARAMS_KEY: {
                "calendar_id": "primary",
            },
        }))
    );
}

#[test]
fn approval_elicitation_meta_merges_session_and_always_persist_with_connector_source() {
    assert_eq!(
        build_mcp_tool_approval_elicitation_meta(
            CODEX_APPS_MCP_SERVER_NAME,
            Some(&approval_metadata(
                Some("calendar"),
                Some("Calendar"),
                Some("Manage events and schedules."),
                Some("Run Action"),
                Some("Runs the selected action."),
            )),
            Some(&serde_json::json!({
                "calendar_id": "primary",
            })),
            None,
            prompt_options(true, true),
        ),
        Some(serde_json::json!({
            MCP_TOOL_APPROVAL_KIND_KEY: MCP_TOOL_APPROVAL_KIND_MCP_TOOL_CALL,
            MCP_TOOL_APPROVAL_PERSIST_KEY: [
                MCP_TOOL_APPROVAL_PERSIST_SESSION,
                MCP_TOOL_APPROVAL_PERSIST_ALWAYS,
            ],
            MCP_TOOL_APPROVAL_SOURCE_KEY: MCP_TOOL_APPROVAL_SOURCE_CONNECTOR,
            MCP_TOOL_APPROVAL_CONNECTOR_ID_KEY: "calendar",
            MCP_TOOL_APPROVAL_CONNECTOR_NAME_KEY: "Calendar",
            MCP_TOOL_APPROVAL_CONNECTOR_DESCRIPTION_KEY: "Manage events and schedules.",
            MCP_TOOL_APPROVAL_TOOL_TITLE_KEY: "Run Action",
            MCP_TOOL_APPROVAL_TOOL_DESCRIPTION_KEY: "Runs the selected action.",
            MCP_TOOL_APPROVAL_TOOL_PARAMS_KEY: {
                "calendar_id": "primary",
            },
        }))
    );
}

#[tokio::test]
async fn approval_callsite_mode_distinguishes_default_always_allow_and_full_access() {
    let (_session, mut turn_context) = make_session_and_context().await;

    assert_eq!(
        mcp_tool_approval_callsite_mode(AppToolApproval::Auto, &turn_context),
        "mcp_tool_call__default"
    );
    assert_eq!(
        mcp_tool_approval_callsite_mode(AppToolApproval::Prompt, &turn_context),
        "mcp_tool_call__default"
    );
    assert_eq!(
        mcp_tool_approval_callsite_mode(AppToolApproval::Approve, &turn_context),
        "mcp_tool_call__always_allow"
    );

    turn_context
        .approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn_context
        .sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");

    assert_eq!(
        mcp_tool_approval_callsite_mode(AppToolApproval::Auto, &turn_context),
        "mcp_tool_call__full_access"
    );
}

#[test]
fn declined_elicitation_response_stays_decline() {
    let response = parse_mcp_tool_approval_elicitation_response(
        Some(ElicitationResponse {
            action: ElicitationAction::Decline,
            content: Some(serde_json::json!({
                "approval": MCP_TOOL_APPROVAL_ACCEPT,
            })),
            meta: None,
        }),
        "approval",
    );

    assert_eq!(response, McpToolApprovalDecision::Decline);
}

#[test]
fn synthetic_decline_request_user_input_response_stays_decline() {
    let response = parse_mcp_tool_approval_response(
        Some(RequestUserInputResponse {
            answers: HashMap::from([(
                "approval".to_string(),
                RequestUserInputAnswer {
                    answers: vec![MCP_TOOL_APPROVAL_DECLINE_SYNTHETIC.to_string()],
                },
            )]),
        }),
        "approval",
    );

    assert_eq!(response, McpToolApprovalDecision::Decline);
}

#[test]
fn accepted_elicitation_response_uses_always_persist_meta() {
    let response = parse_mcp_tool_approval_elicitation_response(
        Some(ElicitationResponse {
            action: ElicitationAction::Accept,
            content: None,
            meta: Some(serde_json::json!({
                MCP_TOOL_APPROVAL_PERSIST_KEY: MCP_TOOL_APPROVAL_PERSIST_ALWAYS,
            })),
        }),
        "approval",
    );

    assert_eq!(response, McpToolApprovalDecision::AcceptAndRemember);
}

#[test]
fn accepted_elicitation_response_uses_session_persist_meta() {
    let response = parse_mcp_tool_approval_elicitation_response(
        Some(ElicitationResponse {
            action: ElicitationAction::Accept,
            content: None,
            meta: Some(serde_json::json!({
                MCP_TOOL_APPROVAL_PERSIST_KEY: MCP_TOOL_APPROVAL_PERSIST_SESSION,
            })),
        }),
        "approval",
    );

    assert_eq!(response, McpToolApprovalDecision::AcceptForSession);
}

#[test]
fn accepted_elicitation_without_content_defaults_to_accept() {
    let response = parse_mcp_tool_approval_elicitation_response(
        Some(ElicitationResponse {
            action: ElicitationAction::Accept,
            content: None,
            meta: None,
        }),
        "approval",
    );

    assert_eq!(response, McpToolApprovalDecision::Accept);
}

#[tokio::test]
async fn persist_codex_app_tool_approval_writes_tool_override() {
    let tmp = tempdir().expect("tempdir");

    persist_codex_app_tool_approval(tmp.path(), "calendar", "calendar/list_events")
        .await
        .expect("persist approval");

    let contents = std::fs::read_to_string(tmp.path().join(CONFIG_TOML_FILE)).expect("read config");
    let parsed: ConfigToml = toml::from_str(&contents).expect("parse config");

    assert_eq!(
        parsed.apps,
        Some(AppsConfigToml {
            default: None,
            apps: HashMap::from([(
                "calendar".to_string(),
                AppConfig {
                    enabled: true,
                    destructive_enabled: None,
                    open_world_enabled: None,
                    default_tools_approval_mode: None,
                    default_tools_enabled: None,
                    tools: Some(AppToolsConfig {
                        tools: HashMap::from([(
                            "calendar/list_events".to_string(),
                            AppToolConfig {
                                enabled: None,
                                approval_mode: Some(AppToolApproval::Approve),
                            },
                        )]),
                    }),
                },
            )]),
        })
    );
    assert!(contents.contains("[apps.calendar.tools.\"calendar/list_events\"]"));
}

#[tokio::test]
async fn maybe_persist_mcp_tool_approval_reloads_session_config() {
    let (session, turn_context) = make_session_and_context().await;
    let codex_home = session.codex_home().await;
    std::fs::create_dir_all(&codex_home).expect("create codex home");
    let key = McpToolApprovalKey {
        server: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        connector_id: Some("calendar".to_string()),
        tool_name: "calendar/list_events".to_string(),
    };

    maybe_persist_mcp_tool_approval(&session, &turn_context, key.clone()).await;

    let config = session.get_config().await;
    let apps_toml = config
        .config_layer_stack
        .effective_config()
        .as_table()
        .and_then(|table| table.get("apps"))
        .cloned()
        .expect("apps table");
    let apps = AppsConfigToml::deserialize(apps_toml).expect("deserialize apps config");
    let tool = apps
        .apps
        .get("calendar")
        .and_then(|app| app.tools.as_ref())
        .and_then(|tools| tools.tools.get("calendar/list_events"))
        .expect("calendar/list_events tool config exists");

    assert_eq!(
        tool,
        &AppToolConfig {
            enabled: None,
            approval_mode: Some(AppToolApproval::Approve),
        }
    );
    assert_eq!(mcp_tool_approval_is_remembered(&session, &key).await, true);
}

#[tokio::test]
async fn approve_mode_skips_when_annotations_do_not_require_approval() {
    let (session, turn_context) = make_session_and_context().await;
    let session = Arc::new(session);
    let turn_context = Arc::new(turn_context);
    let invocation = McpInvocation {
        server: "custom_server".to_string(),
        tool: "read_only_tool".to_string(),
        arguments: None,
    };
    let metadata = McpToolApprovalMetadata {
        annotations: Some(annotations(Some(true), None, None)),
        connector_id: None,
        connector_name: None,
        connector_description: None,
        tool_title: Some("Read Only Tool".to_string()),
        tool_description: None,
        codex_apps_meta: None,
    };

    let decision = maybe_request_mcp_tool_approval(
        &session,
        &turn_context,
        "call-1",
        &invocation,
        Some(&metadata),
        AppToolApproval::Approve,
    )
    .await;

    assert_eq!(decision, None);
}

#[tokio::test]
async fn approve_mode_blocks_when_arc_returns_interrupt_for_model() {
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/codex/safety/arc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "outcome": "steer-model",
            "short_reason": "needs approval",
            "rationale": "high-risk action",
            "risk_score": 96,
            "risk_level": "critical",
            "evidence": [{
                "message": "dangerous_tool",
                "why": "high-risk action",
            }],
        })))
        .expect(1)
        .mount(&server)
        .await;

    let (session, mut turn_context) = make_session_and_context().await;
    turn_context.auth_manager = Some(crate::test_support::auth_manager_from_auth(
        crate::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    ));
    let mut config = (*turn_context.config).clone();
    config.chatgpt_base_url = server.uri();
    turn_context.config = Arc::new(config);

    let session = Arc::new(session);
    let turn_context = Arc::new(turn_context);
    let invocation = McpInvocation {
        server: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        tool: "dangerous_tool".to_string(),
        arguments: Some(serde_json::json!({ "id": 1 })),
    };
    let metadata = McpToolApprovalMetadata {
        annotations: Some(annotations(Some(false), Some(true), Some(true))),
        connector_id: Some("calendar".to_string()),
        connector_name: Some("Calendar".to_string()),
        connector_description: Some("Manage events".to_string()),
        tool_title: Some("Dangerous Tool".to_string()),
        tool_description: Some("Performs a risky action.".to_string()),
        codex_apps_meta: None,
    };

    let decision = maybe_request_mcp_tool_approval(
        &session,
        &turn_context,
        "call-2",
        &invocation,
        Some(&metadata),
        AppToolApproval::Approve,
    )
    .await;

    assert_eq!(
        decision,
        Some(McpToolApprovalDecision::BlockedBySafetyMonitor(
            "Tool call was cancelled because of safety risks: high-risk action".to_string(),
        ))
    );
}

#[tokio::test]
async fn approve_mode_blocks_when_arc_returns_interrupt_without_annotations() {
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/codex/safety/arc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "outcome": "steer-model",
            "short_reason": "needs approval",
            "rationale": "high-risk action",
            "risk_score": 96,
            "risk_level": "critical",
            "evidence": [{
                "message": "dangerous_tool",
                "why": "high-risk action",
            }],
        })))
        .expect(1)
        .mount(&server)
        .await;

    let (session, mut turn_context) = make_session_and_context().await;
    turn_context.auth_manager = Some(crate::test_support::auth_manager_from_auth(
        crate::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    ));
    let mut config = (*turn_context.config).clone();
    config.chatgpt_base_url = server.uri();
    turn_context.config = Arc::new(config);

    let session = Arc::new(session);
    let turn_context = Arc::new(turn_context);
    let invocation = McpInvocation {
        server: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        tool: "dangerous_tool".to_string(),
        arguments: Some(serde_json::json!({ "id": 1 })),
    };
    let metadata = McpToolApprovalMetadata {
        annotations: None,
        connector_id: Some("calendar".to_string()),
        connector_name: Some("Calendar".to_string()),
        connector_description: Some("Manage events".to_string()),
        tool_title: Some("Dangerous Tool".to_string()),
        tool_description: Some("Performs a risky action.".to_string()),
        codex_apps_meta: None,
    };

    let decision = maybe_request_mcp_tool_approval(
        &session,
        &turn_context,
        "call-3",
        &invocation,
        Some(&metadata),
        AppToolApproval::Approve,
    )
    .await;

    assert_eq!(
        decision,
        Some(McpToolApprovalDecision::BlockedBySafetyMonitor(
            "Tool call was cancelled because of safety risks: high-risk action".to_string(),
        ))
    );
}

#[tokio::test]
async fn full_access_auto_mode_blocks_when_arc_returns_interrupt_for_model() {
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/codex/safety/arc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "outcome": "steer-model",
            "short_reason": "needs approval",
            "rationale": "high-risk action",
            "risk_score": 96,
            "risk_level": "critical",
            "evidence": [{
                "message": "dangerous_tool",
                "why": "high-risk action",
            }],
        })))
        .expect(1)
        .mount(&server)
        .await;

    let (session, mut turn_context) = make_session_and_context().await;
    turn_context.auth_manager = Some(crate::test_support::auth_manager_from_auth(
        crate::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    ));
    turn_context
        .approval_policy
        .set(AskForApproval::Never)
        .expect("test setup should allow updating approval policy");
    turn_context
        .sandbox_policy
        .set(SandboxPolicy::DangerFullAccess)
        .expect("test setup should allow updating sandbox policy");
    let mut config = (*turn_context.config).clone();
    config.chatgpt_base_url = server.uri();
    turn_context.config = Arc::new(config);

    let session = Arc::new(session);
    let turn_context = Arc::new(turn_context);
    let invocation = McpInvocation {
        server: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        tool: "dangerous_tool".to_string(),
        arguments: Some(serde_json::json!({ "id": 1 })),
    };
    let metadata = McpToolApprovalMetadata {
        annotations: Some(annotations(Some(false), Some(true), Some(true))),
        connector_id: Some("calendar".to_string()),
        connector_name: Some("Calendar".to_string()),
        connector_description: Some("Manage events".to_string()),
        tool_title: Some("Dangerous Tool".to_string()),
        tool_description: Some("Performs a risky action.".to_string()),
        codex_apps_meta: None,
    };

    let decision = maybe_request_mcp_tool_approval(
        &session,
        &turn_context,
        "call-2",
        &invocation,
        Some(&metadata),
        AppToolApproval::Auto,
    )
    .await;

    assert_eq!(
        decision,
        Some(McpToolApprovalDecision::BlockedBySafetyMonitor(
            "Tool call was cancelled because of safety risks: high-risk action".to_string(),
        ))
    );
}

#[tokio::test]
async fn approve_mode_routes_arc_ask_user_to_guardian_when_guardian_reviewer_is_enabled() {
    use wiremock::Mock;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    let server = start_mock_server().await;
    let guardian_request_log = mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-guardian"),
            ev_assistant_message(
                "msg-guardian",
                &serde_json::json!({
                    "risk_level": "low",
                    "risk_score": 12,
                    "rationale": "The user already configured guardian to review escalated approvals for this session.",
                    "evidence": [{
                        "message": "ARC requested escalation instead of blocking outright.",
                        "why": "Guardian can adjudicate the approval without surfacing a manual prompt.",
                    }],
                })
                .to_string(),
            ),
            ev_completed("resp-guardian"),
        ]),
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/codex/safety/arc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "outcome": "ask-user",
            "short_reason": "needs confirmation",
            "rationale": "ARC wants a second review",
            "risk_score": 65,
            "risk_level": "medium",
            "evidence": [{
                "message": "dangerous_tool",
                "why": "requires review",
            }],
        })))
        .expect(1)
        .mount(&server)
        .await;

    let (mut session, mut turn_context) = make_session_and_context().await;
    turn_context.auth_manager = Some(crate::test_support::auth_manager_from_auth(
        crate::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
    ));
    turn_context
        .approval_policy
        .set(AskForApproval::OnRequest)
        .expect("test setup should allow updating approval policy");
    let mut config = (*turn_context.config).clone();
    config.chatgpt_base_url = server.uri();
    config.model_provider.base_url = Some(format!("{}/v1", server.uri()));
    config.approvals_reviewer = ApprovalsReviewer::GuardianSubagent;
    let config = Arc::new(config);
    let models_manager = Arc::new(crate::test_support::models_manager_with_provider(
        config.codex_home.clone(),
        Arc::clone(&session.services.auth_manager),
        config.model_provider.clone(),
    ));
    session.services.models_manager = models_manager;
    turn_context.config = Arc::clone(&config);
    turn_context.provider = config.model_provider.clone();

    let session = Arc::new(session);
    let turn_context = Arc::new(turn_context);
    let invocation = McpInvocation {
        server: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        tool: "dangerous_tool".to_string(),
        arguments: Some(serde_json::json!({ "id": 1 })),
    };
    let metadata = McpToolApprovalMetadata {
        annotations: Some(annotations(Some(false), Some(true), Some(true))),
        connector_id: Some("calendar".to_string()),
        connector_name: Some("Calendar".to_string()),
        connector_description: Some("Manage events".to_string()),
        tool_title: Some("Dangerous Tool".to_string()),
        tool_description: Some("Performs a risky action.".to_string()),
        codex_apps_meta: None,
    };

    let decision = maybe_request_mcp_tool_approval(
        &session,
        &turn_context,
        "call-3",
        &invocation,
        Some(&metadata),
        AppToolApproval::Approve,
    )
    .await;

    assert_eq!(decision, Some(McpToolApprovalDecision::Accept));
    assert_eq!(
        guardian_request_log.single_request().path(),
        "/v1/responses"
    );
}
