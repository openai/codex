use crate::config_loader::NetworkConstraints;
use async_trait::async_trait;
use codex_network_proxy::BlockedRequestObserver;
use codex_network_proxy::ConfigReloader;
use codex_network_proxy::ConfigState;
use codex_network_proxy::NetworkDecision;
use codex_network_proxy::NetworkPolicyDecider;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::NetworkProxyAuditMetadata;
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
        user_network: Option<&crate::config::permissions::NetworkToml>,
        requirements: Option<NetworkConstraints>,
    ) -> std::io::Result<Self> {
        let (config, constraints) = if let Some(requirements) = requirements {
            Self::apply_requirements(config, user_network, &requirements)
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
        audit_metadata: NetworkProxyAuditMetadata,
    ) -> std::io::Result<StartedNetworkProxy> {
        let state = self.build_state_with_audit_metadata(audit_metadata)?;
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

    fn build_state_with_audit_metadata(
        &self,
        audit_metadata: NetworkProxyAuditMetadata,
    ) -> std::io::Result<NetworkProxyState> {
        let state =
            build_config_state(self.config.clone(), self.constraints.clone()).map_err(|err| {
                std::io::Error::other(format!("failed to build network proxy state: {err}"))
            })?;
        let reloader = Arc::new(StaticNetworkProxyReloader::new(state.clone()));
        Ok(NetworkProxyState::with_reloader_and_audit_metadata(
            state,
            reloader,
            audit_metadata,
        ))
    }

    fn apply_requirements(
        mut config: NetworkProxyConfig,
        user_network: Option<&crate::config::permissions::NetworkToml>,
        requirements: &NetworkConstraints,
    ) -> (NetworkProxyConfig, NetworkProxyConstraints) {
        let mut constraints = NetworkProxyConstraints::default();

        if let Some(enabled) = requirements.enabled {
            if enabled {
                if user_network.and_then(|network| network.enabled).is_none() {
                    config.network.enabled = true;
                }
            } else {
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
        if let Some(dangerously_allow_all_unix_sockets) =
            requirements.dangerously_allow_all_unix_sockets
        {
            config.network.dangerously_allow_all_unix_sockets = dangerously_allow_all_unix_sockets;
            constraints.dangerously_allow_all_unix_sockets =
                Some(dangerously_allow_all_unix_sockets);
        }
        if let Some(allowed_domains) = requirements.allowed_domains.clone() {
            // Seed the managed allowlist when the user left the field
            // unspecified. If the user configured an allowlist, keep that
            // narrower value and enforce the managed list as a subset
            // constraint during validation.
            if user_network
                .and_then(|network| network.allowed_domains.as_ref())
                .is_none()
            {
                config.network.allowed_domains = allowed_domains.clone();
            }
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
    fn build_state_with_audit_metadata_threads_metadata_to_state() {
        let spec = NetworkProxySpec {
            config: NetworkProxyConfig::default(),
            constraints: NetworkProxyConstraints::default(),
        };
        let metadata = NetworkProxyAuditMetadata {
            conversation_id: Some("conversation-1".to_string()),
            app_version: Some("1.2.3".to_string()),
            user_account_id: Some("acct-1".to_string()),
            ..NetworkProxyAuditMetadata::default()
        };

        let state = spec
            .build_state_with_audit_metadata(metadata.clone())
            .expect("state should build");
        assert_eq!(state.audit_metadata(), &metadata);
    }

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
            dangerously_allow_all_unix_sockets: None,
            allowed_domains: Some(vec!["*.openai.com".to_string()]),
            denied_domains: Some(vec!["evil.com".to_string()]),
            allow_unix_sockets: Some(vec!["/tmp/a.sock".to_string(), "/tmp/b.sock".to_string()]),
            allow_local_binding: Some(true),
        };

        let user_network = crate::config::permissions::NetworkToml {
            enabled: Some(false),
            proxy_url: None,
            admin_url: None,
            enable_socks5: None,
            socks_url: None,
            enable_socks5_udp: None,
            allow_upstream_proxy: Some(false),
            dangerously_allow_non_loopback_proxy: Some(false),
            dangerously_allow_non_loopback_admin: Some(false),
            dangerously_allow_all_unix_sockets: None,
            mode: Some(NetworkMode::Limited),
            allowed_domains: Some(vec!["api.openai.com".to_string()]),
            denied_domains: Some(vec!["tracker.com".to_string(), "evil.com".to_string()]),
            allow_unix_sockets: Some(vec!["/tmp/a.sock".to_string()]),
            allow_local_binding: Some(false),
        };

        let spec = NetworkProxySpec::from_config_and_constraints(
            user_config,
            Some(&user_network),
            Some(requirements),
        )?;

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
                dangerously_allow_all_unix_sockets: None,
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
            dangerously_allow_all_unix_sockets: None,
            allowed_domains: None,
            denied_domains: Some(vec!["evil.com".to_string()]),
            allow_unix_sockets: None,
            allow_local_binding: Some(false),
        };

        let user_network = crate::config::permissions::NetworkToml {
            enabled: Some(true),
            proxy_url: None,
            admin_url: None,
            enable_socks5: None,
            socks_url: None,
            enable_socks5_udp: None,
            allow_upstream_proxy: Some(true),
            dangerously_allow_non_loopback_proxy: Some(true),
            dangerously_allow_non_loopback_admin: Some(true),
            dangerously_allow_all_unix_sockets: None,
            mode: Some(NetworkMode::Full),
            allowed_domains: None,
            denied_domains: Some(vec!["tracker.com".to_string()]),
            allow_unix_sockets: None,
            allow_local_binding: Some(true),
        };

        let spec = NetworkProxySpec::from_config_and_constraints(
            user_config,
            Some(&user_network),
            Some(requirements),
        )?;

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
                dangerously_allow_all_unix_sockets: None,
                allowed_domains: None,
                denied_domains: Some(vec!["evil.com".to_string()]),
                allow_unix_sockets: None,
                allow_local_binding: Some(false),
            }
        );

        Ok(())
    }

    #[test]
    fn managed_requirements_enable_proxy_when_user_omits_network_enabled() -> std::io::Result<()> {
        let user_config = NetworkProxyConfig::default();
        let requirements = NetworkConstraints {
            enabled: Some(true),
            http_port: None,
            socks_port: None,
            allow_upstream_proxy: None,
            dangerously_allow_non_loopback_proxy: None,
            dangerously_allow_non_loopback_admin: None,
            dangerously_allow_all_unix_sockets: None,
            allowed_domains: None,
            denied_domains: None,
            allow_unix_sockets: None,
            allow_local_binding: None,
        };

        let spec =
            NetworkProxySpec::from_config_and_constraints(user_config, None, Some(requirements))?;

        let mut expected_config = NetworkProxyConfig::default();
        expected_config.network.enabled = true;

        assert_eq!(spec.config, expected_config);
        assert_eq!(
            spec.constraints,
            NetworkProxyConstraints {
                enabled: Some(true),
                ..NetworkProxyConstraints::default()
            }
        );

        Ok(())
    }

    #[test]
    fn managed_requirements_seed_allowlist_when_user_omits_it() -> std::io::Result<()> {
        let user_config = NetworkProxyConfig::default();
        let requirements = NetworkConstraints {
            enabled: Some(true),
            http_port: None,
            socks_port: None,
            allow_upstream_proxy: None,
            dangerously_allow_non_loopback_proxy: None,
            dangerously_allow_non_loopback_admin: None,
            dangerously_allow_all_unix_sockets: None,
            allowed_domains: Some(vec!["*.openai.com".to_string()]),
            denied_domains: None,
            allow_unix_sockets: None,
            allow_local_binding: None,
        };

        let spec =
            NetworkProxySpec::from_config_and_constraints(user_config, None, Some(requirements))?;

        let mut expected_config = NetworkProxyConfig::default();
        expected_config.network.enabled = true;
        expected_config.network.allowed_domains = vec!["*.openai.com".to_string()];

        assert_eq!(spec.config, expected_config);
        assert_eq!(
            spec.constraints,
            NetworkProxyConstraints {
                enabled: Some(true),
                allowed_domains: Some(vec!["*.openai.com".to_string()]),
                ..NetworkProxyConstraints::default()
            }
        );

        Ok(())
    }
}
