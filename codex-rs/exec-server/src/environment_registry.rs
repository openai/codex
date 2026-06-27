use serde::Deserialize;
use serde::Serialize;

use crate::NoiseChannelPublicKey;

pub const ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION: u32 = 1;
const TRANSPORT_POLICY_TELEMETRY_EPOCH_MAX_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum TransportPolicyCell {
    Unassigned,
    C00,
    C10,
    C01,
    C11,
}

impl TransportPolicyCell {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unassigned => "unassigned",
            Self::C00 => "c00",
            Self::C10 => "c10",
            Self::C01 => "c01",
            Self::C11 => "c11",
        }
    }

    pub(crate) fn from_u8(value: u8) -> Self {
        match value {
            value if value == Self::C00 as u8 => Self::C00,
            value if value == Self::C10 as u8 => Self::C10,
            value if value == Self::C01 as u8 => Self::C01,
            value if value == Self::C11 as u8 => Self::C11,
            _ => Self::Unassigned,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum TransportPolicyState {
    Unassigned,
    Legacy,
    Inactive,
    Active,
    Unknown,
}

impl TransportPolicyState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unassigned => "unassigned",
            Self::Legacy => "legacy",
            Self::Inactive => "inactive",
            Self::Active => "active",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn from_u8(value: u8) -> Self {
        match value {
            value if value == Self::Legacy as u8 => Self::Legacy,
            value if value == Self::Inactive as u8 => Self::Inactive,
            value if value == Self::Active as u8 => Self::Active,
            value if value == Self::Unknown as u8 => Self::Unknown,
            _ => Self::Unassigned,
        }
    }
}

/// Registry-issued transport settings for one rendezvous connection attempt.
///
/// Clients apply settings only when they recognize `version`. Missing or
/// unsupported policies therefore retain the legacy transport behavior.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvironmentRegistryTransportPolicy {
    #[serde(default)]
    pub version: u32,
    #[serde(default = "default_assignment_epoch")]
    pub assignment_epoch: String,
    #[serde(default)]
    pub outbound_tcp_nodelay: bool,
    #[serde(default)]
    pub rendezvous_accepted_tcp_nodelay: bool,
}

impl EnvironmentRegistryTransportPolicy {
    pub fn effective_outbound_tcp_nodelay(&self) -> bool {
        self.effective_state() == TransportPolicyState::Active && self.outbound_tcp_nodelay
    }

    pub fn effective_rendezvous_accepted_tcp_nodelay(&self) -> bool {
        self.effective_state() == TransportPolicyState::Active
            && self.rendezvous_accepted_tcp_nodelay
    }

    /// Returns the bounded experiment cell after unsupported versions fail closed.
    pub(crate) fn effective_cell(&self) -> TransportPolicyCell {
        let settings = (
            self.effective_outbound_tcp_nodelay(),
            self.effective_rendezvous_accepted_tcp_nodelay(),
        );
        match settings {
            (false, false) => TransportPolicyCell::C00,
            (true, false) => TransportPolicyCell::C10,
            (false, true) => TransportPolicyCell::C01,
            (true, true) => TransportPolicyCell::C11,
        }
    }

    /// Classifies assignment provenance without exposing the raw epoch as a tag.
    pub(crate) fn effective_state(&self) -> TransportPolicyState {
        match self.version {
            0 => TransportPolicyState::Legacy,
            ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION if self.assignment_epoch == "off" => {
                TransportPolicyState::Inactive
            }
            ENVIRONMENT_REGISTRY_TRANSPORT_POLICY_VERSION => TransportPolicyState::Active,
            _ => TransportPolicyState::Unknown,
        }
    }

    /// Returns an epoch safe for logs and spans, preserving valid experiment
    /// epochs while bounding untrusted registry response data.
    pub(crate) fn telemetry_assignment_epoch(&self) -> &str {
        if self.assignment_epoch.is_empty()
            || self.assignment_epoch.len() > TRANSPORT_POLICY_TELEMETRY_EPOCH_MAX_BYTES
            || self.assignment_epoch.chars().any(char::is_control)
        {
            "invalid"
        } else {
            &self.assignment_epoch
        }
    }
}

impl Default for EnvironmentRegistryTransportPolicy {
    fn default() -> Self {
        Self {
            version: 0,
            assignment_epoch: "legacy".to_string(),
            outbound_tcp_nodelay: false,
            rendezvous_accepted_tcp_nodelay: false,
        }
    }
}

fn default_assignment_epoch() -> String {
    "off".to_string()
}

/// Request body for registering an executor with the environment registry.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvironmentRegistryRegistrationRequest {
    pub security_profile: String,
    pub executor_public_key: NoiseChannelPublicKey,
}

/// Environment registry response returned after executor registration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvironmentRegistryRegistrationResponse {
    pub environment_id: String,
    pub url: String,
    pub security_profile: String,
    pub executor_registration_id: String,
    #[serde(default)]
    pub transport_policy: EnvironmentRegistryTransportPolicy,
}

/// Request body for connecting a harness key with the environment registry.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvironmentRegistryConnectRequest {
    pub harness_public_key: NoiseChannelPublicKey,
}

/// Environment registry response returned after connecting a harness key.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvironmentRegistryConnectResponse {
    pub environment_id: String,
    pub url: String,
    pub security_profile: String,
    pub executor_registration_id: String,
    pub executor_public_key: NoiseChannelPublicKey,
    pub harness_key_authorization: String,
    #[serde(default)]
    pub transport_policy: EnvironmentRegistryTransportPolicy,
}

impl std::fmt::Debug for EnvironmentRegistryConnectResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnvironmentRegistryConnectResponse")
            .field("environment_id", &self.environment_id)
            .field("url", &"<redacted>")
            .field("security_profile", &self.security_profile)
            .field("executor_registration_id", &self.executor_registration_id)
            .field("executor_public_key", &self.executor_public_key)
            .field("harness_key_authorization", &"<redacted>")
            .field("transport_policy", &self.transport_policy)
            .finish()
    }
}

/// Request body for authorizing a harness key with the environment registry.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvironmentRegistryHarnessKeyValidationRequest {
    pub executor_registration_id: String,
    pub harness_public_key: NoiseChannelPublicKey,
    pub harness_key_authorization: String,
}

/// Environment registry response returned after harness key validation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvironmentRegistryHarnessKeyValidationResponse {
    pub valid: bool,
}

#[cfg(test)]
#[path = "environment_registry_tests.rs"]
mod tests;
