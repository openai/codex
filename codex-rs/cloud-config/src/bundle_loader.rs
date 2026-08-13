use crate::backend::BackendBundleClient;
use crate::backend::BundleClient;
use crate::service::CLOUD_CONFIG_BUNDLE_TIMEOUT;
use crate::service::CloudConfigBundleService;
use codex_config::CloudConfigBundleLoader;
use codex_http_client::HttpClientFactory;
use codex_login::AuthConfig;
use codex_login::AuthManager;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use tokio::task::AbortHandle;
use tokio::task::JoinHandle;

fn refresher_task_slot() -> &'static Mutex<Option<AbortHandle>> {
    static REFRESHER_TASK: OnceLock<Mutex<Option<AbortHandle>>> = OnceLock::new();
    REFRESHER_TASK.get_or_init(|| Mutex::new(None))
}

pub(crate) fn replace_refresh_task(slot: &Mutex<Option<AbortHandle>>, next: AbortHandle) {
    let mut guard = slot.lock().unwrap_or_else(|err| {
        tracing::warn!("cloud config bundle refresher task slot was poisoned");
        err.into_inner()
    });
    if let Some(previous) = guard.replace(next) {
        previous.abort();
    }
}

struct CloudConfigBundleLoaderLifetime<C> {
    service: Arc<CloudConfigBundleService<C>>,
    refresh_task: JoinHandle<()>,
}

impl<C> Drop for CloudConfigBundleLoaderLifetime<C> {
    fn drop(&mut self) {
        self.refresh_task.abort();
    }
}

pub fn cloud_config_bundle_loader(
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
    codex_home: PathBuf,
    http_client_factory: HttpClientFactory,
) -> CloudConfigBundleLoader {
    let service = CloudConfigBundleService::new(
        auth_manager,
        Arc::new(BackendBundleClient::new(
            chatgpt_base_url,
            http_client_factory,
        )),
        codex_home,
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );
    let (loader, refresh_task) = cloud_config_bundle_loader_for_service(service);
    replace_refresh_task(refresher_task_slot(), refresh_task);
    loader
}

pub(crate) fn cloud_config_bundle_loader_for_service<C>(
    service: CloudConfigBundleService<C>,
) -> (CloudConfigBundleLoader, AbortHandle)
where
    C: BundleClient + 'static,
{
    let service = Arc::new(service);
    let background_service = Arc::clone(&service);
    let refresh_task = tokio::spawn(async move {
        let _ = background_service.get_latest().await;
        background_service.refresh_cache_in_background().await;
    });
    let abort_handle = refresh_task.abort_handle();
    let lifetime = Arc::new(CloudConfigBundleLoaderLifetime {
        service,
        refresh_task,
    });

    let loader = CloudConfigBundleLoader::from_getter(move || {
        let lifetime = Arc::clone(&lifetime);
        async move { lifetime.service.get_latest().await }
    });
    (loader, abort_handle)
}

pub async fn cloud_config_bundle_loader_for_storage(
    auth_config: AuthConfig,
    enable_codex_api_key_env: bool,
) -> std::io::Result<CloudConfigBundleLoader> {
    let auth_manager =
        AuthManager::shared_from_auth_config(auth_config.clone(), enable_codex_api_key_env).await?;
    Ok(cloud_config_bundle_loader_from_auth_config(
        auth_config,
        auth_manager,
    ))
}

fn cloud_config_bundle_loader_from_auth_config(
    auth_config: AuthConfig,
    auth_manager: Arc<AuthManager>,
) -> CloudConfigBundleLoader {
    cloud_config_bundle_loader(
        auth_manager,
        auth_config
            .chatgpt_base_url
            .unwrap_or_else(|| "https://chatgpt.com/backend-api/".to_string()),
        auth_config.codex_home,
        auth_config.auth_route_config.http_client_factory().clone(),
    )
}
