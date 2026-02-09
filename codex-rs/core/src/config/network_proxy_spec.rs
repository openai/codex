use crate::config_loader::NetworkConstraints;
use async_trait::async_trait;
use codex_network_proxy::ConfigReloader;
use codex_network_proxy::ConfigState;
use codex_network_proxy::NetworkProxy;
use codex_network_proxy::NetworkProxyConfig;
use codex_network_proxy::NetworkProxyConstraints;
use codex_network_proxy::NetworkProxyHandle;
use codex_network_proxy::NetworkProxyState;
use codex_network_proxy::build_config_state;
use codex_network_proxy::validate_policy_against_constraints;
use std::path::PathBuf;
use std::sync::Arc;
use toml::Value as TomlValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkProxySpec {
    config: NetworkProxyConfig,
    constraints: NetworkProxyConstraints,
    cfg_path: PathBuf,
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
}

impl NetworkProxySpec {
    pub(crate) fn from_constraints(
        merged_toml: TomlValue,
        requirements: NetworkConstraints,
        cfg_path: PathBuf,
    ) -> std::io::Result<Self> {
        let config: NetworkProxyConfig = merged_toml.try_into().map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("failed to deserialize network proxy config: {err}"),
            )
        })?;
        let (config, constraints) = Self::apply_requirements(config, &requirements);
        validate_policy_against_constraints(&config, &constraints).map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("network proxy constraints are invalid: {err}"),
            )
        })?;
        Ok(Self {
            config,
            constraints,
            cfg_path,
        })
    }

    pub async fn start_proxy(&self) -> std::io::Result<(NetworkProxy, NetworkProxyHandle)> {
        let state = build_config_state(
            self.config.clone(),
            self.constraints.clone(),
            self.cfg_path.clone(),
        )
        .map_err(|err| {
            std::io::Error::other(format!("failed to build network proxy state: {err}"))
        })?;
        let reloader = Arc::new(StaticNetworkProxyReloader::new(state.clone()));
        let state = NetworkProxyState::with_reloader(state, reloader);
        let proxy = NetworkProxy::builder()
            .state(Arc::new(state))
            .build()
            .await
            .map_err(|err| {
                std::io::Error::other(format!("failed to build network proxy: {err}"))
            })?;
        let handle = proxy
            .run()
            .await
            .map_err(|err| std::io::Error::other(format!("failed to run network proxy: {err}")))?;
        Ok((proxy, handle))
    }

    fn apply_requirements(
        mut config: NetworkProxyConfig,
        requirements: &NetworkConstraints,
    ) -> (NetworkProxyConfig, NetworkProxyConstraints) {
        let mut constraints = NetworkProxyConstraints::default();

        if let Some(enabled) = requirements.enabled {
            config.network.enabled = enabled;
            constraints.enabled = Some(enabled);
        }
        if let Some(http_port) = requirements.http_port {
            config.network.proxy_url = format!("http://127.0.0.1:{http_port}");
        }
        if let Some(socks_port) = requirements.socks_port {
            config.network.socks_url = format!("http://127.0.0.1:{socks_port}");
        }
        if let Some(allow_upstream_proxy) = requirements.allow_upstream_proxy {
            config.network.allow_upstream_proxy = allow_upstream_proxy;
            constraints.allow_upstream_proxy = Some(allow_upstream_proxy);
        }
        if let Some(dangerously_allow_non_loopback_proxy) =
            requirements.dangerously_allow_non_loopback_proxy
        {
            config.network.dangerously_allow_non_loopback_proxy =
                dangerously_allow_non_loopback_proxy;
            constraints.dangerously_allow_non_loopback_proxy =
                Some(dangerously_allow_non_loopback_proxy);
        }
        if let Some(dangerously_allow_non_loopback_admin) =
            requirements.dangerously_allow_non_loopback_admin
        {
            config.network.dangerously_allow_non_loopback_admin =
                dangerously_allow_non_loopback_admin;
            constraints.dangerously_allow_non_loopback_admin =
                Some(dangerously_allow_non_loopback_admin);
        }
        if let Some(allowed_domains) = requirements.allowed_domains.clone() {
            config.network.allowed_domains = allowed_domains.clone();
            constraints.allowed_domains = Some(allowed_domains);
        }
        if let Some(denied_domains) = requirements.denied_domains.clone() {
            config.network.denied_domains = denied_domains.clone();
            constraints.denied_domains = Some(denied_domains);
        }
        if let Some(allow_unix_sockets) = requirements.allow_unix_sockets.clone() {
            config.network.allow_unix_sockets = allow_unix_sockets.clone();
            constraints.allow_unix_sockets = Some(allow_unix_sockets);
        }
        if let Some(allow_local_binding) = requirements.allow_local_binding {
            config.network.allow_local_binding = allow_local_binding;
            constraints.allow_local_binding = Some(allow_local_binding);
        }

        (config, constraints)
    }
}
