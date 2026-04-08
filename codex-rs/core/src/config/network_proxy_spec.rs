use crate::config_loader::NetworkConstraints;
use async_trait::async_trait;
use codex_execpolicy::Policy;
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
use codex_network_proxy::normalize_host;
use codex_network_proxy::validate_policy_against_constraints;
use codex_protocol::protocol::SandboxPolicy;
use std::collections::HashSet;
use std::sync::Arc;

const GLOBAL_ALLOWLIST_PATTERN: &str = "*";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkProxySpec {
    base_config: NetworkProxyConfig,
    requirements: Option<NetworkConstraints>,
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
        host_and_port_from_network_addr(&self.config.network.proxy_url, /*default_port*/ 3128)
    }

    pub fn socks_enabled(&self) -> bool {
        self.config.network.enable_socks5
    }

    pub(crate) fn from_config_and_constraints(
        config: NetworkProxyConfig,
        requirements: Option<NetworkConstraints>,
        sandbox_policy: &SandboxPolicy,
    ) -> std::io::Result<Self> {
        let base_config = config.clone();
        let hard_deny_allowlist_misses = requirements
            .as_ref()
            .is_some_and(Self::managed_allowed_domains_only);
        let (config, constraints) = if let Some(requirements) = requirements.as_ref() {
            Self::apply_requirements(
                config,
                requirements,
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
            base_config,
            requirements,
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
        if enable_network_approval_flow && !self.hard_deny_allowlist_misses {
            if let Some(policy_decider) = policy_decider {
                builder = builder.policy_decider_arc(policy_decider);
            } else if matches!(
                sandbox_policy,
                SandboxPolicy::ReadOnly { .. } | SandboxPolicy::WorkspaceWrite { .. }
            ) {
                builder = builder
                    .policy_decider(|_request| async { NetworkDecision::ask("not_allowed") });
            }
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

    pub(crate) fn recompute_for_sandbox_policy(
        &self,
        sandbox_policy: &SandboxPolicy,
    ) -> std::io::Result<Self> {
        Self::from_config_and_constraints(
            self.base_config.clone(),
            self.requirements.clone(),
            sandbox_policy,
        )
    }

    pub(crate) fn with_exec_policy_network_rules(
        &self,
        exec_policy: &Policy,
    ) -> std::io::Result<Self> {
        let mut spec = self.clone();
        apply_exec_policy_network_rules(&mut spec.config, exec_policy);
        validate_policy_against_constraints(&spec.config, &spec.constraints).map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("network proxy constraints are invalid: {err}"),
            )
        })?;
        Ok(spec)
    }

    pub(crate) async fn apply_to_started_proxy(
        &self,
        started_proxy: &StartedNetworkProxy,
    ) -> std::io::Result<()> {
        let state = self.build_config_state_for_spec()?;
        started_proxy
            .proxy()
            .replace_config_state(state)
            .await
            .map_err(|err| {
                std::io::Error::other(format!("failed to update network proxy state: {err}"))
            })
    }

    fn build_state_with_audit_metadata(
        &self,
        audit_metadata: NetworkProxyAuditMetadata,
    ) -> std::io::Result<NetworkProxyState> {
        let state = self.build_config_state_for_spec()?;
        let reloader = Arc::new(StaticNetworkProxyReloader::new(state.clone()));
        Ok(NetworkProxyState::with_reloader_and_audit_metadata(
            state,
            reloader,
            audit_metadata,
        ))
    }

    fn build_config_state_for_spec(&self) -> std::io::Result<ConfigState> {
        build_config_state(self.config.clone(), self.constraints.clone()).map_err(|err| {
            std::io::Error::other(format!("failed to build network proxy state: {err}"))
        })
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
        let danger_full_access_denylist_only =
            Self::danger_full_access_denylist_only_enabled(requirements, sandbox_policy);

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
        if danger_full_access_denylist_only {
            config
                .network
                .set_allowed_domains(vec![GLOBAL_ALLOWLIST_PATTERN.to_string()]);
        } else {
            let managed_allowed_domains = if hard_deny_allowlist_misses {
                Some(
                    requirements
                        .domains
                        .as_ref()
                        .and_then(codex_config::NetworkDomainPermissionsToml::allowed_domains)
                        .unwrap_or_default(),
                )
            } else {
                requirements
                    .domains
                    .as_ref()
                    .and_then(codex_config::NetworkDomainPermissionsToml::allowed_domains)
            };
            if let Some(managed_allowed_domains) = managed_allowed_domains {
                // Managed requirements seed the baseline allowlist. User additions
                // can extend that baseline unless managed-only mode pins the
                // effective allowlist to the managed set.
                let effective_allowed_domains = if allowlist_expansion_enabled {
                    Self::merge_domain_lists(
                        managed_allowed_domains.clone(),
                        config.network.allowed_domains().as_deref().unwrap_or(&[]),
                    )
                } else {
                    managed_allowed_domains.clone()
                };
                config
                    .network
                    .set_allowed_domains(effective_allowed_domains);
                constraints.allowed_domains = Some(managed_allowed_domains);
                constraints.allowlist_expansion_enabled = Some(allowlist_expansion_enabled);
            }
        }
        let managed_denied_domains = requirements
            .domains
            .as_ref()
            .and_then(codex_config::NetworkDomainPermissionsToml::denied_domains);
        if let Some(managed_denied_domains) = managed_denied_domains {
            let effective_denied_domains = if denylist_expansion_enabled {
                Self::merge_domain_lists(
                    managed_denied_domains.clone(),
                    config.network.denied_domains().as_deref().unwrap_or(&[]),
                )
            } else {
                managed_denied_domains.clone()
            };
            config.network.set_denied_domains(effective_denied_domains);
            constraints.denied_domains = Some(managed_denied_domains);
            constraints.denylist_expansion_enabled = Some(denylist_expansion_enabled);
        }
        if requirements.unix_sockets.is_some() && !danger_full_access_denylist_only {
            let allow_unix_sockets = requirements
                .unix_sockets
                .as_ref()
                .map(codex_config::NetworkUnixSocketPermissionsToml::allow_unix_sockets)
                .unwrap_or_default();
            config
                .network
                .set_allow_unix_sockets(allow_unix_sockets.clone());
            constraints.allow_unix_sockets = Some(allow_unix_sockets);
        }
        if let Some(allow_local_binding) = requirements.allow_local_binding {
            config.network.allow_local_binding = allow_local_binding;
            constraints.allow_local_binding = Some(allow_local_binding);
        }
        if danger_full_access_denylist_only {
            config.network.allow_upstream_proxy = true;
            constraints.allow_upstream_proxy = Some(true);
            config.network.dangerously_allow_all_unix_sockets = true;
            constraints.dangerously_allow_all_unix_sockets = Some(true);
            config.network.allow_local_binding = true;
            constraints.allow_local_binding = Some(true);
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

    fn danger_full_access_denylist_only_enabled(
        requirements: &NetworkConstraints,
        sandbox_policy: &SandboxPolicy,
    ) -> bool {
        matches!(sandbox_policy, SandboxPolicy::DangerFullAccess)
            && requirements
                .danger_full_access_denylist_only
                .unwrap_or(false)
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

fn apply_exec_policy_network_rules(config: &mut NetworkProxyConfig, exec_policy: &Policy) {
    let (allowed_domains, denied_domains) = exec_policy.compiled_network_domains();
    upsert_network_domains(config, allowed_domains, /*allow*/ true);
    upsert_network_domains(config, denied_domains, /*allow*/ false);
}

fn upsert_network_domains(config: &mut NetworkProxyConfig, hosts: Vec<String>, allow: bool) {
    let mut incoming = HashSet::new();
    for host in hosts {
        if incoming.insert(host.clone()) {
            config.network.upsert_domain_permission(
                host,
                if allow {
                    codex_network_proxy::NetworkDomainPermission::Allow
                } else {
                    codex_network_proxy::NetworkDomainPermission::Deny
                },
                normalize_host,
            );
        }
    }
}

#[cfg(test)]
#[path = "network_proxy_spec_tests.rs"]
mod tests;
