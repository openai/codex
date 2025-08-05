use bytes::BytesMut;
use futures::StreamExt;
use futures::stream::BoxStream;
use serde_json::Value as JsonValue;
use std::collections::VecDeque;
use std::io;

use codex_core::WireApi;

use crate::parser::pull_events_from_value;
use crate::pull::PullEvent;
use crate::pull::PullProgressReporter;
use crate::url::base_url_to_host_root;
use crate::url::is_openai_compatible_base_url;

/// Client for interacting with a local Ollama instance.
pub struct OllamaClient {
    client: reqwest::Client,
    host_root: String,
    uses_openai_compat: bool,
}

impl OllamaClient {
    pub fn from_oss_provider() -> Self {
        #![allow(clippy::expect_used)]
        // Use the built-in OSS provider's base URL.
        let built_in_model_providers = codex_core::built_in_model_providers();
        let provider = built_in_model_providers
            .get(codex_core::BUILT_IN_OSS_MODEL_PROVIDER_ID)
            .expect("oss provider must exist");
        let base_url = provider
            .base_url
            .as_ref()
            .expect("oss provider must have a base_url");
        Self::from_provider(base_url, provider.wire_api)
    }

    /// Construct a client for the built‑in open‑source ("oss") model provider
    /// and verify that a local Ollama server is reachable. If no server is
    /// detected, returns an error with helpful installation/run instructions.
    pub async fn try_from_oss_provider() -> io::Result<Self> {
        let client = Self::from_oss_provider();
        if client.probe_server().await? {
            Ok(client)
        } else {
            Err(io::Error::other(
                "No running Ollama server detected. Start it with: `ollama serve` (after installing). Install instructions: https://github.com/ollama/ollama?tab=readme-ov-file#ollama",
            ))
        }
    }

    /// Build a client from a provider definition. Falls back to the default
    /// local URL if no base_url is configured.
    fn from_provider(base_url: &str, wire_api: WireApi) -> Self {
        let uses_openai_compat = is_openai_compatible_base_url(base_url)
            || matches!(wire_api, WireApi::Chat) && is_openai_compatible_base_url(base_url);
        let host_root = base_url_to_host_root(base_url);
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            host_root,
            uses_openai_compat,
        }
    }

    /// Low-level constructor given a raw host root, e.g. "http://localhost:11434".
    #[cfg(test)]
    fn from_host_root(host_root: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            client,
            host_root: host_root.into(),
            uses_openai_compat: false,
        }
    }

    /// Probe whether the server is reachable by hitting the appropriate health endpoint.
    pub async fn probe_server(&self) -> io::Result<bool> {
        let url = if self.uses_openai_compat {
            format!("{}/v1/models", self.host_root.trim_end_matches('/'))
        } else {
            format!("{}/api/tags", self.host_root.trim_end_matches('/'))
        };
        let resp = self.client.get(url).send().await;
        Ok(matches!(resp, Ok(r) if r.status().is_success()))
    }

    /// Return the list of model names known to the local Ollama instance.
    pub async fn fetch_models(&self) -> io::Result<Vec<String>> {
        let tags_url = format!("{}/api/tags", self.host_root.trim_end_matches('/'));
        let resp = self
            .client
            .get(tags_url)
            .send()
            .await
            .map_err(io::Error::other)?;
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }
        let val = resp.json::<JsonValue>().await.map_err(io::Error::other)?;
        let names = val
            .get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(names)
    }

    /// Start a model pull and emit streaming events. The returned stream ends when
    /// a Success event is observed or the server closes the connection.
    pub async fn pull_model_stream(
        &self,
        model: &str,
    ) -> io::Result<BoxStream<'static, PullEvent>> {
        let url = format!("{}/api/pull", self.host_root.trim_end_matches('/'));
        let resp = self
            .client
            .post(url)
            .json(&serde_json::json!({"model": model, "stream": true}))
            .send()
            .await
            .map_err(io::Error::other)?;
        if !resp.status().is_success() {
            return Err(io::Error::other(format!(
                "failed to start pull: HTTP {}",
                resp.status()
            )));
        }

        let mut stream = resp.bytes_stream();
        let mut buf = BytesMut::new();
        let _pending: VecDeque<PullEvent> = VecDeque::new();

        // Using an async stream adaptor backed by unfold-like manual loop.
        let s = async_stream::stream! {
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        buf.extend_from_slice(&bytes);
                        while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
                            let line = buf.split_to(pos + 1);
                            if let Ok(text) = std::str::from_utf8(&line) {
                                let text = text.trim();
                                if text.is_empty() { continue; }
                                if let Ok(value) = serde_json::from_str::<JsonValue>(text) {
                                    for ev in pull_events_from_value(&value) { yield ev; }
                                    if let Some(err_msg) = value.get("error").and_then(|e| e.as_str()) {
                                        yield PullEvent::Error(err_msg.to_string());
                                        return;
                                    }
                                    if let Some(status) = value.get("status").and_then(|s| s.as_str()) {
                                        if status == "success" { yield PullEvent::Success; return; }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // Connection error: end the stream.
                        return;
                    }
                }
            }
        };

        Ok(Box::pin(s))
    }

    /// High-level helper to pull a model and drive a progress reporter.
    pub async fn pull_with_reporter(
        &self,
        model: &str,
        reporter: &mut dyn PullProgressReporter,
    ) -> io::Result<()> {
        reporter.on_event(&PullEvent::Status(format!("Pulling model {model}...")))?;
        let mut stream = self.pull_model_stream(model).await?;
        while let Some(event) = stream.next().await {
            reporter.on_event(&event)?;
            match event {
                PullEvent::Success => {
                    return Ok(());
                }
                PullEvent::Error(err) => {
                    // Emperically, ollama returns a 200 OK response even when
                    // the output stream includes an error message. Verify with:
                    //
                    // `curl -i http://localhost:11434/api/pull -d '{ "model": "foobarbaz" }'`
                    //
                    // As such, we have to check the event stream, not the
                    // HTTP response status, to determine whether to return Err.
                    return Err(io::Error::other(format!("Pull failed: {err}")));
                }
                PullEvent::ChunkProgress { .. } | PullEvent::Status(_) => {
                    continue;
                }
            }
        }
        Err(io::Error::other(
            "Pull stream ended unexpectedly without success.",
        ))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    /// Simple RAII guard to set an environment variable for the duration of a test
    /// and restore the previous value (or remove it) on drop to avoid cross-test
    /// interference.
    struct EnvVarGuard {
        key: String,
        prev: Option<String>,
    }
    impl EnvVarGuard {
        fn set(key: &str, value: String) -> Self {
            let prev = std::env::var(key).ok();
            // set_var is safe but we mirror existing tests that use an unsafe block
            // to silence edition lints around global mutation during tests.
            unsafe { std::env::set_var(key, value) };
            Self {
                key: key.to_string(),
                prev,
            }
        }
    }
    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { std::env::set_var(&self.key, v) },
                None => unsafe { std::env::remove_var(&self.key) },
            }
        }
    }

    // Happy-path tests using a mock HTTP server; skip if sandbox network is disabled.
    #[tokio::test]
    async fn test_fetch_models_happy_path() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} is set; skipping test_fetch_models_happy_path",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/tags"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_raw(
                    serde_json::json!({
                        "models": [ {"name": "llama3.2:3b"}, {"name":"mistral"} ]
                    })
                    .to_string(),
                    "application/json",
                ),
            )
            .mount(&server)
            .await;

        let client = OllamaClient::from_host_root(server.uri());
        let models = client.fetch_models().await.expect("fetch models");
        assert!(models.contains(&"llama3.2:3b".to_string()));
        assert!(models.contains(&"mistral".to_string()));
    }

    #[tokio::test]
    async fn test_probe_server_happy_path_openai_compat_and_native() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} set; skipping test_probe_server_happy_path_openai_compat_and_native",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;

        // Native endpoint
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/tags"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let native = OllamaClient::from_host_root(server.uri());
        assert!(native.probe_server().await.expect("probe native"));

        // OpenAI compatibility endpoint
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/v1/models"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        // Ensure the built-in OSS provider points at our mock server for this test
        // to avoid depending on any globally configured environment from other tests.
        let _guard = EnvVarGuard::set("CODEX_OSS_BASE_URL", format!("{}/v1", server.uri()));
        let ollama_client = OllamaClient::from_oss_provider();
        assert!(ollama_client.probe_server().await.expect("probe compat"));
    }

    #[tokio::test]
    async fn test_try_from_oss_provider_ok_when_server_running() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} set; skipping test_try_from_oss_provider_ok_when_server_running",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        // Configure built‑in `oss` provider to point at this mock server.
        // set_var is unsafe on Rust 2024 edition; use unsafe block in tests.
        let _guard = EnvVarGuard::set("CODEX_OSS_BASE_URL", format!("{}/v1", server.uri()));

        // OpenAI‑compat models endpoint responds OK.
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/v1/models"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let _client = OllamaClient::try_from_oss_provider()
            .await
            .expect("client should be created when probe succeeds");
    }

    #[tokio::test]
    async fn test_try_from_oss_provider_err_when_server_missing() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} set; skipping test_try_from_oss_provider_err_when_server_missing",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        // Point oss provider at our mock server but do NOT set up a handler
        // for /v1/models so the request returns a non‑success status.
        unsafe { std::env::set_var("CODEX_OSS_BASE_URL", format!("{}/v1", server.uri())) };

        let err = OllamaClient::try_from_oss_provider()
            .await
            .err()
            .expect("expected error");
        let msg = err.to_string();
        assert!(
            msg.contains("No running Ollama server detected."),
            "msg = {msg}"
        );
    }
}
