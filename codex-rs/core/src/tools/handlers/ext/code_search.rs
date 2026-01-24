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

        // 2. Get RetrievalFacade from SessionServices
        let service = invocation
            .session
            .services
            .retrieval_manager
            .clone()
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(
                    "Code search is not enabled. Enable Feature::Retrieval in config.".to_string(),
                )
            })?;

        // 4. Perform search (using facade's simple API)
        let search_output = service
            .search(&args.query)
            .await
            .map_err(|e| FunctionCallError::RespondToModel(format!("Search failed: {e}")))?;

        // 5. Format results
        let output = if search_output.results.is_empty() {
            let mut msg = String::new();
            // Show filter info even with no results
            if let Some(filter) = &search_output.filter {
                msg.push_str(&format!(
                    "[Index Filter: {}]\n\n",
                    filter.to_display_string()
                ));
            }
            msg.push_str("No matching code found.");
            msg
        } else {
            let mut lines = Vec::new();

            // Show filter info at the top
            if let Some(filter) = &search_output.filter {
                lines.push(format!(
                    "[Index Filter: {}]\n\n",
                    filter.to_display_string()
                ));
            }

            lines.push(format!(
                "Found {} result(s):\n",
                search_output.results.len().min(limit as usize)
            ));

            for (i, result) in search_output
                .results
                .iter()
                .take(limit as usize)
                .enumerate()
            {
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
