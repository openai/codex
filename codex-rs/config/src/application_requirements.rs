//! Managed application destinations are independent of agent network permissions.
//! Enabled policies deny unlisted domains and use normal managed TOML precedence.

use crate::NetworkDomainPermissionToml;
use serde::Deserialize;
use serde::de::Error;
use std::collections::BTreeMap;

#[derive(Deserialize, Debug, Clone, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApplicationRequirementsToml {
    pub network: Option<ApplicationNetworkRequirementsToml>,
    // Keep optional storage paths out of config-loading futures' inline state.
    pub storage: Option<Box<crate::ApplicationStorageRequirementsToml>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationNetworkRequirementsToml {
    pub enabled: bool,
    pub domains: BTreeMap<String, NetworkDomainPermissionToml>,
}

impl<'de> Deserialize<'de> for ApplicationNetworkRequirementsToml {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Network {
            enabled: Option<bool>,
            #[serde(default)]
            domains: BTreeMap<String, NetworkDomainPermissionToml>,
        }

        let raw = Network::deserialize(deserializer)?;
        let mut domains = BTreeMap::new();
        for (domain, permission) in raw.domains {
            let normalized = domain
                .strip_suffix('.')
                .unwrap_or(&domain)
                .to_ascii_lowercase();
            if normalized.len() > 253
                || normalized.split('.').any(|label| {
                    label.is_empty()
                        || label.len() > 63
                        || !label.starts_with(|c: char| c.is_ascii_alphanumeric())
                        || !label.ends_with(|c: char| c.is_ascii_alphanumeric())
                        || !label
                            .bytes()
                            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
                })
            {
                return Err(D::Error::custom(
                    "application.network.domains requires exact ASCII domain names without URLs, ports, or wildcards",
                ));
            }
            if domains.insert(normalized, permission).is_some() {
                return Err(D::Error::custom(format!(
                    "duplicate application.network domain after normalization: {domain:?}"
                )));
            }
        }
        Ok(Self {
            enabled: raw.enabled.unwrap_or(true),
            domains,
        })
    }
}

#[cfg(test)]
#[path = "application_requirements_tests.rs"]
mod tests;
