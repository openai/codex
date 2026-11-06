//! Construct and share the harness-owned runtime credential context.

use crate::config::Config;
use codex_features::Feature;
use codex_login::AuthManager;
use codex_protocol::error::Result as CodexResult;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::RuntimeRegistrationMetadata;
use codex_runtime_agent_instance::RuntimeRole;
use std::sync::Arc;
use std::sync::OnceLock;

// A later thread may enable runtime auth even when harness startup left it disabled.
pub(crate) type RuntimeAuthSlot = Arc<OnceLock<Arc<RuntimeAuthManager>>>;

/// Share this owner between thread sessions and first-party API clients.
/// Construction does not register a runtime or generate its key.
pub fn build_runtime_auth_manager(
    config: &Config,
    credentials: Arc<AuthManager>,
    installation_id: &str,
) -> Option<Arc<RuntimeAuthManager>> {
    runtime_for_config(
        &RuntimeAuthSlot::default(),
        config,
        credentials,
        installation_id,
    )
}

pub(crate) fn runtime_for_config(
    slot: &RuntimeAuthSlot,
    config: &Config,
    credentials: Arc<AuthManager>,
    installation_id: &str,
) -> Option<Arc<RuntimeAuthManager>> {
    config
        .features
        .enabled(Feature::RuntimeAgentInstances)
        .then(|| {
            Arc::clone(slot.get_or_init(|| {
                Arc::new(RuntimeAuthManager::for_chatgpt(
                    credentials,
                    installation_id.to_owned(),
                    config.chatgpt_base_url.clone(),
                    RuntimeRegistrationMetadata {
                        harness: Some("codex".to_string()),
                        harness_version: Some(env!("CARGO_PKG_VERSION").to_string()),
                        role: RuntimeRole::Orchestrator,
                    },
                ))
            }))
        })
}

pub(crate) fn is_openai_realtime_url(base_url: &str) -> CodexResult<bool> {
    let Ok(mut url) = url::Url::parse(base_url) else {
        return Ok(false);
    };
    if url.scheme() == "wss" && url.set_scheme("https").is_err() {
        return Ok(false);
    }
    codex_model_provider::validate_runtime_destination(Some(url.as_str()))?;
    Ok(codex_runtime_agent_instance::is_openai_api_url(
        url.as_str(),
    ))
}

#[cfg(test)]
mod tests {
    use super::is_openai_realtime_url;

    #[test]
    fn realtime_runtime_auth_rejects_unsupported_openai_origins() {
        for url in [
            "ws://api.openai.com/v1",
            "http://api.openai.com/v1",
            "wss://api.openai.com:8443/v1",
            "wss://unknown.openai.com/v1",
        ] {
            assert!(is_openai_realtime_url(url).is_err(), "{url}");
        }
        assert!(is_openai_realtime_url("wss://api.openai.com/v1").unwrap());
        assert!(is_openai_realtime_url("https://api.openai.org/v1").unwrap());
        assert!(!is_openai_realtime_url("wss://provider.example.com/v1").unwrap());
    }
}
