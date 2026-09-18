//! Network configuration materialization from explicit, already-selected inputs.
//!
//! Preparing the configured proxy precedes permission requirements fallback in
//! local configuration loading. Keep that phase separate from applying network
//! requirements to the final permission profile so both callers share the same
//! ordering without reading a Config or any host state here.
//! Environment policy conversion drops listener addresses, and final validation
//! uses the same policy resolver as command execution.

use codex_config::NetworkConstraints;
use codex_config::Sourced;
use codex_config::permissions_toml::NetworkToml;
use codex_execpolicy::Policy;
use codex_features::FeatureToml;
use codex_features::FeaturesToml;
use codex_network_proxy::EnvironmentNetworkPolicy;
use codex_network_proxy::NetworkProxyConfig;
use codex_protocol::models::PermissionProfile;
use codex_utils_path_uri::Platform;

use super::NetworkProxySpec;
use super::permissions::apply_network_proxy_feature_config;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid portable environment network policy")]
pub struct EnvironmentNetworkConfigError;

/// Drops listener addresses and rejects unsupported controller fields in an
/// environment's selected network configuration. Use the returned value when
/// deciding whether a policy is present and when preparing it.
///
/// Domain and socket values may be replaced by feature settings or requirements.
/// Validate the composed policy with [`validate_environment_network_policy`].
pub fn project_environment_profile_network(
    network: Option<NetworkToml>,
) -> Result<Option<NetworkToml>, EnvironmentNetworkConfigError> {
    let Some(mut network) = network else {
        return Ok(None);
    };
    network.proxy_url = None;
    network.socks_url = None;
    let supported = NetworkToml {
        enabled: network.enabled,
        allow_upstream_proxy: network.allow_upstream_proxy,
        dangerously_allow_all_unix_sockets: network.dangerously_allow_all_unix_sockets,
        domains: network.domains.clone(),
        unix_sockets: network.unix_sockets.clone(),
        allow_local_binding: network.allow_local_binding,
        ..Default::default()
    };
    if network != supported {
        return Err(EnvironmentNetworkConfigError);
    }
    Ok(Some(network))
}

/// Validates the owner policy for its final permission profile using the native
/// environment resolver. Execution also applies controller and saved exec rules.
/// This checks the supplied executor's path syntax; socket support is checked at execution.
pub fn validate_environment_network_policy(
    policy: &EnvironmentNetworkPolicy,
    permission_profile: &PermissionProfile,
    executor_os: Platform,
) -> Result<(), EnvironmentNetworkConfigError> {
    NetworkProxySpec::for_environment(
        /*controller*/ None,
        policy,
        permission_profile,
        &Policy::empty(),
        codex_network_proxy::LocalBindingPolicy::DefaultFalse,
    )
    .and_then(|spec| spec.build_config_state_for_spec(executor_os).map(|_| ()))
    .map_err(|_| EnvironmentNetworkConfigError)
}

pub struct NetworkConfigInputs<'a> {
    pub configured_proxy: NetworkProxyConfig,
    pub feature_enabled: bool,
    pub features: Option<&'a FeaturesToml>,
    pub candidate_permission_profile: &'a PermissionProfile,
    pub credential_broker_base_url: Option<&'a str>,
}

/// The configured proxy before managed network requirements are applied.
pub struct PreparedNetworkConfig {
    pub(super) configured_proxy: NetworkProxyConfig,
}

impl PreparedNetworkConfig {
    pub fn from_inputs(inputs: NetworkConfigInputs<'_>) -> Self {
        let NetworkConfigInputs {
            mut configured_proxy,
            feature_enabled,
            features,
            candidate_permission_profile,
            credential_broker_base_url,
        } = inputs;
        if feature_enabled
            && candidate_permission_profile
                .network_sandbox_policy()
                .is_enabled()
        {
            if let Some(FeatureToml::Config(config)) =
                features.and_then(|features| features.network_proxy.as_ref())
            {
                apply_network_proxy_feature_config(&mut configured_proxy, config);
            }
            configured_proxy.set_credential_broker_openai_base_url(credential_broker_base_url);
            configured_proxy.enabled = true;
        }
        Self { configured_proxy }
    }

    /// Applies managed network requirements after permission fallback has run.
    pub fn build(
        self,
        requirements: Option<Sourced<NetworkConstraints>>,
        effective_permission_profile: &PermissionProfile,
    ) -> std::io::Result<Option<NetworkProxySpec>> {
        let has_requirements = requirements.is_some();
        let network = self.build_spec(requirements, effective_permission_profile)?;
        Ok(if has_requirements {
            Some(network)
        } else {
            network.enabled().then_some(network)
        })
    }

    /// An attachment's portable policy can be enforced without a local proxy
    /// controller. Its presence must not depend on the controller feature gate.
    pub fn build_environment_policy(
        self,
        requirements: Option<Sourced<NetworkConstraints>>,
        effective_permission_profile: &PermissionProfile,
        has_selected_policy: bool,
    ) -> std::io::Result<Option<EnvironmentNetworkPolicy>> {
        let has_requirements = requirements.is_some();
        let network = self.build_spec(requirements, effective_permission_profile)?;
        Ok(
            (has_selected_policy || has_requirements || network.enabled())
                .then(|| network.environment_policy()),
        )
    }

    fn build_spec(
        self,
        requirements: Option<Sourced<NetworkConstraints>>,
        effective_permission_profile: &PermissionProfile,
    ) -> std::io::Result<NetworkProxySpec> {
        let (requirements, source) = match requirements {
            Some(Sourced { value, source }) => (Some(value), Some(source)),
            None => (None, None),
        };
        let network = NetworkProxySpec::from_config_and_constraints(
            self.configured_proxy,
            requirements,
            effective_permission_profile,
        )
        .map_err(|error| {
            if let Some(source) = source.as_ref() {
                std::io::Error::new(
                    error.kind(),
                    format!("failed to build managed network proxy from {source}: {error}"),
                )
            } else {
                error
            }
        })?;

        Ok(network)
    }
}

#[cfg(test)]
#[path = "network_config_tests.rs"]
mod tests;
