use crate::backend::BackendBundleClient;
use crate::service::CLOUD_CONFIG_BUNDLE_CACHE_REFRESH_RETRY_INTERVAL;
use crate::service::CLOUD_CONFIG_BUNDLE_TIMEOUT;
use crate::service::CloudConfigBundleService;
use crate::service::StartupLoad;
use codex_config::CloudConfigBundleLoader;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::AuthManager;
use std::path::PathBuf;
use std::sync::Arc;

pub fn cloud_config_bundle_loader(
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
    codex_home: PathBuf,
) -> CloudConfigBundleLoader {
    let service = CloudConfigBundleService::new(
        auth_manager,
        Arc::new(BackendBundleClient::new(chatgpt_base_url)),
        codex_home,
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );
    let (loader, publisher) = CloudConfigBundleLoader::pending();
    // Intentionally detach the task that owns startup and refresh. Publishing
    // stops the lifecycle if this loader and all clones were dropped.
    drop(tokio::spawn(async move {
        let result = service.load_startup_bundle().await;
        let (initial_bundle, refresh_in) = match result {
            Ok(StartupLoad::Inactive) => (Ok(None), None),
            Ok(StartupLoad::Active(loaded)) => (Ok(loaded.bundle), Some(loaded.refresh_in)),
            Err(err) => (
                Err(err),
                Some(CLOUD_CONFIG_BUNDLE_CACHE_REFRESH_RETRY_INTERVAL),
            ),
        };
        if !publisher.publish(initial_bundle) {
            return;
        }
        if let Some(refresh_in) = refresh_in {
            service
                .refresh_cache_in_background(refresh_in, publisher)
                .await;
        }
    }));
    loader
}

pub async fn cloud_config_bundle_loader_for_storage(
    codex_home: PathBuf,
    enable_codex_api_key_env: bool,
    credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    chatgpt_base_url: String,
) -> CloudConfigBundleLoader {
    let auth_manager = AuthManager::shared(
        codex_home.clone(),
        enable_codex_api_key_env,
        credentials_store_mode,
        Some(chatgpt_base_url.clone()),
        keyring_backend_kind,
    )
    .await;
    cloud_config_bundle_loader(auth_manager, chatgpt_base_url, codex_home)
}
