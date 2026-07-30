use futures::StreamExt;
use futures::stream::BoxStream;
use semver::Version;
use serde_json::Value as JsonValue;
use std::collections::VecDeque;
use std::io;
use std::time::Duration;

use crate::line_buffer::LineBuffer;
use crate::parser::pull_events_from_value;
use crate::pull::PullEvent;
use crate::pull::PullProgressReporter;
use crate::url::base_url_to_host_root;
use crate::url::is_openai_compatible_base_url;
use codex_core::config::Config;
use codex_http_client::ClientRouteClass;
use codex_http_client::HttpClientFactory;
#[cfg(test)]
use codex_http_client::OutboundProxyPolicy;
use codex_http_client::RouteAwareClientPool;
use codex_http_client::RouteAwareRequestError;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::OLLAMA_OSS_PROVIDER_ID;
#[cfg(test)]
use codex_model_provider_info::WireApi;
#[cfg(test)]
use codex_model_provider_info::create_oss_provider_with_base_url;

const OLLAMA_CONNECTION_ERROR: &str = "No running Ollama server detected. Start it with: `ollama serve` (after installing). Install instructions: https://github.com/ollama/ollama?tab=readme-ov-file#ollama";
const OLLAMA_CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

/// Client for interacting with a local Ollama instance.
pub struct OllamaClient {
    client: RouteAwareClientPool,
    host_root: String,
    uses_openai_compat: bool,
}

impl OllamaClient {
    /// Construct a client for the built‑in open‑source ("oss") model provider
    /// and verify that a local Ollama server is reachable. If no server is
    /// detected, returns an error with helpful installation/run instructions.
    pub async fn try_from_oss_provider(config: &Config) -> io::Result<Self> {
        // Note that we must look up the provider from the Config to ensure that
        // any overrides the user has in their config.toml are taken into
        // account.
        let provider = config
            .model_providers
            .get(OLLAMA_OSS_PROVIDER_ID)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Built-in provider {OLLAMA_OSS_PROVIDER_ID} not found",),
                )
            })?;

        Self::try_from_provider(provider, config.http_client_factory()).await
    }

    #[cfg(test)]
    async fn try_from_provider_with_base_url(base_url: &str) -> io::Result<Self> {
        let provider = create_oss_provider_with_base_url(base_url, WireApi::Responses);
        Self::try_from_provider(
            &provider,
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .await
    }

    /// Build a client from a provider definition and verify the server is reachable.
    pub(crate) async fn try_from_provider(
        provider: &ModelProviderInfo,
        http_client_factory: HttpClientFactory,
    ) -> io::Result<Self> {
        #![expect(clippy::expect_used)]
        let base_url = provider
            .base_url
            .as_ref()
            .expect("oss provider must have a base_url");
        let uses_openai_compat = is_openai_compatible_base_url(base_url);
        let host_root = base_url_to_host_root(base_url);
        let client = RouteAwareClientPool::with_connect_timeout(
            http_client_factory,
            ClientRouteClass::Other,
            OLLAMA_CONNECTION_TIMEOUT,
        )
        .with_legacy_custom_ca_fallback();
        let client = Self {
            client,
            host_root,
            uses_openai_compat,
        };
        client.probe_server().await?;
        Ok(client)
    }

    /// Probe whether the server is reachable by hitting the appropriate health endpoint.
    async fn probe_server(&self) -> io::Result<()> {
        let url = if self.uses_openai_compat {
            format!("{}/v1/models", self.host_root.trim_end_matches('/'))
        } else {
            format!("{}/api/tags", self.host_root.trim_end_matches('/'))
        };
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| match error {
                RouteAwareRequestError::Route(error) => {
                    tracing::warn!(error = %error, "Failed to initialize Ollama HTTP transport");
                    io::Error::other(error)
                }
                error => {
                    tracing::warn!(error = ?error, "Failed to connect to Ollama server");
                    io::Error::other(OLLAMA_CONNECTION_ERROR)
                }
            })?;
        if resp.status().is_success() {
            Ok(())
        } else {
            tracing::warn!(
                "Failed to probe server at {}: HTTP {}",
                self.host_root,
                resp.status()
            );
            Err(io::Error::other(OLLAMA_CONNECTION_ERROR))
        }
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
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Ok(names)
    }

    /// Query the server for its version string, returning `None` when unavailable.
    pub async fn fetch_version(&self) -> io::Result<Option<Version>> {
        let version_url = format!("{}/api/version", self.host_root.trim_end_matches('/'));
        let resp = self
            .client
            .get(version_url)
            .send()
            .await
            .map_err(io::Error::other)?;
        if !resp.status().is_success() {
            return Ok(None);
        }
        let val = resp.json::<JsonValue>().await.map_err(io::Error::other)?;
        let Some(version_str) = val.get("version").and_then(|v| v.as_str()).map(str::trim) else {
            return Ok(None);
        };
        let normalized = version_str.trim_start_matches('v');
        match Version::parse(normalized) {
            Ok(version) => Ok(Some(version)),
            Err(err) => {
                tracing::warn!("Failed to parse Ollama version `{version_str}`: {err}");
                Ok(None)
            }
        }
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
        let mut buf = LineBuffer::default();
        let _pending: VecDeque<PullEvent> = VecDeque::new();

        // Using an async stream adaptor backed by unfold-like manual loop.
        let s = async_stream::stream! {
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        buf.extend_from_slice(&bytes);
                        while let Some(line) = buf.take_line() {
                            if let Ok(text) = std::str::from_utf8(&line) {
                                let text = text.trim();
                                if text.is_empty() { continue; }
                                if let Ok(value) = serde_json::from_str::<JsonValue>(text) {
                                    for ev in pull_events_from_value(&value) { yield ev; }
                                    if let Some(err_msg) = value.get("error").and_then(|e| e.as_str()) {
                                        yield PullEvent::Error(err_msg.to_string());
                                        return;
                                    }
                                    if let Some(status) = value.get("status").and_then(|s| s.as_str())
                                        && status == "success" { yield PullEvent::Success; return; }
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
                    // Empirically, ollama returns a 200 OK response even when
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

    /// Low-level constructor given a raw host root, e.g. "http://localhost:11434".
    #[cfg(test)]
    fn from_host_root(host_root: impl Into<String>) -> Self {
        let client = RouteAwareClientPool::with_connect_timeout(
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
            ClientRouteClass::Other,
            OLLAMA_CONNECTION_TIMEOUT,
        );
        Self {
            client,
            host_root: host_root.into(),
            uses_openai_compat: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use pretty_assertions::assert_eq;

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
    async fn test_fetch_version() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} is set; skipping test_fetch_version",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/tags"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_raw(
                serde_json::json!({ "models": [] }).to_string(),
                "application/json",
            ))
            .mount(&server)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/version"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_raw(
                serde_json::json!({ "version": "0.14.1" }).to_string(),
                "application/json",
            ))
            .mount(&server)
            .await;

        let client = OllamaClient::try_from_provider_with_base_url(server.uri().as_str())
            .await
            .expect("client");

        let version = client.fetch_version().await.expect("version fetch");
        assert_eq!(version, Some(Version::new(0, 14, 1)));
    }

    #[tokio::test]
    async fn test_pull_model_stream_parses_large_json_lines() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} set; skipping test_pull_model_stream_parses_large_json_lines",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        let body = format!(
            "{}\n{}\n",
            serde_json::json!({
                "status": "pulling layers",
                "padding": "x".repeat(128 * 1024),
            }),
            serde_json::json!({"status": "complete"}),
        );
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/pull"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_raw(body, "application/x-ndjson"),
            )
            .mount(&server)
            .await;

        let client = OllamaClient::from_host_root(server.uri());
        let events = client
            .pull_model_stream("test-model")
            .await
            .expect("start pull stream")
            .collect::<Vec<_>>()
            .await;

        assert_matches!(
            events.as_slice(),
            [
                PullEvent::Status(pulling),
                PullEvent::Status(complete),
            ] if pulling == "pulling layers" && complete == "complete"
        );
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
        native.probe_server().await.expect("probe native");

        // OpenAI compatibility endpoint
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/v1/models"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let ollama_client =
            OllamaClient::try_from_provider_with_base_url(&format!("{}/v1", server.uri()))
                .await
                .expect("probe OpenAI compat");
        ollama_client
            .probe_server()
            .await
            .expect("probe OpenAI compat");
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

        // OpenAI‑compat models endpoint responds OK.
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/v1/models"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;

        OllamaClient::try_from_provider_with_base_url(&format!("{}/v1", server.uri()))
            .await
            .expect("client should be created when probe succeeds");
    }

    #[tokio::test]
    async fn test_try_from_provider_preserves_outbound_proxy_policy() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} set; skipping test_try_from_provider_preserves_outbound_proxy_policy",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let proxy = wiremock::MockServer::start().await;
        let base_url = "http://ollama-proxy.invalid";
        let request_url = format!("{base_url}/api/tags");
        codex_http_client::cache_system_proxy_route_for_test(&request_url, proxy.uri());

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/tags"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .expect(1)
            .mount(&proxy)
            .await;

        let provider = create_oss_provider_with_base_url(base_url, WireApi::Responses);
        let client = OllamaClient::try_from_provider(
            &provider,
            HttpClientFactory::new(OutboundProxyPolicy::RespectSystemProxy),
        )
        .await
        .expect("client should preserve the configured outbound proxy policy");

        assert_eq!(
            client.client.outbound_proxy_policy(),
            OutboundProxyPolicy::RespectSystemProxy
        );
    }

    #[tokio::test]
    async fn test_try_from_provider_handles_invalid_custom_ca_by_proxy_policy() {
        const CHILD_POLICY_ENV: &str = "CODEX_OLLAMA_INVALID_CA_TEST_POLICY";

        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} set; skipping test_try_from_provider_handles_invalid_custom_ca_by_proxy_policy",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let Ok(policy_name) = std::env::var(CHILD_POLICY_ENV) else {
            let invalid_ca_path = std::env::temp_dir().join(format!(
                "codex-ollama-invalid-ca-{}.pem",
                std::process::id()
            ));
            std::fs::write(&invalid_ca_path, "not a PEM certificate")
                .expect("invalid CA fixture should be written");

            for ca_env in ["CODEX_CA_CERTIFICATE", "SSL_CERT_FILE"] {
                for policy_name in ["reqwest-default", "respect-system-proxy"] {
                    let output = std::process::Command::new(
                        std::env::current_exe().expect("test executable should be available"),
                    )
                    .arg("--exact")
                    .arg("client::tests::test_try_from_provider_handles_invalid_custom_ca_by_proxy_policy")
                    .arg("--nocapture")
                    .env_remove("CODEX_CA_CERTIFICATE")
                    .env_remove("SSL_CERT_FILE")
                    .env(ca_env, &invalid_ca_path)
                    .env(CHILD_POLICY_ENV, policy_name)
                    .output()
                    .expect("isolated CA subprocess should run");

                    assert!(
                        output.status.success(),
                        "{policy_name} failed with invalid {ca_env}\nstdout:\n{}\nstderr:\n{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr),
                    );
                }
            }

            std::fs::remove_file(invalid_ca_path).expect("invalid CA fixture should be removed");
            return;
        };

        let outbound_proxy_policy = match policy_name.as_str() {
            "reqwest-default" => OutboundProxyPolicy::ReqwestDefault,
            "respect-system-proxy" => OutboundProxyPolicy::RespectSystemProxy,
            _ => panic!("unexpected test proxy policy: {policy_name}"),
        };
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/tags"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let provider = create_oss_provider_with_base_url(&server.uri(), WireApi::Responses);
        let result = OllamaClient::try_from_provider(
            &provider,
            HttpClientFactory::new(outbound_proxy_policy),
        )
        .await;

        match outbound_proxy_policy {
            OutboundProxyPolicy::ReqwestDefault => {
                result.expect("default-routed Ollama should fall back to system roots");
                assert_eq!(
                    server
                        .received_requests()
                        .await
                        .expect("mock server should report requests")
                        .len(),
                    1
                );
            }
            OutboundProxyPolicy::RespectSystemProxy => {
                let error = result
                    .err()
                    .expect("system-proxy Ollama should reject invalid custom CAs");
                let ca_env = if std::env::var_os("CODEX_CA_CERTIFICATE").is_some() {
                    "CODEX_CA_CERTIFICATE"
                } else {
                    "SSL_CERT_FILE"
                };
                assert!(
                    error.to_string().contains(ca_env),
                    "expected actionable {ca_env} error, got: {error}"
                );
                assert_ne!(error.to_string(), OLLAMA_CONNECTION_ERROR);
                assert!(
                    server
                        .received_requests()
                        .await
                        .expect("mock server should report requests")
                        .is_empty()
                );
            }
        }
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
        let err = OllamaClient::try_from_provider_with_base_url(&format!("{}/v1", server.uri()))
            .await
            .err()
            .expect("expected error");
        assert_eq!(OLLAMA_CONNECTION_ERROR, err.to_string());
    }
}
