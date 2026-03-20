use crate::config::NetworkDomainPermissions;
use crate::config::NetworkMode;
use crate::config::NetworkProxyConfig;
use crate::config::NetworkUnixSocketPermissions;
use crate::mitm::MitmState;
use crate::policy::DomainPattern;
use crate::policy::compile_allowlist_globset;
use crate::policy::compile_denylist_globset;
use crate::policy::is_global_wildcard_domain_pattern;
use crate::runtime::ConfigState;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;

pub use crate::runtime::BlockedRequest;
pub use crate::runtime::BlockedRequestArgs;
pub use crate::runtime::NetworkProxyAuditMetadata;
pub use crate::runtime::NetworkProxyState;
#[cfg(test)]
pub(crate) use crate::runtime::network_proxy_state_for_policy;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NetworkProxyConstraints {
    pub enabled: Option<bool>,
    pub mode: Option<NetworkMode>,
    pub allow_upstream_proxy: Option<bool>,
    pub dangerously_allow_non_loopback_proxy: Option<bool>,
    pub dangerously_allow_all_unix_sockets: Option<bool>,
    pub allowed_domains: Option<Vec<String>>,
    pub allowlist_expansion_enabled: Option<bool>,
    pub denied_domains: Option<Vec<String>>,
    pub denylist_expansion_enabled: Option<bool>,
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
    pub dangerously_allow_all_unix_sockets: Option<bool>,
    #[serde(default)]
    pub domains: Option<NetworkDomainPermissions>,
    #[serde(default)]
    pub unix_sockets: Option<NetworkUnixSocketPermissions>,
    pub allow_local_binding: Option<bool>,
}

pub fn build_config_state(
    config: NetworkProxyConfig,
    constraints: NetworkProxyConstraints,
) -> anyhow::Result<ConfigState> {
    crate::config::validate_unix_socket_allowlist_paths(&config)?;
    let allowed_domains = config.network.allowed_domains().unwrap_or_default();
    let denied_domains = config.network.denied_domains().unwrap_or_default();
    validate_non_global_wildcard_domain_patterns("network.denied_domains", &denied_domains)
        .map_err(NetworkProxyConstraintError::into_anyhow)?;
    let deny_set = compile_denylist_globset(&denied_domains)?;
    let allow_set = compile_allowlist_globset(&allowed_domains)?;
    let mitm = if config.network.mitm {
        Some(Arc::new(MitmState::new(
            config.network.allow_upstream_proxy,
        )?))
    } else {
        None
    };
    Ok(ConfigState {
        config,
        allow_set,
        deny_set,
        mitm,
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
    let config_allowed_domains = config.network.allowed_domains().unwrap_or_default();
    let config_denied_domains = config.network.denied_domains().unwrap_or_default();
    let denied_domain_overrides: HashSet<String> = config_denied_domains
        .iter()
        .map(|entry| entry.to_ascii_lowercase())
        .collect();
    let config_allow_unix_sockets = config.network.allow_unix_sockets();
    validate_non_global_wildcard_domain_patterns(
        "network.denied_domains",
        &config_denied_domains,
    )?;
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
        validate_non_global_wildcard_domain_patterns("network.allowed_domains", allowed_domains)?;
        match constraints.allowlist_expansion_enabled {
            Some(true) => {
                let required_set: HashSet<String> = allowed_domains
                    .iter()
                    .map(|entry| entry.to_ascii_lowercase())
                    .collect();
                validate(config_allowed_domains, |candidate| {
                    let candidate_set: HashSet<String> = candidate
                        .iter()
                        .map(|entry| entry.to_ascii_lowercase())
                        .collect();
                    let missing: Vec<String> = required_set
                        .iter()
                        .filter(|entry| {
                            !candidate_set.contains(*entry)
                                && !denied_domain_overrides.contains(*entry)
                        })
                        .cloned()
                        .collect();
                    if missing.is_empty() {
                        Ok(())
                    } else {
                        Err(invalid_value(
                            "network.allowed_domains",
                            "missing managed allowed_domains entries",
                            format!("{missing:?}"),
                        ))
                    }
                })?;
            }
            Some(false) => {
                let required_set: HashSet<String> = allowed_domains
                    .iter()
                    .map(|entry| entry.to_ascii_lowercase())
                    .collect();
                validate(config_allowed_domains, |candidate| {
                    let candidate_set: HashSet<String> = candidate
                        .iter()
                        .map(|entry| entry.to_ascii_lowercase())
                        .collect();
                    let expected_set: HashSet<String> = required_set
                        .difference(&denied_domain_overrides)
                        .cloned()
                        .collect();
                    if candidate_set == expected_set {
                        Ok(())
                    } else {
                        Err(invalid_value(
                            "network.allowed_domains",
                            format!("{candidate:?}"),
                            "must match managed allowed_domains",
                        ))
                    }
                })?;
            }
            None => {
                let managed_patterns: Vec<DomainPattern> = allowed_domains
                    .iter()
                    .map(|entry| DomainPattern::parse_for_constraints(entry))
                    .collect();
                validate(config_allowed_domains, move |candidate| {
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
        }
    }

    if let Some(denied_domains) = &constraints.denied_domains {
        validate_non_global_wildcard_domain_patterns("network.denied_domains", denied_domains)?;
        let required_set: HashSet<String> = denied_domains
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        match constraints.denylist_expansion_enabled {
            Some(false) => {
                validate(config_denied_domains, move |candidate| {
                    let candidate_set: HashSet<String> = candidate
                        .iter()
                        .map(|entry| entry.to_ascii_lowercase())
                        .collect();
                    if candidate_set == required_set {
                        Ok(())
                    } else {
                        Err(invalid_value(
                            "network.denied_domains",
                            format!("{candidate:?}"),
                            "must match managed denied_domains",
                        ))
                    }
                })?;
            }
            Some(true) | None => {
                validate(config_denied_domains, move |candidate| {
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
        }
    }

    if let Some(allow_unix_sockets) = &constraints.allow_unix_sockets {
        let allowed_set: HashSet<String> = allow_unix_sockets
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect();
        validate(config_allow_unix_sockets, move |candidate| {
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
        })?;
    }

    Ok(())
}

fn validate_non_global_wildcard_domain_patterns(
    field_name: &'static str,
    patterns: &[String],
) -> Result<(), NetworkProxyConstraintError> {
    if let Some(pattern) = patterns
        .iter()
        .find(|pattern| is_global_wildcard_domain_pattern(pattern))
    {
        return Err(NetworkProxyConstraintError::InvalidValue {
            field_name,
            candidate: pattern.trim().to_string(),
            allowed: "exact hosts or scoped wildcards like *.example.com or **.example.com"
                .to_string(),
        });
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

#[cfg(test)]
mod tests {}
