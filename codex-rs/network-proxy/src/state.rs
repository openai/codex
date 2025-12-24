use crate::config::Config;
use crate::config::MitmConfig;
use crate::config::NetworkMode;
use crate::mitm::MitmState;
use crate::policy::is_loopback_host;
use crate::policy::method_allowed;
use anyhow::Context;
use anyhow::Result;
use codex_app_server_protocol::ConfigLayerSource;
use codex_core::config::CONFIG_TOML_FILE;
use codex_core::config::ConfigBuilder;
use codex_core::config::Constrained;
use codex_core::config::ConstraintError;
use globset::GlobBuilder;
use globset::GlobSet;
use globset::GlobSetBuilder;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use time::OffsetDateTime;
use tokio::sync::RwLock;
use tracing::info;
use tracing::warn;

const MAX_BLOCKED_EVENTS: usize = 200;

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
struct ConfigState {
    config: Config,
    mtime: Option<SystemTime>,
    allow_set: GlobSet,
    deny_set: GlobSet,
    mitm: Option<Arc<MitmState>>,
    cfg_path: PathBuf,
    blocked: VecDeque<BlockedRequest>,
}

#[derive(Clone)]
pub struct AppState {
    state: Arc<RwLock<ConfigState>>,
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

    pub async fn host_blocked(&self, host: &str) -> Result<(bool, String)> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        // Decision order matters:
        //  1) explicit deny always wins
        //  2) local/loopback is opt-in (defense-in-depth)
        //  3) allowlist is enforced when configured
        if guard.deny_set.is_match(host) {
            return Ok((true, "denied".to_string()));
        }
        let is_loopback = is_loopback_host(host);
        if is_loopback
            && !guard.config.network_proxy.policy.allow_local_binding
            && !guard.allow_set.is_match(host)
        {
            return Ok((true, "not_allowed_local".to_string()));
        }
        if guard.config.network_proxy.policy.allowed_domains.is_empty()
            || !guard.allow_set.is_match(host)
        {
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
        let guard = self.state.read().await;
        Ok(guard
            .config
            .network_proxy
            .policy
            .allow_unix_sockets
            .iter()
            .any(|p| p == path))
    }

    pub async fn method_allowed(&self, method: &str) -> Result<bool> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok(method_allowed(guard.config.network_proxy.mode, method))
    }

    pub async fn network_mode(&self) -> Result<NetworkMode> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok(guard.config.network_proxy.mode)
    }

    pub async fn set_network_mode(&self, mode: NetworkMode) -> Result<()> {
        self.reload_if_needed().await?;
        let mut guard = self.state.write().await;
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

async fn build_config_state() -> Result<ConfigState> {
    // Load config through `codex-core` so we inherit the same layer ordering and semantics as the
    // rest of Codex (system/managed layers, user layers, session flags, etc.).
    let codex_cfg = ConfigBuilder::default()
        .build()
        .await
        .context("failed to load Codex config")?;

    let cfg_path = codex_cfg.codex_home.join(CONFIG_TOML_FILE);

    // Deserialize from the merged effective config, rather than parsing config.toml ourselves.
    // This avoids a second parser/merger implementation (and the drift that comes with it).
    let merged_toml = codex_cfg.config_layer_stack.effective_config();
    let mut config: Config = merged_toml
        .try_into()
        .context("failed to deserialize network proxy config")?;

    // Security boundary: user-controlled layers must not be able to widen restrictions set by
    // trusted/managed layers (e.g., MDM). Enforce this before building runtime state.
    enforce_trusted_constraints(&codex_cfg.config_layer_stack, &config)?;

    // Permit relative MITM paths for ergonomics; resolve them relative to the directory containing
    // `config.toml` so the config is relocatable.
    resolve_mitm_paths(&mut config, &cfg_path);
    let mtime = cfg_path.metadata().and_then(|m| m.modified()).ok();
    let deny_set = compile_globset(&config.network_proxy.policy.denied_domains)?;
    let allow_set = compile_globset(&config.network_proxy.policy.allowed_domains)?;
    let mitm = if config.network_proxy.mitm.enabled {
        build_mitm_state(&config.network_proxy.mitm)?
    } else {
        None
    };
    Ok(ConfigState {
        config,
        mtime,
        allow_set,
        deny_set,
        mitm,
        cfg_path,
        blocked: VecDeque::new(),
    })
}

fn resolve_mitm_paths(config: &mut Config, cfg_path: &Path) {
    let base = cfg_path.parent().unwrap_or_else(|| Path::new("."));
    if config.network_proxy.mitm.ca_cert_path.is_relative() {
        config.network_proxy.mitm.ca_cert_path = base.join(&config.network_proxy.mitm.ca_cert_path);
    }
    if config.network_proxy.mitm.ca_key_path.is_relative() {
        config.network_proxy.mitm.ca_key_path = base.join(&config.network_proxy.mitm.ca_key_path);
    }
}

fn build_mitm_state(config: &MitmConfig) -> Result<Option<Arc<MitmState>>> {
    Ok(Some(Arc::new(MitmState::new(config)?)))
}

#[derive(Debug, Default, Deserialize)]
struct PartialConfig {
    #[serde(default)]
    network_proxy: PartialNetworkProxyConfig,
}

#[derive(Debug, Default, Deserialize)]
struct PartialNetworkProxyConfig {
    enabled: Option<bool>,
    mode: Option<NetworkMode>,
    #[serde(default)]
    policy: PartialNetworkPolicy,
}

#[derive(Debug, Default, Deserialize)]
struct PartialNetworkPolicy {
    #[serde(default)]
    allowed_domains: Option<Vec<String>>,
    #[serde(default)]
    denied_domains: Option<Vec<String>>,
    #[serde(default)]
    allow_unix_sockets: Option<Vec<String>>,
    #[serde(default)]
    allow_local_binding: Option<bool>,
}

#[derive(Debug, Default)]
struct NetworkProxyConstraints {
    enabled: Option<bool>,
    mode: Option<NetworkMode>,
    allowed_domains: Option<Vec<String>>,
    denied_domains: Option<Vec<String>>,
    allow_unix_sockets: Option<Vec<String>>,
    allow_local_binding: Option<bool>,
}

fn enforce_trusted_constraints(
    layers: &codex_core::config_loader::ConfigLayerStack,
    config: &Config,
) -> Result<()> {
    let constraints = network_proxy_constraints_from_trusted_layers(layers)?;
    validate_policy_against_constraints(config, &constraints)
        .context("network proxy constraints")?;
    Ok(())
}

fn network_proxy_constraints_from_trusted_layers(
    layers: &codex_core::config_loader::ConfigLayerStack,
) -> Result<NetworkProxyConstraints> {
    let mut constraints = NetworkProxyConstraints::default();
    for layer in layers
        .get_layers(codex_core::config_loader::ConfigLayerStackOrdering::LowestPrecedenceFirst)
    {
        // Only trusted layers contribute constraints. User-controlled layers can narrow policy but
        // must never widen beyond what managed config allows.
        if is_user_controlled_layer(&layer.name) {
            continue;
        }

        let partial: PartialConfig = layer
            .config
            .clone()
            .try_into()
            .context("failed to deserialize trusted config layer")?;

        if let Some(enabled) = partial.network_proxy.enabled {
            constraints.enabled = Some(enabled);
        }
        if let Some(mode) = partial.network_proxy.mode {
            constraints.mode = Some(mode);
        }

        if let Some(allowed_domains) = partial.network_proxy.policy.allowed_domains {
            constraints.allowed_domains = Some(allowed_domains);
        }
        if let Some(denied_domains) = partial.network_proxy.policy.denied_domains {
            constraints.denied_domains = Some(denied_domains);
        }
        if let Some(allow_unix_sockets) = partial.network_proxy.policy.allow_unix_sockets {
            constraints.allow_unix_sockets = Some(allow_unix_sockets);
        }
        if let Some(allow_local_binding) = partial.network_proxy.policy.allow_local_binding {
            constraints.allow_local_binding = Some(allow_local_binding);
        }
    }
    Ok(constraints)
}

fn is_user_controlled_layer(layer: &ConfigLayerSource) -> bool {
    matches!(
        layer,
        ConfigLayerSource::User { .. }
            | ConfigLayerSource::Project { .. }
            | ConfigLayerSource::SessionFlags
    )
}

fn validate_policy_against_constraints(
    config: &Config,
    constraints: &NetworkProxyConstraints,
) -> std::result::Result<(), ConstraintError> {
    let enabled = config.network_proxy.enabled;
    if let Some(max_enabled) = constraints.enabled {
        let _ = Constrained::new(enabled, move |candidate| {
            if *candidate && !max_enabled {
                Err(ConstraintError::invalid_value(
                    "true",
                    "false (disabled by managed config)",
                ))
            } else {
                Ok(())
            }
        })?;
    }

    if let Some(max_mode) = constraints.mode {
        let _ = Constrained::new(config.network_proxy.mode, move |candidate| {
            if network_mode_rank(*candidate) > network_mode_rank(max_mode) {
                Err(ConstraintError::invalid_value(
                    format!("{candidate:?}"),
                    format!("{max_mode:?} or more restrictive"),
                ))
            } else {
                Ok(())
            }
        })?;
    }

    if let Some(allow_local_binding) = constraints.allow_local_binding {
        let _ = Constrained::new(
            config.network_proxy.policy.allow_local_binding,
            move |candidate| {
                if *candidate && !allow_local_binding {
                    Err(ConstraintError::invalid_value(
                        "true",
                        "false (disabled by managed config)",
                    ))
                } else {
                    Ok(())
                }
            },
        )?;
    }

    if let Some(allowed_domains) = &constraints.allowed_domains {
        let allowed_set: HashSet<String> = allowed_domains
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        let _ = Constrained::new(
            config.network_proxy.policy.allowed_domains.clone(),
            move |candidate| {
                let mut invalid = Vec::new();
                for entry in candidate {
                    if !allowed_set.contains(&entry.to_ascii_lowercase()) {
                        invalid.push(entry.clone());
                    }
                }
                if invalid.is_empty() {
                    Ok(())
                } else {
                    Err(ConstraintError::invalid_value(
                        format!("{invalid:?}"),
                        "subset of managed allowed_domains",
                    ))
                }
            },
        )?;
    }

    if let Some(denied_domains) = &constraints.denied_domains {
        let required_set: HashSet<String> = denied_domains
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        let _ = Constrained::new(
            config.network_proxy.policy.denied_domains.clone(),
            move |candidate| {
                let candidate_set: HashSet<String> =
                    candidate.iter().map(|s| s.to_ascii_lowercase()).collect();
                let missing: Vec<String> = required_set
                    .iter()
                    .filter(|entry| !candidate_set.contains(*entry))
                    .cloned()
                    .collect();
                if missing.is_empty() {
                    Ok(())
                } else {
                    Err(ConstraintError::invalid_value(
                        "missing managed denied_domains entries",
                        format!("{missing:?}"),
                    ))
                }
            },
        )?;
    }

    if let Some(allow_unix_sockets) = &constraints.allow_unix_sockets {
        let allowed_set: HashSet<String> = allow_unix_sockets
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        let _ = Constrained::new(
            config.network_proxy.policy.allow_unix_sockets.clone(),
            move |candidate| {
                let mut invalid = Vec::new();
                for entry in candidate {
                    if !allowed_set.contains(&entry.to_ascii_lowercase()) {
                        invalid.push(entry.clone());
                    }
                }
                if invalid.is_empty() {
                    Ok(())
                } else {
                    Err(ConstraintError::invalid_value(
                        format!("{invalid:?}"),
                        "subset of managed allow_unix_sockets",
                    ))
                }
            },
        )?;
    }

    Ok(())
}

fn network_mode_rank(mode: NetworkMode) -> u8 {
    match mode {
        NetworkMode::Limited => 0,
        NetworkMode::Full => 1,
    }
}

fn compile_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let mut seen = HashSet::new();
    for pattern in patterns {
        // Operator ergonomics: `*.example.com` usually intends to include both `a.example.com` and
        // the apex `example.com`. We expand that here so policy matches expectation.
        let mut expanded = Vec::with_capacity(2);
        expanded.push(pattern.as_str());
        if let Some(apex) = pattern.strip_prefix("*.") {
            expanded.push(apex);
        }
        for candidate in expanded {
            if !seen.insert(candidate.to_string()) {
                continue;
            }
            let glob = GlobBuilder::new(candidate)
                .case_insensitive(true)
                .build()
                .with_context(|| format!("invalid glob pattern: {candidate}"))?;
            builder.add(glob);
        }
    }
    Ok(builder.build()?)
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

fn unix_timestamp() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::NetworkPolicy;
    use crate::config::NetworkProxyConfig;
    use pretty_assertions::assert_eq;

    fn app_state_for_policy(policy: NetworkPolicy) -> AppState {
        let config = Config {
            network_proxy: NetworkProxyConfig {
                enabled: true,
                mode: NetworkMode::Full,
                policy,
                ..NetworkProxyConfig::default()
            },
        };

        let allow_set = compile_globset(&config.network_proxy.policy.allowed_domains).unwrap();
        let deny_set = compile_globset(&config.network_proxy.policy.denied_domains).unwrap();

        let state = ConfigState {
            config,
            mtime: None,
            allow_set,
            deny_set,
            mitm: None,
            cfg_path: PathBuf::from("/nonexistent/config.toml"),
            blocked: VecDeque::new(),
        };

        AppState {
            state: Arc::new(RwLock::new(state)),
        }
    }

    #[tokio::test]
    async fn host_blocked_denied_wins_over_allowed() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["example.com".to_string()],
            denied_domains: vec!["example.com".to_string()],
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("example.com").await.unwrap(),
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
            state.host_blocked("example.com").await.unwrap(),
            (false, String::new())
        );
        assert_eq!(
            state.host_blocked("not-example.com").await.unwrap(),
            (true, "not_allowed".to_string())
        );
    }

    #[tokio::test]
    async fn host_blocked_expands_apex_for_wildcard_patterns() {
        let state = app_state_for_policy(NetworkPolicy {
            allowed_domains: vec!["*.openai.com".to_string()],
            ..NetworkPolicy::default()
        });

        assert_eq!(
            state.host_blocked("openai.com").await.unwrap(),
            (false, String::new())
        );
        assert_eq!(
            state.host_blocked("api.openai.com").await.unwrap(),
            (false, String::new())
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
            state.host_blocked("127.0.0.1").await.unwrap(),
            (true, "not_allowed_local".to_string())
        );
        assert_eq!(
            state.host_blocked("localhost").await.unwrap(),
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
            state.host_blocked("localhost").await.unwrap(),
            (false, String::new())
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
    fn compile_globset_is_case_insensitive() {
        let patterns = vec!["ExAmPle.CoM".to_string()];
        let set = compile_globset(&patterns).unwrap();
        assert!(set.is_match("example.com"));
        assert!(set.is_match("EXAMPLE.COM"));
    }

    #[test]
    fn compile_globset_expands_apex_for_wildcard_patterns() {
        let patterns = vec!["*.openai.com".to_string()];
        let set = compile_globset(&patterns).unwrap();
        assert!(set.is_match("openai.com"));
        assert!(set.is_match("api.openai.com"));
        assert!(!set.is_match("evilopenai.com"));
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
