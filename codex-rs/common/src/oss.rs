//! OSS provider utilities shared between TUI and exec.

use codex_core::LMSTUDIO_OSS_PROVIDER_ID;
use codex_core::OLLAMA_CHAT_PROVIDER_ID;
use codex_core::OLLAMA_OSS_PROVIDER_ID;
use codex_core::WireApi;
use codex_core::config::Config;
use codex_core::protocol::DeprecationNoticeEvent;
use std::io;

/// Returns the default model for a given OSS provider.
pub fn get_default_model_for_oss_provider(provider_id: &str) -> Option<&'static str> {
    match provider_id {
        LMSTUDIO_OSS_PROVIDER_ID => Some(codex_lmstudio::DEFAULT_OSS_MODEL),
        OLLAMA_OSS_PROVIDER_ID | OLLAMA_CHAT_PROVIDER_ID => Some(codex_ollama::DEFAULT_OSS_MODEL),
        _ => None,
    }
}

/// Returns a deprecation notice if Ollama doesn't support the responses wire API.
pub async fn ollama_chat_deprecation_notice(
    config: &Config,
) -> io::Result<Option<DeprecationNoticeEvent>> {
    if config.model_provider_id != OLLAMA_OSS_PROVIDER_ID
        || config.model_provider.wire_api != WireApi::Responses
    {
        return Ok(None);
    }

    if let Some(detection) = codex_ollama::detect_wire_api(&config.model_provider).await?
        && detection.wire_api == WireApi::Chat
    {
        let version_suffix = detection
            .version
            .as_ref()
            .map(|version| format!(" (version {version})"))
            .unwrap_or_default();
        let summary = format!(
            "Your Ollama server{version_suffix} doesn't support the Responses API. Either update Ollama or set `oss_provider = \"{OLLAMA_CHAT_PROVIDER_ID}\"` (or `model_provider = \"{OLLAMA_CHAT_PROVIDER_ID}\"`) in your config.toml to use the \"chat\" wire API. Support for the \"chat\" wire API is deprecated and will soon be removed."
        );
        return Ok(Some(DeprecationNoticeEvent {
            summary,
            details: None,
        }));
    }

    Ok(None)
}

/// Ensures the specified OSS provider is ready (models downloaded, service reachable).
pub async fn ensure_oss_provider_ready(
    provider_id: &str,
    config: &Config,
) -> Result<(), std::io::Error> {
    match provider_id {
        LMSTUDIO_OSS_PROVIDER_ID => {
            codex_lmstudio::ensure_oss_ready(config)
                .await
                .map_err(|e| std::io::Error::other(format!("OSS setup failed: {e}")))?;
        }
        OLLAMA_OSS_PROVIDER_ID | OLLAMA_CHAT_PROVIDER_ID => {
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
