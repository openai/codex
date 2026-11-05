use crate::backend::BackendBundleClient;
use crate::chatgpt::ChatGptSource;
use crate::chatgpt::plan_requires_managed_config;
use crate::config_server::config_server_http_client_factory;
use crate::config_source::BundleSource;
use crate::service::CLOUD_CONFIG_BUNDLE_TIMEOUT;
use crate::service::CloudConfigBundleService;
use codex_config::CloudConfigBundleLoader;
use codex_config::ConfigRequirementsToml;
use codex_http_client::HttpClientFactory;
use codex_login::AuthConfig;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_protocol::account::PlanType;
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

/// Existing grants are left to the loader's normal cache and refresh handling.
pub async fn ensure_config_server_login(
    auth: AuthConfig,
    requirements: &ConfigRequirementsToml,
) -> std::io::Result<()> {
    let Some(manager) = managed_config_auth_manager(auth, requirements)? else {
        return Ok(());
    };
    if let Err(error) = manager.authorization_id().await {
        if error
            .get_ref()
            .and_then(|source| source.downcast_ref::<codex_login::OAuthSessionError>())
            != Some(&codex_login::OAuthSessionError::LoginRequired)
        {
            return Err(error);
        }
        manager.login().await?;
    }
    Ok(())
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
    chatgpt_config_bundle_loader(
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
        chatgpt_config_bundle_service_for_storage(auth_config, enable_codex_api_key_env).await?;
    Ok(managed_config_bundle_loader_for_service(service).0)
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

pub fn chatgpt_config_bundle_loader(
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
    let (loader, _) = managed_config_bundle_loader_for_service(service);
    loader
}

pub(crate) fn managed_config_bundle_loader_for_service<C>(
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

pub async fn chatgpt_config_bundle_loader_for_storage(
    auth_config: AuthConfig,
    enable_codex_api_key_env: bool,
) -> std::io::Result<CloudConfigBundleLoader> {
    let service =
        chatgpt_config_bundle_service_for_storage(auth_config, enable_codex_api_key_env).await?;
    let (loader, _) = managed_config_bundle_loader_for_service(service);
    Ok(loader)
}

/// Use the configured policy server or the supplied account's ChatGPT policy when eligible.
pub async fn managed_config_bundle_loader_for_auth(
    config: AuthConfig,
    auth: &CodexAuth,
    requirements: &ConfigRequirementsToml,
) -> std::io::Result<CloudConfigBundleLoader> {
    if requirements.config_server.is_some() {
        return managed_config_bundle_loader_for_storage(
            config,
            requirements,
            /*enable_codex_api_key_env*/ false,
        )
        .await;
    }
    // Only skip the policy lookup when the supplied account's plan is known.
    if auth
        .account_plan_type()
        .is_some_and(|plan| plan != PlanType::Unknown && !plan_requires_managed_config(plan))
    {
        return Ok(CloudConfigBundleLoader::default());
    }

    let base_url = config
        .chatgpt_base_url
        .as_deref()
        .unwrap_or("https://chatgpt.com/backend-api/");
    codex_http_client::validate_https_endpoint_url(base_url, "ChatGPT policy endpoint")
        .map_err(std::io::Error::other)?;
    let mut client = codex_backend_client::Client::new_without_redirects(
        base_url,
        config.auth_route_config.http_client_factory().clone(),
    )
    .with_user_agent(codex_login::default_client::get_codex_user_agent())
    .with_bearer_token(auth.get_token()?)
    .map_err(std::io::Error::other)?
    .with_chatgpt_account_id(
        auth.get_account_id()
            .ok_or_else(|| std::io::Error::other("missing ChatGPT account ID"))?,
    );
    if auth.is_fedramp_account() {
        client = client.with_fedramp_routing_header();
    }
    let response = tokio::time::timeout(CLOUD_CONFIG_BUNDLE_TIMEOUT, client.get_config_bundle())
        .await
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "timed out loading ChatGPT account policy",
            )
        })?
        .map_err(|_| std::io::Error::other("could not load ChatGPT account policy"))?;
    let bundle = crate::backend::bundle_from_response(response);
    Ok(CloudConfigBundleLoader::new(
        async move { Ok(Some(bundle)) },
    ))
}

/// Fetches directly from the network on each load, without reading or writing
/// the disk cache or starting a background refresher.
pub async fn chatgpt_config_bundle_loader_for_storage_without_cache(
    auth_config: AuthConfig,
    enable_codex_api_key_env: bool,
) -> std::io::Result<CloudConfigBundleLoader> {
    let service = Arc::new(
        chatgpt_config_bundle_service_for_storage(auth_config, enable_codex_api_key_env)
            .await?
            .without_cache(),
    );
    Ok(CloudConfigBundleLoader::from_getter(move || {
        let service = Arc::clone(&service);
        async move { service.load_startup_bundle_with_timeout().await }
    }))
}

async fn chatgpt_config_bundle_service_for_storage(
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
