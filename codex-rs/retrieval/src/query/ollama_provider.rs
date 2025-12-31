//! Ollama LLM provider for query rewriting.
//!
//! Provides local LLM inference via Ollama's OpenAI-compatible API.
//! Requires Ollama to be running locally (`ollama serve`).
//!
//! ## Recommended Models
//!
//! - `qwen3:0.6b`: Smallest viable model (~400MB)
//! - `qwen2.5:1.5b`: Better quality (~1GB)
//! - `gemma2:2b`: Good balance (~1.6GB)
//!
//! ## Example
//!
//! ```toml
//! [retrieval.query_rewrite.llm]
//! provider = "ollama"
//! model = "qwen3:0.6b"
//! base_url = "http://localhost:11434/v1"
//! ```

use async_trait::async_trait;
use serde::Deserialize;

use crate::config::LlmConfig;
use crate::error::Result;
use crate::error::RetrievalErr;
use crate::query::llm_provider::CompletionRequest;
use crate::query::llm_provider::CompletionResponse;
use crate::query::llm_provider::LlmProvider;
use crate::query::llm_provider::TokenUsage;

/// Default Ollama base URL (OpenAI-compatible endpoint).
const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434/v1";

/// Default Ollama model for query rewriting.
const DEFAULT_OLLAMA_MODEL: &str = "qwen3:0.6b";

/// Ollama LLM provider using OpenAI-compatible API.
pub struct OllamaLlmProvider {
    config: LlmConfig,
    client: reqwest::Client,
    base_url: String,
    model: String,
}

impl OllamaLlmProvider {
    /// Create a new Ollama provider from config.
    pub fn new(config: LlmConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs as u64))
            .build()
            .unwrap_or_default();

        let base_url = config
            .base_url
            .clone()
            .unwrap_or_else(|| DEFAULT_OLLAMA_BASE_URL.to_string());

        // Use configured model or default to qwen3:0.6b
        let model = if config.model.is_empty() || config.model == "gpt-4o-mini" {
            DEFAULT_OLLAMA_MODEL.to_string()
        } else {
            config.model.clone()
        };

        Self {
            config,
            client,
            base_url,
            model,
        }
    }

    /// Get the chat completions endpoint.
    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    /// Check if Ollama server is running.
    #[allow(dead_code)]
    async fn check_server(&self) -> bool {
        // Try the models endpoint to verify server is running
        let models_url = self
            .base_url
            .replace("/v1", "/api/tags")
            .replace("/v1/", "/api/tags");

        self.client
            .get(&models_url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

#[async_trait]
impl LlmProvider for OllamaLlmProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    async fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": request.system},
                {"role": "user", "content": request.user}
            ],
            "max_tokens": request.max_tokens,
            "temperature": request.temperature,
            "stream": false
        });

        let mut last_error = None;
        for attempt in 0..=self.config.max_retries {
            let response = self
                .client
                .post(&self.endpoint())
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let json: OllamaResponse = resp
                        .json()
                        .await
                        .map_err(|e| RetrievalErr::json_parse("Ollama response", e))?;

                    let content = json
                        .choices
                        .first()
                        .map(|c| c.message.content.clone())
                        .unwrap_or_default();

                    return Ok(CompletionResponse {
                        content,
                        usage: json.usage.map(|u| TokenUsage {
                            prompt_tokens: u.prompt_tokens,
                            completion_tokens: u.completion_tokens,
                        }),
                    });
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();

                    // Check for model not found error
                    if status.as_u16() == 404
                        || body.contains("model") && body.contains("not found")
                    {
                        return Err(RetrievalErr::ConfigError {
                            field: "llm.model".to_string(),
                            cause: format!(
                                "Model '{}' not found. Run: ollama pull {}",
                                self.model, self.model
                            ),
                        });
                    }

                    last_error = Some(format!("HTTP {status}: {body}"));
                }
                Err(e) => {
                    // Check for connection refused (Ollama not running)
                    if e.is_connect() {
                        return Err(RetrievalErr::ConfigError {
                            field: "llm.base_url".to_string(),
                            cause: format!(
                                "Cannot connect to Ollama at '{}'. Start Ollama with: ollama serve",
                                self.base_url
                            ),
                        });
                    }
                    last_error = Some(e.to_string());
                }
            }

            if attempt < self.config.max_retries {
                let delay = 100 * (1 << attempt);
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }
        }

        Err(RetrievalErr::EmbeddingFailed {
            cause: last_error.unwrap_or_else(|| "Unknown error".to_string()),
        })
    }

    fn is_available(&self) -> bool {
        // Do a quick sync check - for full async check use check_server()
        // We assume Ollama is available if config specifies it
        true
    }
}

/// Ollama API response structure (OpenAI-compatible).
#[derive(Debug, Deserialize)]
struct OllamaResponse {
    choices: Vec<OllamaChoice>,
    usage: Option<OllamaUsage>,
}

#[derive(Debug, Deserialize)]
struct OllamaChoice {
    message: OllamaMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaUsage {
    prompt_tokens: i32,
    completion_tokens: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_provider() {
        let config = LlmConfig {
            provider: "ollama".to_string(),
            model: "qwen3:0.6b".to_string(),
            base_url: Some("http://localhost:11434/v1".to_string()),
            ..Default::default()
        };

        let provider = OllamaLlmProvider::new(config);
        assert_eq!(provider.name(), "ollama");
        assert_eq!(provider.model, "qwen3:0.6b");
    }

    #[test]
    fn test_default_model_override() {
        // When model is the OpenAI default, use Ollama default instead
        let config = LlmConfig {
            provider: "ollama".to_string(),
            model: "gpt-4o-mini".to_string(),
            ..Default::default()
        };

        let provider = OllamaLlmProvider::new(config);
        assert_eq!(provider.model, "qwen3:0.6b");
    }

    #[test]
    fn test_endpoint() {
        let config = LlmConfig {
            base_url: Some("http://localhost:11434/v1".to_string()),
            ..Default::default()
        };

        let provider = OllamaLlmProvider::new(config);
        assert_eq!(
            provider.endpoint(),
            "http://localhost:11434/v1/chat/completions"
        );
    }
}
