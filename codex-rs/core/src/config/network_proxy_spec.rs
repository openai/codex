use crate::config_loader::NetworkConstraints;
use async_trait::async_trait;
use codex_network_proxy::BlockedRequestObserver;
use codex_network_proxy::ConfigReloader;
use codex_network_proxy::ConfigState;
use codex_network_proxy::NetworkDecision;
use codex_network_proxy::NetworkPolicyDecider;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::NetworkProxyConfig;
use codex_network_proxy::NetworkProxyConstraints;
use codex_network_proxy::NetworkProxyHandle;
use codex_network_proxy::NetworkProxyState;
use codex_network_proxy::build_config_state;
use codex_network_proxy::host_and_port_from_network_addr;
use codex_network_proxy::validate_policy_against_constraints;
use codex_protocol::protocol::SandboxPolicy;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkProxySpec {
    config: NetworkProxyConfig,
    constraints: NetworkProxyConstraints,
}

pub struct StartedNetworkProxy {
    proxy: NetworkProxy,
    _handle: NetworkProxyHandle,
}

impl StartedNetworkProxy {
    fn new(proxy: NetworkProxy, handle: NetworkProxyHandle) -> Self {
        Self {
            proxy,
            _handle: handle,
        }
    }

    pub fn proxy(&self) -> NetworkProxy {
        self.proxy.clone()
    }
}

#[derive(Clone)]
struct StaticNetworkProxyReloader {
    state: ConfigState,
}

impl StaticNetworkProxyReloader {
    fn new(state: ConfigState) -> Self {
        Self { state }
    }
}

#[async_trait]
impl ConfigReloader for StaticNetworkProxyReloader {
    async fn maybe_reload(&self) -> anyhow::Result<Option<ConfigState>> {
        Ok(None)
    }

    async fn reload_now(&self) -> anyhow::Result<ConfigState> {
        Ok(self.state.clone())
    }

    fn source_label(&self) -> String {
        "StaticNetworkProxyReloader".to_string()
    }
}

impl NetworkProxySpec {
    pub(crate) fn enabled(&self) -> bool {
        self.config.network.enabled
    }

    pub fn proxy_host_and_port(&self) -> String {
        host_and_port_from_network_addr(&self.config.network.proxy_url, 3128)
    }

    pub fn socks_enabled(&self) -> bool {
        self.config.network.enable_socks5
    }

    pub(crate) fn from_config_and_constraints(
        config: NetworkProxyConfig,
        requirements: Option<NetworkConstraints>,
    ) -> std::io::Result<Self> {
        let (config, constraints) = if let Some(requirements) = requirements {
            Self::apply_requirements(config, &requirements)
        } else {
            (config, NetworkProxyConstraints::default())
        };
        validate_policy_against_constraints(&config, &constraints).map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("network proxy constraints are invalid: {err}"),
            )
        })?;
        Ok(Self {
            config,
            constraints,
        })
    }

    pub async fn start_proxy(
        &self,
        sandbox_policy: &SandboxPolicy,
        policy_decider: Option<Arc<dyn NetworkPolicyDecider>>,
        blocked_request_observer: Option<Arc<dyn BlockedRequestObserver>>,
        enable_network_approval_flow: bool,
    ) -> std::io::Result<StartedNetworkProxy> {
        let state =
            build_config_state(self.config.clone(), self.constraints.clone()).map_err(|err| {
                std::io::Error::other(format!("failed to build network proxy state: {err}"))
            })?;
        let reloader = Arc::new(StaticNetworkProxyReloader::new(state.clone()));
        let state = NetworkProxyState::with_reloader(state, reloader);
        let mut builder = NetworkProxy::builder().state(Arc::new(state));
        if enable_network_approval_flow
            && matches!(
                sandbox_policy,
                SandboxPolicy::ReadOnly { .. } | SandboxPolicy::WorkspaceWrite { .. }
            )
        {
            builder = match policy_decider {
                Some(policy_decider) => builder.policy_decider_arc(policy_decider),
                None => builder.policy_decider(|_request| async {
                    // In restricted sandbox modes, allowlist misses should ask for
                    // explicit network approval instead of hard-denying.
                    NetworkDecision::ask("not_allowed")
                }),
            };
        }
        if let Some(blocked_request_observer) = blocked_request_observer {
            builder = builder.blocked_request_observer_arc(blocked_request_observer);
        }
        let proxy = builder.build().await.map_err(|err| {
            std::io::Error::other(format!("failed to build network proxy: {err}"))
        })?;
        let handle = proxy
            .run()
            .await
            .map_err(|err| std::io::Error::other(format!("failed to run network proxy: {err}")))?;
        Ok(StartedNetworkProxy::new(proxy, handle))
    }

    fn apply_requirements(
        mut config: NetworkProxyConfig,
        requirements: &NetworkConstraints,
    ) -> (NetworkProxyConfig, NetworkProxyConstraints) {
        let mut constraints = NetworkProxyConstraints::default();

        if let Some(enabled) = requirements.enabled {
            if !enabled {
                config.network.enabled = false;
            }
            constraints.enabled = Some(enabled);
        }
        if let Some(http_port) = requirements.http_port {
            config.network.proxy_url = format!("http://127.0.0.1:{http_port}");
        }
        if let Some(socks_port) = requirements.socks_port {
            config.network.socks_url = format!("http://127.0.0.1:{socks_port}");
        }
        if let Some(allow_upstream_proxy) = requirements.allow_upstream_proxy {
            if !allow_upstream_proxy {
                config.network.allow_upstream_proxy = false;
            }
            constraints.allow_upstream_proxy = Some(allow_upstream_proxy);
        }
        if let Some(dangerously_allow_non_loopback_proxy) =
            requirements.dangerously_allow_non_loopback_proxy
        {
            if !dangerously_allow_non_loopback_proxy {
                config.network.dangerously_allow_non_loopback_proxy = false;
            }
            constraints.dangerously_allow_non_loopback_proxy =
                Some(dangerously_allow_non_loopback_proxy);
        }
        if let Some(dangerously_allow_non_loopback_admin) =
            requirements.dangerously_allow_non_loopback_admin
        {
            if !dangerously_allow_non_loopback_admin {
                config.network.dangerously_allow_non_loopback_admin = false;
            }
            constraints.dangerously_allow_non_loopback_admin =
                Some(dangerously_allow_non_loopback_admin);
        }
        if let Some(allowed_domains) = requirements.allowed_domains.clone() {
            // Keep the user-configured allowlist if it is narrower. Managed
            // requirements act as a subset constraint and are enforced by
            // `validate_policy_against_constraints`.
            constraints.allowed_domains = Some(allowed_domains);
        }
        if let Some(denied_domains) = requirements.denied_domains.clone() {
            // Managed denied domains are a floor. Preserve user-added entries,
            // but ensure all managed entries are present.
            for denied_domain in &denied_domains {
                if !config
                    .network
                    .denied_domains
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(denied_domain))
                {
                    config.network.denied_domains.push(denied_domain.clone());
                }
            }
            constraints.denied_domains = Some(denied_domains);
        }
        if let Some(allow_unix_sockets) = requirements.allow_unix_sockets.clone() {
            // Keep the user-configured socket allowlist if it is narrower.
            // Managed requirements are enforced as a subset constraint during
            // validation.
            constraints.allow_unix_sockets = Some(allow_unix_sockets);
        }
        if let Some(allow_local_binding) = requirements.allow_local_binding {
            if !allow_local_binding {
                config.network.allow_local_binding = false;
            }
            constraints.allow_local_binding = Some(allow_local_binding);
        }

        (config, constraints)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use codex_network_proxy::NetworkMode;
    use pretty_assertions::assert_eq;

    #[test]
    fn managed_requirements_preserve_more_restrictive_user_network_settings() -> std::io::Result<()>
    {
        let mut user_config = NetworkProxyConfig::default();
        user_config.network.enabled = false;
        user_config.network.mode = NetworkMode::Limited;
        user_config.network.allow_upstream_proxy = false;
        user_config.network.dangerously_allow_non_loopback_proxy = false;
        user_config.network.dangerously_allow_non_loopback_admin = false;
        user_config.network.allowed_domains = vec!["api.openai.com".to_string()];
        user_config.network.denied_domains =
            vec!["tracker.com".to_string(), "evil.com".to_string()];
        user_config.network.allow_unix_sockets = vec!["/tmp/a.sock".to_string()];
        user_config.network.allow_local_binding = false;

        let requirements = NetworkConstraints {
            enabled: Some(true),
            http_port: Some(43128),
            socks_port: Some(43129),
            allow_upstream_proxy: Some(true),
            dangerously_allow_non_loopback_proxy: Some(true),
            dangerously_allow_non_loopback_admin: Some(true),
            allowed_domains: Some(vec!["*.openai.com".to_string()]),
            denied_domains: Some(vec!["evil.com".to_string()]),
            allow_unix_sockets: Some(vec!["/tmp/a.sock".to_string(), "/tmp/b.sock".to_string()]),
            allow_local_binding: Some(true),
        };

        let spec = NetworkProxySpec::from_config_and_constraints(user_config, Some(requirements))?;

        let mut expected_config = NetworkProxyConfig::default();
        expected_config.network.enabled = false;
        expected_config.network.proxy_url = "http://127.0.0.1:43128".to_string();
        expected_config.network.mode = NetworkMode::Limited;
        expected_config.network.allow_upstream_proxy = false;
        expected_config.network.dangerously_allow_non_loopback_proxy = false;
        expected_config.network.dangerously_allow_non_loopback_admin = false;
        expected_config.network.enable_socks5 = true;
        expected_config.network.socks_url = "http://127.0.0.1:43129".to_string();
        expected_config.network.enable_socks5_udp = true;
        expected_config.network.allowed_domains = vec!["api.openai.com".to_string()];
        expected_config.network.denied_domains =
            vec!["tracker.com".to_string(), "evil.com".to_string()];
        expected_config.network.allow_unix_sockets = vec!["/tmp/a.sock".to_string()];
        expected_config.network.allow_local_binding = false;

        assert_eq!(spec.config, expected_config);
        assert_eq!(
            spec.constraints,
            NetworkProxyConstraints {
                enabled: Some(true),
                mode: None,
                allow_upstream_proxy: Some(true),
                dangerously_allow_non_loopback_proxy: Some(true),
                dangerously_allow_non_loopback_admin: Some(true),
                allowed_domains: Some(vec!["*.openai.com".to_string()]),
                denied_domains: Some(vec!["evil.com".to_string()]),
                allow_unix_sockets: Some(vec![
                    "/tmp/a.sock".to_string(),
                    "/tmp/b.sock".to_string()
                ]),
                allow_local_binding: Some(true),
            }
        );

        Ok(())
    }

    #[test]
    fn managed_requirements_clamp_and_union_user_network_settings() -> std::io::Result<()> {
        let mut user_config = NetworkProxyConfig::default();
        user_config.network.enabled = true;
        user_config.network.mode = NetworkMode::Full;
        user_config.network.allow_upstream_proxy = true;
        user_config.network.dangerously_allow_non_loopback_proxy = true;
        user_config.network.dangerously_allow_non_loopback_admin = true;
        user_config.network.denied_domains = vec!["tracker.com".to_string()];
        user_config.network.allow_local_binding = true;

        let requirements = NetworkConstraints {
            enabled: Some(false),
            http_port: None,
            socks_port: None,
            allow_upstream_proxy: Some(false),
            dangerously_allow_non_loopback_proxy: Some(false),
            dangerously_allow_non_loopback_admin: Some(false),
            allowed_domains: None,
            denied_domains: Some(vec!["evil.com".to_string()]),
            allow_unix_sockets: None,
            allow_local_binding: Some(false),
        };

        let spec = NetworkProxySpec::from_config_and_constraints(user_config, Some(requirements))?;

        let mut expected_config = NetworkProxyConfig::default();
        expected_config.network.enabled = false;
        expected_config.network.mode = NetworkMode::Full;
        expected_config.network.allow_upstream_proxy = false;
        expected_config.network.dangerously_allow_non_loopback_proxy = false;
        expected_config.network.dangerously_allow_non_loopback_admin = false;
        expected_config.network.denied_domains =
            vec!["tracker.com".to_string(), "evil.com".to_string()];
        expected_config.network.allow_local_binding = false;

        assert_eq!(spec.config, expected_config);
        assert_eq!(
            spec.constraints,
            NetworkProxyConstraints {
                enabled: Some(false),
                mode: None,
                allow_upstream_proxy: Some(false),
                dangerously_allow_non_loopback_proxy: Some(false),
                dangerously_allow_non_loopback_admin: Some(false),
                allowed_domains: None,
                denied_domains: Some(vec!["evil.com".to_string()]),
                allow_unix_sockets: None,
                allow_local_binding: Some(false),
            }
        );

        Ok(())
    }
}
