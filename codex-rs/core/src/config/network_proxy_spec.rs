use crate::config_loader::NetworkConstraints;
use crate::skills::model::SkillManagedNetworkOverride;
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct SkillNetworkProxyKey(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkProxySpec {
    config: NetworkProxyConfig,
    constraints: NetworkProxyConstraints,
    hard_deny_allowlist_misses: bool,
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

    pub(crate) fn with_skill_managed_network_override(
        &self,
        managed_network_override: &SkillManagedNetworkOverride,
    ) -> Self {
        let mut spec = self.clone();
        if let Some(allowed_domains) = managed_network_override.allowed_domains.as_ref() {
            spec.config.network.allowed_domains = allowed_domains.clone();
            spec.constraints.allowed_domains = Some(allowed_domains.clone());
        }
        if let Some(denied_domains) = managed_network_override.denied_domains.as_ref() {
            spec.config.network.denied_domains = denied_domains.clone();
            spec.constraints.denied_domains = Some(denied_domains.clone());
        }
        spec
    }

    pub(crate) fn shared_skill_proxy_key(&self) -> SkillNetworkProxyKey {
        let mut normalized = self.clone();
        sort_string_list(&mut normalized.config.network.allowed_domains);
        sort_string_list(&mut normalized.config.network.denied_domains);
        sort_string_list(&mut normalized.config.network.allow_unix_sockets);
        sort_option_string_list(&mut normalized.constraints.allowed_domains);
        sort_option_string_list(&mut normalized.constraints.denied_domains);
        sort_option_string_list(&mut normalized.constraints.allow_unix_sockets);
        SkillNetworkProxyKey(format!("{normalized:?}"))
    }

    pub(crate) fn from_config_and_constraints(
        config: NetworkProxyConfig,
        requirements: Option<NetworkConstraints>,
        sandbox_policy: &SandboxPolicy,
    ) -> std::io::Result<Self> {
        let hard_deny_allowlist_misses = requirements
            .as_ref()
            .is_some_and(Self::managed_allowed_domains_only);
        let (config, constraints) = if let Some(requirements) = requirements {
            Self::apply_requirements(
                config,
                &requirements,
                sandbox_policy,
                hard_deny_allowlist_misses,
            )
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
            hard_deny_allowlist_misses,
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
            && !self.hard_deny_allowlist_misses
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
        requirements: &NetworkConstraints,
        sandbox_policy: &SandboxPolicy,
        hard_deny_allowlist_misses: bool,
    ) -> (NetworkProxyConfig, NetworkProxyConstraints) {
        let mut constraints = NetworkProxyConstraints::default();
        let allowlist_expansion_enabled =
            Self::allowlist_expansion_enabled(sandbox_policy, hard_deny_allowlist_misses);
        let denylist_expansion_enabled = Self::denylist_expansion_enabled(sandbox_policy);

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
        if let Some(dangerously_allow_all_unix_sockets) =
            requirements.dangerously_allow_all_unix_sockets
        {
            config.network.dangerously_allow_all_unix_sockets = dangerously_allow_all_unix_sockets;
            constraints.dangerously_allow_all_unix_sockets =
                Some(dangerously_allow_all_unix_sockets);
        }
        let managed_allowed_domains = if hard_deny_allowlist_misses {
            Some(requirements.allowed_domains.clone().unwrap_or_default())
        } else {
            requirements.allowed_domains.clone()
        };
        if let Some(allowed_domains) = managed_allowed_domains {
            // Managed requirements seed the baseline allowlist. User additions
            // can extend that baseline unless managed-only mode pins the
            // effective allowlist to the managed set.
            config.network.allowed_domains = if allowlist_expansion_enabled {
                Self::merge_domain_lists(allowed_domains.clone(), &config.network.allowed_domains)
            } else {
                allowed_domains.clone()
            };
            constraints.allowed_domains = Some(allowed_domains);
            constraints.allowlist_expansion_enabled = Some(allowlist_expansion_enabled);
        }
        if let Some(denied_domains) = requirements.denied_domains.clone() {
            config.network.denied_domains = if denylist_expansion_enabled {
                Self::merge_domain_lists(denied_domains.clone(), &config.network.denied_domains)
            } else {
                denied_domains.clone()
            };
            constraints.denied_domains = Some(denied_domains);
            constraints.denylist_expansion_enabled = Some(denylist_expansion_enabled);
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

    fn allowlist_expansion_enabled(
        sandbox_policy: &SandboxPolicy,
        hard_deny_allowlist_misses: bool,
    ) -> bool {
        matches!(
            sandbox_policy,
            SandboxPolicy::ReadOnly { .. } | SandboxPolicy::WorkspaceWrite { .. }
        ) && !hard_deny_allowlist_misses
    }

    fn managed_allowed_domains_only(requirements: &NetworkConstraints) -> bool {
        requirements.managed_allowed_domains_only.unwrap_or(false)
    }

    fn denylist_expansion_enabled(sandbox_policy: &SandboxPolicy) -> bool {
        matches!(
            sandbox_policy,
            SandboxPolicy::ReadOnly { .. } | SandboxPolicy::WorkspaceWrite { .. }
        )
    }

    fn merge_domain_lists(mut managed: Vec<String>, user_entries: &[String]) -> Vec<String> {
        for entry in user_entries {
            if !managed
                .iter()
                .any(|managed_entry| managed_entry.eq_ignore_ascii_case(entry))
            {
                managed.push(entry.clone());
            }
        }
        managed
    }
}

fn sort_string_list(values: &mut [String]) {
    values.sort_unstable();
}

fn sort_option_string_list(values: &mut Option<Vec<String>>) {
    if let Some(values) = values.as_mut() {
        sort_string_list(values);
    }
}

#[cfg(test)]
#[path = "network_proxy_spec_tests.rs"]
mod tests;
