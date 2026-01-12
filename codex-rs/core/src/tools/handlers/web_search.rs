use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::function_tool::FunctionCallError;
use crate::tavily::TavilyRequest;
use crate::tavily::search_tavily;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct WebSearchHandler;

const DEFAULT_LIMIT: usize = 10;

#[derive(Deserialize)]
struct WebSearchArgs {
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    DEFAULT_LIMIT
}

#[async_trait]
impl ToolHandler for WebSearchHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation { payload, turn, .. } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "web_search handler received unsupported payload".to_string(),
                ));
            }
        };

        let args: WebSearchArgs = serde_json::from_str(&arguments).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to parse web_search arguments: {err:?}"
            ))
        })?;

        if args.limit == 0 {
            return Err(FunctionCallError::RespondToModel(
                "limit must be greater than zero".to_string(),
            ));
        }

        let config = turn.client.config();
        let api_key = match config
            .tavily_api_key
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(key) => key.to_string(),
            None => {
                return Err(FunctionCallError::RespondToModel(
                    "Tavily API key is not set; enable it via tavily_api_key in ~/.codex/config.toml"
                        .to_string(),
                ));
            }
        };

        let results = search_tavily(TavilyRequest {
            api_key,
            query: args.query.clone(),
            limit: args.limit,
        })
        .await
        .map_err(|err| FunctionCallError::RespondToModel(format!("Tavily search failed: {err}")))?;

        let payload = json!({
            "query": args.query,
            "limit": args.limit,
            "results": results,
            "source": "tavily",
        });
        let content = serde_json::to_string(&payload).map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to serialize web_search output: {err}"
            ))
        })?;

        Ok(ToolOutput::Function {
            content,
            content_items: None,
            success: Some(true),
        })
    }
}
