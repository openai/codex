use crate::config::Config;
use crate::config::MitmConfig;
use crate::config::NetworkMode;
use crate::mitm::MitmState;
use crate::policy::is_loopback_host;
use crate::policy::method_allowed;
use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use globset::GlobBuilder;
use globset::GlobSet;
use globset::GlobSetBuilder;
use serde::Serialize;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
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
    cfg: Config,
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
    pub async fn new(cfg_path: PathBuf) -> Result<Self> {
        let cfg_state = build_config_state(cfg_path)?;
        Ok(Self {
            state: Arc::new(RwLock::new(cfg_state)),
        })
    }

    pub async fn current_cfg(&self) -> Result<Config> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok(guard.cfg.clone())
    }

    pub async fn current_patterns(&self) -> Result<(Vec<String>, Vec<String>)> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok((
            guard.cfg.network_proxy.policy.allowed_domains.clone(),
            guard.cfg.network_proxy.policy.denied_domains.clone(),
        ))
    }

    pub async fn force_reload(&self) -> Result<()> {
        let mut guard = self.state.write().await;
        let previous_cfg = guard.cfg.clone();
        let blocked = guard.blocked.clone();
        let cfg_path = guard.cfg_path.clone();
        match build_config_state(cfg_path.clone()) {
            Ok(mut new_state) => {
                log_policy_changes(&previous_cfg, &new_state.cfg);
                new_state.blocked = blocked;
                *guard = new_state;
                info!(path = %cfg_path.display(), "reloaded config");
                Ok(())
            }
            Err(err) => {
                warn!(error = %err, path = %cfg_path.display(), "failed to reload config; keeping previous config");
                Err(err)
            }
        }
    }

    pub async fn host_blocked(&self, host: &str) -> Result<(bool, String)> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        if guard.deny_set.is_match(host) {
            return Ok((true, "denied".to_string()));
        }
        let is_loopback = is_loopback_host(host);
        if is_loopback
            && !guard.cfg.network_proxy.policy.allow_local_binding
            && !guard.allow_set.is_match(host)
        {
            return Ok((true, "not_allowed_local".to_string()));
        }
        if guard.cfg.network_proxy.policy.allowed_domains.is_empty()
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
            .cfg
            .network_proxy
            .policy
            .allow_unix_sockets
            .iter()
            .any(|p| p == path))
    }

    pub async fn method_allowed(&self, method: &str) -> Result<bool> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok(method_allowed(guard.cfg.network_proxy.mode, method))
    }

    pub async fn network_mode(&self) -> Result<NetworkMode> {
        self.reload_if_needed().await?;
        let guard = self.state.read().await;
        Ok(guard.cfg.network_proxy.mode)
    }

    pub async fn set_network_mode(&self, mode: NetworkMode) -> Result<()> {
        self.reload_if_needed().await?;
        let mut guard = self.state.write().await;
        guard.cfg.network_proxy.mode = mode;
        info!(mode = ?mode, "updated network mode");
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
                true
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

fn build_config_state(cfg_path: PathBuf) -> Result<ConfigState> {
    let mut cfg = if cfg_path.exists() {
        load_config_from_path(&cfg_path).with_context(|| {
            format!(
                "failed to load config from {}",
                cfg_path.as_path().display()
            )
        })?
    } else {
        Config::default()
    };
    resolve_mitm_paths(&mut cfg, &cfg_path);
    let mtime = cfg_path.metadata().and_then(|m| m.modified()).ok();
    let deny_set = compile_globset(&cfg.network_proxy.policy.denied_domains)?;
    let allow_set = compile_globset(&cfg.network_proxy.policy.allowed_domains)?;
    let mitm = if cfg.network_proxy.mitm.enabled {
        build_mitm_state(&cfg.network_proxy.mitm)?
    } else {
        None
    };
    Ok(ConfigState {
        cfg,
        mtime,
        allow_set,
        deny_set,
        mitm,
        cfg_path,
        blocked: VecDeque::new(),
    })
}

fn resolve_mitm_paths(cfg: &mut Config, cfg_path: &Path) {
    let base = cfg_path.parent().unwrap_or_else(|| Path::new("."));
    if cfg.network_proxy.mitm.ca_cert_path.is_relative() {
        cfg.network_proxy.mitm.ca_cert_path = base.join(&cfg.network_proxy.mitm.ca_cert_path);
    }
    if cfg.network_proxy.mitm.ca_key_path.is_relative() {
        cfg.network_proxy.mitm.ca_key_path = base.join(&cfg.network_proxy.mitm.ca_key_path);
    }
}

fn build_mitm_state(_cfg: &MitmConfig) -> Result<Option<Arc<MitmState>>> {
    #[cfg(feature = "mitm")]
    {
        return Ok(Some(Arc::new(MitmState::new(_cfg)?)));
    }
    #[cfg(not(feature = "mitm"))]
    {
        warn!("MITM enabled in config but binary built without mitm feature");
        Ok(None)
    }
}

fn compile_globset(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    let mut seen = HashSet::new();
    for pattern in patterns {
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
            info!(list = list_name, entry = %entry, "config entry added");
        }
    }

    let mut seen_previous = HashSet::new();
    for entry in previous {
        let key = entry.to_ascii_lowercase();
        if seen_previous.insert(key.clone()) && !next_set.contains(&key) {
            info!(list = list_name, entry = %entry, "config entry removed");
        }
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn load_config_from_path(path: &Path) -> Result<Config> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("unable to read config file {}", path.display()))?;
    toml::from_str(&raw).map_err(|err| anyhow!("unable to parse config: {err}"))
}
