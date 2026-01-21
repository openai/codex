use crate::config::Config;
use crate::config::MitmConfig;
use crate::config::NetworkMode;
use crate::mitm::MitmState;
use crate::policy::DomainPattern;
use crate::policy::compile_globset;
use crate::runtime::ConfigState;
use anyhow::Context;
use anyhow::Result;
use codex_app_server_protocol::ConfigLayerSource;
use codex_core::config::CONFIG_TOML_FILE;
use codex_core::config::ConfigBuilder;
use codex_core::config::Constrained;
use codex_core::config::ConstraintError;
use codex_core::config_loader::RequirementSource;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

pub use crate::runtime::AppState;
pub use crate::runtime::BlockedRequest;
#[cfg(test)]
pub(crate) use crate::runtime::app_state_for_policy;

pub(crate) async fn build_config_state() -> Result<ConfigState> {
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
    let constraints = enforce_trusted_constraints(&codex_cfg.config_layer_stack, &config)?;

    // Permit relative MITM paths for ergonomics; resolve them relative to CODEX_HOME so the
    // proxy can be configured from multiple config locations without changing cert paths.
    resolve_mitm_paths(&mut config, &codex_cfg.codex_home);
    let mtime = cfg_path.metadata().and_then(|m| m.modified()).ok();
    let deny_set = compile_globset(&config.network_proxy.policy.denied_domains)?;
    let allow_set = compile_globset(&config.network_proxy.policy.allowed_domains)?;
    let mitm = if config.network_proxy.mitm.enabled {
        build_mitm_state(
            &config.network_proxy.mitm,
            config.network_proxy.allow_upstream_proxy,
        )?
    } else {
        None
    };
    Ok(ConfigState {
        config,
        mtime,
        allow_set,
        deny_set,
        mitm,
        constraints,
        cfg_path,
        blocked: std::collections::VecDeque::new(),
    })
}

fn resolve_mitm_paths(config: &mut Config, codex_home: &Path) {
    let base = codex_home;
    if config.network_proxy.mitm.ca_cert_path.is_relative() {
        config.network_proxy.mitm.ca_cert_path = base.join(&config.network_proxy.mitm.ca_cert_path);
    }
    if config.network_proxy.mitm.ca_key_path.is_relative() {
        config.network_proxy.mitm.ca_key_path = base.join(&config.network_proxy.mitm.ca_key_path);
    }
}

fn build_mitm_state(
    config: &MitmConfig,
    allow_upstream_proxy: bool,
) -> Result<Option<Arc<MitmState>>> {
    Ok(Some(Arc::new(MitmState::new(
        config,
        allow_upstream_proxy,
    )?)))
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
    allow_upstream_proxy: Option<bool>,
    dangerously_allow_non_loopback_proxy: Option<bool>,
    dangerously_allow_non_loopback_admin: Option<bool>,
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

#[derive(Debug, Default, Clone)]
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
    layers: &codex_core::config_loader::ConfigLayerStack,
    config: &Config,
) -> Result<NetworkProxyConstraints> {
    let constraints = network_proxy_constraints_from_trusted_layers(layers)?;
    validate_policy_against_constraints(config, &constraints)
        .context("network proxy constraints")?;
    Ok(constraints)
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
        if let Some(allow_upstream_proxy) = partial.network_proxy.allow_upstream_proxy {
            constraints.allow_upstream_proxy = Some(allow_upstream_proxy);
        }
        if let Some(dangerously_allow_non_loopback_proxy) =
            partial.network_proxy.dangerously_allow_non_loopback_proxy
        {
            constraints.dangerously_allow_non_loopback_proxy =
                Some(dangerously_allow_non_loopback_proxy);
        }
        if let Some(dangerously_allow_non_loopback_admin) =
            partial.network_proxy.dangerously_allow_non_loopback_admin
        {
            constraints.dangerously_allow_non_loopback_admin =
                Some(dangerously_allow_non_loopback_admin);
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

pub(crate) fn validate_policy_against_constraints(
    config: &Config,
    constraints: &NetworkProxyConstraints,
) -> std::result::Result<(), ConstraintError> {
    fn invalid_value(
        field_name: &'static str,
        candidate: impl Into<String>,
        allowed: impl Into<String>,
    ) -> ConstraintError {
        ConstraintError::InvalidValue {
            field_name,
            candidate: candidate.into(),
            allowed: allowed.into(),
            requirement_source: RequirementSource::Unknown,
        }
    }

    let enabled = config.network_proxy.enabled;
    if let Some(max_enabled) = constraints.enabled {
        let _ = Constrained::new(enabled, move |candidate| {
            if *candidate && !max_enabled {
                Err(invalid_value(
                    "network_proxy.enabled",
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
                Err(invalid_value(
                    "network_proxy.mode",
                    format!("{candidate:?}"),
                    format!("{max_mode:?} or more restrictive"),
                ))
            } else {
                Ok(())
            }
        })?;
    }

    let allow_upstream_proxy = constraints.allow_upstream_proxy;
    let _ = Constrained::new(
        config.network_proxy.allow_upstream_proxy,
        move |candidate| match allow_upstream_proxy {
            Some(true) | None => Ok(()),
            Some(false) => {
                if *candidate {
                    Err(invalid_value(
                        "network_proxy.allow_upstream_proxy",
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
    let _ = Constrained::new(
        config.network_proxy.dangerously_allow_non_loopback_admin,
        move |candidate| match allow_non_loopback_admin {
            Some(true) | None => Ok(()),
            Some(false) => {
                if *candidate {
                    Err(invalid_value(
                        "network_proxy.dangerously_allow_non_loopback_admin",
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
    let _ = Constrained::new(
        config.network_proxy.dangerously_allow_non_loopback_proxy,
        move |candidate| match allow_non_loopback_proxy {
            Some(true) | None => Ok(()),
            Some(false) => {
                if *candidate {
                    Err(invalid_value(
                        "network_proxy.dangerously_allow_non_loopback_proxy",
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
        let _ = Constrained::new(
            config.network_proxy.policy.allow_local_binding,
            move |candidate| {
                if *candidate && !allow_local_binding {
                    Err(invalid_value(
                        "network_proxy.policy.allow_local_binding",
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
        let managed_patterns: Vec<DomainPattern> = allowed_domains
            .iter()
            .map(|entry| DomainPattern::parse(entry))
            .collect();
        let _ = Constrained::new(
            config.network_proxy.policy.allowed_domains.clone(),
            move |candidate| {
                let mut invalid = Vec::new();
                for entry in candidate {
                    let candidate_pattern = DomainPattern::parse(entry);
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
                        "network_proxy.policy.allowed_domains",
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
                    Err(invalid_value(
                        "network_proxy.policy.denied_domains",
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
                    Err(invalid_value(
                        "network_proxy.policy.allow_unix_sockets",
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
