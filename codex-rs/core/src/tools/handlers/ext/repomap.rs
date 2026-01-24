//! RepoMap Handler - Generate codebase structure map
//!
//! This module provides the RepoMapHandler which generates a condensed
//! map of the codebase structure using the retrieval system.
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
use std::collections::HashSet;

/// Maximum tokens limit
const MAX_TOKENS: i32 = 8192;
/// Default tokens
const DEFAULT_TOKENS: i32 = 1024;

/// RepoMap tool arguments
#[derive(Debug, Clone, Deserialize)]
struct RepoMapArgs {
    #[serde(default)]
    max_tokens: Option<i32>,
    #[serde(default)]
    symbols: Option<Vec<String>>,
}

/// RepoMap Handler - simple wrapper around RetrievalService.
///
/// This handler is stateless. It obtains the RetrievalService from
/// the retrieval crate using `RetrievalService::for_workdir()` which
/// loads configuration from retrieval.toml files.
pub struct RepoMapHandler;

impl RepoMapHandler {
    /// Create a new stateless handler.
    pub fn new() -> Self {
        Self
    }
}

impl Default for RepoMapHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolHandler for RepoMapHandler {
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
                    "Invalid payload type for repomap".to_string(),
                ));
            }
        };

        let args: RepoMapArgs = serde_json::from_str(arguments)
            .map_err(|e| FunctionCallError::RespondToModel(format!("Invalid arguments: {e}")))?;

        // Clamp max_tokens
        let max_tokens = args
            .max_tokens
            .unwrap_or(DEFAULT_TOKENS)
            .min(MAX_TOKENS)
            .max(256);

        // 2. Get RetrievalFacade from SessionServices
        let service = invocation
            .session
            .services
            .retrieval_manager
            .clone()
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(
                    "RepoMap is not enabled. Enable Feature::Retrieval in config.".to_string(),
                )
            })?;

        // 4. Build request
        let mentioned_idents: HashSet<String> =
            args.symbols.unwrap_or_default().into_iter().collect();

        let request = codex_retrieval::RepoMapRequest {
            chat_files: Vec::new(),
            other_files: Vec::new(),
            mentioned_fnames: HashSet::new(),
            mentioned_idents,
            max_tokens,
        };

        // 5. Generate repomap (using facade's simple API)
        let result = service.generate_repomap(request).await.map_err(|e| {
            FunctionCallError::RespondToModel(format!("RepoMap generation failed: {e}"))
        })?;

        // 6. Format output
        let mut output = String::new();

        // Show filter info at the top
        if let Some(filter) = &result.filter {
            output.push_str(&format!(
                "[Index Filter: {}]\n\n",
                filter.to_display_string()
            ));
        }

        output.push_str(&format!(
            "Repository Map ({} tokens, {} files):\n\n",
            result.tokens, result.files_included
        ));
        output.push_str(&result.content);

        Ok(ToolOutput::Function {
            content: output,
            content_items: None,
            success: Some(true),
        })
    }
}
