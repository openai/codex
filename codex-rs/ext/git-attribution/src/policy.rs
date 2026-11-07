use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use codex_backend_client::Client as BackendClient;
use codex_extension_api::ExtensionData;
use codex_features::RuntimeAuthMode;
use codex_http_client::HttpClientFactory;
use codex_login::AuthManager;
use codex_runtime_agent_instance::RuntimeAuthManager;
use tokio::time::timeout;

#[derive(Clone, Debug)]
pub(super) struct GitAttributionPolicy {
    pub(super) auth_generation: u64,
    pub(super) enabled: bool,
}

pub(super) struct GitAttributionRetry {
    pub(super) auth_generation: u64,
    pub(super) retry_at: Instant,
}

pub(super) fn retry_deferred(thread_store: &ExtensionData, auth_generation: u64) -> bool {
    thread_store
        .get::<GitAttributionRetry>()
        .is_some_and(|retry| {
            retry.auth_generation == auth_generation && retry.retry_at > Instant::now()
        })
}

pub(super) fn cached_attribution_policy(
    thread_store: &ExtensionData,
    turn_store: &ExtensionData,
    auth_generation: u64,
) -> Option<GitAttributionPolicy> {
    thread_store
        .get::<GitAttributionPolicy>()
        .filter(|policy| policy.auth_generation == auth_generation)
        .or_else(|| {
            turn_store
                .get::<GitAttributionPolicy>()
                .filter(|policy| policy.auth_generation == auth_generation)
        })
        .map(|policy| policy.as_ref().clone())
}

#[cfg(not(test))]
const POLICY_RESOLUTION_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(test)]
const POLICY_RESOLUTION_TIMEOUT: Duration = Duration::from_millis(500);
pub(super) const POLICY_RETRY_DELAY: Duration = Duration::from_secs(30);

pub(super) async fn resolve_attribution_policy(
    auth_manager: &Arc<AuthManager>,
    base_url: &str,
    http_client_factory: &HttpClientFactory,
    runtime_auth: Option<&(Arc<RuntimeAuthManager>, RuntimeAuthMode)>,
) -> Result<Option<GitAttributionPolicy>, tokio::time::error::Elapsed> {
    timeout(POLICY_RESOLUTION_TIMEOUT, async {
        let mut recovery_generation = auth_generation(auth_manager);
        let mut auth_recovery = auth_manager.unauthorized_recovery();
        loop {
            let auth_generation_at_start = auth_generation(auth_manager);
            if auth_generation_at_start != recovery_generation {
                auth_recovery = auth_manager.unauthorized_recovery();
                recovery_generation = auth_generation_at_start;
            }
            let auth = auth_manager.auth_with_http_client_factory().await;
            if auth_generation(auth_manager) != auth_generation_at_start {
                continue;
            }
            let enabled = match auth {
                Some((auth, captured_factory)) if auth.uses_codex_backend() => {
                    let policy = http_client_factory
                        .network_policy()
                        .clone()
                        .with_account_binding(captured_factory.network_policy())?;
                    let client = BackendClient::from_auth_with_runtime(
                        base_url,
                        &auth,
                        http_client_factory.clone().with_network_policy(policy),
                        runtime_auth,
                        /*headers*/ &Default::default(),
                    )
                    .await;
                    if auth_generation(auth_manager) != auth_generation_at_start {
                        continue;
                    }
                    let settings = client.ok()?.get_user_settings().await;
                    if auth_generation(auth_manager) != auth_generation_at_start {
                        continue;
                    }
                    match settings {
                        Ok(settings) => Some(settings.commit_attribution_enabled),
                        Err(err) if err.is_unauthorized() && auth_recovery.has_next() => {
                            if auth_recovery.next().await.is_ok() {
                                recovery_generation = auth_generation(auth_manager);
                                continue;
                            }
                            None
                        }
                        Err(_) => None,
                    }
                }
                Some(_) | None => Some(false),
            };
            if auth_generation(auth_manager) != auth_generation_at_start {
                continue;
            }
            return enabled.map(|enabled| GitAttributionPolicy {
                auth_generation: auth_generation_at_start,
                enabled,
            });
        }
    })
    .await
}

pub(super) fn auth_generation(auth_manager: &AuthManager) -> u64 {
    *auth_manager.auth_change_receiver().borrow()
}
