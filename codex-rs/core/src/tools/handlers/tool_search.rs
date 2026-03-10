use crate::client_common::tools::ResponsesApiNamespace;
use crate::client_common::tools::ResponsesApiNamespaceTool;
use crate::client_common::tools::ToolSearchOutputTool;
use crate::connectors::sanitize_name;
use crate::function_tool::FunctionCallError;
use crate::mcp_connection_manager::ToolInfo;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::ToolSearchOutput;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use crate::tools::spec::mcp_tool_to_deferred_openai_tool;
use async_trait::async_trait;
use bm25::Document;
use bm25::Language;
use bm25::SearchEngineBuilder;
use serde::Deserialize;
use serde_json::Value;
use serde_json::to_value;
use std::collections::BTreeMap;
use std::collections::HashMap;

pub struct ToolSearchHandler {
    tools: HashMap<String, ToolInfo>,
}

pub(crate) const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";
pub(crate) const DEFAULT_LIMIT: usize = 8;

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

#[derive(Deserialize)]
struct ToolSearchArgs {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Clone)]
struct ToolEntry {
    name: String,
    info: ToolInfo,
    search_text: String,
}

impl From<HashMap<String, ToolInfo>> for ToolSearchHandler {
    fn from(tools: HashMap<String, ToolInfo>) -> Self {
        Self { tools }
    }
}

impl ToolEntry {
    fn new(name: String, info: ToolInfo) -> Self {
        let input_keys = info
            .tool
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .map(|map| map.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let search_text = build_search_text(&name, &info, &input_keys);
        Self {
            name,
            info,
            search_text,
        }
    }
}

#[async_trait]
impl ToolHandler for ToolSearchHandler {
    type Output = ToolSearchOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<ToolSearchOutput, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;

        let arguments = match payload {
            ToolPayload::ToolSearch { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::Fatal(format!(
                    "{TOOL_SEARCH_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };

        let args: ToolSearchArgs = serde_json::from_value(arguments).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to parse tool_search arguments: {err}"
            ))
        })?;
        let query = args.query.trim();
        if query.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "query must not be empty".to_string(),
            ));
        }

        if args.limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        let limit = args.limit;

        let mut entries: Vec<ToolEntry> = self
            .tools
            .clone()
            .into_iter()
            .map(|(name, info)| ToolEntry::new(name, info))
            .collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        if entries.is_empty() {
            return Ok(ToolSearchOutput { tools: Vec::new() });
        }

        let documents: Vec<Document<usize>> = entries
            .iter()
            .enumerate()
            .map(|(idx, entry)| Document::new(idx, entry.search_text.clone()))
            .collect();
        let search_engine =
            SearchEngineBuilder::<usize>::with_documents(Language::English, documents).build();
        let results = search_engine.search(query, limit);

        let matched_entries = results
            .into_iter()
            .filter_map(|result| entries.get(result.document.id))
            .collect::<Vec<_>>();
        let tools = serialize_tool_search_output_tools(&matched_entries).map_err(|err| {
            FunctionCallError::Fatal(format!("failed to encode tool_search output: {err}"))
        })?;

        Ok(ToolSearchOutput { tools })
    }
}

fn serialize_tool_search_output_tools(
    matched_entries: &[&ToolEntry],
) -> Result<Vec<Value>, serde_json::Error> {
    let grouped: BTreeMap<String, Vec<ToolEntry>> =
        matched_entries
            .iter()
            .fold(BTreeMap::new(), |mut acc, tool| {
                let (namespace, _) = split_namespace_and_name(&tool.name, &tool.info);
                acc.entry(namespace).or_default().push((*tool).clone());

                acc
            });

    let mut serialized = Vec::with_capacity(grouped.len());
    for (namespace, tools) in grouped {
        let Some(first_tool) = tools.first() else {
            continue;
        };

        let description = first_tool.info.connector_description.clone().or_else(|| {
            first_tool
                .info
                .connector_name
                .as_deref()
                .map(str::trim)
                .filter(|connector_name| !connector_name.is_empty())
                .map(|connector_name| format!("Tools for working with {connector_name}."))
        });

        let tools = tools
            .iter()
            .map(|tool| {
                let (_, tool_name) = split_namespace_and_name(&tool.name, &tool.info);
                mcp_tool_to_deferred_openai_tool(tool_name, tool.info.tool.clone())
                    .map(ResponsesApiNamespaceTool::Function)
            })
            .collect::<Result<Vec<_>, _>>()?;

        serialized.push(to_value(ToolSearchOutputTool::Namespace(
            ResponsesApiNamespace {
                name: namespace,
                description: description.unwrap_or_default(),
                tools,
            },
        ))?);
    }

    Ok(serialized)
}

pub(crate) fn split_namespace_and_name(
    qualified_tool_name: &str,
    tool: &ToolInfo,
) -> (String, String) {
    let namespace = if let Some(connector_name) = tool.connector_name.clone() {
        format!(
            "mcp__{}__{}",
            tool.server_name,
            sanitize_name(&connector_name)
        )
    } else {
        format!("mcp__{}", tool.server_name)
    };
    let tool_name = qualified_tool_name
        .strip_prefix(&namespace)
        .unwrap_or(&tool.tool_name);

    (namespace, tool_name.to_string())
}

fn build_search_text(name: &str, info: &ToolInfo, input_keys: &[String]) -> String {
    let mut parts = vec![
        name.to_string(),
        info.tool_name.clone(),
        info.server_name.clone(),
    ];

    if let Some(title) = info.tool.title.as_deref()
        && !title.trim().is_empty()
    {
        parts.push(title.to_string());
    }

    if let Some(description) = info.tool.description.as_deref()
        && !description.trim().is_empty()
    {
        parts.push(description.to_string());
    }

    if let Some(connector_name) = info.connector_name.as_deref()
        && !connector_name.trim().is_empty()
    {
        parts.push(connector_name.to_string());
    }

    if let Some(connector_description) = info.connector_description.as_deref()
        && !connector_description.trim().is_empty()
    {
        parts.push(connector_description.to_string());
    }

    if !input_keys.is_empty() {
        parts.extend(input_keys.iter().cloned());
    }

    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
    use pretty_assertions::assert_eq;
    use rmcp::model::JsonObject;
    use rmcp::model::Tool;
    use serde_json::json;
    use std::sync::Arc;

    fn test_tool_info(tool_name: &str, connector_name: Option<&str>) -> ToolInfo {
        ToolInfo {
            server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
            tool_name: tool_name.to_string(),
            tool: Tool {
                name: tool_name.to_string().into(),
                title: None,
                description: Some("Test tool.".into()),
                input_schema: Arc::new(JsonObject::from_iter([(
                    "type".to_string(),
                    json!("object"),
                )])),
                output_schema: None,
                annotations: None,
                execution: None,
                icons: None,
                meta: None,
            },
            connector_id: None,
            connector_name: connector_name.map(str::to_string),
            plugin_display_names: Vec::new(),
            connector_description: None,
        }
    }

    #[test]
    fn serialize_tool_search_output_tools_groups_results_by_namespace() {
        let entries = [
            ToolEntry::new(
                "mcp__codex_apps__calendar_create_event".to_string(),
                ToolInfo {
                    server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                    tool_name: "calendar_create_event".to_string(),
                    tool: Tool {
                        name: "calendar_create_event".to_string().into(),
                        title: None,
                        description: Some("Create a calendar event.".into()),
                        input_schema: Arc::new(JsonObject::from_iter([(
                            "type".to_string(),
                            json!("object"),
                        )])),
                        output_schema: None,
                        annotations: None,
                        execution: None,
                        icons: None,
                        meta: None,
                    },
                    connector_id: Some("calendar".to_string()),
                    connector_name: Some("Calendar".to_string()),
                    plugin_display_names: Vec::new(),
                    connector_description: Some("Plan events".to_string()),
                },
            ),
            ToolEntry::new(
                "mcp__codex_apps__gmail_read_email".to_string(),
                ToolInfo {
                    server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                    tool_name: "gmail_read_email".to_string(),
                    tool: Tool {
                        name: "gmail_read_email".to_string().into(),
                        title: None,
                        description: Some("Read an email.".into()),
                        input_schema: Arc::new(JsonObject::from_iter([(
                            "type".to_string(),
                            json!("object"),
                        )])),
                        output_schema: None,
                        annotations: None,
                        execution: None,
                        icons: None,
                        meta: None,
                    },
                    connector_id: Some("gmail".to_string()),
                    connector_name: Some("Gmail".to_string()),
                    plugin_display_names: Vec::new(),
                    connector_description: Some("Read mail".to_string()),
                },
            ),
            ToolEntry::new(
                "mcp__codex_apps__calendar_list_events".to_string(),
                ToolInfo {
                    server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                    tool_name: "calendar_list_events".to_string(),
                    tool: Tool {
                        name: "calendar_list_events".to_string().into(),
                        title: None,
                        description: Some("List calendar events.".into()),
                        input_schema: Arc::new(JsonObject::from_iter([(
                            "type".to_string(),
                            json!("object"),
                        )])),
                        output_schema: None,
                        annotations: None,
                        execution: None,
                        icons: None,
                        meta: None,
                    },
                    connector_id: Some("calendar".to_string()),
                    connector_name: Some("Calendar".to_string()),
                    plugin_display_names: Vec::new(),
                    connector_description: Some("Plan events".to_string()),
                },
            ),
        ];

        let tools = serialize_tool_search_output_tools(&[&entries[0], &entries[1], &entries[2]])
            .expect("serialize tool search output");

        assert_eq!(
            tools,
            vec![
                json!({
                    "type": "namespace",
                    "name": "mcp__codex_apps__calendar",
                    "description": "Plan events",
                    "tools": [
                        {
                            "type": "function",
                            "name": "create_event",
                            "description": "Create a calendar event.",
                            "strict": false,
                            "defer_loading": true,
                            "parameters": {
                                "type": "object",
                                "properties": {}
                            }
                        },
                        {
                            "type": "function",
                            "name": "list_events",
                            "description": "List calendar events.",
                            "strict": false,
                            "defer_loading": true,
                            "parameters": {
                                "type": "object",
                                "properties": {}
                            }
                        }
                    ]
                }),
                json!({
                    "type": "namespace",
                    "name": "mcp__codex_apps__gmail",
                    "description": "Read mail",
                    "tools": [
                        {
                            "type": "function",
                            "name": "read_email",
                            "description": "Read an email.",
                            "strict": false,
                            "defer_loading": true,
                            "parameters": {
                                "type": "object",
                                "properties": {}
                            }
                        }
                    ]
                })
            ]
        );
    }

    #[test]
    fn serialize_tool_search_output_tools_falls_back_to_connector_name_description() {
        let entries = [ToolEntry::new(
            "mcp__codex_apps__gmail_batch_read_email".to_string(),
            ToolInfo {
                server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                tool_name: "gmail_batch_read_email".to_string(),
                tool: Tool {
                    name: "gmail_batch_read_email".to_string().into(),
                    title: None,
                    description: Some("Read multiple emails.".into()),
                    input_schema: Arc::new(JsonObject::from_iter([(
                        "type".to_string(),
                        json!("object"),
                    )])),
                    output_schema: None,
                    annotations: None,
                    execution: None,
                    icons: None,
                    meta: None,
                },
                connector_id: Some("connector_gmail_456".to_string()),
                connector_name: Some("Gmail".to_string()),
                plugin_display_names: Vec::new(),
                connector_description: None,
            },
        )];

        let tools = serialize_tool_search_output_tools(&[&entries[0]]).expect("serialize");

        assert_eq!(
            tools,
            vec![json!({
                "type": "namespace",
                "name": "mcp__codex_apps__gmail",
                "description": "Tools for working with Gmail.",
                "tools": [
                    {
                        "type": "function",
                        "name": "batch_read_email",
                        "description": "Read multiple emails.",
                        "strict": false,
                        "defer_loading": true,
                        "parameters": {
                            "type": "object",
                            "properties": {}
                        }
                    }
                ]
            })]
        );
    }

    #[test]
    fn split_namespace_and_name_splits_qualified_tool_name() {
        assert_eq!(
            split_namespace_and_name(
                "mcp__codex_apps__gmail_batch_read_email",
                &test_tool_info("gmail_batch_read_email", Some("Gmail"))
            ),
            (
                "mcp__codex_apps__gmail".to_string(),
                "batch_read_email".to_string()
            )
        );
        assert_eq!(
            split_namespace_and_name(
                "mcp__codex_apps__gmail_search_emails_personalization",
                &test_tool_info("gmail_search_emails_personalization", Some("Gmail"))
            ),
            (
                "mcp__codex_apps__gmail".to_string(),
                "search_emails_personalization".to_string()
            )
        );
    }

    #[test]
    fn split_namespace_and_name_uses_server_name_without_connector_name() {
        assert_eq!(
            split_namespace_and_name(
                "mcp__codex_apps__read_email",
                &test_tool_info("read_email", None)
            ),
            ("mcp__codex_apps".to_string(), "read_email".to_string())
        );
    }
}
