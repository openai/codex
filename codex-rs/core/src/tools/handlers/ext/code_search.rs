//! Code Search Handler - Search indexed codebase
//!
//! This module provides the CodeSearchHandler which searches the indexed
//! codebase using the retrieval system (BM25 + optional vector search).
//!
//! Retrieval has its own independent configuration system:
//! - ~/.codex/retrieval.toml (global)
//! - .codex/retrieval.toml (project-level)

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use serde::Deserialize;

/// Maximum results limit
const MAX_LIMIT: i32 = 50;
/// Default results limit
const DEFAULT_LIMIT: i32 = 10;

/// Code Search tool arguments
#[derive(Debug, Clone, Deserialize)]
struct CodeSearchArgs {
    query: String,
    #[serde(default)]
    limit: Option<i32>,
}

/// Code Search Handler - simple wrapper around RetrievalService.
///
/// This handler is stateless. It obtains the RetrievalService from
/// the retrieval crate using `RetrievalService::for_workdir()` which
/// loads configuration from retrieval.toml files.
pub struct CodeSearchHandler;

impl CodeSearchHandler {
    /// Create a new stateless handler.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodeSearchHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolHandler for CodeSearchHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        // 1. Parse arguments
        let arguments = match &invocation.payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "Invalid payload type for code_search".to_string(),
                ));
            }
        };

        let args: CodeSearchArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        // Validate query
        if args.query.trim().is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "Query must not be empty".to_string(),
            ));
        }

        // Clamp limit
        let limit = args.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT).max(1);

        // 2. Get working directory from invocation context
        let cwd = invocation.turn.cwd.clone();

        // 3. Try to get RetrievalService (loads config from retrieval.toml)
        let service = match codex_retrieval::RetrievalService::for_workdir(&cwd).await {
            Ok(s) => s,
            Err(codex_retrieval::RetrievalErr::NotEnabled) => {
                return Ok(ToolOutput::Function {
                    content: "Code search is not enabled.\n\n\
                        To enable, create ~/.codex/retrieval.toml with:\n\
                        ```toml\n\
                        [retrieval]\n\
                        enabled = true\n\
                        ```\n\n\
                        Or create .codex/retrieval.toml in your project directory."
                        .to_string(),
                    content_items: None,
                    success: Some(false),
                });
            }
            Err(e) => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "Failed to initialize code search: {e}"
                )));
            }
        };

        // 4. Perform search
        let results = service
            .search(&args.query)
            .await
            .map_err(|e| FunctionCallError::RespondToModel(format!("Search failed: {e}")))?;

        // 5. Format results
        let output = if results.is_empty() {
            "No matching code found.".to_string()
        } else {
            let mut lines = Vec::new();
            lines.push(format!(
                "Found {} result(s):\n",
                results.len().min(limit as usize)
            ));

            for (i, result) in results.iter().take(limit as usize).enumerate() {
                lines.push(format!(
                    "--- Result {} ---\nFile: {}:{}-{}\nScore: {:.3}\n```{}\n{}\n```\n",
                    i + 1,
                    result.chunk.filepath,
                    result.chunk.start_line,
                    result.chunk.end_line,
                    result.score,
                    result.chunk.language,
                    result.chunk.content.trim(),
                ));
            }

            lines.join("\n")
        };

        Ok(ToolOutput::Function {
            content: output,
            content_items: None,
            success: Some(true),
        })
    }
}
