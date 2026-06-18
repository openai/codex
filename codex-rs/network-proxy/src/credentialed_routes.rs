use crate::CredentialedRouteProxyActionConfig;
use crate::CredentialedRouteProxyHeader;
use crate::MitmHookActionsConfig;
use crate::MitmHookConfig;
use crate::MitmHookMatchConfig;
use crate::NetworkMode;
use crate::NetworkProxyConfig;
use crate::NetworkProxyConstraints;
use crate::build_config_state;
use crate::normalize_host;
use crate::runtime::ConfigReloader;
use crate::runtime::ConfigReloaderFuture;
use crate::runtime::ConfigState;
use crate::validate_policy_against_constraints;
use anyhow::Result;
use std::cmp::Reverse;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tracing::warn;
use url::Url;

const CREDENTIALED_ROUTE_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialedRoute {
    pub connector_id: String,
    pub link_id: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CredentialedRoutesConfig {
    pub routes: Vec<CredentialedRoute>,
    pub proxy_headers: Vec<CredentialedRouteProxyHeader>,
    pub proxy_url: Option<String>,
}

impl CredentialedRoutesConfig {
    pub fn route_prefixes(&self) -> Vec<String> {
        self.routes
            .iter()
            .filter_map(|route| parse_route_base_url(route).ok())
            .map(|url| url.to_string())
            .collect()
    }

    fn apply_to_network_proxy_config(&self, config: &mut NetworkProxyConfig) {
        let credentialed_route_hooks = self.mitm_hooks();
        let mut allowed_domains = config.network.allowed_domains().unwrap_or_default();
        for hook in &credentialed_route_hooks {
            if !allowed_domains
                .iter()
                .any(|allowed_domain| normalize_host(allowed_domain) == normalize_host(&hook.host))
            {
                allowed_domains.push(hook.host.clone());
            }
        }
        config.network.set_allowed_domains(allowed_domains);
        let mut mitm_hooks = credentialed_route_hooks;
        mitm_hooks.extend(config.network.mitm_hooks.clone());
        config.network.mitm_hooks = mitm_hooks;
        config.network.mitm =
            config.network.mode == NetworkMode::Limited || !config.network.mitm_hooks.is_empty();
    }

    fn mitm_hooks(&self) -> Vec<MitmHookConfig> {
        let Some(proxy_url) = self.proxy_url.as_ref() else {
            return Vec::new();
        };

        let mut routes = self.routes.iter().collect::<Vec<_>>();
        routes.sort_by_key(|route| Reverse(route.base_url.trim_end_matches('/').len()));
        routes
            .into_iter()
            .filter_map(
                |route| match route_mitm_hook(route, &self.proxy_headers, proxy_url) {
                    Ok(hook) => Some(hook),
                    Err(err) => {
                        warn!(
                            connector_id = %route.connector_id,
                            link_id = %route.link_id,
                            base_url = %route.base_url,
                            error = %err,
                            "skipping invalid credentialed route"
                        );
                        None
                    }
                },
            )
            .collect()
    }
}

/// Loads the latest credentialed routes for the managed network proxy.
pub trait CredentialedRoutesSource: Send + Sync {
    fn load(&self) -> Pin<Box<dyn Future<Output = Result<CredentialedRoutesConfig>> + Send + '_>>;
}

impl<F, Fut> CredentialedRoutesSource for F
where
    F: Fn() -> Fut + Send + Sync,
    Fut: Future<Output = Result<CredentialedRoutesConfig>> + Send + 'static,
{
    fn load(&self) -> Pin<Box<dyn Future<Output = Result<CredentialedRoutesConfig>> + Send + '_>> {
        Box::pin((self)())
    }
}

pub struct CredentialedRoutesReloader {
    base_state: Arc<RwLock<ConfigState>>,
    credentialed_routes: Arc<RwLock<CredentialedRoutesConfig>>,
    next_refresh_at: Mutex<Instant>,
    source: Arc<dyn CredentialedRoutesSource>,
}

impl CredentialedRoutesReloader {
    pub fn new(
        base_state: ConfigState,
        credentialed_routes: CredentialedRoutesConfig,
        source: Arc<dyn CredentialedRoutesSource>,
    ) -> Self {
        Self {
            base_state: Arc::new(RwLock::new(base_state)),
            credentialed_routes: Arc::new(RwLock::new(credentialed_routes)),
            next_refresh_at: Mutex::new(Instant::now() + CREDENTIALED_ROUTE_REFRESH_INTERVAL),
            source,
        }
    }

    pub async fn current_routes(&self) -> CredentialedRoutesConfig {
        self.credentialed_routes.read().await.clone()
    }

    async fn build_state(
        &self,
        credentialed_routes: &CredentialedRoutesConfig,
    ) -> Result<ConfigState> {
        let base_state = self.base_state.read().await.clone();
        build_config_state_with_credentialed_routes(
            base_state.config,
            base_state.constraints,
            credentialed_routes,
        )
    }
}

impl ConfigReloader for CredentialedRoutesReloader {
    fn source_label(&self) -> String {
        "CredentialedRoutesReloader".to_string()
    }

    fn maybe_reload(&self) -> ConfigReloaderFuture<'_, Option<ConfigState>> {
        Box::pin(async move {
            {
                let mut next_refresh_at = self.next_refresh_at.lock().await;
                let now = Instant::now();
                if now < *next_refresh_at {
                    return Ok(None);
                }
                *next_refresh_at = now + CREDENTIALED_ROUTE_REFRESH_INTERVAL;
            }
            let credentialed_routes = match self.source.load().await {
                Ok(credentialed_routes) => credentialed_routes,
                Err(err) => {
                    warn!(error = %err, "failed to refresh credentialed routes");
                    return Ok(None);
                }
            };
            let previous_routes = self.credentialed_routes.read().await.clone();
            if credentialed_routes == previous_routes {
                return Ok(None);
            }

            let state = self.build_state(&credentialed_routes).await?;
            *self.credentialed_routes.write().await = credentialed_routes;
            Ok(Some(state))
        })
    }

    fn reload_now(&self) -> ConfigReloaderFuture<'_, ConfigState> {
        Box::pin(async move {
            let credentialed_routes = self.credentialed_routes.read().await.clone();
            self.build_state(&credentialed_routes).await
        })
    }

    fn replace_base_state(&self, base_state: ConfigState) -> ConfigReloaderFuture<'_, ConfigState> {
        Box::pin(async move {
            let credentialed_routes = self.credentialed_routes.read().await.clone();
            let state = build_config_state_with_credentialed_routes(
                base_state.config.clone(),
                base_state.constraints.clone(),
                &credentialed_routes,
            )?;
            *self.base_state.write().await = base_state;
            Ok(state)
        })
    }
}

fn build_config_state_with_credentialed_routes(
    mut config: NetworkProxyConfig,
    constraints: NetworkProxyConstraints,
    credentialed_routes: &CredentialedRoutesConfig,
) -> Result<ConfigState> {
    credentialed_routes.apply_to_network_proxy_config(&mut config);
    validate_policy_against_constraints(&config, &constraints)?;
    build_config_state(config, constraints)
}

fn route_mitm_hook(
    route: &CredentialedRoute,
    proxy_headers: &[CredentialedRouteProxyHeader],
    proxy_url: &str,
) -> Result<MitmHookConfig> {
    let base_url = parse_route_base_url(route)?;
    let host = base_url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("credentialed route must include a host"))?
        .to_string();
    let path = base_url.path().trim_end_matches('/');
    let path_prefixes = if path.is_empty() {
        vec!["/".to_string()]
    } else {
        let escaped_path = globset::escape(path);
        vec![
            format!("pattern:{escaped_path}"),
            format!("pattern:{escaped_path}/**"),
        ]
    };

    Ok(MitmHookConfig {
        host,
        matcher: MitmHookMatchConfig {
            methods: vec![
                "DELETE".to_string(),
                "GET".to_string(),
                "HEAD".to_string(),
                "PATCH".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
            ],
            path_prefixes,
            ..MitmHookMatchConfig::default()
        },
        actions: MitmHookActionsConfig {
            credentialed_route_proxy: Some(CredentialedRouteProxyActionConfig {
                connector_id: route.connector_id.clone(),
                link_id: route.link_id.clone(),
                proxy_headers: proxy_headers.to_vec(),
                proxy_url: proxy_url.to_string(),
            }),
            ..MitmHookActionsConfig::default()
        },
    })
}

fn parse_route_base_url(route: &CredentialedRoute) -> Result<Url> {
    let base_url = Url::parse(&route.base_url)?;
    anyhow::ensure!(
        base_url.scheme() == "https",
        "credentialed route must use https"
    );
    anyhow::ensure!(
        base_url.host_str().is_some(),
        "credentialed route must include a host"
    );
    anyhow::ensure!(
        base_url.username().is_empty() && base_url.password().is_none(),
        "credentialed route must not include user info"
    );
    anyhow::ensure!(
        base_url.fragment().is_none() && base_url.query().is_none(),
        "credentialed route must not include query or fragment"
    );
    Ok(base_url)
}

#[cfg(test)]
#[path = "credentialed_routes_tests.rs"]
mod tests;
