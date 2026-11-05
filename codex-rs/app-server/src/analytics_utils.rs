use std::sync::Arc;

use codex_analytics::AnalyticsEventsClient;
use codex_core::config::Config;
use codex_login::AuthManager;
use codex_runtime_agent_instance::RuntimeAuthSlot;

pub(crate) fn analytics_events_client_from_config(
    auth_manager: Arc<AuthManager>,
    config: &Config,
    runtime_auth_slot: Option<RuntimeAuthSlot>,
) -> AnalyticsEventsClient {
    let api_key_headers =
        if codex_model_provider::provider_uses_first_party_auth_path(&config.model_provider) {
            match config.model_provider.build_header_map() {
                Ok(headers) => headers,
                Err(error) => {
                    tracing::warn!(%error, "failed to resolve analytics resource selectors");
                    return AnalyticsEventsClient::disabled();
                }
            }
        } else {
            Default::default()
        };
    AnalyticsEventsClient::new_with_runtime(
        auth_manager,
        config.chatgpt_backend_url(),
        config.analytics_enabled,
        runtime_auth_slot.map(|slot| (slot, config.runtime_auth_mode)),
        api_key_headers,
    )
}
