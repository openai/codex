use codex_core::config::Config;
use codex_http_client::ClientRouteClass;
use codex_http_client::RouteAwareClientPool;
use codex_model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
use std::io;
use std::path::Path;
use std::time::Duration;

#[derive(Clone)]
pub struct LMStudioClient {
    client: RouteAwareClientPool,
    base_url: String,
}

const LMSTUDIO_CONNECTION_ERROR: &str = "LM Studio is not responding. Install from https://lmstudio.ai/download and run 'lms server start'.";
const LMSTUDIO_CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

impl LMStudioClient {
    pub async fn try_from_provider(config: &Config) -> std::io::Result<Self> {
        let provider = config
            .model_providers
            .get(LMSTUDIO_OSS_PROVIDER_ID)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Built-in provider {LMSTUDIO_OSS_PROVIDER_ID} not found",),
                )
            })?;
        let base_url = provider.base_url.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "oss provider must have a base_url",
            )
        })?;

        let client = RouteAwareClientPool::with_connect_timeout(
            config.http_client_factory(),
            ClientRouteClass::Other,
            LMSTUDIO_CONNECTION_TIMEOUT,
        );

        let client = LMStudioClient {
            client,
            base_url: base_url.to_string(),
        };
        client.check_server().await?;

        Ok(client)
    }

    async fn check_server(&self) -> io::Result<()> {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let response = self.client.get(&url).send().await;

        if let Ok(resp) = response {
            if resp.status().is_success() {
                Ok(())
            } else {
                Err(io::Error::other(format!(
                    "Server returned error: {} {LMSTUDIO_CONNECTION_ERROR}",
                    resp.status()
                )))
            }
        } else {
            Err(io::Error::other(LMSTUDIO_CONNECTION_ERROR))
        }
    }

    // Load a model by sending an empty request with max_tokens 1
    pub async fn load_model(&self, model: &str) -> io::Result<()> {
        let url = format!("{}/responses", self.base_url.trim_end_matches('/'));

        let request_body = serde_json::json!({
            "model": model,
            "input": "",
            "max_output_tokens": 1
        });

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| io::Error::other(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            tracing::info!("Successfully loaded model '{model}'");
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "Failed to load model: {}",
                response.status()
            )))
        }
    }

    // Return the list of models available on the LM Studio server.
    pub async fn fetch_models(&self) -> io::Result<Vec<String>> {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| io::Error::other(format!("Request failed: {e}")))?;

        if response.status().is_success() {
            let json: serde_json::Value = response.json().await.map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("JSON parse error: {e}"))
            })?;
            let models = json["data"]
                .as_array()
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "No 'data' array in response")
                })?
                .iter()
                .filter_map(|model| model["id"].as_str())
                .map(std::string::ToString::to_string)
                .collect();
            Ok(models)
        } else {
            Err(io::Error::other(format!(
                "Failed to fetch models: {}",
                response.status()
            )))
        }
    }

    // Find lms, checking fallback paths if not in PATH
    fn find_lms() -> std::io::Result<String> {
        Self::find_lms_with_home_dir(/*home_dir*/ None)
    }

    fn find_lms_with_home_dir(home_dir: Option<&str>) -> std::io::Result<String> {
        // First try 'lms' in PATH
        if which::which("lms").is_ok() {
            return Ok("lms".to_string());
        }

        // Platform-specific fallback paths
        let home = match home_dir {
            Some(dir) => dir.to_string(),
            None => {
                #[cfg(unix)]
                {
                    std::env::var("HOME").unwrap_or_default()
                }
                #[cfg(windows)]
                {
                    std::env::var("USERPROFILE").unwrap_or_default()
                }
            }
        };

        #[cfg(unix)]
        let fallback_path = format!("{home}/.lmstudio/bin/lms");

        #[cfg(windows)]
        let fallback_path = format!("{home}/.lmstudio/bin/lms.exe");

        if Path::new(&fallback_path).exists() {
            Ok(fallback_path)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "LM Studio not found. Please install LM Studio from https://lmstudio.ai/",
            ))
        }
    }

    pub async fn download_model(&self, model: &str) -> std::io::Result<()> {
        let lms = Self::find_lms()?;
        eprintln!("Downloading model: {model}");

        let status = std::process::Command::new(&lms)
            .args(["get", "--yes", model])
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| {
                std::io::Error::other(format!("Failed to execute '{lms} get --yes {model}': {e}"))
            })?;

        if !status.success() {
            return Err(std::io::Error::other(format!(
                "Model download failed with exit code: {}",
                status.code().unwrap_or(-1)
            )));
        }

        tracing::info!("Successfully downloaded model '{model}'");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    fn client_from_host_root(
        host_root: impl Into<String>,
        connection_timeout: Duration,
    ) -> LMStudioClient {
        let client = RouteAwareClientPool::with_connect_timeout(
            codex_http_client::HttpClientFactory::new(
                codex_http_client::OutboundProxyPolicy::ReqwestDefault,
            ),
            ClientRouteClass::Other,
            connection_timeout,
        );
        LMStudioClient {
            client,
            base_url: host_root.into(),
        }
    }

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
            .and(wiremock::matchers::path("/models"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_raw(
                    serde_json::json!({
                        "data": [
                            {"id": "openai/gpt-oss-20b"},
                        ]
                    })
                    .to_string(),
                    "application/json",
                ),
            )
            .mount(&server)
            .await;

        let client = client_from_host_root(server.uri(), LMSTUDIO_CONNECTION_TIMEOUT);
        let models = client.fetch_models().await.expect("fetch models");
        assert!(models.contains(&"openai/gpt-oss-20b".to_string()));
    }

    #[tokio::test]
    async fn test_fetch_models_no_data_array() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} is set; skipping test_fetch_models_no_data_array",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/models"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_raw(serde_json::json!({}).to_string(), "application/json"),
            )
            .mount(&server)
            .await;

        let client = client_from_host_root(server.uri(), LMSTUDIO_CONNECTION_TIMEOUT);
        let result = client.fetch_models().await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No 'data' array in response")
        );
    }

    #[tokio::test]
    async fn test_fetch_models_server_error() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} is set; skipping test_fetch_models_server_error",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/models"))
            .respond_with(wiremock::ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = client_from_host_root(server.uri(), LMSTUDIO_CONNECTION_TIMEOUT);
        let result = client.fetch_models().await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to fetch models: 500")
        );
    }

    #[tokio::test]
    async fn test_check_server_happy_path() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} is set; skipping test_check_server_happy_path",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/models"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = client_from_host_root(server.uri(), LMSTUDIO_CONNECTION_TIMEOUT);
        client
            .check_server()
            .await
            .expect("server check should pass");
    }

    #[tokio::test]
    async fn test_check_server_allows_slow_response_after_connect() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} is set; skipping test_check_server_allows_slow_response_after_connect",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/models"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_delay(Duration::from_millis(250)),
            )
            .mount(&server)
            .await;

        let client = client_from_host_root(server.uri(), Duration::from_millis(100));

        client
            .check_server()
            .await
            .expect("server check should allow a slow response after connecting");
    }

    #[tokio::test]
    async fn test_check_server_error() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} is set; skipping test_check_server_error",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/models"))
            .respond_with(wiremock::ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = client_from_host_root(server.uri(), LMSTUDIO_CONNECTION_TIMEOUT);
        let result = client.check_server().await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Server returned error: 404")
        );
    }

    #[test]
    fn test_find_lms() {
        let result = LMStudioClient::find_lms();

        match result {
            Ok(_) => {
                // lms was found in PATH - that's fine
            }
            Err(e) => {
                // Expected error when LM Studio not installed
                assert!(e.to_string().contains("LM Studio not found"));
            }
        }
    }

    #[test]
    fn test_find_lms_with_mock_home() {
        // Test fallback path construction without touching env vars
        #[cfg(unix)]
        {
            let result = LMStudioClient::find_lms_with_home_dir(Some("/test/home"));
            if let Err(e) = result {
                assert!(e.to_string().contains("LM Studio not found"));
            }
        }

        #[cfg(windows)]
        {
            let result = LMStudioClient::find_lms_with_home_dir(Some("C:\\test\\home"));
            if let Err(e) = result {
                assert!(e.to_string().contains("LM Studio not found"));
            }
        }
    }
}
