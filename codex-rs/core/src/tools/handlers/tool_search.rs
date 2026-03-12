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
#[path = "tool_search_tests.rs"]
mod tests;
