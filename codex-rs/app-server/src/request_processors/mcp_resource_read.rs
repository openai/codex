use super::mcp_processor::with_mcp_tool_call_thread_id_meta;
use super::*;
use crate::mcp_resource_origin::McpResourceOrigin;
use anyhow::Context;
use codex_mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_mcp::MCP_TOOL_CODEX_APPS_META_KEY;
use codex_mcp::ToolInfo;
use serde_json::Value;

const LEGACY_CODEX_APPS_IMPLEMENTATION_NAME: &str = "codex-connectors-mcp";
const PLUGIN_RUNTIME_IMPLEMENTATION_NAME: &str = "plugin-runtime";
const FETCH_RESOURCE_TOOL_NAME: &str = "fetch_resource";

pub(super) async fn read_thread_mcp_resource(
    thread_state_manager: &ThreadStateManager,
    thread_id: ThreadId,
    thread_id_string: &str,
    thread: &Arc<CodexThread>,
    server: &str,
    uri: &str,
    origin_call_id: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let Some(origin_call_id) = origin_call_id else {
        return thread.read_mcp_resource(server, uri).await;
    };
    if server != CODEX_APPS_MCP_SERVER_NAME {
        return thread.read_mcp_resource(server, uri).await;
    }

    let runtime = thread.current_mcp_runtime().await;
    let connection = runtime.manager().server_connection(server).await?;
    match connection.server_info().name.as_str() {
        LEGACY_CODEX_APPS_IMPLEMENTATION_NAME => {
            let result = connection.read_resource(uri).await?;
            return Ok(serde_json::to_value(result)?);
        }
        PLUGIN_RUNTIME_IMPLEMENTATION_NAME => {}
        implementation => {
            anyhow::bail!("unsupported codex_apps MCP server implementation `{implementation}`")
        }
    }

    let origin =
        resolve_mcp_resource_origin(thread_state_manager, thread_id, thread, origin_call_id)
            .await?;
    if origin.server != server {
        anyhow::bail!(
            "originating MCP tool call server `{}` does not match resource server `{server}`",
            origin.server
        );
    }
    if origin.resource_uri != uri {
        anyhow::bail!(
            "originating MCP tool call resource URI `{}` does not match requested URI `{uri}`",
            origin.resource_uri
        );
    }

    let tool_info = connection.tool_info(&origin.tool).with_context(|| {
        format!(
            "originating MCP tool `{}` is not available on server `{server}`",
            origin.tool
        )
    })?;
    let meta = build_plugin_runtime_fetch_resource_meta(&origin, uri, &tool_info)?;
    let meta = with_mcp_tool_call_thread_id_meta(Some(meta), thread_id_string);
    let result = connection
        .call_tool(
            FETCH_RESOURCE_TOOL_NAME,
            Some(serde_json::json!({ "uri": uri })),
            meta,
        )
        .await?;
    plugin_runtime_fetch_resource_response(result)
}

async fn resolve_mcp_resource_origin(
    thread_state_manager: &ThreadStateManager,
    thread_id: ThreadId,
    thread: &Arc<CodexThread>,
    origin_call_id: &str,
) -> anyhow::Result<McpResourceOrigin> {
    let thread_state = thread_state_manager.thread_state(thread_id).await;
    if let Some(origin) = thread_state
        .lock()
        .await
        .mcp_resource_origin(origin_call_id)
    {
        return Ok(origin);
    }

    let history = thread.load_history(/*include_archived*/ true).await;
    if let Some(origin) = thread_state
        .lock()
        .await
        .mcp_resource_origin(origin_call_id)
    {
        return Ok(origin);
    }
    let history = history?;
    let history_origin = build_api_turns_from_rollout_items(&history.items)
        .iter()
        .rev()
        .find_map(|turn| McpResourceOrigin::find(&turn.items, origin_call_id));
    let mut thread_state = thread_state.lock().await;
    if let Some(origin) = thread_state.mcp_resource_origin(origin_call_id) {
        return Ok(origin);
    }
    let origin = history_origin
        .with_context(|| format!("originating MCP tool call `{origin_call_id}` was not found"))?;
    thread_state
        .mcp_resource_origins
        .insert(origin_call_id.to_string(), origin.clone());
    Ok(origin)
}

fn build_plugin_runtime_fetch_resource_meta(
    origin: &McpResourceOrigin,
    uri: &str,
    tool_info: &ToolInfo,
) -> anyhow::Result<serde_json::Value> {
    // Resolve the current server-owned tool identity.
    let tool_meta = tool_info
        .tool
        .meta
        .as_ref()
        .context("originating MCP tool is missing metadata")?;
    let connector_id = tool_info
        .connector_id
        .as_deref()
        .context("originating MCP tool is missing connector_id")?;
    let link_id = tool_meta
        .0
        .get("link_id")
        .and_then(Value::as_str)
        .context("originating MCP tool is missing link_id")?;
    let codex_apps_meta = tool_meta
        .0
        .get(MCP_TOOL_CODEX_APPS_META_KEY)
        .and_then(Value::as_object)
        .context("originating MCP tool is missing _codex_apps metadata")?;
    let resource_uri = mcp_app_resource_uri(&tool_meta.0)
        .context("originating MCP tool is missing its app resource URI")?;

    // Cross-check persisted provenance, the request, and current tool metadata.
    if resource_uri != uri {
        anyhow::bail!(
            "originating MCP tool resource URI `{resource_uri}` does not match requested URI `{uri}`"
        );
    }
    if connector_id != origin.connector_id {
        anyhow::bail!(
            "originating MCP tool connector `{connector_id}` does not match app context connector `{}`",
            origin.connector_id
        );
    }

    let synthetic_link = codex_apps_meta
        .get("synthetic_link")
        .and_then(Value::as_bool)
        == Some(true);
    match origin.link_id.as_deref() {
        Some(origin_link_id) if origin_link_id == link_id => {}
        None if synthetic_link => {}
        _ => anyhow::bail!("originating MCP tool link does not match app context link"),
    }

    let tool_resource_route = codex_apps_meta
        .get("resource_uri")
        .and_then(Value::as_str)
        .context("originating MCP tool is missing its resource route")?;
    let route_segments = tool_resource_route
        .strip_prefix('/')
        .map(|route| route.split('/').collect::<Vec<_>>())
        .filter(|segments| segments.len() == 3 && segments.iter().all(|part| !part.is_empty()))
        .context("originating MCP tool has an invalid resource route")?;
    if route_segments[0] != connector_id || route_segments[1] != link_id {
        anyhow::bail!("originating MCP tool resource route does not match connector and link");
    }

    // Construct the canonical route without forwarding client-provided routing.
    let mut fetch_resource_meta = serde_json::json!({
        "resource_uri": format!("/{connector_id}/{link_id}/{FETCH_RESOURCE_TOOL_NAME}"),
        "contains_mcp_source": codex_apps_meta
            .get("contains_mcp_source")
            .and_then(Value::as_bool)
            == Some(true),
    });
    if synthetic_link {
        fetch_resource_meta["synthetic_link"] = Value::Bool(true);
    }

    let mut meta = serde_json::json!({
        "_codex_apps": fetch_resource_meta,
        "x-codex-turn-metadata": {
            "mcp_request_meta": {
                "selected_connector_ids": [connector_id],
            }
        },
    });
    if let Some(connector_name) = tool_info.connector_name.as_ref() {
        meta["connector_name"] = Value::String(connector_name.clone());
    }
    if let Some(connector_description) = tool_info.namespace_description.as_ref() {
        meta["connector_description"] = Value::String(connector_description.clone());
    }
    Ok(meta)
}

fn mcp_app_resource_uri(meta: &serde_json::Map<String, Value>) -> Option<&str> {
    meta.get("ui")
        .and_then(Value::as_object)
        .and_then(|ui| ui.get("resourceUri"))
        .and_then(Value::as_str)
        .or_else(|| meta.get("ui/resourceUri").and_then(Value::as_str))
        .or_else(|| meta.get("openai/outputTemplate").and_then(Value::as_str))
}

fn plugin_runtime_fetch_resource_response(
    result: codex_protocol::mcp::CallToolResult,
) -> anyhow::Result<serde_json::Value> {
    if result.is_error == Some(true) {
        anyhow::bail!("plugin-runtime fetch_resource returned an error");
    }
    let mut structured_content = result
        .structured_content
        .context("plugin-runtime fetch_resource returned no contents")?;
    let contents = structured_content
        .get_mut("contents")
        .and_then(Value::as_array_mut)
        .context("plugin-runtime fetch_resource returned invalid contents")?;
    for content in contents {
        if let Some(content) = content.as_object_mut()
            && !content.contains_key("_meta")
            && let Some(meta) = content.remove("meta")
        {
            content.insert("_meta".to_string(), meta);
        }
    }
    let response = serde_json::from_value::<McpResourceReadResponse>(structured_content)?;
    Ok(serde_json::to_value(response)?)
}

#[cfg(test)]
#[path = "mcp_resource_read_tests.rs"]
mod tests;
