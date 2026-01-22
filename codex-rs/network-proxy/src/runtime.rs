use crate::config::Config;
use crate::config::NetworkMode;
use crate::mitm::MitmState;
use crate::policy::is_loopback_host;
use crate::policy::is_non_public_ip;
use crate::policy::method_allowed;
use crate::policy::normalize_host;
use crate::state::NetworkProxyConstraints;
use crate::state::build_config_state;
use crate::state::validate_policy_against_constraints;
use anyhow::Context;
use anyhow::Result;
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

impl BlockedRequest {
    pub fn new(
        host: String,
        reason: String,
        client: Option<String>,
        method: Option<String>,
        mode: Option<NetworkMode>,
        protocol: String,
    ) -> Self {
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
    pub(crate) config: Config,
    pub(crate) mtime: Option<SystemTime>,
    pub(crate) allow_set: GlobSet,
    pub(crate) deny_set: GlobSet,
    pub(crate) mitm: Option<Arc<MitmState>>,
    pub(crate) constraints: NetworkProxyConstraints,
    pub(crate) cfg_path: PathBuf,
    pub(crate) blocked: VecDeque<BlockedRequest>,
}

#[derive(Clone)]
pub struct AppState {
    state: Arc<RwLock<ConfigState>>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Avoid logging internal state (config contents, derived globsets, etc.) which can be noisy
        // and may contain sensitive paths.
        f.debug_struct("AppState").finish_non_exhaustive()
    }
}

impl AppState {
    pub async fn new() -> Result<Self> {
        let cfg_state = build_config_state().await?;
        Ok(Self {
            state: Arc::new(RwLock::new(cfg_state)),
        })
    }

    pub async fn current_cfg(&self) -> Result<Config> {
        // Callers treat `AppState` as a live view of policy. We reload-on-demand so edits to
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
        let mut guard = self.state.write().await;
        let previous_cfg = guard.config.clone();
        let blocked = guard.blocked.clone();
        match build_config_state().await {
            Ok(mut new_state) => {
                // Policy changes are operationally sensitive; logging diffs makes changes traceable
                // without needing to dump full config blobs (which can include unrelated settings).
                log_policy_changes(&previous_cfg, &new_state.config);
                new_state.blocked = blocked;
                *guard = new_state;
                let path = guard.cfg_path.display();
                info!("reloaded config from {path}");
                Ok(())
            }
            Err(err) => {
                let path = guard.cfg_path.display();
                warn!("failed to reload config from {path}: {err}; keeping previous config");
                Err(err)
            }
        }
    }

    pub async fn host_blocked(&self, host: &str, port: u16) -> Result<(bool, String)> {
        self.reload_if_needed().await?;
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

        // Decision order matters:
        //  1) explicit deny always wins
        //  2) local/private networking is opt-in (defense-in-depth)
        //  3) allowlist is enforced when configured
        if deny_set.is_match(host) {
            return Ok((true, "denied".to_string()));
        }

        let is_allowlisted = allow_set.is_match(host);
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
                let host = host.trim();
                let host = host.split_once('%').map(|(ip, _)| ip).unwrap_or(host);
                if is_loopback_host(host) {
                    true
                } else if let Ok(ip) = host.parse::<IpAddr>() {
                    is_non_public_ip(ip)
                } else {
                    false
                }
            };

            if local_literal {
                if !is_explicit_local_allowlisted(&allowed_domains, host) {
                    return Ok((true, "not_allowed_local".to_string()));
                }
            } else if host_resolves_to_non_public_ip(host, port).await? {
                return Ok((true, "not_allowed_local".to_string()));
            }
        }

        if allowed_domains_empty {
            return Ok((true, "not_allowed".to_string()));
        }

        if !is_allowlisted {
            return Ok((true, "not_allowed".to_string()));
        }
        Ok((false, String::new()))
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

    pub async fn drain_blocked(&self) -> Result<Vec<BlockedRequest>> {
        self.reload_if_needed().await?;
        let mut guard = self.state.write().await;
        let blocked = std::mem::take(&mut guard.blocked);
        Ok(blocked.into_iter().collect())
    }

    pub async fn is_unix_socket_allowed(&self, path: &str) -> Result<bool> {
        self.reload_if_needed().await?;
        if cfg!(not(target_os = "macos")) {
            return Ok(false);
        }

        // We only support absolute unix socket paths (a relative path would be ambiguous with
        // respect to the proxy process's CWD and can lead to confusing allowlist behavior).
        if !Path::new(path).is_absolute() {
            return Ok(false);
        }

        let guard = self.state.read().await;
        let requested_canonical = std::fs::canonicalize(path).ok();
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
        Ok(method_allowed(guard.config.network_proxy.mode, method))
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
        self.reload_if_needed().await?;
        let mut guard = self.state.write().await;
        let mut candidate = guard.config.clone();
        candidate.network_proxy.mode = mode;
        validate_policy_against_constraints(&candidate, &guard.constraints)
            .context("network_proxy.mode constrained by managed config")?;
        guard.config.network_proxy.mode = mode;
        info!("updated network mode to {mode:?}");
        Ok(())
    }

    pub async fn mitm_state(&self) -> Result<Option<Arc<MitmState>>> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok(guard.mitm.clone())
    }

    async fn reload_if_needed(&self) -> Result<()> {
        let needs_reload = {
            let guard = self.state.read().await;
            if !guard.cfg_path.exists() {
                // If the config file is missing, only reload when it *used to* exist (mtime set).
                // This avoids forcing a reload on every request when running with the default config.
                guard.mtime.is_some()
            } else {
                let metadata = std::fs::metadata(&guard.cfg_path).ok();
                match (metadata.and_then(|m| m.modified().ok()), guard.mtime) {
                    (Some(new_mtime), Some(old_mtime)) => new_mtime > old_mtime,
                    (Some(_), None) => true,
                    _ => false,
                }
            }
        };

        if !needs_reload {
            return Ok(());
        }

        self.force_reload().await
    }
}

async fn host_resolves_to_non_public_ip(host: &str, port: u16) -> Result<bool> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(is_non_public_ip(ip));
    }

    // If DNS lookup fails, default to "not local/private" rather than blocking. In practice, the
    // subsequent connect attempt will fail anyway, and blocking on transient resolver issues would
    // make the proxy fragile. The allowlist/denylist remains the primary control plane.
    let addrs = match timeout(DNS_LOOKUP_TIMEOUT, lookup_host((host, port))).await {
        Ok(Ok(addrs)) => addrs,
        Ok(Err(_)) | Err(_) => return Ok(false),
    };

    for addr in addrs {
        if is_non_public_ip(addr.ip()) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn log_policy_changes(previous: &Config, next: &Config) {
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

    let mut seen_next = HashSet::new();
    for entry in next {
        let key = entry.to_ascii_lowercase();
        if seen_next.insert(key.clone()) && !previous_set.contains(&key) {
            info!("config entry added to {list_name}: {entry}");
        }
    }

    let mut seen_previous = HashSet::new();
    for entry in previous {
        let key = entry.to_ascii_lowercase();
        if seen_previous.insert(key.clone()) && !next_set.contains(&key) {
            info!("config entry removed from {list_name}: {entry}");
        }
    }
}

fn is_explicit_local_allowlisted(allowed_domains: &[String], host: &str) -> bool {
    let normalized_host = normalize_host(host);
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
pub(crate) fn app_state_for_policy(policy: crate::config::NetworkPolicy) -> AppState {
    let config = Config {
        network_proxy: crate::config::NetworkProxyConfig {
            enabled: true,
            mode: NetworkMode::Full,
            policy,
            ..crate::config::NetworkProxyConfig::default()
        },
    };

    let allow_set =
        crate::policy::compile_globset(&config.network_proxy.policy.allowed_domains).unwrap();
    let deny_set =
        crate::policy::compile_globset(&config.network_proxy.policy.denied_domains).unwrap();

    let state = ConfigState {
        config,
        mtime: None,
        allow_set,
        deny_set,
        mitm: None,
        constraints: NetworkProxyConstraints::default(),
        cfg_path: PathBuf::from("/nonexistent/config.toml"),
        blocked: VecDeque::new(),
    };

    AppState {
        state: Arc::new(RwLock::new(state)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::NetworkPolicy;
    use crate::config::NetworkProxyConfig;
    use crate::policy::compile_globset;
    use crate::state::NetworkProxyConstraints;
    use crate::state::validate_policy_against_constraints;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn host_blocked_denied_wins_over_allowed() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            denied_domains: vec!["example.com".to_string()],
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("example.com", 80).await.unwrap(),
            (true, "denied".to_string())
        );
    }

    #[tokio::test]
    async fn host_blocked_requires_allowlist_match() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("example.com", 80).await.unwrap(),
            (false, String::new())
        );
        assert_eq!(
            // Use a public IP literal to avoid relying on ambient DNS behavior (some networks
            // resolve unknown hostnames to private IPs, which would trigger `not_allowed_local`).
            state.host_blocked("8.8.8.8", 80).await.unwrap(),
            (true, "not_allowed".to_string())
        );
    }

    #[tokio::test]
    async fn host_blocked_subdomain_wildcards_exclude_apex() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["*.openai.com".to_string()],
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("api.openai.com", 80).await.unwrap(),
            (false, String::new())
        );
        assert_eq!(
            state.host_blocked("openai.com", 80).await.unwrap(),
            (true, "not_allowed".to_string())
        );
    }

    #[tokio::test]
    async fn host_blocked_rejects_loopback_when_local_binding_disabled() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("127.0.0.1", 80).await.unwrap(),
            (true, "not_allowed_local".to_string())
        );
        assert_eq!(
            state.host_blocked("localhost", 80).await.unwrap(),
            (true, "not_allowed_local".to_string())
        );
    }

    #[tokio::test]
    async fn host_blocked_rejects_loopback_when_allowlist_is_wildcard() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["*".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("127.0.0.1", 80).await.unwrap(),
            (true, "not_allowed_local".to_string())
        );
    }

    #[tokio::test]
    async fn host_blocked_rejects_private_ip_literal_when_allowlist_is_wildcard() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["*".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("10.0.0.1", 80).await.unwrap(),
            (true, "not_allowed_local".to_string())
        );
    }

    #[tokio::test]
    async fn host_blocked_allows_loopback_when_explicitly_allowlisted_and_local_binding_disabled() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["localhost".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("localhost", 80).await.unwrap(),
            (false, String::new())
        );
    }

    #[tokio::test]
    async fn host_blocked_allows_private_ip_literal_when_explicitly_allowlisted() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["10.0.0.1".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("10.0.0.1", 80).await.unwrap(),
            (false, String::new())
        );
    }

    #[tokio::test]
    async fn host_blocked_rejects_scoped_ipv6_literal_when_not_allowlisted() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("fe80::1%lo0", 80).await.unwrap(),
            (true, "not_allowed_local".to_string())
        );
    }

    #[tokio::test]
    async fn host_blocked_allows_scoped_ipv6_literal_when_explicitly_allowlisted() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["fe80::1%lo0".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("fe80::1%lo0", 80).await.unwrap(),
            (false, String::new())
        );
    }

    #[tokio::test]
    async fn host_blocked_rejects_private_ip_literals_when_local_binding_disabled() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("10.0.0.1", 80).await.unwrap(),
            (true, "not_allowed_local".to_string())
        );
    }

    #[tokio::test]
    async fn host_blocked_rejects_loopback_when_allowlist_empty() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec![],
            allow_local_binding: false,
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("127.0.0.1", 80).await.unwrap(),
            (true, "not_allowed_local".to_string())
        );
    }

    #[test]
    fn validate_policy_against_constraints_disallows_widening_allowed_domains() {
        let constraints = NetworkProxyConstraints {
            allowed_domains: Some(vec!["example.com".to_string()]),
            ..NetworkProxyConstraints::default()
        };

        let config = Config {
            network_proxy: NetworkProxyConfig {
                enabled: true,
                policy: NetworkPolicy {
                    allowed_domains: vec!["example.com".to_string(), "evil.com".to_string()],
                    ..NetworkPolicy::default()
                },
                ..NetworkProxyConfig::default()
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

        let config = Config {
            network_proxy: NetworkProxyConfig {
                enabled: true,
                mode: NetworkMode::Full,
                ..NetworkProxyConfig::default()
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

        let config = Config {
            network_proxy: NetworkProxyConfig {
                enabled: true,
                policy: NetworkPolicy {
                    allowed_domains: vec!["api.example.com".to_string()],
                    ..NetworkPolicy::default()
                },
                ..NetworkProxyConfig::default()
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

        let config = Config {
            network_proxy: NetworkProxyConfig {
                enabled: true,
                policy: NetworkPolicy {
                    allowed_domains: vec!["**.example.com".to_string()],
                    ..NetworkPolicy::default()
                },
                ..NetworkProxyConfig::default()
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

        let config = Config {
            network_proxy: NetworkProxyConfig {
                enabled: true,
                policy: NetworkPolicy {
                    denied_domains: vec![],
                    ..NetworkPolicy::default()
                },
                ..NetworkProxyConfig::default()
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

        let config = Config {
            network_proxy: NetworkProxyConfig {
                enabled: true,
                ..NetworkProxyConfig::default()
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

        let config = Config {
            network_proxy: NetworkProxyConfig {
                enabled: true,
                policy: NetworkPolicy {
                    allow_local_binding: true,
                    ..NetworkPolicy::default()
                },
                ..NetworkProxyConfig::default()
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

        let config = Config {
            network_proxy: NetworkProxyConfig {
                enabled: true,
                dangerously_allow_non_loopback_admin: true,
                ..NetworkProxyConfig::default()
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

        let config = Config {
            network_proxy: NetworkProxyConfig {
                enabled: true,
                dangerously_allow_non_loopback_admin: true,
                ..NetworkProxyConfig::default()
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
        let state = app_state_for_policy(NetworkPolicy {
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

        let unique = OffsetDateTime::now_utc().unix_timestamp_nanos();
        let dir = std::env::temp_dir().join(format!("codex-network-proxy-test-{unique}"));
        std::fs::create_dir_all(&dir).unwrap();

        let real = dir.join("real.sock");
        let link = dir.join("link.sock");

        // The allowlist mechanism is path-based; for test purposes we don't need an actual unix
        // domain socket. Any filesystem entry works for canonicalization.
        std::fs::write(&real, b"not a socket").unwrap();
        symlink(&real, &link).unwrap();

        let real_s = real.to_str().unwrap().to_string();
        let link_s = link.to_str().unwrap().to_string();

        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            allow_unix_sockets: vec![real_s],
            ..NetworkPolicy::default()
        });

        assert!(state.is_unix_socket_allowed(&link_s).await.unwrap());

        let _ = std::fs::remove_file(&link);
        let _ = std::fs::remove_file(&real);
        let _ = std::fs::remove_dir(&dir);
    }

    #[cfg(not(target_os = "macos"))]
    #[tokio::test]
    async fn unix_socket_allowlist_is_rejected_on_non_macos() {
        let socket_path = "/tmp/example.sock".to_string();
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            allow_unix_sockets: vec![socket_path.clone()],
            ..NetworkPolicy::default()
        });

        assert!(!state.is_unix_socket_allowed(&socket_path).await.unwrap());
    }
}
