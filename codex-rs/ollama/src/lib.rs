mod client;
mod parser;
mod pull;
mod url;

pub use client::OllamaClient;
pub use pull::CliProgressReporter;
pub use pull::PullEvent;
pub use pull::PullProgressReporter;
pub use pull::TuiProgressReporter;

/// Default OSS model to use when `--oss` is passed without an explicit `-m`.
pub const DEFAULT_OSS_MODEL: &str = "gpt-oss:20b";

/// Prepare the local OSS environment when `--oss` is selected.
///
/// - Ensures a local Ollama server is reachable.
/// - Selects the final model name (CLI override or default).
/// - Checks if the model exists locally and pulls it if missing.
///
/// Returns the final model name that should be used by the caller.
pub async fn ensure_oss_ready(cli_model: Option<String>) -> std::io::Result<String> {
    // Only download when the requested model is the default OSS model (or when -m is not provided).
    let should_download = cli_model
        .as_deref()
        .map(|name| name == DEFAULT_OSS_MODEL)
        .unwrap_or(true);
    let model = cli_model.unwrap_or_else(|| DEFAULT_OSS_MODEL.to_string());

    // Verify local Ollama is reachable.
    let ollama_client = crate::OllamaClient::try_from_oss_provider().await?;

    if should_download {
        // If the model is not present locally, pull it.
        match ollama_client.fetch_models().await {
            Ok(models) => {
                if !models.iter().any(|m| m == &model) {
                    let mut reporter = crate::CliProgressReporter::new();
                    ollama_client
                        .pull_with_reporter(&model, &mut reporter)
                        .await?;
                }
            }
            Err(err) => {
                // Not fatal; higher layers may still proceed and surface errors later.
                tracing::warn!("Failed to query local models from Ollama: {}.", err);
            }
        }
    }

    Ok(model)
}
