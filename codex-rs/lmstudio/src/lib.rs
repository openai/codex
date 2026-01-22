mod client;

pub use client::LMStudioClient;
use codex_core::config::Config;

/// Default OSS model to use when `--oss` is passed without an explicit `-m`.
pub const DEFAULT_OSS_MODEL: &str = "openai/gpt-oss-20b";

/// Prepare the local OSS environment when `--oss` is selected.
///
/// - Ensures a local LM Studio server is reachable.
/// - Checks if the model exists locally and downloads it if missing.
pub async fn ensure_oss_ready(config: &Config) -> std::io::Result<()> {
    let model = match config.model.as_ref() {
        Some(model) => model,
        None => DEFAULT_OSS_MODEL,
    };

    // Verify local LM Studio is reachable.
    let lmstudio_client = LMStudioClient::try_from_provider(config).await?;

    match lmstudio_client.fetch_models().await {
        Ok(models) => {
            if !models.iter().any(|m| m == model) {
                lmstudio_client.download_model(model).await?;
            }
        }
        Err(err) => {
            // Not fatal; higher layers may still proceed and surface errors later.
            tracing::warn!("Failed to query local models from LM Studio: {}.", err);
        }
    }

    // Load the model in the background
    tokio::spawn({
        let client = lmstudio_client.clone();
        let model = model.to_string();
        async move {
            if let Err(e) = client.load_model(&model).await {
                tracing::warn!("Failed to load model {}: {}", model, e);
            }
        }
    });

    Ok(())
}
