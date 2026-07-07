use super::*;
use pretty_assertions::assert_eq;
use rmcp::model::JsonObject;
use rmcp::model::Meta;
use rmcp::model::Tool;
use serde_json::json;
use std::borrow::Cow;
use std::sync::Arc;

const CONNECTOR_ID: &str = "catalog-a";
const LINK_ID: &str = "link-catalog-a";
const RESOURCE_URI: &str = "ui://widget/product-search.html";
const TOOL_NAME: &str = "catalog_a_search";

#[test]
fn rejects_connector_link_uri_and_route_identity_mismatches() {
    let mut mismatched_connector = origin(Some(LINK_ID));
    mismatched_connector.connector_id = "catalog-b".to_string();
    assert_error_contains(
        build_plugin_runtime_fetch_resource_meta(&mismatched_connector, RESOURCE_URI, &tool_info()),
        "does not match app context connector",
    );

    let mismatched_link = origin(Some("link-catalog-b"));
    assert_error_contains(
        build_plugin_runtime_fetch_resource_meta(&mismatched_link, RESOURCE_URI, &tool_info()),
        "link does not match app context link",
    );

    assert_error_contains(
        build_plugin_runtime_fetch_resource_meta(
            &origin(Some(LINK_ID)),
            "ui://widget/other.html",
            &tool_info(),
        ),
        "does not match requested URI",
    );

    let mut mismatched_route = tool_info();
    set_codex_apps_meta(
        &mut mismatched_route,
        json!({
            "resource_uri": "/catalog-b/link-catalog-b/catalog_b_search",
            "contains_mcp_source": true,
        }),
    );
    assert_error_contains(
        build_plugin_runtime_fetch_resource_meta(
            &origin(Some(LINK_ID)),
            RESOURCE_URI,
            &mismatched_route,
        ),
        "resource route does not match connector and link",
    );
}

#[test]
fn accepts_missing_origin_link_only_for_synthetic_tools() {
    let mut synthetic_tool = tool_info();
    set_codex_apps_meta(
        &mut synthetic_tool,
        json!({
            "resource_uri": format!("/{CONNECTOR_ID}/{LINK_ID}/{TOOL_NAME}"),
            "contains_mcp_source": true,
            "synthetic_link": true,
        }),
    );

    assert_eq!(
        build_plugin_runtime_fetch_resource_meta(
            &origin(/*link_id*/ None),
            RESOURCE_URI,
            &synthetic_tool,
        )
        .expect("synthetic tool accepts missing origin link")
        .pointer("/_codex_apps/synthetic_link"),
        Some(&json!(true))
    );
    assert_error_contains(
        build_plugin_runtime_fetch_resource_meta(
            &origin(/*link_id*/ None),
            RESOURCE_URI,
            &tool_info(),
        ),
        "link does not match app context link",
    );
}

#[test]
fn rejects_missing_or_malformed_tool_metadata() {
    let mut missing_metadata = tool_info();
    missing_metadata.tool.meta = None;
    assert_error_contains(
        build_plugin_runtime_fetch_resource_meta(
            &origin(Some(LINK_ID)),
            RESOURCE_URI,
            &missing_metadata,
        ),
        "missing metadata",
    );

    let mut malformed_route = tool_info();
    set_codex_apps_meta(
        &mut malformed_route,
        json!({ "resource_uri": "/catalog-a/link-catalog-a" }),
    );
    assert_error_contains(
        build_plugin_runtime_fetch_resource_meta(
            &origin(Some(LINK_ID)),
            RESOURCE_URI,
            &malformed_route,
        ),
        "invalid resource route",
    );
}

fn origin(link_id: Option<&str>) -> McpResourceOrigin {
    McpResourceOrigin {
        server: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        tool: TOOL_NAME.to_string(),
        connector_id: CONNECTOR_ID.to_string(),
        link_id: link_id.map(str::to_string),
        resource_uri: RESOURCE_URI.to_string(),
    }
}

fn set_codex_apps_meta(tool_info: &mut ToolInfo, value: serde_json::Value) {
    tool_info
        .tool
        .meta
        .as_mut()
        .expect("tool metadata")
        .0
        .insert(MCP_TOOL_CODEX_APPS_META_KEY.to_string(), value);
}

fn tool_info() -> ToolInfo {
    let mut tool = Tool::new(
        Cow::Borrowed(TOOL_NAME),
        Cow::Borrowed("Search catalog A."),
        Arc::new(JsonObject::new()),
    );
    tool.meta = Some(Meta(serde_json::Map::from_iter([
        ("link_id".to_string(), json!(LINK_ID)),
        ("openai/outputTemplate".to_string(), json!(RESOURCE_URI)),
        (
            MCP_TOOL_CODEX_APPS_META_KEY.to_string(),
            json!({
                "resource_uri": format!("/{CONNECTOR_ID}/{LINK_ID}/{TOOL_NAME}"),
                "contains_mcp_source": true,
            }),
        ),
    ])));
    ToolInfo {
        server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        supports_parallel_tool_calls: false,
        server_origin: None,
        callable_name: "_product_search".to_string(),
        callable_namespace: "mcp__codex_apps__catalog_a".to_string(),
        namespace_description: Some("Search catalog A.".to_string()),
        tool,
        connector_id: Some(CONNECTOR_ID.to_string()),
        connector_name: Some("Catalog A".to_string()),
        plugin_display_names: Vec::new(),
    }
}

fn assert_error_contains<T>(result: anyhow::Result<T>, expected: &str) {
    let error = result.err().expect("expected validation failure");
    assert!(
        error.to_string().contains(expected),
        "expected error containing {expected:?}, got {error:#}"
    );
}
