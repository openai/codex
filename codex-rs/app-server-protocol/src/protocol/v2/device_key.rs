use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

/// Device-key algorithm reported at enrollment and signing boundaries.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case", export_to = "v2/")]
pub enum DeviceKeyAlgorithm {
    EcdsaP256Sha256,
}

/// Platform protection class for a controller-local device key.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case", export_to = "v2/")]
pub enum DeviceKeyProtectionClass {
    HardwareSecureEnclave,
    HardwareTpm,
    OsProtectedNonextractable,
}

/// Protection policy for creating or loading a controller-local device key.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case", export_to = "v2/")]
pub enum DeviceKeyProtectionPolicy {
    HardwareOnly,
    AllowOsProtectedNonextractable,
}

/// Create a controller-local device key with a random key id.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DeviceKeyCreateParams {
    /// Defaults to `hardware_only` when omitted.
    #[ts(optional = nullable)]
    pub protection_policy: Option<DeviceKeyProtectionPolicy>,
    pub account_user_id: String,
    pub client_id: String,
}

/// Device-key metadata and public key returned by create/public APIs.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DeviceKeyCreateResponse {
    pub key_id: String,
    /// SubjectPublicKeyInfo DER encoded as base64.
    pub public_key_spki_der_base64: String,
    pub algorithm: DeviceKeyAlgorithm,
    pub protection_class: DeviceKeyProtectionClass,
}

/// Fetch a controller-local device key public key by id.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DeviceKeyPublicParams {
    pub key_id: String,
}

/// Device-key public metadata returned by `device/key/public`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DeviceKeyPublicResponse {
    pub key_id: String,
    /// SubjectPublicKeyInfo DER encoded as base64.
    pub public_key_spki_der_base64: String,
    pub algorithm: DeviceKeyAlgorithm,
    pub protection_class: DeviceKeyProtectionClass,
}

/// Current remote-control connection status and environment id exposed to clients.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct RemoteControlStatusChangedNotification {
    pub status: RemoteControlConnectionStatus,
    pub environment_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum RemoteControlConnectionStatus {
    Disabled,
    Connecting,
    Connected,
    Errored,
}

/// Audience for a remote-control client connection device-key proof.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case", export_to = "v2/")]
pub enum RemoteControlClientConnectionAudience {
    RemoteControlClientWebsocket,
}

/// Audience for a remote-control client enrollment device-key proof.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case", export_to = "v2/")]
pub enum RemoteControlClientEnrollmentAudience {
    RemoteControlClientEnrollment,
}

/// Structured payloads accepted by `device/key/sign`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(tag = "type", export_to = "v2/")]
pub enum DeviceKeySignPayload {
    /// Payload bound to one remote-control controller websocket `/client` connection challenge.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    RemoteControlClientConnection {
        nonce: String,
        audience: RemoteControlClientConnectionAudience,
        /// Backend-issued websocket session id that this proof authorizes.
        session_id: String,
        /// Origin of the backend endpoint that issued the challenge and will verify this proof.
        target_origin: String,
        /// Websocket route path that this proof authorizes.
        target_path: String,
        account_user_id: String,
        client_id: String,
        /// Remote-control token expiration as Unix seconds.
        #[ts(type = "number")]
        token_expires_at: i64,
        /// SHA-256 of the controller-scoped remote-control token, encoded as unpadded base64url.
        token_sha256_base64url: String,
        /// Must contain exactly `remote_control_controller_websocket`.
        scopes: Vec<String>,
    },
    /// Payload bound to a remote-control client `/client/enroll` ownership challenge.
    #[serde(rename_all = "camelCase")]
    #[ts(rename_all = "camelCase")]
    RemoteControlClientEnrollment {
        nonce: String,
        audience: RemoteControlClientEnrollmentAudience,
        /// Backend-issued enrollment challenge id that this proof authorizes.
        challenge_id: String,
        /// Origin of the backend endpoint that issued the challenge and will verify this proof.
        target_origin: String,
        /// HTTP route path that this proof authorizes.
        target_path: String,
        account_user_id: String,
        client_id: String,
        /// SHA-256 of the requested device identity operation, encoded as unpadded base64url.
        device_identity_sha256_base64url: String,
        /// Enrollment challenge expiration as Unix seconds.
        #[ts(type = "number")]
        challenge_expires_at: i64,
    },
}

/// Sign an accepted structured payload with a controller-local device key.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DeviceKeySignParams {
    pub key_id: String,
    pub payload: DeviceKeySignPayload,
}

/// ASN.1 DER signature returned by `device/key/sign`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct DeviceKeySignResponse {
    /// ECDSA signature DER encoded as base64.
    pub signature_der_base64: String,
    /// Exact bytes signed by the device key, encoded as base64. Verifiers must verify this byte
    /// string directly and must not reserialize `payload`.
    pub signed_payload_base64: String,
    pub algorithm: DeviceKeyAlgorithm,
}
