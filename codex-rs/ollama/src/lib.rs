mod client;
mod parser;
mod pull;
pub mod url;

pub use client::OllamaClient;
use codex_core::BUILT_IN_OSS_MODEL_PROVIDER_ID;
use codex_core::config::Config;
pub use pull::CliProgressReporter;
pub use pull::PullEvent;
pub use pull::PullProgressReporter;
pub use pull::TuiProgressReporter;

/// Default OSS model to use when `--oss` is passed without an explicit `-m`.
pub const DEFAULT_OSS_MODEL: &str = "gpt-oss:20b";

/// Prepare the local OSS environment when `--oss` is selected.
///
/// - Ensures a local Ollama server is reachable.
/// - Checks if the model exists locally and pulls it if missing.
pub async fn ensure_oss_ready(config: &Config) -> std::io::Result<()> {
    // Only download when the requested model is the default OSS model (or when -m is not provided).
    let model = config.model.as_ref();

    tracing::info!("Starting OSS setup for model: {:?}", model);

    // Verify local Ollama is reachable with enhanced error handling.
    let ollama_client = match crate::OllamaClient::try_from_oss_provider(config).await {
        Ok(client) => {
            tracing::info!("Successfully connected to Ollama server");
            client
        }
        Err(e) => {
            // Provide more useful debugging information
            tracing::error!("Failed to connect to Ollama: {}", e);

            // Check user configuration
            if let Some(provider) = config.model_providers.get(BUILT_IN_OSS_MODEL_PROVIDER_ID)
                && let Some(base_url) = &provider.base_url {
                    tracing::info!("Configured Ollama base_url: {}", base_url);
                }

            // Check environment variables
            if let Ok(env_url) = std::env::var("CODEX_OSS_BASE_URL") {
                tracing::info!("CODEX_OSS_BASE_URL environment variable: {}", env_url);
            }
            if let Ok(env_port) = std::env::var("CODEX_OSS_PORT") {
                tracing::info!("CODEX_OSS_PORT environment variable: {}", env_port);
            }

            return Err(std::io::Error::other(format!(
                "OSS setup failed: {e}. Please ensure Ollama is running with 'ollama serve' and accessible at the configured URL."
            )));
        }
    };

    // If the model is not present locally, pull it with better error handling.
    match ollama_client.fetch_models().await {
        Ok(models) => {
            tracing::debug!("Available Ollama models: {:?}", models);

            if !models.iter().any(|m| m == model) {
                tracing::info!("Model '{}' not found locally, attempting to pull", model);
                let mut reporter = crate::CliProgressReporter::new();

                match ollama_client.pull_with_reporter(model, &mut reporter).await {
                    Ok(_) => tracing::info!("Successfully pulled model: {}", model),
                    Err(e) => {
                        tracing::error!("Failed to pull model '{}': {}", model, e);
                        return Err(e);
                    }
                }
            } else {
                tracing::info!("Model '{}' is already available locally", model);
            }
        }
        Err(err) => {
            // Not fatal; higher layers may still proceed and surface errors later.
            tracing::warn!(
                "Failed to query local models from Ollama: {}. Will attempt to proceed.",
                err
            );
        }
    }

    Ok(())
}
