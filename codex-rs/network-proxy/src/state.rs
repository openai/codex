use crate::config::NetworkMode;
use crate::config::NetworkProxyConfig;
use crate::policy::DomainPattern;
use crate::policy::compile_globset;
use crate::runtime::ConfigState;
use serde::Deserialize;
use std::collections::HashSet;

pub use crate::runtime::BlockedRequest;
pub use crate::runtime::BlockedRequestArgs;
pub use crate::runtime::NetworkProxyState;
#[cfg(test)]
pub(crate) use crate::runtime::network_proxy_state_for_policy;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NetworkProxyConstraints {
    pub enabled: Option<bool>,
    pub mode: Option<NetworkMode>,
    pub allow_upstream_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_admin: Option<bool>,
    pub dangerously_allow_all_unix_sockets: Option<bool>,
    pub allowed_domains: Option<Vec<String>>,
    pub denied_domains: Option<Vec<String>>,
    pub allow_unix_sockets: Option<Vec<String>>,
    pub allow_local_binding: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PartialNetworkProxyConfig {
    #[serde(default)]
    pub network: PartialNetworkConfig,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct PartialNetworkConfig {
    pub enabled: Option<bool>,
    pub mode: Option<NetworkMode>,
    pub allow_upstream_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_admin: Option<bool>,
    pub dangerously_allow_all_unix_sockets: Option<bool>,
    #[serde(default)]
    pub allowed_domains: Option<Vec<String>>,
    #[serde(default)]
    pub denied_domains: Option<Vec<String>>,
    #[serde(default)]
    pub allow_unix_sockets: Option<Vec<String>>,
    #[serde(default)]
    pub allow_local_binding: Option<bool>,
}

pub fn build_config_state(
    config: NetworkProxyConfig,
    constraints: NetworkProxyConstraints,
) -> anyhow::Result<ConfigState> {
    crate::config::validate_unix_socket_allowlist_paths(&config)?;
    let deny_set = compile_globset(&config.network.denied_domains)?;
    let allow_set = compile_globset(&config.network.allowed_domains)?;
    Ok(ConfigState {
        config,
        allow_set,
        deny_set,
        constraints,
        blocked: std::collections::VecDeque::new(),
        blocked_total: 0,
    })
}

pub fn validate_policy_against_constraints(
    config: &NetworkProxyConfig,
    constraints: &NetworkProxyConstraints,
) -> Result<(), NetworkProxyConstraintError> {
    fn invalid_value(
        field_name: &'static str,
        candidate: impl Into<String>,
        allowed: impl Into<String>,
    ) -> NetworkProxyConstraintError {
        NetworkProxyConstraintError::InvalidValue {
            field_name,
            candidate: candidate.into(),
            allowed: allowed.into(),
        }
    }

    fn validate<T>(
        candidate: T,
        validator: impl FnOnce(&T) -> Result<(), NetworkProxyConstraintError>,
    ) -> Result<(), NetworkProxyConstraintError> {
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

    let allow_all_unix_sockets = constraints
        .dangerously_allow_all_unix_sockets
        .unwrap_or(constraints.allow_unix_sockets.is_none());
    validate(
        config.network.dangerously_allow_all_unix_sockets,
        move |candidate| {
            if *candidate && !allow_all_unix_sockets {
                Err(invalid_value(
                    "network.dangerously_allow_all_unix_sockets",
                    "true",
                    "false (disabled by managed config)",
                ))
            } else {
                Ok(())
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

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetworkProxyConstraintError {
    #[error("invalid value for {field_name}: {candidate} (allowed {allowed})")]
    InvalidValue {
        field_name: &'static str,
        candidate: String,
        allowed: String,
    },
}

impl NetworkProxyConstraintError {
    pub fn into_anyhow(self) -> anyhow::Error {
        anyhow::anyhow!(self)
    }
}

fn network_mode_rank(mode: NetworkMode) -> u8 {
    match mode {
        NetworkMode::Limited => 0,
        NetworkMode::Full => 1,
    }
}
