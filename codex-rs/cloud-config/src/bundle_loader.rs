use crate::backend::BackendBundleClient;
use crate::service::CLOUD_CONFIG_BUNDLE_TIMEOUT;
use crate::service::CloudConfigBundleService;
use codex_config::CloudConfigBundleLoadError;
use codex_config::CloudConfigBundleLoadErrorCode;
use codex_config::CloudConfigBundleLoader;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthManager;
use codex_login::AuthRouteConfig;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use tokio::task::JoinHandle;

fn refresher_task_slot() -> &'static Mutex<Option<JoinHandle<()>>> {
    static REFRESHER_TASK: OnceLock<Mutex<Option<JoinHandle<()>>>> = OnceLock::new();
    REFRESHER_TASK.get_or_init(|| Mutex::new(None))
}

pub fn cloud_config_bundle_loader(
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
    codex_home: PathBuf,
) -> CloudConfigBundleLoader {
    let backend_client = BackendBundleClient::from_auth_manager(chatgpt_base_url, &auth_manager);
    let service = CloudConfigBundleService::new(
        auth_manager,
        Arc::new(backend_client),
        codex_home,
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );
    let refresh_service = service.clone();
    let task = tokio::spawn(async move { service.load_startup_bundle_with_timeout().await });
    let refresh_task =
        tokio::spawn(async move { refresh_service.refresh_cache_in_background().await });
    let mut refresher_guard = refresher_task_slot().lock().unwrap_or_else(|err| {
        tracing::warn!("cloud config bundle refresher task slot was poisoned");
        err.into_inner()
    });
    if let Some(existing_task) = refresher_guard.replace(refresh_task) {
        existing_task.abort();
    }
    CloudConfigBundleLoader::new(async move {
        task.await.map_err(|err| {
            tracing::error!(error = %err, "Cloud config bundle task failed");
            CloudConfigBundleLoadError::new(
                CloudConfigBundleLoadErrorCode::Internal,
                /*status_code*/ None,
                format!("cloud config bundle load failed: {err}"),
            )
        })?
    })
}

pub async fn cloud_config_bundle_loader_for_storage(
    codex_home: PathBuf,
    enable_codex_api_key_env: bool,
    credentials_store_mode: AuthCredentialsStoreMode,
    chatgpt_base_url: String,
) -> CloudConfigBundleLoader {
    cloud_config_bundle_loader_for_storage_with_auth_route_config(
        codex_home,
        enable_codex_api_key_env,
        credentials_store_mode,
        chatgpt_base_url,
        /*auth_route_config*/ None,
    )
    .await
}

/// Loads cloud config with the explicit startup-only auth route policy.
pub async fn cloud_config_bundle_loader_for_storage_with_auth_route_config(
    codex_home: PathBuf,
    enable_codex_api_key_env: bool,
    credentials_store_mode: AuthCredentialsStoreMode,
    chatgpt_base_url: String,
    auth_route_config: Option<AuthRouteConfig>,
) -> CloudConfigBundleLoader {
    let auth_manager = if let Some(auth_route_config) = auth_route_config {
        Arc::new(
            AuthManager::new_with_auth_route_config(
                codex_home.clone(),
                enable_codex_api_key_env,
                credentials_store_mode,
                Some(chatgpt_base_url.clone()),
                Some(auth_route_config),
            )
            .await,
        )
    } else {
        AuthManager::shared(
            codex_home.clone(),
            enable_codex_api_key_env,
            credentials_store_mode,
            Some(chatgpt_base_url.clone()),
        )
        .await
    };
    cloud_config_bundle_loader(auth_manager, chatgpt_base_url, codex_home)
}
