mod client;
mod line_buffer;
mod parser;
mod pull;
mod url;

pub use client::OllamaClient;
use codex_core::config::Config;
pub use pull::CliProgressReporter;
pub use pull::PullEvent;
pub use pull::PullProgressReporter;
pub use pull::TuiProgressReporter;
use semver::Version;

/// Default OSS model to use when `--oss` is passed without an explicit `-m`.
pub const DEFAULT_OSS_MODEL: &str = "gpt-oss:20b";

/// Prepare the local OSS environment when `--oss` is selected.
///
/// - Ensures a local Ollama server is reachable.
/// - Checks if the model exists locally and pulls it if missing.
pub async fn ensure_oss_ready(config: &Config, client: &OllamaClient) -> std::io::Result<()> {
    // Only download when the requested model is the default OSS model (or when -m is not provided).
    let model = match config.model.as_ref() {
        Some(model) => model,
        None => DEFAULT_OSS_MODEL,
    };

    // If the model is not present locally, pull it.
    match client.fetch_models().await {
        Ok(models) => {
            if !models.iter().any(|m| m == model) {
                let mut reporter = crate::CliProgressReporter::new();
                client.pull_with_reporter(model, &mut reporter).await?;
            }
        }
        Err(err) => {
            // Not fatal; higher layers may still proceed and surface errors later.
            tracing::warn!("Failed to query local models from Ollama: {}.", err);
        }
    }

    Ok(())
}

fn min_responses_version() -> Version {
    Version::new(0, 13, 4)
}

fn supports_responses(version: &Version) -> bool {
    *version == Version::new(0, 0, 0) || *version >= min_responses_version()
}

/// Ensure the running Ollama server is new enough to support the Responses API.
///
/// Returns `Ok(())` when the version endpoint is missing or unparsable.
pub async fn ensure_responses_supported(client: &OllamaClient) -> std::io::Result<()> {
    let Some(version) = client.fetch_version().await? else {
        return Ok(());
    };

    if supports_responses(&version) {
        return Ok(());
    }

    let min = min_responses_version();
    Err(std::io::Error::other(format!(
        "Ollama {version} is too old. Codex requires Ollama {min} or newer."
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_http_client::HttpClientFactory;
    use codex_http_client::OutboundProxyPolicy;
    use codex_model_provider_info::WireApi;
    use codex_model_provider_info::create_oss_provider_with_base_url;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn version_check_reuses_existing_ollama_client() {
        if std::env::var(codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR).is_ok() {
            tracing::info!(
                "{} set; skipping version_check_reuses_existing_ollama_client",
                codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR
            );
            return;
        }

        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/tags"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"models": [{"name": "gpt-oss:20b"}]})),
            )
            .expect(2)
            .mount(&server)
            .await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/api/version"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"version": "0.14.1"})),
            )
            .expect(1)
            .mount(&server)
            .await;

        let provider = create_oss_provider_with_base_url(&server.uri(), WireApi::Responses);
        let client = OllamaClient::try_from_provider(
            &provider,
            HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        )
        .await
        .expect("create Ollama client");

        ensure_responses_supported(&client)
            .await
            .expect("version check should reuse the existing client");
        assert_eq!(
            client.fetch_models().await.expect("fetch models"),
            vec!["gpt-oss:20b"]
        );

        server.verify().await;
    }

    #[test]
    fn supports_responses_for_dev_zero() {
        assert!(supports_responses(&Version::new(0, 0, 0)));
    }

    #[test]
    fn does_not_support_responses_before_cutoff() {
        assert!(!supports_responses(&Version::new(0, 13, 3)));
    }

    #[test]
    fn supports_responses_at_or_after_cutoff() {
        assert!(supports_responses(&Version::new(0, 13, 4)));
        assert!(supports_responses(&Version::new(0, 14, 0)));
    }
}
