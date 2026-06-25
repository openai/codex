use super::*;
use pretty_assertions::assert_eq;

fn metric_call_tool_result(
    is_error: bool,
    structured_content: Option<serde_json::Value>,
) -> CallToolResult {
    CallToolResult {
        content: Vec::new(),
        structured_content,
        is_error: Some(is_error),
        meta: None,
    }
}

#[test]
fn mcp_call_metric_tags_include_server_name() {
    assert_eq!(
        mcp_call_metric_tags("error", "docs server", "search docs", &[]),
        vec![
            ("status".to_string(), "error".to_string()),
            ("server".to_string(), "docs_server".to_string()),
            ("tool".to_string(), "search_docs".to_string()),
        ],
    );
}

#[test]
fn mcp_call_metric_tags_append_runtime_labels_without_overriding_core_tags() {
    let runtime_labels = vec![
        ("source_id".to_string(), "docs source".to_string()),
        ("status".to_string(), "runtime status".to_string()),
        ("error_code".to_string(), "runtime error".to_string()),
    ];

    assert_eq!(
        mcp_call_metric_tags("ok", "docs", "search", &runtime_labels),
        vec![
            ("status".to_string(), "ok".to_string()),
            ("server".to_string(), "docs".to_string()),
            ("tool".to_string(), "search".to_string()),
            ("source_id".to_string(), "docs_source".to_string()),
        ],
    );
}

#[test]
fn mcp_call_metric_outcome_distinguishes_request_and_tool_errors() {
    assert_eq!(
        mcp_call_metric_outcome(&Ok(metric_call_tool_result(
            /*is_error*/ false, /*structured_content*/ None,
        )),),
        McpCallMetricOutcome {
            status: "ok",
            error_type: None,
            error_code: None,
        }
    );
    assert_eq!(
        mcp_call_metric_outcome(&Ok(metric_call_tool_result(
            /*is_error*/ true,
            Some(serde_json::json!({"error_code": "RATE_LIMITED"})),
        )),),
        McpCallMetricOutcome {
            status: "error",
            error_type: Some(MCP_CALL_ERROR_TYPE_TOOL_RESULT),
            error_code: Some("RATE_LIMITED".to_string()),
        }
    );
    assert_eq!(
        mcp_call_metric_outcome(&Err("connection closed".to_string())),
        McpCallMetricOutcome {
            status: "error",
            error_type: Some(MCP_CALL_ERROR_TYPE_MCP_REQUEST),
            error_code: Some(MCP_CALL_ERROR_CODE_UNKNOWN.to_string()),
        }
    );
}

#[test]
fn mcp_call_metric_outcome_reports_server_tool_error_codes() {
    let result = Ok(metric_call_tool_result(
        /*is_error*/ true,
        Some(serde_json::json!({"error_code": "arbitrary-user-value"})),
    ));

    assert_eq!(
        mcp_call_metric_outcome(&result),
        McpCallMetricOutcome {
            status: "error",
            error_type: Some(MCP_CALL_ERROR_TYPE_TOOL_RESULT),
            error_code: Some("arbitrary-user-value".to_string()),
        }
    );
}

#[test]
fn mcp_call_metric_outcome_reads_model_private_error_code_metadata() {
    let mut result =
        metric_call_tool_result(/*is_error*/ true, /*structured_content*/ None);
    result.meta = Some(serde_json::json!({
        MCP_ERROR_CODE_META_KEY: "AUTH_REQUIRED",
    }));

    assert_eq!(
        mcp_call_metric_outcome(&Ok(result)),
        McpCallMetricOutcome {
            status: "error",
            error_type: Some(MCP_CALL_ERROR_TYPE_TOOL_RESULT),
            error_code: Some("AUTH_REQUIRED".to_string()),
        }
    );
}

#[test]
fn mcp_call_metric_outcome_bounds_and_sanitizes_error_code() {
    let raw_error_code = format!("BAD CODE {}", "x".repeat(300));
    let result = Ok(metric_call_tool_result(
        /*is_error*/ true,
        Some(serde_json::json!({"error_code": raw_error_code})),
    ));

    assert_eq!(
        mcp_call_metric_outcome(&result),
        McpCallMetricOutcome {
            status: "error",
            error_type: Some(MCP_CALL_ERROR_TYPE_TOOL_RESULT),
            error_code: Some(format!("BAD_CODE_{}", "x".repeat(247))),
        }
    );
}
