use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::ToolSearchOutput;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use bm25::Document;
use bm25::Language;
use bm25::SearchEngineBuilder;
use codex_mcp::ToolInfo;
use codex_tools::TOOL_SEARCH_DEFAULT_LIMIT;
use codex_tools::TOOL_SEARCH_TOOL_NAME;
use codex_tools::ToolSearchResultSource;
use codex_tools::collect_tool_search_output_tools;
use std::collections::HashMap;

pub struct ToolSearchHandler {
    tools: HashMap<String, ToolInfo>,
}

impl ToolSearchHandler {
    pub fn new(tools: HashMap<String, ToolInfo>) -> Self {
        Self { tools }
    }
}

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
        let limit = args.limit.unwrap_or(TOOL_SEARCH_DEFAULT_LIMIT);

        if limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        let mut entries: Vec<(String, ToolInfo)> = self.tools.clone().into_iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (_name, tool) in &mut entries {
            tool.tool = tool.model_visible_tool();
        }

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

        let tools = collect_tool_search_output_tools(
            results
                .into_iter()
                .filter_map(|result| entries.get(result.document.id))
                .map(|(_name, tool)| ToolSearchResultSource {
                    tool_namespace: tool.tool_namespace.as_str(),
                    tool_name: tool.tool_name.as_str(),
                    tool: &tool.tool,
                    connector_name: tool.connector_name.as_deref(),
                    connector_description: tool.connector_description.as_deref(),
                }),
        )
        .map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to encode {TOOL_SEARCH_TOOL_NAME} output: {err}"
            ))
        })?;

        Ok(ToolSearchOutput { tools })
    }
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
