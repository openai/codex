//! LLM-based path extraction from shell command output.
//!
//! This module provides an LLM-based implementation of the `PathExtractor` trait
//! from `cocode-shell`, enabling fast model pre-reading of files that commands
//! read or modify.
//!
//! ## Usage
//!
//! ```no_run
//! use cocode_tools::builtin::path_extraction::LlmPathExtractor;
//! use cocode_protocol::model::{ModelRoles, ModelRole, ModelSpec};
//! use hyper_sdk::HyperClient;
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = Arc::new(HyperClient::from_env()?);
//! let model_spec = ModelSpec::new("anthropic", "claude-haiku");
//!
//! let extractor = LlmPathExtractor::new(client, model_spec);
//! // Use with ShellExecutor::with_path_extractor()
//! # Ok(())
//! # }
//! ```

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use cocode_protocol::model::ModelRole;
use cocode_protocol::model::ModelRoles;
use cocode_protocol::model::ModelSpec;
use cocode_shell::path_extractor::BoxFuture;
use cocode_shell::path_extractor::PathExtractionResult;
use cocode_shell::path_extractor::PathExtractor;
use cocode_shell::path_extractor::filter_existing_files;
use cocode_shell::path_extractor::truncate_for_extraction;
use hyper_sdk::GenerateRequest;
use hyper_sdk::HyperClient;
use hyper_sdk::Message;
use tracing::debug;
use tracing::warn;

/// System prompt for path extraction (matches Claude Code).
const PATH_EXTRACTION_PROMPT: &str = r#"Extract any file paths that this command reads or modifies from the output.
Rules:
- Return only file paths, one per line
- Include both relative and absolute paths
- Do not include directories (only files)
- If no file paths found, return empty response"#;

/// LLM-based path extractor using a fast model.
///
/// This extractor uses an LLM (typically a fast model like Haiku) to analyze
/// command output and extract file paths that the command read or modified.
///
/// The extractor is designed to be used with `ShellExecutor::with_path_extractor()`.
pub struct LlmPathExtractor {
    /// HTTP client for LLM API calls.
    client: Arc<HyperClient>,
    /// Model specification (provider + model ID).
    model_spec: ModelSpec,
}

impl LlmPathExtractor {
    /// Create a new LLM path extractor with the given client and model.
    pub fn new(client: Arc<HyperClient>, model_spec: ModelSpec) -> Self {
        Self { client, model_spec }
    }

    /// Create from ModelRoles - uses Fast role (falls back to Main if not configured).
    ///
    /// Returns `None` if no model is configured (both fast and main are None).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use cocode_tools::builtin::path_extraction::LlmPathExtractor;
    /// use cocode_protocol::model::{ModelRoles, ModelRole, ModelSpec};
    /// use hyper_sdk::HyperClient;
    /// use std::sync::Arc;
    ///
    /// # fn example() -> Option<LlmPathExtractor> {
    /// let client = Arc::new(HyperClient::from_env().ok()?);
    /// let mut roles = ModelRoles::default();
    /// roles.set(ModelRole::Main, ModelSpec::new("anthropic", "claude-sonnet-4-20250514"));
    /// roles.set(ModelRole::Fast, ModelSpec::new("anthropic", "claude-haiku"));
    ///
    /// // Uses fast model (claude-haiku)
    /// LlmPathExtractor::from_model_roles(client, &roles)
    /// # }
    /// ```
    pub fn from_model_roles(client: Arc<HyperClient>, roles: &ModelRoles) -> Option<Self> {
        // ModelRoles.get(ModelRole::Fast) returns fast if set, otherwise main
        let model_spec = roles.get(ModelRole::Fast)?.clone();
        Some(Self::new(client, model_spec))
    }

    /// Returns the model spec being used.
    pub fn model_spec(&self) -> &ModelSpec {
        &self.model_spec
    }

    /// Parse paths from LLM response.
    fn parse_paths(response: &str) -> Vec<PathBuf> {
        response
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return None;
                }
                // Skip lines that look like explanatory text
                if trimmed.starts_with("The ")
                    || trimmed.starts_with("No ")
                    || trimmed.starts_with("Note:")
                    || trimmed.contains("file paths")
                    || trimmed.contains("not found")
                {
                    return None;
                }
                // Must look like a path (starts with / or ./ or has extension)
                if trimmed.starts_with('/')
                    || trimmed.starts_with("./")
                    || trimmed.starts_with("../")
                    || trimmed.contains('.')
                {
                    Some(PathBuf::from(trimmed))
                } else {
                    None
                }
            })
            .collect()
    }
}

impl std::fmt::Debug for LlmPathExtractor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmPathExtractor")
            .field("model_spec", &self.model_spec)
            .finish()
    }
}

impl PathExtractor for LlmPathExtractor {
    fn extract_paths<'a>(
        &'a self,
        command: &'a str,
        output: &'a str,
        cwd: &'a Path,
    ) -> BoxFuture<'a, anyhow::Result<PathExtractionResult>> {
        Box::pin(async move {
            let start = Instant::now();

            // Truncate output for efficiency
            let truncated_output = truncate_for_extraction(output);

            // Skip extraction for empty or very short output
            if truncated_output.trim().is_empty() {
                debug!("Skipping path extraction: empty output");
                return Ok(PathExtractionResult::empty());
            }

            // Build the prompt
            let user_message =
                format!("Command: {command}\n\nOutput:\n{truncated_output}\n\nExtract file paths:");

            // Get model from client
            let model = match self
                .client
                .model(&self.model_spec.provider, &self.model_spec.model)
            {
                Ok(m) => m,
                Err(e) => {
                    warn!("Failed to get model for path extraction: {e}");
                    return Ok(PathExtractionResult::empty());
                }
            };

            // Create request
            let request = GenerateRequest::new(vec![
                Message::system(PATH_EXTRACTION_PROMPT),
                Message::user(user_message),
            ]);

            // Generate response
            let response = match model.generate(request).await {
                Ok(r) => r,
                Err(e) => {
                    warn!("Path extraction API call failed: {e}");
                    return Ok(PathExtractionResult::empty());
                }
            };

            // Parse paths from response
            let response_text = response.text();
            let raw_paths = Self::parse_paths(&response_text);

            // Filter to existing files
            let existing_paths = filter_existing_files(raw_paths, cwd);

            let extraction_ms = start.elapsed().as_millis() as i64;

            debug!(
                paths_found = existing_paths.len(),
                extraction_ms, "Extracted paths from command output"
            );

            Ok(PathExtractionResult::new(existing_paths, extraction_ms))
        })
    }

    fn is_enabled(&self) -> bool {
        // Always enabled when configured
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_paths_simple() {
        let response = "/path/to/file.txt\n./relative/file.rs\n../parent/file.go";
        let paths = LlmPathExtractor::parse_paths(response);

        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0], PathBuf::from("/path/to/file.txt"));
        assert_eq!(paths[1], PathBuf::from("./relative/file.rs"));
        assert_eq!(paths[2], PathBuf::from("../parent/file.go"));
    }

    #[test]
    fn test_parse_paths_with_noise() {
        let response = "The command modified:\n/file1.txt\n\nNote: some text\n./file2.rs";
        let paths = LlmPathExtractor::parse_paths(response);

        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], PathBuf::from("/file1.txt"));
        assert_eq!(paths[1], PathBuf::from("./file2.rs"));
    }

    #[test]
    fn test_parse_paths_empty() {
        let response = "No file paths found";
        let paths = LlmPathExtractor::parse_paths(response);

        assert!(paths.is_empty());
    }

    #[test]
    fn test_parse_paths_with_extensions() {
        let response = "main.rs\nCargo.toml\nREADME.md";
        let paths = LlmPathExtractor::parse_paths(response);

        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn test_from_model_roles_fast() {
        // Can't test without a real client, but we can test the logic
        let mut roles = ModelRoles::default();
        roles.set(
            ModelRole::Main,
            ModelSpec::new("anthropic", "claude-sonnet"),
        );
        roles.set(ModelRole::Fast, ModelSpec::new("anthropic", "claude-haiku"));

        // Fast role should be returned (not main)
        let fast_spec = roles.get(ModelRole::Fast).unwrap();
        assert_eq!(fast_spec.model, "claude-haiku");
    }

    #[test]
    fn test_from_model_roles_fallback() {
        let mut roles = ModelRoles::default();
        roles.set(
            ModelRole::Main,
            ModelSpec::new("anthropic", "claude-sonnet"),
        );
        // No fast role set

        // Should fall back to main
        let fast_spec = roles.get(ModelRole::Fast).unwrap();
        assert_eq!(fast_spec.model, "claude-sonnet");
    }

    #[test]
    fn test_from_model_roles_none() {
        let roles = ModelRoles::default();
        // No roles set

        // Should return None
        assert!(roles.get(ModelRole::Fast).is_none());
    }
}
