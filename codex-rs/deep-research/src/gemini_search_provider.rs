// Copyright 2025 zapabob
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::provider::ResearchProvider;
use crate::types::Source;
use anyhow::Result;
use codex_rmcp_client::RmcpClient;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use tracing::info;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

impl From<GeminiSearchResult> for Source {
    fn from(result: GeminiSearchResult) -> Self {
        Source {
            title: result.title,
            url: result.url,
            snippet: result.snippet,
            relevance_score: 0.8, // Default relevance score
        }
    }
}

pub struct GeminiSearchProvider {
    pub model: String,
    /// MCP client for Gemini search tool calls (optional, for rmcp integration)
    mcp_client: Option<Arc<RmcpClient>>,
}

impl GeminiSearchProvider {
    pub fn new(model: String) -> Self {
        Self {
            model,
            mcp_client: None,
        }
    }

    /// Create with MCP client for real Gemini search integration
    pub fn with_mcp_client(model: String, mcp_client: Arc<RmcpClient>) -> Self {
        Self {
            model,
            mcp_client: Some(mcp_client),
        }
    }

    /// Execute search via Gemini CLI (direct command execution)
    async fn execute_gemini_search_direct(&self, query: &str) -> Result<Vec<GeminiSearchResult>> {
        info!("🔍 Executing Gemini CLI search directly");

        // Gemini CLI コマンド実行
        let output = tokio::process::Command::new("gemini")
            .args(&["search", "--model", &self.model, query])
            .output()
            .await?;

        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Gemini CLI error: {}", error));
        }

        let response = String::from_utf8(output.stdout)?;
        self.parse_text_response(&response)
    }

    /// Parse Gemini CLI text response into structured results
    fn parse_text_response(&self, text: &str) -> Result<Vec<GeminiSearchResult>> {
        let mut results = Vec::new();

        // Simple parsing logic - can be enhanced
        let lines: Vec<&str> = text.lines().collect();
        for line in lines {
            if line.starts_with("http") {
                let url = line.trim().to_string();
                let title = format!("Search Result for {}", url);
                let snippet = "Gemini CLI search result".to_string();

                results.push(GeminiSearchResult {
                    title,
                    url,
                    snippet,
                });
            }
        }

        if results.is_empty() {
            // Fallback: create a generic result
            results.push(GeminiSearchResult {
                title: "Gemini Search".to_string(),
                url: "https://gemini.google.com".to_string(),
                snippet: text.chars().take(200).collect(),
            });
        }

        Ok(results)
    }

    /// Execute search via MCP (Codex → MCP → Gemini CLI)
    /// This uses the gemini-cli MCP server defined in config.toml
    async fn execute_gemini_search_via_mcp(
        &self,
        query: &str,
        max_results: u8,
    ) -> Result<Vec<GeminiSearchResult>> {
        info!("🔧 Executing Gemini search via MCP tool");

        let client = self.mcp_client.as_ref().ok_or_else(|| {
            anyhow::anyhow!("MCP client not configured. Use with_mcp_client() constructor.")
        })?;

        // Prepare tool call arguments for Gemini Google Search
        let arguments = json!({
            "query": query,
            "model": self.model,
        });

        // Call the Gemini googleSearch MCP tool
        let result = client
            .call_tool("googleSearch".to_string(), Some(arguments), None)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to call Gemini googleSearch tool: {}", e))?;

        // Parse the result
        let mut results = Vec::new();
        for item in result.content {
            if let mcp_types::ContentBlock::TextContent(text_content) = item {
                let text = &text_content.text;
                // Try to parse as JSON array first
                if let Ok(json_results) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
                    for json_result in json_results {
                        if let Ok(gemini_result) =
                            serde_json::from_value::<GeminiSearchResult>(json_result)
                        {
                            results.push(gemini_result);
                        }
                    }
                } else {
                    // Fallback: parse as text response
                    results.extend(self.parse_text_response(&text)?);
                }
            }
        }

        if results.is_empty() {
            warn!("No search results from Gemini MCP tool, falling back to direct CLI");
            return self.execute_gemini_search_direct(query).await;
        }

        Ok(results.into_iter().take(max_results as usize).collect())
    }
}

#[async_trait::async_trait]
impl ResearchProvider for GeminiSearchProvider {
    async fn search(&self, query: &str, max_results: u8) -> Result<Vec<Source>> {
        info!("🔍 Starting Gemini search for: {}", query);

        // Try direct execution first
        match self.execute_gemini_search_direct(query).await {
            Ok(results) => {
                info!(
                    "✅ Direct Gemini search successful: {} results",
                    results.len()
                );
                let sources: Vec<Source> = results.into_iter().map(|r| r.into()).collect();
                return Ok(sources.into_iter().take(max_results as usize).collect());
            }
            Err(e) => {
                info!("⚠️ Direct Gemini search failed: {}", e);
            }
        }

        // Try MCP execution as fallback
        match self.execute_gemini_search_via_mcp(query, max_results).await {
            Ok(results) => {
                info!("✅ MCP Gemini search successful: {} results", results.len());
                let sources: Vec<Source> = results.into_iter().map(|r| r.into()).collect();
                Ok(sources)
            }
            Err(e) => {
                warn!("MCP Gemini search failed: {}, trying direct CLI", e);
                // Fallback to direct CLI
                match self.execute_gemini_search_direct(query).await {
                    Ok(results) => {
                        info!(
                            "✅ Direct CLI Gemini search successful: {} results",
                            results.len()
                        );
                        let sources: Vec<Source> = results.into_iter().map(|r| r.into()).collect();
                        Ok(sources.into_iter().take(max_results as usize).collect())
                    }
                    Err(e) => {
                        info!("❌ All Gemini search methods failed: {}", e);
                        Err(e)
                    }
                }
            }
        }
    }

    async fn retrieve(&self, url: &str) -> Result<String> {
        info!("📥 Retrieving content from: {}", url);

        // Simple HTTP retrieval - can be enhanced
        let response = reqwest::get(url).await?;
        let content = response.text().await?;

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gemini_search_provider() {
        let provider = GeminiSearchProvider::new("gemini-2.5-flash".to_string());

        // Test with a simple query
        let results = provider.search("Rust programming", 5).await;
        assert!(results.is_ok());
    }

    #[test]
    fn test_parse_text_response() {
        let provider = GeminiSearchProvider::new("gemini-2.5-flash".to_string());

        let text = r#"
        Here are some search results:
        
        https://rust-lang.github.io/async-book/
        https://doc.rust-lang.org/book/
        
        These are great resources for Rust programming.
        "#;

        let results = provider.parse_text_response(text);
        assert_eq!(results.len(), 2);
        assert_eq!(
            results[0].title,
            "Search Result for https://rust-lang.github.io/async-book/"
        );
        assert_eq!(results[0].url, "https://rust-lang.github.io/async-book/");
    }
}
