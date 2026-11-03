//! Construct and share the harness-owned runtime credential context.

use crate::config::Config;
use codex_features::Feature;
use codex_login::AuthManager;
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
