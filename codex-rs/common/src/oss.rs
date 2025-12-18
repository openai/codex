//! OSS provider utilities shared between TUI and exec.

use codex_core::LMSTUDIO_OSS_PROVIDER_ID;
use codex_core::OLLAMA_OSS_PROVIDER_ID;
use codex_core::WireApi;
use codex_core::config::Config;

/// Returns the default model for a given OSS provider.
pub fn get_default_model_for_oss_provider(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        LMSTUDIO_OSS_PROVIDER_ID => Some(codex_lmstudio::DEFAULT_OSS_MODEL),
        OLLAMA_OSS_PROVIDER_ID => Some(codex_ollama::DEFAULT_OSS_MODEL),
        _ => None,
    }
}

/// Ensures the specified OSS provider is ready (models downloaded, service reachable).
pub async fn ensure_oss_provider_ready(
    provider_id: &str,
    config: &mut Config,
) -> Result<(), std::io::Error> {
    match provider_id {
        LMSTUDIO_OSS_PROVIDER_ID => {
            codex_lmstudio::ensure_oss_ready(config)
                .await
                .map_err(|e| std::io::Error::other(format!("OSS setup failed: {e}")))?;
        }
        OLLAMA_OSS_PROVIDER_ID => {
            if config.model_provider.wire_api == WireApi::Responses {
                match codex_ollama::detect_wire_api(&config.model_provider).await {
                    Ok(Some(detection)) if detection.wire_api == WireApi::Chat => {
                        config.model_provider.wire_api = WireApi::Chat;
                    }
                    Ok(_) | Err(_) => {}
                }
            }

            codex_ollama::ensure_oss_ready(config)
                .await
                .map_err(|e| std::io::Error::other(format!("OSS setup failed: {e}")))?;
        }
        _ => {
            // Unknown provider, skip setup
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_default_model_for_provider_lmstudio() {
        let result = get_default_model_for_oss_provider(LMSTUDIO_OSS_PROVIDER_ID);
        assert_eq!(result, Some(codex_lmstudio::DEFAULT_OSS_MODEL));
    }

    #[test]
    fn test_get_default_model_for_provider_ollama() {
        let result = get_default_model_for_oss_provider(OLLAMA_OSS_PROVIDER_ID);
        assert_eq!(result, Some(codex_ollama::DEFAULT_OSS_MODEL));
    }

    #[test]
    fn test_get_default_model_for_provider_unknown() {
        let result = get_default_model_for_oss_provider("unknown-provider");
        assert_eq!(result, None);
    }
}
