use crate::config::NetworkMode;
use crate::config::NetworkProxyConfig;
use crate::mitm::MitmState;
use crate::policy::DomainPattern;
use crate::policy::compile_globset;
use crate::runtime::ConfigState;
use anyhow::Context;
use codex_utils_home_dir::find_codex_home;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

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
    mut config: NetworkProxyConfig,
    constraints: NetworkProxyConstraints,
) -> anyhow::Result<ConfigState> {
    let deny_set = compile_globset(&config.network.denied_domains)?;
    let allow_set = compile_globset(&config.network.allowed_domains)?;
    let mitm = if config.network.mitm.enabled {
        resolve_relative_mitm_paths(&mut config)?;
        Some(Arc::new(MitmState::new(
            &config.network.mitm,
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
    })
}

fn resolve_relative_mitm_paths(config: &mut NetworkProxyConfig) -> anyhow::Result<()> {
    if !config.network.mitm.ca_cert_path.is_relative()
        && !config.network.mitm.ca_key_path.is_relative()
    {
        return Ok(());
    }

    let codex_home =
        find_codex_home().context("failed to resolve CODEX_HOME for network.mitm paths")?;
    resolve_relative_mitm_paths_for_base(config, &codex_home);
    Ok(())
}

fn resolve_relative_mitm_paths_for_base(config: &mut NetworkProxyConfig, base: &Path) {
    if config.network.mitm.ca_cert_path.is_relative() {
        config.network.mitm.ca_cert_path = base.join(&config.network.mitm.ca_cert_path);
    }
    if config.network.mitm.ca_key_path.is_relative() {
        config.network.mitm.ca_key_path = base.join(&config.network.mitm.ca_key_path);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MitmConfig;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn resolve_relative_mitm_paths_for_base_converts_relative_paths() {
        let codex_home = tempfile::tempdir().expect("create temp codex home");
        let base = codex_home.path().to_path_buf();
        let mut config = NetworkProxyConfig::default();
        config.network.mitm = MitmConfig {
            ca_cert_path: PathBuf::from("proxy/ca.pem"),
            ca_key_path: PathBuf::from("proxy/ca.key"),
            ..MitmConfig::default()
        };

        resolve_relative_mitm_paths_for_base(&mut config, &base);

        assert_eq!(config.network.mitm.ca_cert_path, base.join("proxy/ca.pem"));
        assert_eq!(config.network.mitm.ca_key_path, base.join("proxy/ca.key"));
    }

    #[test]
    fn resolve_relative_mitm_paths_for_base_preserves_absolute_paths() {
        let codex_home = tempfile::tempdir().expect("create temp codex home");
        let base = codex_home.path().to_path_buf();
        let mut config = NetworkProxyConfig::default();
        let cert = std::env::temp_dir().join("mitm-cert.pem");
        let key = std::env::temp_dir().join("mitm-key.pem");
        config.network.mitm = MitmConfig {
            ca_cert_path: cert.clone(),
            ca_key_path: key.clone(),
            ..MitmConfig::default()
        };

        resolve_relative_mitm_paths_for_base(&mut config, &base);

        assert_eq!(config.network.mitm.ca_cert_path, cert);
        assert_eq!(config.network.mitm.ca_key_path, key);
    }
}
