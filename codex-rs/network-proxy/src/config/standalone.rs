use super::NetworkMode;
use super::NetworkProxyConfig;
use crate::ConfigState;
use crate::NetworkProxyConstraints;
use crate::Platform;
use crate::build_config_state;
use crate::validate_policy_against_constraints;
use anyhow::Result;
use anyhow::ensure;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;

/// A resolved policy, including the ceilings applied by managed requirements.
///
/// Keep this document outside the workload's writable filesystem. It is not an
/// attestation of enforcement: the host must separately ensure that all workload
/// egress traverses the proxy and bind readiness to the current environment.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct StaticNetworkProxyConfig {
    pub network: NetworkProxyConfig,
    /// Standalone configurations may omit constraints. Managed callers must export
    /// the resolved requirements and use `from_slice_strict` to require this field.
    pub constraints: NetworkProxyConstraints,
}

impl<'de> Deserialize<'de> for StaticNetworkProxyConfig {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Document {
            network: NetworkProxyConfig,
            #[serde(default)]
            constraints: NetworkProxyConstraints,
        }

        let config: Document = deserialize_document(deserializer, /*allow_metadata*/ true)?;
        Ok(Self {
            network: config.network,
            constraints: config.constraints,
        })
    }
}

// Only the top-level metadata object is extensible. Ignoring a matcher key can widen access.
fn deserialize_document<'de, D, T>(
    deserializer: D,
    allow_metadata: bool,
) -> std::result::Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let mut unknown = None;
    let config = serde_ignored::deserialize(deserializer, |path| {
        let path = path.to_string();
        if unknown.is_none() && !(allow_metadata && path == "metadata") {
            unknown = Some(path);
        }
    })?;
    if let Some(path) = unknown {
        return Err(serde::de::Error::custom(format!("unknown field: {path}")));
    }
    Ok(config)
}

impl StaticNetworkProxyConfig {
    /// Parse standalone policy, allowing optional constraints and top-level metadata.
    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        Ok(serde_json::from_slice::<Self>(bytes)?.normalize())
    }

    /// Require explicit constraints and reject unknown fields at every typed level.
    pub fn from_slice_strict(bytes: &[u8]) -> Result<Self> {
        #[derive(Deserialize)]
        struct RequiredConstraints {
            network: NetworkProxyConfig,
            constraints: NetworkProxyConstraints,
        }

        let mut deserializer = serde_json::Deserializer::from_slice(bytes);
        let config: RequiredConstraints =
            deserialize_document(&mut deserializer, /*allow_metadata*/ false)?;
        deserializer.end()?;
        Ok(Self {
            network: config.network,
            constraints: config.constraints,
        }
        .normalize())
    }

    fn normalize(mut self) -> Self {
        self.network.mitm |=
            self.network.mode == NetworkMode::Limited || !self.network.mitm_hooks.is_empty();
        self
    }

    /// Validate before creating listeners or loading credential/CA material.
    pub fn build_state(self, platform: Platform) -> Result<ConfigState> {
        let config = self.normalize();
        ensure!(
            config.network.enabled,
            "standalone network proxy requires network.enabled = true"
        );
        validate_policy_against_constraints(&config.network, &config.constraints)?;
        build_config_state(config.network, config.constraints, platform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn static_config_preserves_constraints_and_rejects_weakened_policy() {
        let bytes = serde_json::to_vec(&json!({
            "network": {
                "enabled": true,
                "mode": "limited",
                "allow_upstream_proxy": false,
                "domains": {"*.example.com": "allow", "blocked.example.com": "deny"}
            },
            "constraints": {
                "enabled": true,
                "allow_upstream_proxy": false,
                "allowed_domains": ["*.example.com"],
                "allowlist_expansion_enabled": false,
                "denied_domains": ["blocked.example.com"],
                "denylist_expansion_enabled": true
            }
        }))
        .unwrap();
        let config = StaticNetworkProxyConfig::from_slice(&bytes).unwrap();
        let round_trip =
            StaticNetworkProxyConfig::from_slice(&serde_json::to_vec(&config).unwrap()).unwrap();
        assert_eq!(round_trip, config);
        let direct: StaticNetworkProxyConfig = serde_json::from_slice(&bytes).unwrap();
        assert!(!direct.network.mitm);
        let dir = tempfile::tempdir().unwrap();
        let ca = crate::certs::ManagedMitmCa::for_test(dir.path()).unwrap();
        let state = crate::certs::TEST_MITM_CA
            .sync_scope(ca, || direct.build_state(Platform::Linux))
            .unwrap();
        assert!(state.mitm.is_some());
        assert_eq!(state.config, config.network);
        assert_eq!(state.constraints, config.constraints);

        let mut weakened = config;
        weakened.network.allow_upstream_proxy = true;
        assert!(weakened.build_state(Platform::Linux).is_err());

        for bytes in [
            br#"{"network":{"enabled":true},"constraints":{"enabled":false}}"#.as_slice(),
            br#"{"network":{"enabled":false},"constraints":{}}"#.as_slice(),
        ] {
            assert!(
                StaticNetworkProxyConfig::from_slice(bytes)
                    .unwrap()
                    .build_state(Platform::Linux)
                    .is_err()
            );
        }
    }

    #[test]
    fn managed_socket_paths_match_exactly() {
        let mut policy = StaticNetworkProxyConfig::from_slice_strict(
            br#"{"network":{"enabled":true,"unix_sockets":{"/run/Secret.sock":"allow"}},
                 "constraints":{"allow_unix_sockets":["/run/Secret.sock"]}}"#,
        )
        .unwrap();
        assert!(validate_policy_against_constraints(&policy.network, &policy.constraints).is_ok());
        policy
            .network
            .set_allow_unix_sockets(vec!["/run/secret.sock".to_string()]);
        assert!(validate_policy_against_constraints(&policy.network, &policy.constraints).is_err());
    }

    #[test]
    fn parsing_modes_handle_constraints_and_unknown_fields() {
        let without_constraints = br#"{"network":{"enabled":true}}"#;
        assert!(StaticNetworkProxyConfig::from_slice(without_constraints).is_ok());
        assert!(StaticNetworkProxyConfig::from_slice_strict(without_constraints).is_err());

        let baseline = json!({
            "network": {
                "enabled": true,
                "mitm_hooks": [{
                    "host": "example.com",
                    "match": {"methods": ["POST"]},
                    "actions": {"strip_request_headers": ["x-debug"]}
                }]
            },
            "constraints": {"allow_upstream_proxy": false}
        });
        let expected =
            StaticNetworkProxyConfig::from_slice_strict(&serde_json::to_vec(&baseline).unwrap())
                .unwrap();
        let mut annotated = baseline.clone();
        annotated["metadata"] = json!({"producer_version": 2});
        let bytes = serde_json::to_vec(&annotated).unwrap();
        assert_eq!(
            StaticNetworkProxyConfig::from_slice(&bytes).unwrap(),
            expected
        );
        assert!(StaticNetworkProxyConfig::from_slice_strict(&bytes).is_err());

        for pointer in [
            "",
            "/network",
            "/constraints",
            "/network/mitm_hooks/0/match",
            "/network/mitm_hooks/0/actions",
        ] {
            let mut config = baseline.clone();
            config.pointer_mut(pointer).unwrap()["future_setting"] = json!(true);
            let bytes = serde_json::to_vec(&config).unwrap();
            for parse in [
                StaticNetworkProxyConfig::from_slice,
                StaticNetworkProxyConfig::from_slice_strict,
            ] {
                let error = parse(&bytes).unwrap_err();
                assert!(
                    error.to_string().contains("future_setting"),
                    "{pointer}: {error}"
                );
            }
        }

        for bytes in [
            br#"{"constraints":{}}"#.as_slice(),
            br#"{"network":{"mitm_hooks":[{"host":"example.com","match":{"header":{}}}]},"constraints":{}}"#,
            br#"{"network":{},"constraint":{}}"#,
            br#"{"network":{"enabled":"true"},"constraints":{}}"#,
            br#"{"network":{},"constraints":null}"#,
            br#"{"network":{},"constraints":{}} {}"#,
        ] {
            assert!(StaticNetworkProxyConfig::from_slice(bytes).is_err());
            assert!(StaticNetworkProxyConfig::from_slice_strict(bytes).is_err());
        }
    }
}
