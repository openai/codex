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
use crate::runtime::ConfigState;
use anyhow::Result;
use async_trait::async_trait;
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

        self.routes
            .iter()
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

    pub async fn replace_base_state(&self, base_state: ConfigState) -> Result<ConfigState> {
        *self.base_state.write().await = base_state;
        self.reload_now().await
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

#[async_trait]
impl ConfigReloader for CredentialedRoutesReloader {
    fn source_label(&self) -> String {
        "CredentialedRoutesReloader".to_string()
    }

    async fn maybe_reload(&self) -> Result<Option<ConfigState>> {
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

        *self.credentialed_routes.write().await = credentialed_routes.clone();
        Ok(Some(self.build_state(&credentialed_routes).await?))
    }

    async fn reload_now(&self) -> Result<ConfigState> {
        let credentialed_routes = self.credentialed_routes.read().await.clone();
        self.build_state(&credentialed_routes).await
    }
}

fn build_config_state_with_credentialed_routes(
    mut config: NetworkProxyConfig,
    constraints: NetworkProxyConstraints,
    credentialed_routes: &CredentialedRoutesConfig,
) -> Result<ConfigState> {
    credentialed_routes.apply_to_network_proxy_config(&mut config);
    build_config_state(config, constraints)
}

fn route_mitm_hook(
    route: &CredentialedRoute,
    proxy_headers: &[CredentialedRouteProxyHeader],
    proxy_url: &str,
) -> Result<MitmHookConfig> {
    let base_url = Url::parse(&route.base_url)?;
    anyhow::ensure!(
        base_url.scheme() == "https",
        "credentialed route must use https"
    );
    let host = base_url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("credentialed route must include a host"))?;
    anyhow::ensure!(
        base_url.username().is_empty() && base_url.password().is_none(),
        "credentialed route must not include user info"
    );
    anyhow::ensure!(
        base_url.fragment().is_none() && base_url.query().is_none(),
        "credentialed route must not include query or fragment"
    );
    let path_prefix = match base_url.path() {
        "" => "/",
        path => path,
    };

    Ok(MitmHookConfig {
        host: host.to_string(),
        matcher: MitmHookMatchConfig {
            methods: vec![
                "DELETE".to_string(),
                "GET".to_string(),
                "HEAD".to_string(),
                "PATCH".to_string(),
                "POST".to_string(),
                "PUT".to_string(),
            ],
            path_prefixes: vec![path_prefix.to_string()],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CredentialedRouteProxyHeader;
    use crate::NetworkDomainPermission;
    use crate::NetworkProxyConfig;
    use crate::NetworkProxyConstraints;
    use crate::build_config_state;
    use pretty_assertions::assert_eq;
    use rama_http::HeaderValue;
    use rama_http::header::AUTHORIZATION;

    #[test]
    fn credentialed_routes_compile_to_internal_mitm_hooks() {
        let config = CredentialedRoutesConfig {
            routes: vec![CredentialedRoute {
                connector_id: "connector_123".to_string(),
                link_id: "link_123".to_string(),
                base_url: "https://api.example.com/v1".to_string(),
            }],
            proxy_headers: vec![CredentialedRouteProxyHeader {
                name: AUTHORIZATION,
                value: HeaderValue::from_static("Bearer codex-token"),
            }],
            proxy_url: Some(
                "https://chatgpt.com/backend-api/ps/credential_routes/proxy".to_string(),
            ),
        };

        let hooks = config.mitm_hooks();

        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].host, "api.example.com");
        assert_eq!(hooks[0].matcher.path_prefixes, vec!["/v1".to_string()]);
        assert_eq!(
            hooks[0].actions.credentialed_route_proxy,
            Some(CredentialedRouteProxyActionConfig {
                connector_id: "connector_123".to_string(),
                link_id: "link_123".to_string(),
                proxy_headers: vec![CredentialedRouteProxyHeader {
                    name: AUTHORIZATION,
                    value: HeaderValue::from_static("Bearer codex-token"),
                }],
                proxy_url: "https://chatgpt.com/backend-api/ps/credential_routes/proxy".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn credentialed_routes_reloader_rebuilds_generated_hooks() {
        let mut base_config = NetworkProxyConfig::default();
        base_config.network.enabled = true;
        base_config.network.upsert_domain_permission(
            "existing.example.com".to_string(),
            NetworkDomainPermission::Allow,
            normalize_host,
        );
        let base_state =
            build_config_state(base_config, NetworkProxyConstraints::default()).unwrap();
        let updated_routes = CredentialedRoutesConfig {
            routes: vec![CredentialedRoute {
                connector_id: "connector_123".to_string(),
                link_id: "link_123".to_string(),
                base_url: "https://api.example.com/v1".to_string(),
            }],
            proxy_headers: Vec::new(),
            proxy_url: Some(
                "https://chatgpt.com/backend-api/ps/credential_routes/proxy".to_string(),
            ),
        };
        let reloader = CredentialedRoutesReloader::new(
            base_state,
            CredentialedRoutesConfig::default(),
            Arc::new({
                let updated_routes = updated_routes.clone();
                move || {
                    let updated_routes = updated_routes.clone();
                    async move { Ok(updated_routes) }
                }
            }),
        );
        *reloader.next_refresh_at.lock().await = Instant::now();

        let state = reloader.maybe_reload().await.unwrap().unwrap();

        assert_eq!(
            state.config.network.allowed_domains().unwrap(),
            vec![
                "existing.example.com".to_string(),
                "api.example.com".to_string()
            ]
        );
        assert_eq!(state.config.network.mitm_hooks.len(), 1);
    }
}
