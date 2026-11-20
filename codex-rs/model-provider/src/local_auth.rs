//! Local credential and runtime ownership for frontends that prepare provider requests.

use crate::DEFAULT_CHATGPT_BASE_URL;
use crate::ProviderAuthContext;
use crate::SharedModelProvider;
use crate::provider::provider_uses_first_party_auth_path;
use codex_features::RuntimeAuthMode;
use codex_login::AuthConfig;
use codex_login::AuthManager;
use codex_model_provider_info::ModelProviderInfo;
use codex_runtime_agent_instance::RuntimeAuthManager;
use codex_runtime_agent_instance::resolve_installation_id;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::OnceCell;

/// Reuse local credentials and their runtime while the login configuration stays the same.
#[derive(Default)]
pub struct LocalProviderAuth {
    auth: Mutex<Option<Arc<LocalAuth>>>,
}

struct LocalAuth {
    config: AuthConfig,
    credentials: OnceCell<Arc<AuthManager>>,
    runtime: OnceCell<Arc<RuntimeAuthManager>>,
}

impl LocalProviderAuth {
    pub async fn model_provider(
        &self,
        info: ModelProviderInfo,
        config: AuthConfig,
        runtime_auth_mode: Option<RuntimeAuthMode>,
    ) -> std::io::Result<SharedModelProvider> {
        let auth = {
            let mut cached = self.auth.lock().await;
            if cached.as_ref().is_some_and(|auth| auth.config != config) {
                *cached = None;
            }
            Arc::clone(cached.get_or_insert_with(|| {
                Arc::new(LocalAuth {
                    config,
                    credentials: OnceCell::new(),
                    runtime: OnceCell::new(),
                })
            }))
        };
        let credentials = auth
            .credentials
            .get_or_try_init(|| AuthManager::shared_for_auxiliary_client(auth.config.clone()))
            .await?;
        if provider_uses_first_party_auth_path(&info) {
            credentials.reload_with_cached_auth().await?;
        }
        let runtime_auth = if let Some(mode) = runtime_auth_mode {
            let runtime = auth
                .runtime
                .get_or_try_init(|| async {
                    let codex_home = auth.config.codex_home.clone().try_into()?;
                    let installation_id = resolve_installation_id(&codex_home).await?;
                    Ok::<_, std::io::Error>(Arc::new(RuntimeAuthManager::for_codex(
                        Arc::clone(credentials),
                        auth.config
                            .chatgpt_base_url
                            .as_deref()
                            .unwrap_or(DEFAULT_CHATGPT_BASE_URL),
                        &installation_id,
                    )))
                })
                .await?;
            Some((Arc::clone(runtime), mode))
        } else {
            None
        };
        Ok(ProviderAuthContext {
            credentials: Arc::clone(credentials),
            runtime_auth,
        }
        .model_provider(info))
    }
}
