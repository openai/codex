mod client;
mod parser;
mod pull;
mod url;

pub use client::OllamaClient;
use codex_core::ModelProviderInfo;
use codex_core::WireApi;
use codex_core::config::Config;
pub use pull::CliProgressReporter;
pub use pull::PullEvent;
pub use pull::PullProgressReporter;
pub use pull::TuiProgressReporter;
use semver::Version;

/// Default OSS model to use when `--oss` is passed without an explicit `-m`.
pub const DEFAULT_OSS_MODEL: &str = "gpt-oss:20b";

pub struct WireApiDetection {
    pub wire_api: WireApi,
    pub version: Option<Version>,
}

/// Prepare the local OSS environment when `--oss` is selected.
///
/// - Ensures a local Ollama server is reachable.
/// - Checks if the model exists locally and pulls it if missing.
pub async fn ensure_oss_ready(config: &Config) -> std::io::Result<()> {
    // Only download when the requested model is the default OSS model (or when -m is not provided).
    let model = match config.model.as_ref() {
        Some(model) => model,
        None => DEFAULT_OSS_MODEL,
    };

    // Verify local Ollama is reachable.
    let ollama_client = crate::OllamaClient::try_from_oss_provider(config).await?;

    // If the model is not present locally, pull it.
    match ollama_client.fetch_models().await {
        Ok(models) => {
            if !models.iter().any(|m| m == model) {
                let mut reporter = crate::CliProgressReporter::new();
                ollama_client
                    .pull_with_reporter(model, &mut reporter)
                    .await?;
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

fn wire_api_for_version(version: &Version) -> WireApi {
    if *version == Version::new(0, 0, 0) || *version >= min_responses_version() {
        WireApi::Responses
    } else {
        WireApi::Chat
    }
}

/// Detect which wire API the running Ollama server supports based on its version.
/// Returns `Ok(None)` when the version endpoint is missing or unparsable; callers
/// should keep the configured default in that case.
pub async fn detect_wire_api(
    provider: &ModelProviderInfo,
) -> std::io::Result<Option<WireApiDetection>> {
    let client = crate::OllamaClient::try_from_provider(provider).await?;
    let Some(version) = client.fetch_version().await? else {
        return Ok(None);
    };

    let wire_api = wire_api_for_version(&version);

    Ok(Some(WireApiDetection {
        wire_api,
        version: Some(version),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_wire_api_for_version_dev_zero_keeps_responses() {
        assert_eq!(
            wire_api_for_version(&Version::new(0, 0, 0)),
            WireApi::Responses
        );
    }

    #[test]
    fn test_wire_api_for_version_before_cutoff_is_chat() {
        assert_eq!(wire_api_for_version(&Version::new(0, 13, 3)), WireApi::Chat);
    }

    #[test]
    fn test_wire_api_for_version_at_or_after_cutoff_is_responses() {
        assert_eq!(
            wire_api_for_version(&Version::new(0, 13, 4)),
            WireApi::Responses
        );
        assert_eq!(
            wire_api_for_version(&Version::new(0, 14, 0)),
            WireApi::Responses
        );
    }
}
