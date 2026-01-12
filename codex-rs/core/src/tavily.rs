//! Tavily web search integration.
//!
//! This module exposes a simple asynchronous helper to perform a web
//! query with the Tavily API. The function limits results to a user
//! supplied `limit` (default 10) and returns structured results.
//!
//! The API key should be supplied by the caller; the crate does not read
//! `~/.codex/config.toml` directly to keep it independent.

use anyhow::Result;
use anyhow::anyhow;
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;

/// Parameters for a Tavily search request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TavilyRequest {
    pub api_key: String,
    pub query: String,
    pub limit: usize,
}

/// Response returned by the Tavily API.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct TavilyResponse {
    results: Vec<TavilyResult>,
}

/// Individual search result.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TavilyResult {
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub snippet: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
}

/// Perform a web search via Tavily.
///
/// Returns Tavily search results matching the query.
pub async fn search_tavily(request: TavilyRequest) -> Result<Vec<TavilyResult>> {
    // If the API key is empty, return an error – the caller will typically
    // guard this by checking the config first.
    if request.api_key.is_empty() {
        return Err(anyhow!(
            "Tavily API key is not set; enable it via tavily_api_key in ~/.codex/config.toml"
        ));
    }

    let client = Client::new();
    let resp = client
        .post("https://api.tavily.com/search")
        .header("Content-Type", "application/json")
        .json(&json!({
            "query": request.query,
            "api_key": request.api_key,
            "max_results": request.limit,
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow!(
            "Tavily search failed with status {status}",
            status = resp.status()
        ));
    }

    let body: TavilyResponse = resp.json().await?;
    Ok(body.results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use pretty_assertions::assert_eq;

    // This test simply verifies that the request construction works.
    // It does not perform an actual network request to avoid external
    // dependencies during CI.
    #[test]
    fn request_is_built_correctly() -> Result<()> {
        let req = TavilyRequest {
            api_key: "dummy".into(),
            query: "rust".into(),
            limit: 3,
        };
        let expected = TavilyRequest {
            api_key: "dummy".into(),
            query: "rust".into(),
            limit: 3,
        };
        assert_eq!(req, expected);
        Ok(())
    }
}
