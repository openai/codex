use serde::Deserialize;
use serde::Serialize;

use crate::NoiseChannelPublicKey;

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
}

/// Request body for connecting a harness key with the environment registry.
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvironmentRegistryConnectRequest {
    pub harness_public_key: NoiseChannelPublicKey,
}

/// Environment registry response returned after connecting a harness key.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvironmentRegistryConnectResponse {
    pub environment_id: String,
    pub url: String,
    pub security_profile: String,
    pub executor_registration_id: String,
    pub executor_public_key: NoiseChannelPublicKey,
    pub harness_key_authorization: String,
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
