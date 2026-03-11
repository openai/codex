use crate::client_common::tools::ResponsesApiNamespace;
use crate::client_common::tools::ResponsesApiNamespaceTool;
use crate::client_common::tools::ToolSearchOutputTool;
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
use std::collections::BTreeMap;
use std::collections::HashMap;

#[cfg(test)]
use crate::client_common::tools::ResponsesApiTool;

pub struct ToolSearchHandler {
    tools: HashMap<String, ToolInfo>,
}

pub(crate) const TOOL_SEARCH_TOOL_NAME: &str = "tool_search";
pub(crate) const DEFAULT_LIMIT: usize = 8;

impl ToolSearchHandler {
    pub fn new(tools: HashMap<String, ToolInfo>) -> Self {
        Self { tools }
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

        let args = match payload {
            ToolPayload::ToolSearch { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::Fatal(format!(
                    "{TOOL_SEARCH_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };

        let query = args.query.trim();
        if query.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "query must not be empty".to_string(),
            ));
        }
        let limit = args.limit.unwrap_or(DEFAULT_LIMIT);

        if limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        let mut entries: Vec<(String, ToolInfo)> = self.tools.clone().into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));

        if entries.is_empty() {
            return Ok(ToolSearchOutput { tools: Vec::new() });
        }

        let documents: Vec<Document<usize>> = entries
            .iter()
            .enumerate()
            .map(|(idx, (name, info))| Document::new(idx, build_search_text(name, info)))
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
    matched_entries: &[&(String, ToolInfo)],
) -> Result<Vec<ToolSearchOutputTool>, serde_json::Error> {
    let grouped: BTreeMap<String, Vec<ToolInfo>> =
        matched_entries
            .iter()
            .fold(BTreeMap::new(), |mut acc, (_name, tool)| {
                acc.entry(tool.tool_namespace.clone())
                    .or_default()
                    .push(tool.clone());

                acc
            });

    let mut results = Vec::with_capacity(grouped.len());
    for (namespace, tools) in grouped {
        let Some(first_tool) = tools.first() else {
            continue;
        };

        let description = first_tool.connector_description.clone().or_else(|| {
            first_tool
                .connector_name
                .as_deref()
                .map(str::trim)
                .filter(|connector_name| !connector_name.is_empty())
                .map(|connector_name| format!("Tools for working with {connector_name}."))
        });

        let tools = tools
            .iter()
            .map(|tool| {
                mcp_tool_to_deferred_openai_tool(tool.tool_name.clone(), tool.tool.clone())
                    .map(ResponsesApiNamespaceTool::Function)
            })
            .collect::<Result<Vec<_>, _>>()?;

        results.push(ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
            name: namespace,
            description: description.unwrap_or_default(),
            tools,
        }));
    }

    Ok(results)
}

fn build_search_text(name: &str, info: &ToolInfo) -> String {
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

    parts.extend(
        info.tool
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .map(|map| map.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default(),
    );

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

    #[test]
    fn serialize_tool_search_output_tools_groups_results_by_namespace() {
        let entries = [
            (
                "mcp__codex_apps__calendar-create-event".to_string(),
                ToolInfo {
                    server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                    tool_name: "-create-event".to_string(),
                    tool_namespace: "mcp__codex_apps__calendar".to_string(),
                    tool: Tool {
                        name: "calendar-create-event".to_string().into(),
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
            (
                "mcp__codex_apps__gmail-read-email".to_string(),
                ToolInfo {
                    server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                    tool_name: "-read-email".to_string(),
                    tool_namespace: "mcp__codex_apps__gmail".to_string(),
                    tool: Tool {
                        name: "gmail-read-email".to_string().into(),
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
            (
                "mcp__codex_apps__calendar-list-events".to_string(),
                ToolInfo {
                    server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                    tool_name: "-list-events".to_string(),
                    tool_namespace: "mcp__codex_apps__calendar".to_string(),
                    tool: Tool {
                        name: "calendar-list-events".to_string().into(),
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
                ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
                    name: "mcp__codex_apps__calendar".to_string(),
                    description: "Plan events".to_string(),
                    tools: vec![
                        ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                            name: "-create-event".to_string(),
                            description: "Create a calendar event.".to_string(),
                            strict: false,
                            defer_loading: Some(true),
                            parameters: crate::tools::spec::JsonSchema::Object {
                                properties: Default::default(),
                                required: None,
                                additional_properties: None,
                            },
                            output_schema: None,
                        }),
                        ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                            name: "-list-events".to_string(),
                            description: "List calendar events.".to_string(),
                            strict: false,
                            defer_loading: Some(true),
                            parameters: crate::tools::spec::JsonSchema::Object {
                                properties: Default::default(),
                                required: None,
                                additional_properties: None,
                            },
                            output_schema: None,
                        }),
                    ],
                }),
                ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
                    name: "mcp__codex_apps__gmail".to_string(),
                    description: "Read mail".to_string(),
                    tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                        name: "-read-email".to_string(),
                        description: "Read an email.".to_string(),
                        strict: false,
                        defer_loading: Some(true),
                        parameters: crate::tools::spec::JsonSchema::Object {
                            properties: Default::default(),
                            required: None,
                            additional_properties: None,
                        },
                        output_schema: None,
                    })],
                })
            ]
        );
    }

    #[test]
    fn serialize_tool_search_output_tools_falls_back_to_connector_name_description() {
        let entries = [(
            "mcp__codex_apps__gmail-batch-read-email".to_string(),
            ToolInfo {
                server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                tool_name: "-batch-read-email".to_string(),
                tool_namespace: "mcp__codex_apps__gmail".to_string(),
                tool: Tool {
                    name: "gmail-batch-read-email".to_string().into(),
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
            vec![ToolSearchOutputTool::Namespace(ResponsesApiNamespace {
                name: "mcp__codex_apps__gmail".to_string(),
                description: "Tools for working with Gmail.".to_string(),
                tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                    name: "-batch-read-email".to_string(),
                    description: "Read multiple emails.".to_string(),
                    strict: false,
                    defer_loading: Some(true),
                    parameters: crate::tools::spec::JsonSchema::Object {
                        properties: Default::default(),
                        required: None,
                        additional_properties: None,
                    },
                    output_schema: None,
                })],
            })]
        );
    }
}
