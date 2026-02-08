use crate::config::NetworkMode;
use crate::config::NetworkProxyConfig;
use crate::policy::DomainPattern;
use crate::policy::compile_globset;
use crate::runtime::ConfigState;
use crate::runtime::LayerMtime;
use anyhow::Context;
use anyhow::Result;
use codex_utils_home_dir::find_codex_home;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use tokio::fs;
use toml::Value as TomlValue;

const CONFIG_TOML_FILE: &str = "config.toml";

#[cfg(unix)]
const SYSTEM_CONFIG_TOML_FILE_UNIX: &str = "/etc/codex/config.toml";

pub use crate::runtime::BlockedRequest;
pub use crate::runtime::BlockedRequestArgs;
pub use crate::runtime::NetworkProxyState;
#[cfg(test)]
pub(crate) use crate::runtime::network_proxy_state_for_policy;

pub(crate) async fn build_config_state() -> Result<ConfigState> {
    let codex_home = find_codex_home().context("failed to resolve CODEX_HOME")?;
    let cfg_path = codex_home.join(CONFIG_TOML_FILE);

    let system_cfg_path = system_config_path();
    let system_config = read_toml_file_best_effort(&system_cfg_path)
        .await
        .context("failed to read system config")?;
    let user_config = read_toml_file_best_effort(&cfg_path)
        .await
        .context("failed to read user config")?;

    let mut merged_toml = TomlValue::Table(toml::map::Map::new());
    if let Some(system_config) = system_config.clone() {
        merge_toml_values(&mut merged_toml, system_config);
    }
    if let Some(user_config) = user_config {
        merge_toml_values(&mut merged_toml, user_config);
    }

    let config: NetworkProxyConfig = merged_toml
        .try_into()
        .context("failed to deserialize network proxy config")?;

    // Security boundary: user-controlled layers must not be able to widen restrictions set by
    // trusted/managed layers (e.g., MDM). Enforce this before building runtime state.
    let constraints = enforce_trusted_constraints(system_config.as_ref(), &config)?;

    let layer_mtimes = vec![
        LayerMtime::new(system_cfg_path),
        LayerMtime::new(cfg_path.clone()),
    ];
    let deny_set = compile_globset(&config.network.denied_domains)?;
    let allow_set = compile_globset(&config.network.allowed_domains)?;
    Ok(ConfigState {
        config,
        allow_set,
        deny_set,
        constraints,
        layer_mtimes,
        cfg_path,
        blocked: std::collections::VecDeque::new(),
    })
}

fn system_config_path() -> std::path::PathBuf {
    #[cfg(unix)]
    {
        Path::new(SYSTEM_CONFIG_TOML_FILE_UNIX).to_path_buf()
    }

    #[cfg(not(unix))]
    {
        // Use a dummy path on non-Unix platforms. This keeps the reload logic stable without
        // needing per-platform config stack logic yet.
        std::path::PathBuf::from("__no_system_config.toml")
    }
}

async fn read_toml_file_best_effort(path: &Path) -> Result<Option<TomlValue>> {
    let contents = match fs::read_to_string(path).await {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "permission denied reading config; ignoring"
            );
            return Ok(None);
        }
        Err(err) => return Err(err).context(format!("read {}", path.display())),
    };

    let parsed: TomlValue = contents
        .parse()
        .with_context(|| format!("parse {}", path.display()))?;
    Ok(Some(parsed))
}

fn merge_toml_values(base: &mut TomlValue, overlay: TomlValue) {
    match (base, overlay) {
        (TomlValue::Table(base_table), TomlValue::Table(overlay_table)) => {
            for (key, value) in overlay_table {
                match base_table.get_mut(&key) {
                    Some(existing) => merge_toml_values(existing, value),
                    None => {
                        base_table.insert(key, value);
                    }
                }
            }
        }
        (base_slot, overlay_value) => {
            *base_slot = overlay_value;
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct PartialConfig {
    #[serde(default)]
    network: PartialNetworkConfig,
}

#[derive(Debug, Default, Deserialize)]
struct PartialNetworkConfig {
    enabled: Option<bool>,
    mode: Option<NetworkMode>,
    allow_upstream_proxy: Option<bool>,
    dangerously_allow_non_loopback_proxy: Option<bool>,
    dangerously_allow_non_loopback_admin: Option<bool>,
    #[serde(default)]
    allowed_domains: Option<Vec<String>>,
    #[serde(default)]
    denied_domains: Option<Vec<String>>,
    #[serde(default)]
    allow_unix_sockets: Option<Vec<String>>,
    #[serde(default)]
    allow_local_binding: Option<bool>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct NetworkProxyConstraints {
    pub(crate) enabled: Option<bool>,
    pub(crate) mode: Option<NetworkMode>,
    pub(crate) allow_upstream_proxy: Option<bool>,
    pub(crate) dangerously_allow_non_loopback_proxy: Option<bool>,
    pub(crate) dangerously_allow_non_loopback_admin: Option<bool>,
    pub(crate) allowed_domains: Option<Vec<String>>,
    pub(crate) denied_domains: Option<Vec<String>>,
    pub(crate) allow_unix_sockets: Option<Vec<String>>,
    pub(crate) allow_local_binding: Option<bool>,
}

fn enforce_trusted_constraints(
    system_config: Option<&TomlValue>,
    config: &NetworkProxyConfig,
) -> Result<NetworkProxyConstraints> {
    let constraints = network_constraints_from_system_config(system_config)?;
    validate_policy_against_constraints(config, &constraints)
        .context("network proxy constraints")?;
    Ok(constraints)
}

fn network_constraints_from_system_config(
    system_config: Option<&TomlValue>,
) -> Result<NetworkProxyConstraints> {
    let mut constraints = NetworkProxyConstraints::default();
    let Some(system_config) = system_config else {
        return Ok(constraints);
    };

    let partial: PartialConfig = system_config
        .clone()
        .try_into()
        .context("failed to deserialize system config constraints")?;

    if let Some(enabled) = partial.network.enabled {
        constraints.enabled = Some(enabled);
    }
    if let Some(mode) = partial.network.mode {
        constraints.mode = Some(mode);
    }
    if let Some(allow_upstream_proxy) = partial.network.allow_upstream_proxy {
        constraints.allow_upstream_proxy = Some(allow_upstream_proxy);
    }
    if let Some(dangerously_allow_non_loopback_proxy) =
        partial.network.dangerously_allow_non_loopback_proxy
    {
        constraints.dangerously_allow_non_loopback_proxy =
            Some(dangerously_allow_non_loopback_proxy);
    }
    if let Some(dangerously_allow_non_loopback_admin) =
        partial.network.dangerously_allow_non_loopback_admin
    {
        constraints.dangerously_allow_non_loopback_admin =
            Some(dangerously_allow_non_loopback_admin);
    }

    if let Some(allowed_domains) = partial.network.allowed_domains {
        constraints.allowed_domains = Some(allowed_domains);
    }
    if let Some(denied_domains) = partial.network.denied_domains {
        constraints.denied_domains = Some(denied_domains);
    }
    if let Some(allow_unix_sockets) = partial.network.allow_unix_sockets {
        constraints.allow_unix_sockets = Some(allow_unix_sockets);
    }
    if let Some(allow_local_binding) = partial.network.allow_local_binding {
        constraints.allow_local_binding = Some(allow_local_binding);
    }

    Ok(constraints)
}

pub(crate) fn validate_policy_against_constraints(
    config: &NetworkProxyConfig,
    constraints: &NetworkProxyConstraints,
) -> Result<()> {
    fn invalid_value(
        field_name: &'static str,
        candidate: impl Into<String>,
        allowed: impl Into<String>,
    ) -> anyhow::Error {
        anyhow::anyhow!(
            "invalid value for {field_name}: candidate={} allowed={}",
            candidate.into(),
            allowed.into()
        )
    }

    fn validate<T>(candidate: T, validator: impl FnOnce(&T) -> Result<()>) -> Result<()> {
        validator(&candidate)
    }

    let enabled = config.network.enabled;
    if let Some(max_enabled) = constraints.enabled {
        validate(enabled, move |candidate| {
            if *candidate && !max_enabled {
                Err(invalid_value(
                    "network.enabled",
                    "true",
                    "false (disabled by managed config)",
                ))
            } else {
                Ok(())
            }
        })?;
    }

    if let Some(max_mode) = constraints.mode {
        validate(config.network.mode, move |candidate| {
            if network_mode_rank(*candidate) > network_mode_rank(max_mode) {
                Err(invalid_value(
                    "network.mode",
                    format!("{candidate:?}"),
                    format!("{max_mode:?} or more restrictive"),
                ))
            } else {
                Ok(())
            }
        })?;
    }

    let allow_upstream_proxy = constraints.allow_upstream_proxy;
    validate(
        config.network.allow_upstream_proxy,
        move |candidate| match allow_upstream_proxy {
            Some(true) | None => Ok(()),
            Some(false) => {
                if *candidate {
                    Err(invalid_value(
                        "network.allow_upstream_proxy",
                        "true",
                        "false (disabled by managed config)",
                    ))
                } else {
                    Ok(())
                }
            }
        },
    )?;

    let allow_non_loopback_admin = constraints.dangerously_allow_non_loopback_admin;
    validate(
        config.network.dangerously_allow_non_loopback_admin,
        move |candidate| match allow_non_loopback_admin {
            Some(true) | None => Ok(()),
            Some(false) => {
                if *candidate {
                    Err(invalid_value(
                        "network.dangerously_allow_non_loopback_admin",
                        "true",
                        "false (disabled by managed config)",
                    ))
                } else {
                    Ok(())
                }
            }
        },
    )?;

    let allow_non_loopback_proxy = constraints.dangerously_allow_non_loopback_proxy;
    validate(
        config.network.dangerously_allow_non_loopback_proxy,
        move |candidate| match allow_non_loopback_proxy {
            Some(true) | None => Ok(()),
            Some(false) => {
                if *candidate {
                    Err(invalid_value(
                        "network.dangerously_allow_non_loopback_proxy",
                        "true",
                        "false (disabled by managed config)",
                    ))
                } else {
                    Ok(())
                }
            }
        },
    )?;

    if let Some(allow_local_binding) = constraints.allow_local_binding {
        validate(config.network.allow_local_binding, move |candidate| {
            if *candidate && !allow_local_binding {
                Err(invalid_value(
                    "network.allow_local_binding",
                    "true",
                    "false (disabled by managed config)",
                ))
            } else {
                Ok(())
            }
        })?;
    }

    if let Some(allowed_domains) = &constraints.allowed_domains {
        let managed_patterns: Vec<DomainPattern> = allowed_domains
            .iter()
            .map(|entry| DomainPattern::parse_for_constraints(entry))
            .collect();
        validate(config.network.allowed_domains.clone(), move |candidate| {
            let mut invalid = Vec::new();
            for entry in candidate {
                let candidate_pattern = DomainPattern::parse_for_constraints(entry);
                if !managed_patterns
                    .iter()
                    .any(|managed| managed.allows(&candidate_pattern))
                {
                    invalid.push(entry.clone());
                }
            }
            if invalid.is_empty() {
                Ok(())
            } else {
                Err(invalid_value(
                    "network.allowed_domains",
                    format!("{invalid:?}"),
                    "subset of managed allowed_domains",
                ))
            }
        })?;
    }

    if let Some(denied_domains) = &constraints.denied_domains {
        let required_set: HashSet<String> = denied_domains
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        validate(config.network.denied_domains.clone(), move |candidate| {
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
                Err(invalid_value(
                    "network.denied_domains",
                    "missing managed denied_domains entries",
                    format!("{missing:?}"),
                ))
            }
        })?;
    }

    if let Some(allow_unix_sockets) = &constraints.allow_unix_sockets {
        let allowed_set: HashSet<String> = allow_unix_sockets
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        validate(
            config.network.allow_unix_sockets.clone(),
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
                    Err(invalid_value(
                        "network.allow_unix_sockets",
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
