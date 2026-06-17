use crate::backend::BackendBundleClient;
use crate::service::CLOUD_CONFIG_BUNDLE_TIMEOUT;
use crate::service::CloudConfigBundleService;
use codex_config::CloudConfigBundleLoadError;
use codex_config::CloudConfigBundleLoadErrorCode;
use codex_config::CloudConfigBundleLoader;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::AuthManager;
use codex_login::AuthManagerConfig;
use codex_login::WorkloadIdentityConfig;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use tokio::task::JoinHandle;

fn refresher_task_slot() -> &'static Mutex<Option<JoinHandle<()>>> {
    static REFRESHER_TASK: OnceLock<Mutex<Option<JoinHandle<()>>>> = OnceLock::new();
    REFRESHER_TASK.get_or_init(|| Mutex::new(None))
}

struct StorageAuthManagerConfig {
    codex_home: PathBuf,
    credentials_store_mode: AuthCredentialsStoreMode,
    keyring_backend_kind: AuthKeyringBackendKind,
    chatgpt_base_url: String,
    workload_identity: Option<WorkloadIdentityConfig>,
    forced_chatgpt_workspace_id: Option<Vec<String>>,
}

impl AuthManagerConfig for StorageAuthManagerConfig {
    fn codex_home(&self) -> PathBuf {
        self.codex_home.clone()
    }

    fn cli_auth_credentials_store_mode(&self) -> AuthCredentialsStoreMode {
        self.credentials_store_mode
    }

    fn auth_keyring_backend_kind(&self) -> AuthKeyringBackendKind {
        self.keyring_backend_kind
    }

    fn forced_chatgpt_workspace_id(&self) -> Option<Vec<String>> {
        self.forced_chatgpt_workspace_id.clone()
    }

    fn chatgpt_base_url(&self) -> String {
        self.chatgpt_base_url.clone()
    }

    fn workload_identity(&self) -> Option<WorkloadIdentityConfig> {
        self.workload_identity.clone()
    }
}

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
    keyring_backend_kind: AuthKeyringBackendKind,
    chatgpt_base_url: String,
    workload_identity: Option<WorkloadIdentityConfig>,
    forced_chatgpt_workspace_id: Option<Vec<String>>,
) -> CloudConfigBundleLoader {
    let auth_config = StorageAuthManagerConfig {
        codex_home: codex_home.clone(),
        credentials_store_mode,
        keyring_backend_kind,
        chatgpt_base_url: chatgpt_base_url.clone(),
        workload_identity,
        forced_chatgpt_workspace_id,
    };
    let auth_manager =
        AuthManager::shared_from_config(&auth_config, enable_codex_api_key_env).await;
    cloud_config_bundle_loader(auth_manager, chatgpt_base_url, codex_home)
}

#[cfg(test)]
#[path = "bundle_loader_tests.rs"]
mod tests;
