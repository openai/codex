use crate::config::NetworkMode;
use crate::config::NetworkProxyConfig;
use crate::mitm::MitmState;
use crate::policy::Host;
use crate::policy::is_loopback_host;
use crate::policy::is_non_public_ip;
use crate::policy::normalize_host;
use crate::reasons::REASON_DENIED;
use crate::reasons::REASON_NOT_ALLOWED;
use crate::reasons::REASON_NOT_ALLOWED_LOCAL;
use crate::state::NetworkProxyConstraints;
use crate::state::build_config_state;
use crate::state::validate_policy_against_constraints;
use anyhow::Context;
use anyhow::Result;
use codex_utils_absolute_path::AbsolutePathBuf;
use globset::GlobSet;
use serde::Serialize;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::net::IpAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use time::OffsetDateTime;
use tokio::net::lookup_host;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::info;
use tracing::warn;

const MAX_BLOCKED_EVENTS: usize = 200;
const DNS_LOOKUP_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostBlockReason {
    Denied,
    NotAllowed,
    NotAllowedLocal,
}

impl HostBlockReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Denied => REASON_DENIED,
            Self::NotAllowed => REASON_NOT_ALLOWED,
            Self::NotAllowedLocal => REASON_NOT_ALLOWED_LOCAL,
        }
    }
}

impl std::fmt::Display for HostBlockReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostBlockDecision {
    Allowed,
    Blocked(HostBlockReason),
}

#[derive(Clone, Debug, Serialize)]
pub struct BlockedRequest {
    pub host: String,
    pub reason: String,
    pub client: Option<String>,
    pub method: Option<String>,
    pub mode: Option<NetworkMode>,
    pub protocol: String,
    pub timestamp: i64,
}

pub struct BlockedRequestArgs {
    pub host: String,
    pub reason: String,
    pub client: Option<String>,
    pub method: Option<String>,
    pub mode: Option<NetworkMode>,
    pub protocol: String,
}

impl BlockedRequest {
    pub fn new(args: BlockedRequestArgs) -> Self {
        let BlockedRequestArgs {
            host,
            reason,
            client,
            method,
            mode,
            protocol,
        } = args;
        Self {
            host,
            reason,
            client,
            method,
            mode,
            protocol,
            timestamp: unix_timestamp(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct ConfigState {
    pub(crate) config: NetworkProxyConfig,
    pub(crate) allow_set: GlobSet,
    pub(crate) deny_set: GlobSet,
    pub(crate) mitm: Option<Arc<MitmState>>,
    pub(crate) constraints: NetworkProxyConstraints,
    pub(crate) layer_mtimes: Vec<LayerMtime>,
    pub(crate) cfg_path: PathBuf,
    pub(crate) blocked: VecDeque<BlockedRequest>,
}

#[derive(Clone)]
pub(crate) struct LayerMtime {
    pub(crate) path: PathBuf,
    pub(crate) mtime: Option<SystemTime>,
}

impl LayerMtime {
    pub(crate) fn new(path: PathBuf) -> Self {
        let mtime = path.metadata().and_then(|m| m.modified()).ok();
        Self { path, mtime }
    }
}

#[derive(Clone)]
pub struct NetworkProxyState {
    state: Arc<RwLock<ConfigState>>,
}

impl std::fmt::Debug for NetworkProxyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Avoid logging internal state (config contents, derived globsets, etc.) which can be noisy
        // and may contain sensitive paths.
        f.debug_struct("NetworkProxyState").finish_non_exhaustive()
    }
}

impl NetworkProxyState {
    pub async fn new() -> Result<Self> {
        let cfg_state = build_config_state().await?;
        Ok(Self {
            state: Arc::new(RwLock::new(cfg_state)),
        })
    }

    pub async fn current_cfg(&self) -> Result<NetworkProxyConfig> {
        // Callers treat `NetworkProxyState` as a live view of policy. We reload-on-demand so edits to
        // `config.toml` (including Codex-managed writes) take effect without a restart.
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok(guard.config.clone())
    }

    pub async fn current_patterns(&self) -> Result<(Vec<String>, Vec<String>)> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok((
            guard.config.network_proxy.policy.allowed_domains.clone(),
            guard.config.network_proxy.policy.denied_domains.clone(),
        ))
    }

    pub async fn enabled(&self) -> Result<bool> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok(guard.config.network_proxy.enabled)
    }

    pub async fn force_reload(&self) -> Result<()> {
        let (previous_cfg, cfg_path) = {
            let guard = self.state.read().await;
            (guard.config.clone(), guard.cfg_path.clone())
        };

        match build_config_state().await {
            Ok(mut new_state) => {
                // Policy changes are operationally sensitive; logging diffs makes changes traceable
                // without needing to dump full config blobs (which can include unrelated settings).
                log_policy_changes(&previous_cfg, &new_state.config);
                let mut guard = self.state.write().await;
                new_state.blocked = guard.blocked.clone();
                *guard = new_state;
                let path = guard.cfg_path.display();
                info!("reloaded config from {path}");
                Ok(())
            }
            Err(err) => {
                let path = cfg_path.display();
                warn!("failed to reload config from {path}: {err}; keeping previous config");
                Err(err)
            }
        }
    }

    pub async fn host_blocked(&self, host: &str, port: u16) -> Result<HostBlockDecision> {
        self.reload_if_needed().await?;
        let host = match Host::parse(host) {
            Ok(host) => host,
            Err(_) => return Ok(HostBlockDecision::Blocked(HostBlockReason::NotAllowed)),
        };
        let (deny_set, allow_set, allow_local_binding, allowed_domains_empty, allowed_domains) = {
            let guard = self.state.read().await;
            (
                guard.deny_set.clone(),
                guard.allow_set.clone(),
                guard.config.network_proxy.policy.allow_local_binding,
                guard.config.network_proxy.policy.allowed_domains.is_empty(),
                guard.config.network_proxy.policy.allowed_domains.clone(),
            )
        };

        let host_str = host.as_str();

        // Decision order matters:
        //  1) explicit deny always wins
        //  2) local/private networking is opt-in (defense-in-depth)
        //  3) allowlist is enforced when configured
        if deny_set.is_match(host_str) {
            return Ok(HostBlockDecision::Blocked(HostBlockReason::Denied));
        }

        let is_allowlisted = allow_set.is_match(host_str);
        if !allow_local_binding {
            // If the intent is "prevent access to local/internal networks", we must not rely solely
            // on string checks like `localhost` / `127.0.0.1`. Attackers can use DNS rebinding or
            // public suffix services that map hostnames onto private IPs.
            //
            // We therefore do a best-effort DNS + IP classification check before allowing the
            // request. Explicit local/loopback literals are allowed only when explicitly
            // allowlisted; hostnames that resolve to local/private IPs are blocked even if
            // allowlisted.
            let local_literal = {
                let host_no_scope = host_str
                    .split_once('%')
                    .map(|(ip, _)| ip)
                    .unwrap_or(host_str);
                if is_loopback_host(&host) {
                    true
                } else if let Ok(ip) = host_no_scope.parse::<IpAddr>() {
                    is_non_public_ip(ip)
                } else {
                    false
                }
            };

            if local_literal {
                if !is_explicit_local_allowlisted(&allowed_domains, &host) {
                    return Ok(HostBlockDecision::Blocked(HostBlockReason::NotAllowedLocal));
                }
            } else if host_resolves_to_non_public_ip(host_str, port).await {
                return Ok(HostBlockDecision::Blocked(HostBlockReason::NotAllowedLocal));
            }
        }

        if allowed_domains_empty || !is_allowlisted {
            Ok(HostBlockDecision::Blocked(HostBlockReason::NotAllowed))
        } else {
            Ok(HostBlockDecision::Allowed)
        }
    }

    pub async fn record_blocked(&self, entry: BlockedRequest) -> Result<()> {
        self.reload_if_needed().await?;
        let mut guard = self.state.write().await;
        guard.blocked.push_back(entry);
        while guard.blocked.len() > MAX_BLOCKED_EVENTS {
            guard.blocked.pop_front();
        }
        Ok(())
    }

    /// Drain and return the buffered blocked-request entries in FIFO order.
    pub async fn drain_blocked(&self) -> Result<Vec<BlockedRequest>> {
        self.reload_if_needed().await?;
        let blocked = {
            let mut guard = self.state.write().await;
            std::mem::take(&mut guard.blocked)
        };
        Ok(blocked.into_iter().collect())
    }

    pub async fn is_unix_socket_allowed(&self, path: &str) -> Result<bool> {
        self.reload_if_needed().await?;
        if !unix_socket_permissions_supported() {
            return Ok(false);
        }

        // We only support absolute unix socket paths (a relative path would be ambiguous with
        // respect to the proxy process's CWD and can lead to confusing allowlist behavior).
        let requested_path = Path::new(path);
        if !requested_path.is_absolute() {
            return Ok(false);
        }

        let guard = self.state.read().await;
        // Normalize the path while keeping the absolute-path requirement explicit.
        let requested_abs = match AbsolutePathBuf::from_absolute_path(requested_path) {
            Ok(path) => path,
            Err(_) => return Ok(false),
        };
        let requested_canonical = std::fs::canonicalize(requested_abs.as_path()).ok();
        for allowed in &guard.config.network_proxy.policy.allow_unix_sockets {
            if allowed == path {
                return Ok(true);
            }

            // Best-effort canonicalization to reduce surprises with symlinks.
            // If canonicalization fails (e.g., socket not created yet), fall back to raw comparison.
            let Some(requested_canonical) = &requested_canonical else {
                continue;
            };
            if let Ok(allowed_canonical) = std::fs::canonicalize(allowed)
                && &allowed_canonical == requested_canonical
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub async fn method_allowed(&self, method: &str) -> Result<bool> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok(guard.config.network_proxy.mode.allows_method(method))
    }

    pub async fn allow_upstream_proxy(&self) -> Result<bool> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok(guard.config.network_proxy.allow_upstream_proxy)
    }

    pub async fn network_mode(&self) -> Result<NetworkMode> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok(guard.config.network_proxy.mode)
    }

    pub async fn set_network_mode(&self, mode: NetworkMode) -> Result<()> {
        loop {
            self.reload_if_needed().await?;
            let (candidate, constraints) = {
                let guard = self.state.read().await;
                let mut candidate = guard.config.clone();
                candidate.network_proxy.mode = mode;
                (candidate, guard.constraints.clone())
            };

            validate_policy_against_constraints(&candidate, &constraints)
                .context("network_proxy.mode constrained by managed config")?;

            let mut guard = self.state.write().await;
            if guard.constraints != constraints {
                drop(guard);
                continue;
            }
            guard.config.network_proxy.mode = mode;
            info!("updated network mode to {mode:?}");
            return Ok(());
        }
    }

    pub async fn mitm_state(&self) -> Result<Option<Arc<MitmState>>> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok(guard.mitm.clone())
    }

    async fn reload_if_needed(&self) -> Result<()> {
        let needs_reload = {
            let guard = self.state.read().await;
            guard.layer_mtimes.iter().any(|layer| {
                let metadata = std::fs::metadata(&layer.path).ok();
                match (metadata.and_then(|m| m.modified().ok()), layer.mtime) {
                    (Some(new_mtime), Some(old_mtime)) => new_mtime > old_mtime,
                    (Some(_), None) => true,
                    (None, Some(_)) => true,
                    (None, None) => false,
                }
            })
        };

        if !needs_reload {
            return Ok(());
        }

        self.force_reload().await
    }
}

pub(crate) fn unix_socket_permissions_supported() -> bool {
    cfg!(target_os = "macos")
}

async fn host_resolves_to_non_public_ip(host: &str, port: u16) -> bool {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_non_public_ip(ip);
    }

    // If DNS lookup fails, default to "not local/private" rather than blocking. In practice, the
    // subsequent connect attempt will fail anyway, and blocking on transient resolver issues would
    // make the proxy fragile. The allowlist/denylist remains the primary control plane.
    let addrs = match timeout(DNS_LOOKUP_TIMEOUT, lookup_host((host, port))).await {
        Ok(Ok(addrs)) => addrs,
        Ok(Err(_)) | Err(_) => return false,
    };

    for addr in addrs {
        if is_non_public_ip(addr.ip()) {
            return true;
        }
    }

    false
}

fn log_policy_changes(previous: &NetworkProxyConfig, next: &NetworkProxyConfig) {
    log_domain_list_changes(
        "allowlist",
        &previous.network_proxy.policy.allowed_domains,
        &next.network_proxy.policy.allowed_domains,
    );
    log_domain_list_changes(
        "denylist",
        &previous.network_proxy.policy.denied_domains,
        &next.network_proxy.policy.denied_domains,
    );
}

fn log_domain_list_changes(list_name: &str, previous: &[String], next: &[String]) {
    let previous_set: HashSet<String> = previous
        .iter()
        .map(|entry| entry.to_ascii_lowercase())
        .collect();
    let next_set: HashSet<String> = next
        .iter()
        .map(|entry| entry.to_ascii_lowercase())
        .collect();

    let added = next_set
        .difference(&previous_set)
        .cloned()
        .collect::<HashSet<_>>();
    let removed = previous_set
        .difference(&next_set)
        .cloned()
        .collect::<HashSet<_>>();

    let mut seen_next = HashSet::new();
    for entry in next {
        let key = entry.to_ascii_lowercase();
        if seen_next.insert(key.clone()) && added.contains(&key) {
            info!("config entry added to {list_name}: {entry}");
        }
    }

    let mut seen_previous = HashSet::new();
    for entry in previous {
        let key = entry.to_ascii_lowercase();
        if seen_previous.insert(key.clone()) && removed.contains(&key) {
            info!("config entry removed from {list_name}: {entry}");
        }
    }
}

fn is_explicit_local_allowlisted(allowed_domains: &[String], host: &Host) -> bool {
    let normalized_host = host.as_str();
    allowed_domains.iter().any(|pattern| {
        let pattern = pattern.trim();
        if pattern == "*" || pattern.starts_with("*.") || pattern.starts_with("**.") {
            return false;
        }
        if pattern.contains('*') || pattern.contains('?') {
            return false;
        }
        normalize_host(pattern) == normalized_host
    })
}

fn unix_timestamp() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

#[cfg(test)]
pub(crate) fn network_proxy_state_for_policy(
    policy: crate::config::NetworkPolicy,
) -> NetworkProxyState {
    let config = NetworkProxyConfig {
        network_proxy: crate::config::NetworkProxySettings {
            enabled: true,
            mode: NetworkMode::Full,
            policy,
            ..crate::config::NetworkProxySettings::default()
        },
    };

    let allow_set =
        crate::policy::compile_globset(&config.network_proxy.policy.allowed_domains).unwrap();
    let deny_set =
        crate::policy::compile_globset(&config.network_proxy.policy.denied_domains).unwrap();

    let state = ConfigState {
        config,
        allow_set,
        deny_set,
        mitm: None,
        constraints: NetworkProxyConstraints::default(),
        layer_mtimes: Vec::new(),
        cfg_path: PathBuf::from("/nonexistent/config.toml"),
        blocked: VecDeque::new(),
    };

    NetworkProxyState {
        state: Arc::new(RwLock::new(state)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::NetworkPolicy;
    use crate::config::NetworkProxyConfig;
    use crate::config::NetworkProxySettings;
    use crate::policy::compile_globset;
    use crate::state::NetworkProxyConstraints;
    use crate::state::validate_policy_against_constraints;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn host_blocked_denied_wins_over_allowed() {
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            denied_domains: vec!["example.com".to_string()],
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("example.com", 80).await.unwrap(),
            HostBlockDecision::Blocked(HostBlockReason::Denied)
        );
    }

    #[tokio::test]
    async fn host_blocked_requires_allowlist_match() {
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("example.com", 80).await.unwrap(),
            HostBlockDecision::Allowed
        );
        assert_eq!(
            // Use a public IP literal to avoid relying on ambient DNS behavior (some networks
            // resolve unknown hostnames to private IPs, which would trigger `not_allowed_local`).
            state.host_blocked("8.8.8.8", 80).await.unwrap(),
            HostBlockDecision::Blocked(HostBlockReason::NotAllowed)
        );
    }

    #[tokio::test]
    async fn host_blocked_subdomain_wildcards_exclude_apex() {
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["*.openai.com".to_string()],
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("api.openai.com", 80).await.unwrap(),
            HostBlockDecision::Allowed
        );
        assert_eq!(
            state.host_blocked("openai.com", 80).await.unwrap(),
            HostBlockDecision::Blocked(HostBlockReason::NotAllowed)
        );
    }

    #[tokio::test]
    async fn host_blocked_rejects_loopback_when_local_binding_disabled() {
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("127.0.0.1", 80).await.unwrap(),
            HostBlockDecision::Blocked(HostBlockReason::NotAllowedLocal)
        );
        assert_eq!(
            state.host_blocked("localhost", 80).await.unwrap(),
            HostBlockDecision::Blocked(HostBlockReason::NotAllowedLocal)
        );
    }

    #[tokio::test]
    async fn host_blocked_rejects_loopback_when_allowlist_is_wildcard() {
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["*".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("127.0.0.1", 80).await.unwrap(),
            HostBlockDecision::Blocked(HostBlockReason::NotAllowedLocal)
        );
    }

    #[tokio::test]
    async fn host_blocked_rejects_private_ip_literal_when_allowlist_is_wildcard() {
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["*".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("10.0.0.1", 80).await.unwrap(),
            HostBlockDecision::Blocked(HostBlockReason::NotAllowedLocal)
        );
    }

    #[tokio::test]
    async fn host_blocked_allows_loopback_when_explicitly_allowlisted_and_local_binding_disabled() {
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["localhost".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("localhost", 80).await.unwrap(),
            HostBlockDecision::Allowed
        );
    }

    #[tokio::test]
    async fn host_blocked_allows_private_ip_literal_when_explicitly_allowlisted() {
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["10.0.0.1".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("10.0.0.1", 80).await.unwrap(),
            HostBlockDecision::Allowed
        );
    }

    #[tokio::test]
    async fn host_blocked_rejects_scoped_ipv6_literal_when_not_allowlisted() {
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("fe80::1%lo0", 80).await.unwrap(),
            HostBlockDecision::Blocked(HostBlockReason::NotAllowedLocal)
        );
    }

    #[tokio::test]
    async fn host_blocked_allows_scoped_ipv6_literal_when_explicitly_allowlisted() {
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["fe80::1%lo0".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("fe80::1%lo0", 80).await.unwrap(),
            HostBlockDecision::Allowed
        );
    }

    #[tokio::test]
    async fn host_blocked_rejects_private_ip_literals_when_local_binding_disabled() {
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("10.0.0.1", 80).await.unwrap(),
            HostBlockDecision::Blocked(HostBlockReason::NotAllowedLocal)
        );
    }

    #[tokio::test]
    async fn host_blocked_rejects_loopback_when_allowlist_empty() {
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec![],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("127.0.0.1", 80).await.unwrap(),
            HostBlockDecision::Blocked(HostBlockReason::NotAllowedLocal)
        );
    }

    #[test]
    fn validate_policy_against_constraints_disallows_widening_allowed_domains() {
        let constraints = NetworkProxyConstraints {
            allowed_domains: Some(vec!["example.com".to_string()]),
            ..NetworkProxyConstraints::default()
        };

        let config = NetworkProxyConfig {
            network_proxy: NetworkProxySettings {
                enabled: true,
                policy: NetworkPolicy {
                    allowed_domains: vec!["example.com".to_string(), "evil.com".to_string()],
                    ..NetworkPolicy::default()
                },
                ..NetworkProxySettings::default()
            },
        };

        assert!(validate_policy_against_constraints(&config, &constraints).is_err());
    }

    #[test]
    fn validate_policy_against_constraints_disallows_widening_mode() {
        let constraints = NetworkProxyConstraints {
            mode: Some(NetworkMode::Limited),
            ..NetworkProxyConstraints::default()
        };

        let config = NetworkProxyConfig {
            network_proxy: NetworkProxySettings {
                enabled: true,
                mode: NetworkMode::Full,
                ..NetworkProxySettings::default()
            },
        };

        assert!(validate_policy_against_constraints(&config, &constraints).is_err());
    }

    #[test]
    fn validate_policy_against_constraints_allows_narrowing_wildcard_allowlist() {
        let constraints = NetworkProxyConstraints {
            allowed_domains: Some(vec!["*.example.com".to_string()]),
            ..NetworkProxyConstraints::default()
        };

        let config = NetworkProxyConfig {
            network_proxy: NetworkProxySettings {
                enabled: true,
                policy: NetworkPolicy {
                    allowed_domains: vec!["api.example.com".to_string()],
                    ..NetworkPolicy::default()
                },
                ..NetworkProxySettings::default()
            },
        };

        assert!(validate_policy_against_constraints(&config, &constraints).is_ok());
    }

    #[test]
    fn validate_policy_against_constraints_rejects_widening_wildcard_allowlist() {
        let constraints = NetworkProxyConstraints {
            allowed_domains: Some(vec!["*.example.com".to_string()]),
            ..NetworkProxyConstraints::default()
        };

        let config = NetworkProxyConfig {
            network_proxy: NetworkProxySettings {
                enabled: true,
                policy: NetworkPolicy {
                    allowed_domains: vec!["**.example.com".to_string()],
                    ..NetworkPolicy::default()
                },
                ..NetworkProxySettings::default()
            },
        };

        assert!(validate_policy_against_constraints(&config, &constraints).is_err());
    }

    #[test]
    fn validate_policy_against_constraints_requires_managed_denied_domains_entries() {
        let constraints = NetworkProxyConstraints {
            denied_domains: Some(vec!["evil.com".to_string()]),
            ..NetworkProxyConstraints::default()
        };

        let config = NetworkProxyConfig {
            network_proxy: NetworkProxySettings {
                enabled: true,
                policy: NetworkPolicy {
                    denied_domains: vec![],
                    ..NetworkPolicy::default()
                },
                ..NetworkProxySettings::default()
            },
        };

        assert!(validate_policy_against_constraints(&config, &constraints).is_err());
    }

    #[test]
    fn validate_policy_against_constraints_disallows_enabling_when_managed_disabled() {
        let constraints = NetworkProxyConstraints {
            enabled: Some(false),
            ..NetworkProxyConstraints::default()
        };

        let config = NetworkProxyConfig {
            network_proxy: NetworkProxySettings {
                enabled: true,
                ..NetworkProxySettings::default()
            },
        };

        assert!(validate_policy_against_constraints(&config, &constraints).is_err());
    }

    #[test]
    fn validate_policy_against_constraints_disallows_allow_local_binding_when_managed_disabled() {
        let constraints = NetworkProxyConstraints {
            allow_local_binding: Some(false),
            ..NetworkProxyConstraints::default()
        };

        let config = NetworkProxyConfig {
            network_proxy: NetworkProxySettings {
                enabled: true,
                policy: NetworkPolicy {
                    allow_local_binding: true,
                    ..NetworkPolicy::default()
                },
                ..NetworkProxySettings::default()
            },
        };

        assert!(validate_policy_against_constraints(&config, &constraints).is_err());
    }

    #[test]
    fn validate_policy_against_constraints_disallows_non_loopback_admin_without_managed_opt_in() {
        let constraints = NetworkProxyConstraints {
            dangerously_allow_non_loopback_admin: Some(false),
            ..NetworkProxyConstraints::default()
        };

        let config = NetworkProxyConfig {
            network_proxy: NetworkProxySettings {
                enabled: true,
                dangerously_allow_non_loopback_admin: true,
                ..NetworkProxySettings::default()
            },
        };

        assert!(validate_policy_against_constraints(&config, &constraints).is_err());
    }

    #[test]
    fn validate_policy_against_constraints_allows_non_loopback_admin_with_managed_opt_in() {
        let constraints = NetworkProxyConstraints {
            dangerously_allow_non_loopback_admin: Some(true),
            ..NetworkProxyConstraints::default()
        };

        let config = NetworkProxyConfig {
            network_proxy: NetworkProxySettings {
                enabled: true,
                dangerously_allow_non_loopback_admin: true,
                ..NetworkProxySettings::default()
            },
        };

        assert!(validate_policy_against_constraints(&config, &constraints).is_ok());
    }

    #[test]
    fn compile_globset_is_case_insensitive() {
        let patterns = vec!["ExAmPle.CoM".to_string()];
        let set = compile_globset(&patterns).unwrap();
        assert!(set.is_match("example.com"));
        assert!(set.is_match("EXAMPLE.COM"));
    }

    #[test]
    fn compile_globset_excludes_apex_for_subdomain_patterns() {
        let patterns = vec!["*.openai.com".to_string()];
        let set = compile_globset(&patterns).unwrap();
        assert!(set.is_match("api.openai.com"));
        assert!(!set.is_match("openai.com"));
        assert!(!set.is_match("evilopenai.com"));
    }

    #[test]
    fn compile_globset_includes_apex_for_double_wildcard_patterns() {
        let patterns = vec!["**.openai.com".to_string()];
        let set = compile_globset(&patterns).unwrap();
        assert!(set.is_match("openai.com"));
        assert!(set.is_match("api.openai.com"));
        assert!(!set.is_match("evilopenai.com"));
    }

    #[test]
    fn compile_globset_matches_all_with_star() {
        let patterns = vec!["*".to_string()];
        let set = compile_globset(&patterns).unwrap();
        assert!(set.is_match("openai.com"));
        assert!(set.is_match("api.openai.com"));
    }

    #[test]
    fn compile_globset_dedupes_patterns_without_changing_behavior() {
        let patterns = vec!["example.com".to_string(), "example.com".to_string()];
        let set = compile_globset(&patterns).unwrap();
        assert!(set.is_match("example.com"));
        assert!(set.is_match("EXAMPLE.COM"));
        assert!(!set.is_match("not-example.com"));
    }

    #[test]
    fn compile_globset_rejects_invalid_patterns() {
        let patterns = vec!["[".to_string()];
        assert!(compile_globset(&patterns).is_err());
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn unix_socket_allowlist_is_respected_on_macos() {
        let socket_path = "/tmp/example.sock".to_string();
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            allow_unix_sockets: vec![socket_path.clone()],
            ..NetworkPolicy::default()
        });

        assert!(state.is_unix_socket_allowed(&socket_path).await.unwrap());
        assert!(
            !state
                .is_unix_socket_allowed("/tmp/not-allowed.sock")
                .await
                .unwrap()
        );
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn unix_socket_allowlist_resolves_symlinks() {
        use std::os::unix::fs::symlink;
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let dir = temp_dir.path();

        let real = dir.join("real.sock");
        let link = dir.join("link.sock");

        // The allowlist mechanism is path-based; for test purposes we don't need an actual unix
        // domain socket. Any filesystem entry works for canonicalization.
        std::fs::write(&real, b"not a socket").unwrap();
        symlink(&real, &link).unwrap();

        let real_s = real.to_str().unwrap().to_string();
        let link_s = link.to_str().unwrap().to_string();

        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            allow_unix_sockets: vec![real_s],
            ..NetworkPolicy::default()
        });

        assert!(state.is_unix_socket_allowed(&link_s).await.unwrap());
    }

    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn unix_socket_allowlist_is_rejected_on_non_macos() {
        let socket_path = "/tmp/example.sock".to_string();
        let state = network_proxy_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            allow_unix_sockets: vec![socket_path.clone()],
            ..NetworkPolicy::default()
        });

        assert!(!state.is_unix_socket_allowed(&socket_path).await.unwrap());
    }
}
