use codex_backend_client::Client as BackendClient;
use codex_backend_client::ResolvedCredentialRoute;
use codex_login::CodexAuth;
use codex_network_proxy::CredentialedRouteProxyActionConfig;
use codex_network_proxy::MitmHookActionsConfig;
use codex_network_proxy::MitmHookConfig;
use codex_network_proxy::MitmHookMatchConfig;
use tracing::debug;
use tracing::warn;
use url::Url;

#[derive(Debug, Clone, Default)]
pub(crate) struct CredentialedRoutesSessionConfig {
    pub(crate) routes: Vec<ResolvedCredentialRoute>,
    pub(crate) proxy_url: Option<String>,
}

pub(crate) async fn load_for_session(
    chatgpt_base_url: &str,
    auth: Option<&CodexAuth>,
) -> CredentialedRoutesSessionConfig {
    let Some(auth) = auth.filter(|auth| auth.uses_codex_backend()) else {
        return CredentialedRoutesSessionConfig::default();
    };

    let client = match BackendClient::from_auth(chatgpt_base_url.to_string(), auth) {
        Ok(client) => client,
        Err(err) => {
            warn!(error = %err, "failed to initialize credentialed routes client");
            return CredentialedRoutesSessionConfig::default();
        }
    };

    match client.list_credential_routes().await {
        Ok(response) => {
            debug!(
                credentialed_routes = response.routes.len(),
                "loaded credentialed routes for session"
            );
            CredentialedRoutesSessionConfig {
                routes: response.routes,
                proxy_url: Some(client.credential_routes_proxy_url()),
            }
        }
        Err(err) => {
            warn!(error = %err, "failed to load credentialed routes for session");
            CredentialedRoutesSessionConfig::default()
        }
    }
}

impl CredentialedRoutesSessionConfig {
    pub(crate) fn mitm_hooks(&self) -> Vec<MitmHookConfig> {
        let Some(proxy_url) = self.proxy_url.as_ref() else {
            return Vec::new();
        };

        self.routes
            .iter()
            .filter_map(|route| match route_mitm_hook(route, proxy_url) {
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
            })
            .collect()
    }
}

fn route_mitm_hook(
    route: &ResolvedCredentialRoute,
    proxy_url: &str,
) -> anyhow::Result<MitmHookConfig> {
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
                proxy_url: proxy_url.to_string(),
            }),
            ..MitmHookActionsConfig::default()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_backend_client::CredentialRouteAuthType;
    use pretty_assertions::assert_eq;

    #[test]
    fn credentialed_routes_compile_to_internal_mitm_hooks() {
        let config = CredentialedRoutesSessionConfig {
            routes: vec![ResolvedCredentialRoute {
                connector_id: "connector_123".to_string(),
                link_id: "link_123".to_string(),
                auth_type: CredentialRouteAuthType::OAuth,
                base_url: "https://api.example.com/v1".to_string(),
            }],
            proxy_url: Some(
                "https://chatgpt.com/backend-api/wham/credential_routes/proxy".to_string(),
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
                proxy_url: "https://chatgpt.com/backend-api/wham/credential_routes/proxy"
                    .to_string(),
            })
        );
    }
}
