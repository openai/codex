use crate::backend::BackendBundleClient;
use crate::chatgpt::ChatGptSource;
use crate::config_server::config_server_http_client_factory;
use crate::config_source::BundleSource;
use crate::service::CLOUD_CONFIG_BUNDLE_TIMEOUT;
use crate::service::CloudConfigBundleService;
use codex_config::CloudConfigBundleLoader;
use codex_config::ConfigRequirementsToml;
use codex_http_client::HttpClientFactory;
use codex_login::AuthConfig;
use codex_login::AuthManager;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::AbortHandle;
use tokio::task::JoinHandle;

pub fn managed_config_auth_manager(
    auth: AuthConfig,
    requirements: &ConfigRequirementsToml,
) -> std::io::Result<Option<codex_login::OAuthSessionManager>> {
    let Some(service_config) = requirements.config_server.as_ref() else {
        return Ok(None);
    };
    let factory = config_server_http_client_factory(service_config, &auth.auth_route_config);
    crate::config_server_auth_manager(service_config, auth.codex_home, &factory)
}

pub fn managed_config_bundle_loader(
    auth_manager: Arc<AuthManager>,
    chatgpt_base_url: String,
    codex_home: PathBuf,
    factory: HttpClientFactory,
    requirements: &ConfigRequirementsToml,
) -> CloudConfigBundleLoader {
    if let Some(service_config) = requirements.config_server.as_ref() {
        let factory = config_server_http_client_factory(
            service_config,
            &codex_login::AuthRouteConfig::from_http_client_factory(factory),
        );
        return crate::config_server_bundle_loader(service_config.clone(), codex_home, factory);
    }
    let endpoint = codex_backend_client::Client::new(chatgpt_base_url.clone(), factory.clone())
        .config_bundle_url();
    let policy = factory
        .network_policy()
        .clone()
        .restrict_to_endpoints(endpoint.parse().into_iter().collect());
    cloud_config_bundle_loader(
        auth_manager,
        chatgpt_base_url,
        codex_home,
        factory.with_network_policy(policy),
    )
}

pub async fn managed_config_bundle_loader_for_storage(
    auth_config: AuthConfig,
    requirements: &ConfigRequirementsToml,
    enable_codex_api_key_env: bool,
) -> std::io::Result<CloudConfigBundleLoader> {
    if let Some(service_config) = requirements.config_server.as_ref() {
        let factory =
            config_server_http_client_factory(service_config, &auth_config.auth_route_config);
        return Ok(crate::config_server_bundle_loader(
            service_config.clone(),
            auth_config.codex_home,
            factory,
        ));
    }
    let service =
        cloud_config_bundle_service_for_storage(auth_config, enable_codex_api_key_env).await?;
    Ok(cloud_config_bundle_loader_for_service(service).0)
}

struct CloudConfigBundleLoaderLifetime<C: BundleSource> {
    service: Arc<CloudConfigBundleService<C>>,
    refresh_task: AbortHandle,
}

impl<C: BundleSource> Drop for CloudConfigBundleLoaderLifetime<C> {
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
    let (loader, _) = cloud_config_bundle_loader_for_service(service);
    loader
}

pub(crate) fn cloud_config_bundle_loader_for_service<C>(
    service: CloudConfigBundleService<C>,
) -> (CloudConfigBundleLoader, JoinHandle<()>)
where
    C: BundleSource + 'static,
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
        refresh_task: abort_handle,
    });

    let snapshot_lifetime = Arc::clone(&lifetime);
    let policy = lifetime.service.policy.clone();
    let loader = CloudConfigBundleLoader::from_getter(move || {
        let lifetime = Arc::clone(&lifetime);
        async move { lifetime.service.get_latest().await }
    })
    .with_ema_policy_snapshots(policy, move || {
        let lifetime = Arc::clone(&snapshot_lifetime);
        async move { lifetime.service.get_latest_snapshot().await }
    });
    (loader, refresh_task)
}

pub async fn cloud_config_bundle_loader_for_storage(
    auth_config: AuthConfig,
    enable_codex_api_key_env: bool,
) -> std::io::Result<CloudConfigBundleLoader> {
    let service =
        cloud_config_bundle_service_for_storage(auth_config, enable_codex_api_key_env).await?;
    let (loader, _) = cloud_config_bundle_loader_for_service(service);
    Ok(loader)
}

/// Fetches directly from the network on each load, without reading or writing
/// the disk cache or starting a background refresher.
pub async fn cloud_config_bundle_loader_for_storage_without_cache(
    auth_config: AuthConfig,
    enable_codex_api_key_env: bool,
) -> std::io::Result<CloudConfigBundleLoader> {
    let service = Arc::new(
        cloud_config_bundle_service_for_storage(auth_config, enable_codex_api_key_env)
            .await?
            .without_cache(),
    );
    Ok(CloudConfigBundleLoader::from_getter(move || {
        let service = Arc::clone(&service);
        async move { service.load_startup_bundle_with_timeout().await }
    }))
}

async fn cloud_config_bundle_service_for_storage(
    auth_config: AuthConfig,
    enable_codex_api_key_env: bool,
) -> std::io::Result<CloudConfigBundleService<ChatGptSource<BackendBundleClient>>> {
    let auth_manager =
        AuthManager::shared_from_auth_config(auth_config.clone(), enable_codex_api_key_env).await?;
    Ok(CloudConfigBundleService::new(
        auth_manager,
        Arc::new(BackendBundleClient::new(
            auth_config
                .chatgpt_base_url
                .unwrap_or_else(|| "https://chatgpt.com/backend-api/".to_string()),
            auth_config.auth_route_config.http_client_factory().clone(),
        )),
        auth_config.codex_home,
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    ))
}
