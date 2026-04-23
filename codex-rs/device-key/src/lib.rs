use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::pkcs8::EncodePublicKey;
use rand::random;
use serde::Deserialize;
use serde::Serialize;
use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;
use thiserror::Error;
use url::Host;
use url::Url;

mod platform;

const SIGNING_DOMAIN: &str = "codex-device-key-sign-payload/v1";
const DEVICE_KEY_ID_RANDOM_BYTES: usize = 32;
const DEVICE_KEY_ID_ENCODED_BYTES: usize = 43;
const DEVICE_KEY_ID_HARDWARE_SECURE_ENCLAVE_PREFIX: &str = "dk_hse_";
const DEVICE_KEY_ID_HARDWARE_TPM_PREFIX: &str = "dk_tpm_";
const DEVICE_KEY_ID_OS_PROTECTED_NONEXTRACTABLE_PREFIX: &str = "dk_osn_";
const DEVICE_KEY_ID_PREFIX_LEN: usize = DEVICE_KEY_ID_HARDWARE_SECURE_ENCLAVE_PREFIX.len();
const DEVICE_KEY_ID_LEN: usize = DEVICE_KEY_ID_PREFIX_LEN + DEVICE_KEY_ID_ENCODED_BYTES;
const INVALID_DEVICE_KEY_ID_MESSAGE: &str =
    "keyId must be dk_hse_, dk_tpm_, or dk_osn_ followed by unpadded base64url-encoded 32 bytes";
const REMOTE_CONTROL_CONTROLLER_WEBSOCKET_SCOPE: &str = "remote_control_controller_websocket";
const MAX_REMOTE_CONTROL_DEVICE_KEY_PROOF_TTL_SECONDS: i64 = 15 * 60;
const REMOTE_CONTROL_CLIENT_CONNECTION_PATHS: &[&str] = &[
    "/api/codex/remote/control/client",
    "/wham/remote/control/client",
];
const REMOTE_CONTROL_CLIENT_ENROLLMENT_PATHS: &[&str] = &[
    "/api/codex/remote/control/client/enroll",
    "/wham/remote/control/client/enroll",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKeyAlgorithm {
    EcdsaP256Sha256,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKeyProtectionClass {
    HardwareSecureEnclave,
    HardwareTpm,
    OsProtectedNonextractable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKeyProtectionPolicy {
    HardwareOnly,
    AllowOsProtectedNonextractable,
}

impl DeviceKeyProtectionPolicy {
    fn allows(self, protection_class: DeviceKeyProtectionClass) -> bool {
        match self {
            Self::HardwareOnly => !protection_class.is_degraded(),
            Self::AllowOsProtectedNonextractable => matches!(
                protection_class,
                DeviceKeyProtectionClass::HardwareSecureEnclave
                    | DeviceKeyProtectionClass::HardwareTpm
                    | DeviceKeyProtectionClass::OsProtectedNonextractable
            ),
        }
    }
}

impl DeviceKeyProtectionClass {
    pub fn is_degraded(self) -> bool {
        match self {
            Self::HardwareSecureEnclave | Self::HardwareTpm => false,
            Self::OsProtectedNonextractable => true,
        }
    }

    fn key_id_prefix(self) -> &'static str {
        match self {
            Self::HardwareSecureEnclave => DEVICE_KEY_ID_HARDWARE_SECURE_ENCLAVE_PREFIX,
            Self::HardwareTpm => DEVICE_KEY_ID_HARDWARE_TPM_PREFIX,
            Self::OsProtectedNonextractable => DEVICE_KEY_ID_OS_PROTECTED_NONEXTRACTABLE_PREFIX,
        }
    }
}

impl fmt::Display for DeviceKeyProtectionClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HardwareSecureEnclave => f.write_str("hardware_secure_enclave"),
            Self::HardwareTpm => f.write_str("hardware_tpm"),
            Self::OsProtectedNonextractable => f.write_str("os_protected_nonextractable"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceKeyCreateRequest {
    pub protection_policy: DeviceKeyProtectionPolicy,
    pub binding: DeviceKeyBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceKeyGetPublicRequest {
    pub key_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceKeySignRequest {
    pub key_id: String,
    pub payload: DeviceKeySignPayload,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceKeyBinding {
    pub account_user_id: String,
    pub client_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceKeyInfo {
    pub key_id: String,
    pub public_key_spki_der: Vec<u8>,
    pub algorithm: DeviceKeyAlgorithm,
    pub protection_class: DeviceKeyProtectionClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceKeySignature {
    pub signature_der: Vec<u8>,
    /// Exact payload bytes covered by `signature_der`.
    pub signed_payload: Vec<u8>,
    pub algorithm: DeviceKeyAlgorithm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderSignature {
    signature_der: Vec<u8>,
    algorithm: DeviceKeyAlgorithm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DeviceKeySignPayload {
    RemoteControlClientConnection(RemoteControlClientConnectionSignPayload),
    RemoteControlClientEnrollment(RemoteControlClientEnrollmentSignPayload),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteControlClientConnectionAudience {
    RemoteControlClientWebsocket,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlClientConnectionSignPayload {
    pub nonce: String,
    pub audience: RemoteControlClientConnectionAudience,
    pub session_id: String,
    pub target_origin: String,
    pub target_path: String,
    pub account_user_id: String,
    pub client_id: String,
    pub token_sha256_base64url: String,
    pub token_expires_at: i64,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteControlClientEnrollmentAudience {
    RemoteControlClientEnrollment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteControlClientEnrollmentSignPayload {
    pub nonce: String,
    pub audience: RemoteControlClientEnrollmentAudience,
    pub challenge_id: String,
    pub target_origin: String,
    pub target_path: String,
    pub account_user_id: String,
    pub client_id: String,
    pub device_identity_sha256_base64url: String,
    pub challenge_expires_at: i64,
}

#[derive(Debug, Error)]
pub enum DeviceKeyError {
    #[error(
        "hardware-backed device keys are not available; set protectionPolicy to allow_os_protected_nonextractable to allow key protection class {available}"
    )]
    DegradedProtectionNotAllowed { available: DeviceKeyProtectionClass },
    #[error("hardware-backed device keys are not available on this platform")]
    HardwareBackedKeysUnavailable,
    #[error("device key not found")]
    KeyNotFound,
    #[error("invalid device key payload: {0}")]
    InvalidPayload(&'static str),
    #[error("device key platform error: {0}")]
    Platform(String),
    #[error("device key cryptography error: {0}")]
    Crypto(String),
}

#[derive(Debug, Clone)]
pub struct DeviceKeyStore {
    provider: Arc<dyn DeviceKeyProvider>,
}

impl Default for DeviceKeyStore {
    fn default() -> Self {
        Self {
            provider: platform::default_provider(),
        }
    }
}

impl DeviceKeyStore {
    pub fn create(&self, request: DeviceKeyCreateRequest) -> Result<DeviceKeyInfo, DeviceKeyError> {
        let key_id_random = random_key_id_random();
        validate_binding(&request.binding.account_user_id, &request.binding.client_id)?;
        self.provider.create(ProviderCreateRequest {
            key_id_random: &key_id_random,
            protection_policy: request.protection_policy,
            binding: &request.binding,
        })
    }

    pub fn get_public(
        &self,
        request: DeviceKeyGetPublicRequest,
    ) -> Result<DeviceKeyInfo, DeviceKeyError> {
        let protection_class = validate_key_id(&request.key_id)?;
        self.provider.get_public(&request.key_id, protection_class)
    }

    pub fn sign(
        &self,
        request: DeviceKeySignRequest,
    ) -> Result<DeviceKeySignature, DeviceKeyError> {
        let protection_class = validate_key_id(&request.key_id)?;
        validate_payload(&request.payload)?;
        let binding = self.provider.binding(&request.key_id, protection_class)?;
        validate_payload_binding(&request.payload, &binding)?;
        let signed_payload = device_key_signing_payload_bytes(&request.payload)?;
        let signature = self
            .provider
            .sign(&request.key_id, protection_class, &signed_payload)?;
        Ok(DeviceKeySignature {
            signature_der: signature.signature_der,
            signed_payload,
            algorithm: signature.algorithm,
        })
    }

    #[cfg(test)]
    fn with_provider(provider: Arc<dyn DeviceKeyProvider>) -> Self {
        Self { provider }
    }
}

#[derive(Debug)]
struct ProviderCreateRequest<'a> {
    key_id_random: &'a str,
    protection_policy: DeviceKeyProtectionPolicy,
    binding: &'a DeviceKeyBinding,
}

impl ProviderCreateRequest<'_> {
    fn key_id_for(&self, protection_class: DeviceKeyProtectionClass) -> String {
        key_id_for_protection_class(protection_class, self.key_id_random)
    }
}

/// Owns platform-specific non-exportable key operations for device signing.
///
/// Implementations must never expose a generic arbitrary-byte signing API outside this crate. The
/// crate validates and serializes accepted structured payloads before calling `sign`.
trait DeviceKeyProvider: Debug + Send + Sync {
    fn create(&self, request: ProviderCreateRequest<'_>) -> Result<DeviceKeyInfo, DeviceKeyError>;
    fn get_public(
        &self,
        key_id: &str,
        protection_class: DeviceKeyProtectionClass,
    ) -> Result<DeviceKeyInfo, DeviceKeyError>;
    fn binding(
        &self,
        key_id: &str,
        protection_class: DeviceKeyProtectionClass,
    ) -> Result<DeviceKeyBinding, DeviceKeyError>;
    fn sign(
        &self,
        key_id: &str,
        protection_class: DeviceKeyProtectionClass,
        payload: &[u8],
    ) -> Result<ProviderSignature, DeviceKeyError>;
}

fn random_key_id_random() -> String {
    URL_SAFE_NO_PAD.encode(random::<[u8; DEVICE_KEY_ID_RANDOM_BYTES]>())
}

fn key_id_for_protection_class(
    protection_class: DeviceKeyProtectionClass,
    encoded_random: &str,
) -> String {
    format!("{}{encoded_random}", protection_class.key_id_prefix())
}

/// Validates the account/client binding stored with a key or embedded in an accepted payload.
///
/// Providers treat the binding as metadata, so this crate keeps empty values from entering the
/// store and later matching every other empty value by accident.
fn validate_binding(account_user_id: &str, client_id: &str) -> Result<(), DeviceKeyError> {
    if account_user_id.is_empty() {
        return Err(DeviceKeyError::InvalidPayload(
            "accountUserId must not be empty",
        ));
    }
    if client_id.is_empty() {
        return Err(DeviceKeyError::InvalidPayload("clientId must not be empty"));
    }
    Ok(())
}

/// Keeps all externally supplied key IDs inside the random `dk_*_` namespaces created by this crate.
///
/// Platform providers use the key ID in OS-specific labels, tags, and metadata paths. Requiring the
/// exact generated shape avoids path or tag surprises and makes the namespace auditable.
fn validate_key_id(key_id: &str) -> Result<DeviceKeyProtectionClass, DeviceKeyError> {
    let (protection_class, encoded_key) = parse_key_id(key_id).ok_or(
        DeviceKeyError::InvalidPayload(INVALID_DEVICE_KEY_ID_MESSAGE),
    )?;
    if key_id.len() != DEVICE_KEY_ID_LEN {
        return Err(DeviceKeyError::InvalidPayload(
            INVALID_DEVICE_KEY_ID_MESSAGE,
        ));
    }
    if !URL_SAFE_NO_PAD
        .decode(encoded_key)
        .is_ok_and(|decoded| decoded.len() == DEVICE_KEY_ID_RANDOM_BYTES)
    {
        return Err(DeviceKeyError::InvalidPayload(
            INVALID_DEVICE_KEY_ID_MESSAGE,
        ));
    }
    Ok(protection_class)
}

fn parse_key_id(key_id: &str) -> Option<(DeviceKeyProtectionClass, &str)> {
    for protection_class in [
        DeviceKeyProtectionClass::HardwareSecureEnclave,
        DeviceKeyProtectionClass::HardwareTpm,
        DeviceKeyProtectionClass::OsProtectedNonextractable,
    ] {
        if let Some(encoded_key) = key_id.strip_prefix(protection_class.key_id_prefix()) {
            return Some((protection_class, encoded_key));
        }
    }
    None
}

/// Confirms the signed payload is for the same account/client binding as the selected device key.
///
/// The provider can prove continuity of the key material, but app-server authorization depends on
/// binding that key to the same account and client identity used by the remote-control flow.
fn validate_payload_binding(
    payload: &DeviceKeySignPayload,
    binding: &DeviceKeyBinding,
) -> Result<(), DeviceKeyError> {
    let (account_user_id, client_id) = match payload {
        DeviceKeySignPayload::RemoteControlClientConnection(payload) => {
            (&payload.account_user_id, &payload.client_id)
        }
        DeviceKeySignPayload::RemoteControlClientEnrollment(payload) => {
            (&payload.account_user_id, &payload.client_id)
        }
    };
    if account_user_id != &binding.account_user_id || client_id != &binding.client_id {
        return Err(DeviceKeyError::InvalidPayload(
            "payload accountUserId/clientId does not match device key binding",
        ));
    }
    Ok(())
}

/// Dispatches validation by accepted payload shape before any provider sees bytes to sign.
///
/// The enum is intentionally narrow so adding another signing use case requires defining and
/// validating a new structured payload variant here.
fn validate_payload(payload: &DeviceKeySignPayload) -> Result<(), DeviceKeyError> {
    match payload {
        DeviceKeySignPayload::RemoteControlClientConnection(payload) => {
            validate_remote_control_client_connection_payload(payload)
        }
        DeviceKeySignPayload::RemoteControlClientEnrollment(payload) => {
            validate_remote_control_client_enrollment_payload(payload)
        }
    }
}

/// Validates payloads used to prove device-key ownership while opening `/client`.
///
/// This shape is scoped to a single controller websocket connection and is only allowed to target
/// the non-enrollment remote-control client endpoints.
fn validate_remote_control_client_connection_payload(
    payload: &RemoteControlClientConnectionSignPayload,
) -> Result<(), DeviceKeyError> {
    validate_nonce(&payload.nonce)?;
    validate_remote_control_target(
        &payload.target_origin,
        &payload.target_path,
        REMOTE_CONTROL_CLIENT_CONNECTION_PATHS,
    )?;
    if payload.session_id.is_empty() {
        return Err(DeviceKeyError::InvalidPayload(
            "sessionId must not be empty",
        ));
    }
    validate_binding(&payload.account_user_id, &payload.client_id)?;
    if !is_base64url_sha256(&payload.token_sha256_base64url) {
        return Err(DeviceKeyError::InvalidPayload(
            "tokenSha256Base64url must be a SHA-256 digest encoded as unpadded base64url",
        ));
    }
    if payload.scopes != [REMOTE_CONTROL_CONTROLLER_WEBSOCKET_SCOPE] {
        return Err(DeviceKeyError::InvalidPayload(
            "scopes must contain exactly remote_control_controller_websocket",
        ));
    }
    validate_remote_control_expiry(payload.token_expires_at, "remote-control token")?;
    Ok(())
}

/// Validates payloads used during device-key enrollment.
///
/// Enrollment has a distinct payload shape and challenge identifier, so it also carries a distinct
/// endpoint allowlist from connection proofs.
fn validate_remote_control_client_enrollment_payload(
    payload: &RemoteControlClientEnrollmentSignPayload,
) -> Result<(), DeviceKeyError> {
    validate_nonce(&payload.nonce)?;
    if payload.challenge_id.is_empty() {
        return Err(DeviceKeyError::InvalidPayload(
            "challengeId must not be empty",
        ));
    }
    validate_remote_control_target(
        &payload.target_origin,
        &payload.target_path,
        REMOTE_CONTROL_CLIENT_ENROLLMENT_PATHS,
    )?;
    validate_binding(&payload.account_user_id, &payload.client_id)?;
    if !is_base64url_sha256(&payload.device_identity_sha256_base64url) {
        return Err(DeviceKeyError::InvalidPayload(
            "deviceIdentitySha256Base64url must be a SHA-256 digest encoded as unpadded base64url",
        ));
    }
    validate_remote_control_expiry(payload.challenge_expires_at, "enrollment challenge")?;
    Ok(())
}

/// Requires a fresh server-issued challenge with enough entropy to prevent replay guessing.
fn validate_nonce(nonce: &str) -> Result<(), DeviceKeyError> {
    if !URL_SAFE_NO_PAD
        .decode(nonce)
        .is_ok_and(|decoded| decoded.len() >= 32)
    {
        return Err(DeviceKeyError::InvalidPayload(
            "nonce must be at least 32 random bytes encoded as unpadded base64url",
        ));
    }
    Ok(())
}

/// Validates the remote backend origin and the endpoint set for the specific signed payload shape.
///
/// Keeping the path allowlist as an argument makes it hard to accidentally let enrollment payloads
/// sign connection endpoints, or connection payloads sign enrollment endpoints.
fn validate_remote_control_target(
    target_origin: &str,
    target_path: &str,
    allowed_target_paths: &[&str],
) -> Result<(), DeviceKeyError> {
    if !is_allowed_remote_control_origin(target_origin) {
        return Err(DeviceKeyError::InvalidPayload(
            "targetOrigin must be an allowed remote-control backend origin",
        ));
    }
    if !allowed_target_paths.contains(&target_path) {
        return Err(DeviceKeyError::InvalidPayload(
            "targetPath must match the signed payload type's remote-control endpoint",
        ));
    }
    Ok(())
}

/// Mirrors the remote-control transport allowlist for origins that may receive signed proofs.
fn is_allowed_remote_control_origin(target_origin: &str) -> bool {
    let Ok(url) = Url::parse(target_origin) else {
        return false;
    };
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return false;
    }
    let host = url.host();
    match url.scheme() {
        "https" if is_localhost(&host) || is_allowed_chatgpt_host(&host) => true,
        "http" if is_localhost(&host) => true,
        _ => false,
    }
}

/// Accepts first-party chatgpt.com hosts and staging equivalents, including subdomains.
fn is_allowed_chatgpt_host(host: &Option<Host<&str>>) -> bool {
    let Some(Host::Domain(host)) = *host else {
        return false;
    };
    host == "chatgpt.com"
        || host == "chatgpt-staging.com"
        || host.ends_with(".chatgpt.com")
        || host.ends_with(".chatgpt-staging.com")
}

/// Allows local development endpoints without opening access to arbitrary private-network hosts.
fn is_localhost(host: &Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Domain("localhost")) => true,
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    }
}

/// Bounds remote-control proofs to the connection or enrollment attempt that requested them.
fn validate_remote_control_expiry(
    expires_at: i64,
    label: &'static str,
) -> Result<(), DeviceKeyError> {
    let now = current_unix_seconds()?;
    if expires_at <= now {
        return Err(DeviceKeyError::InvalidPayload(match label {
            "enrollment challenge" => "enrollment challenge is expired",
            _ => "remote-control token is expired",
        }));
    }
    if expires_at > now + MAX_REMOTE_CONTROL_DEVICE_KEY_PROOF_TTL_SECONDS {
        return Err(DeviceKeyError::InvalidPayload(match label {
            "enrollment challenge" => "enrollment challenge expires too far in the future",
            _ => "remote-control token expires too far in the future",
        }));
    }
    Ok(())
}

/// Checks the exact digest encoding used in remote-control challenge and token bindings.
fn is_base64url_sha256(value: &str) -> bool {
    URL_SAFE_NO_PAD
        .decode(value)
        .is_ok_and(|digest| digest.len() == 32)
}

fn current_unix_seconds() -> Result<i64, DeviceKeyError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DeviceKeyError::InvalidPayload("system clock is before Unix epoch"))?;
    i64::try_from(duration.as_secs())
        .map_err(|_| DeviceKeyError::InvalidPayload("current time does not fit in i64"))
}

/// Returns the exact bytes that device-key providers sign and verifiers must check.
///
/// The representation is UTF-8 JSON with an explicit domain separator, sorted object keys, no
/// insignificant whitespace, and the accepted structured payload. Test vectors in this crate
/// intentionally lock the field names and ordering so non-Rust verifiers can reproduce the same
/// bytes.
pub fn device_key_signing_payload_bytes(
    payload: &DeviceKeySignPayload,
) -> Result<Vec<u8>, DeviceKeyError> {
    let mut canonical = serde_json::to_value(SignedPayload {
        domain: SIGNING_DOMAIN,
        payload,
    })
    .map_err(|err| DeviceKeyError::Crypto(err.to_string()))?;
    canonical.sort_all_objects();
    serde_json::to_vec(&canonical).map_err(|err| DeviceKeyError::Crypto(err.to_string()))
}

#[derive(Serialize)]
struct SignedPayload<'a> {
    domain: &'static str,
    payload: &'a DeviceKeySignPayload,
}

#[allow(dead_code)]
fn sec1_public_key_to_spki_der(sec1_public_key: &[u8]) -> Result<Vec<u8>, DeviceKeyError> {
    let public_key = p256::PublicKey::from_sec1_bytes(sec1_public_key)
        .map_err(|err| DeviceKeyError::Crypto(err.to_string()))?;
    public_key
        .to_public_key_der()
        .map(|der| der.as_bytes().to_vec())
        .map_err(|err| DeviceKeyError::Crypto(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::Signature;
    use p256::ecdsa::SigningKey;
    use p256::ecdsa::VerifyingKey;
    use p256::ecdsa::signature::Signer;
    use p256::ecdsa::signature::Verifier;
    use p256::elliptic_curve::rand_core::OsRng;
    use p256::pkcs8::DecodePublicKey;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::sync::Mutex;

    const TEST_TOKEN_SHA256_BASE64URL: &str = "47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU";
    const TEST_NONCE_BASE64URL: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

    #[derive(Debug)]
    struct MemoryProvider {
        class: DeviceKeyProtectionClass,
        keys: Mutex<HashMap<String, SigningKey>>,
        bindings: Mutex<HashMap<String, DeviceKeyBinding>>,
    }

    impl MemoryProvider {
        fn new(class: DeviceKeyProtectionClass) -> Self {
            Self {
                class,
                keys: Mutex::new(HashMap::new()),
                bindings: Mutex::new(HashMap::new()),
            }
        }
    }

    impl DeviceKeyProvider for MemoryProvider {
        fn create(
            &self,
            request: ProviderCreateRequest<'_>,
        ) -> Result<DeviceKeyInfo, DeviceKeyError> {
            if !request.protection_policy.allows(self.class) {
                return Err(DeviceKeyError::DegradedProtectionNotAllowed {
                    available: self.class,
                });
            }
            let key_id = request.key_id_for(self.class);
            let mut keys = self
                .keys
                .lock()
                .map_err(|err| DeviceKeyError::Platform(err.to_string()))?;
            let signing_key = keys
                .entry(key_id.clone())
                .or_insert_with(|| SigningKey::random(&mut OsRng));
            self.bindings
                .lock()
                .map_err(|err| DeviceKeyError::Platform(err.to_string()))?
                .insert(key_id.clone(), request.binding.clone());
            memory_key_info(&key_id, signing_key, self.class)
        }

        fn get_public(
            &self,
            key_id: &str,
            protection_class: DeviceKeyProtectionClass,
        ) -> Result<DeviceKeyInfo, DeviceKeyError> {
            if protection_class != self.class {
                return Err(DeviceKeyError::KeyNotFound);
            }
            let keys = self
                .keys
                .lock()
                .map_err(|err| DeviceKeyError::Platform(err.to_string()))?;
            let signing_key = keys.get(key_id).ok_or(DeviceKeyError::KeyNotFound)?;
            memory_key_info(key_id, signing_key, self.class)
        }

        fn binding(
            &self,
            key_id: &str,
            protection_class: DeviceKeyProtectionClass,
        ) -> Result<DeviceKeyBinding, DeviceKeyError> {
            if protection_class != self.class {
                return Err(DeviceKeyError::KeyNotFound);
            }
            self.bindings
                .lock()
                .map_err(|err| DeviceKeyError::Platform(err.to_string()))?
                .get(key_id)
                .cloned()
                .ok_or(DeviceKeyError::KeyNotFound)
        }

        fn sign(
            &self,
            key_id: &str,
            protection_class: DeviceKeyProtectionClass,
            payload: &[u8],
        ) -> Result<ProviderSignature, DeviceKeyError> {
            if protection_class != self.class {
                return Err(DeviceKeyError::KeyNotFound);
            }
            let keys = self
                .keys
                .lock()
                .map_err(|err| DeviceKeyError::Platform(err.to_string()))?;
            let signing_key = keys.get(key_id).ok_or(DeviceKeyError::KeyNotFound)?;
            let signature: Signature = signing_key.sign(payload);
            Ok(ProviderSignature {
                signature_der: signature.to_der().as_bytes().to_vec(),
                algorithm: DeviceKeyAlgorithm::EcdsaP256Sha256,
            })
        }
    }

    fn memory_key_info(
        key_id: &str,
        signing_key: &SigningKey,
        class: DeviceKeyProtectionClass,
    ) -> Result<DeviceKeyInfo, DeviceKeyError> {
        let public_key_spki_der = signing_key
            .verifying_key()
            .to_public_key_der()
            .map_err(|err| DeviceKeyError::Crypto(err.to_string()))?
            .as_bytes()
            .to_vec();
        Ok(DeviceKeyInfo {
            key_id: key_id.to_string(),
            public_key_spki_der,
            algorithm: DeviceKeyAlgorithm::EcdsaP256Sha256,
            protection_class: class,
        })
    }

    fn store(class: DeviceKeyProtectionClass) -> DeviceKeyStore {
        DeviceKeyStore::with_provider(Arc::new(MemoryProvider::new(class)))
    }

    fn create_request(protection_policy: DeviceKeyProtectionPolicy) -> DeviceKeyCreateRequest {
        DeviceKeyCreateRequest {
            protection_policy,
            binding: DeviceKeyBinding {
                account_user_id: "account-user-1".to_string(),
                client_id: "cli_123".to_string(),
            },
        }
    }

    fn remote_control_client_connection_payload() -> DeviceKeySignPayload {
        DeviceKeySignPayload::RemoteControlClientConnection(
            RemoteControlClientConnectionSignPayload {
                nonce: TEST_NONCE_BASE64URL.to_string(),
                audience: RemoteControlClientConnectionAudience::RemoteControlClientWebsocket,
                session_id: "wssess_123".to_string(),
                target_origin: "https://chatgpt.com".to_string(),
                target_path: "/api/codex/remote/control/client".to_string(),
                account_user_id: "account-user-1".to_string(),
                client_id: "cli_123".to_string(),
                token_sha256_base64url: TEST_TOKEN_SHA256_BASE64URL.to_string(),
                token_expires_at: current_unix_seconds().expect("time should be valid") + 60,
                scopes: vec![REMOTE_CONTROL_CONTROLLER_WEBSOCKET_SCOPE.to_string()],
            },
        )
    }

    fn remote_control_client_enrollment_payload() -> DeviceKeySignPayload {
        DeviceKeySignPayload::RemoteControlClientEnrollment(
            RemoteControlClientEnrollmentSignPayload {
                nonce: TEST_NONCE_BASE64URL.to_string(),
                audience: RemoteControlClientEnrollmentAudience::RemoteControlClientEnrollment,
                challenge_id: "rch_123".to_string(),
                target_origin: "https://chatgpt.com".to_string(),
                target_path: "/wham/remote/control/client/enroll".to_string(),
                account_user_id: "account-user-1".to_string(),
                client_id: "cli_123".to_string(),
                device_identity_sha256_base64url: TEST_TOKEN_SHA256_BASE64URL.to_string(),
                challenge_expires_at: current_unix_seconds().expect("time should be valid") + 60,
            },
        )
    }

    fn assert_valid_generated_key_id(key_id: &str, expected_class: DeviceKeyProtectionClass) {
        assert_eq!(key_id.len(), DEVICE_KEY_ID_LEN);
        assert_eq!(
            validate_key_id(key_id).expect("generated key id should be valid"),
            expected_class
        );
        let encoded_key = key_id
            .strip_prefix(expected_class.key_id_prefix())
            .expect("generated key id should use protection-class prefix");
        assert_eq!(encoded_key.len(), DEVICE_KEY_ID_ENCODED_BYTES);
        assert_eq!(
            URL_SAFE_NO_PAD
                .decode(encoded_key)
                .expect("generated key id should be base64url")
                .len(),
            DEVICE_KEY_ID_RANDOM_BYTES
        );
    }

    #[test]
    fn create_requires_explicit_degraded_protection() {
        let err = store(DeviceKeyProtectionClass::OsProtectedNonextractable)
            .create(create_request(DeviceKeyProtectionPolicy::HardwareOnly))
            .expect_err("OS-protected fallback should require opt-in");

        assert!(
            matches!(
                err,
                DeviceKeyError::DegradedProtectionNotAllowed {
                    available: DeviceKeyProtectionClass::OsProtectedNonextractable,
                }
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn create_allows_os_protected_nonextractable_policy() {
        let info = store(DeviceKeyProtectionClass::OsProtectedNonextractable)
            .create(create_request(
                DeviceKeyProtectionPolicy::AllowOsProtectedNonextractable,
            ))
            .expect("OS-protected fallback should be allowed by policy");

        assert_eq!(
            info.protection_class,
            DeviceKeyProtectionClass::OsProtectedNonextractable
        );
        assert_valid_generated_key_id(
            &info.key_id,
            DeviceKeyProtectionClass::OsProtectedNonextractable,
        );
    }

    #[test]
    fn create_generates_distinct_key_ids() {
        let store = store(DeviceKeyProtectionClass::HardwareTpm);
        let first = store
            .create(create_request(DeviceKeyProtectionPolicy::HardwareOnly))
            .expect("create should succeed");
        let second = store
            .create(create_request(DeviceKeyProtectionPolicy::HardwareOnly))
            .expect("create should succeed");

        assert_ne!(second.key_id, first.key_id);
        assert_valid_generated_key_id(&first.key_id, DeviceKeyProtectionClass::HardwareTpm);
        assert_valid_generated_key_id(&second.key_id, DeviceKeyProtectionClass::HardwareTpm);
    }

    #[test]
    fn key_id_validation_rejects_untrusted_namespaces() {
        let valid_suffix = URL_SAFE_NO_PAD.encode([0_u8; DEVICE_KEY_ID_RANDOM_BYTES]);

        for key_id in [
            String::new(),
            "dk_".to_string(),
            "dk_hse_".to_string(),
            format!("bad_{valid_suffix}"),
            format!("dk_bad_{valid_suffix}"),
            format!(
                "{}{}",
                DeviceKeyProtectionClass::HardwareSecureEnclave.key_id_prefix(),
                &valid_suffix[..DEVICE_KEY_ID_ENCODED_BYTES - 1]
            ),
            format!(
                "{}{valid_suffix}A",
                DeviceKeyProtectionClass::HardwareTpm.key_id_prefix()
            ),
            format!(
                "{}{}=",
                DeviceKeyProtectionClass::OsProtectedNonextractable.key_id_prefix(),
                &valid_suffix[..DEVICE_KEY_ID_ENCODED_BYTES - 1]
            ),
            format!(
                "{}{}+",
                DeviceKeyProtectionClass::HardwareSecureEnclave.key_id_prefix(),
                &valid_suffix[..DEVICE_KEY_ID_ENCODED_BYTES - 1]
            ),
        ] {
            let err = validate_key_id(&key_id).expect_err("malformed key id should fail");
            assert!(
                matches!(
                    err,
                    DeviceKeyError::InvalidPayload(INVALID_DEVICE_KEY_ID_MESSAGE)
                ),
                "unexpected error for {key_id:?}: {err:?}"
            );
        }
    }

    #[test]
    fn public_operations_reject_malformed_key_id_before_provider_use() {
        let store = store(DeviceKeyProtectionClass::HardwareTpm);
        let malformed_key_id = "not-a-device-key".to_string();

        let err = store
            .get_public(DeviceKeyGetPublicRequest {
                key_id: malformed_key_id.clone(),
            })
            .expect_err("malformed get_public key id should fail");
        assert!(
            matches!(
                err,
                DeviceKeyError::InvalidPayload(INVALID_DEVICE_KEY_ID_MESSAGE)
            ),
            "unexpected get_public error: {err:?}"
        );

        let err = store
            .sign(DeviceKeySignRequest {
                key_id: malformed_key_id,
                payload: remote_control_client_connection_payload(),
            })
            .expect_err("malformed sign key id should fail");
        assert!(
            matches!(
                err,
                DeviceKeyError::InvalidPayload(INVALID_DEVICE_KEY_ID_MESSAGE)
            ),
            "unexpected sign error: {err:?}"
        );
    }

    #[test]
    fn sign_rejects_empty_account_user_id() {
        let store = store(DeviceKeyProtectionClass::HardwareTpm);
        let info = store
            .create(create_request(DeviceKeyProtectionPolicy::HardwareOnly))
            .expect("create should succeed");
        let mut payload = remote_control_client_connection_payload();
        match &mut payload {
            DeviceKeySignPayload::RemoteControlClientConnection(connection_payload) => {
                connection_payload.account_user_id.clear();
            }
            DeviceKeySignPayload::RemoteControlClientEnrollment(_) => unreachable!(),
        }

        let err = store
            .sign(DeviceKeySignRequest {
                key_id: info.key_id,
                payload,
            })
            .expect_err("empty account user id should fail");

        assert!(
            matches!(
                err,
                DeviceKeyError::InvalidPayload("accountUserId must not be empty")
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn sign_uses_structured_payload() {
        let store = store(DeviceKeyProtectionClass::HardwareTpm);
        let info = store
            .create(create_request(DeviceKeyProtectionPolicy::HardwareOnly))
            .expect("create should succeed");
        let payload = remote_control_client_connection_payload();
        let signed_payload =
            device_key_signing_payload_bytes(&payload).expect("payload should serialize");
        let signature = store
            .sign(DeviceKeySignRequest {
                key_id: info.key_id,
                payload,
            })
            .expect("sign should succeed");
        assert_eq!(signature.signed_payload, signed_payload);

        let verifying_key = VerifyingKey::from_public_key_der(&info.public_key_spki_der)
            .expect("public key should decode");
        let signature =
            Signature::from_der(&signature.signature_der).expect("signature should decode");
        verifying_key
            .verify(&signed_payload, &signature)
            .expect("signature should verify against structured payload");
    }

    #[test]
    fn signing_payload_bytes_are_stable() {
        let payload = DeviceKeySignPayload::RemoteControlClientConnection(
            RemoteControlClientConnectionSignPayload {
                nonce: TEST_NONCE_BASE64URL.to_string(),
                audience: RemoteControlClientConnectionAudience::RemoteControlClientWebsocket,
                session_id: "wssess_123".to_string(),
                target_origin: "https://chatgpt.com".to_string(),
                target_path: "/api/codex/remote/control/client".to_string(),
                account_user_id: "account-user-1".to_string(),
                client_id: "cli_123".to_string(),
                token_sha256_base64url: TEST_TOKEN_SHA256_BASE64URL.to_string(),
                token_expires_at: 1_700_000_000,
                scopes: vec![REMOTE_CONTROL_CONTROLLER_WEBSOCKET_SCOPE.to_string()],
            },
        );

        let bytes = device_key_signing_payload_bytes(&payload).expect("payload should serialize");

        assert_eq!(
            String::from_utf8(bytes).expect("payload should be utf-8"),
            concat!(
                "{\"domain\":\"codex-device-key-sign-payload/v1\",",
                "\"payload\":{\"accountUserId\":\"account-user-1\",",
                "\"audience\":\"remote_control_client_websocket\",",
                "\"clientId\":\"cli_123\",",
                "\"nonce\":\"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\",",
                "\"scopes\":[\"remote_control_controller_websocket\"],",
                "\"sessionId\":\"wssess_123\",",
                "\"targetOrigin\":\"https://chatgpt.com\",",
                "\"targetPath\":\"/api/codex/remote/control/client\",",
                "\"tokenExpiresAt\":1700000000,",
                "\"tokenSha256Base64url\":\"47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU\",",
                "\"type\":\"remoteControlClientConnection\"}}"
            )
        );
    }

    #[test]
    fn enrollment_signing_payload_bytes_are_stable() {
        let payload = DeviceKeySignPayload::RemoteControlClientEnrollment(
            RemoteControlClientEnrollmentSignPayload {
                nonce: TEST_NONCE_BASE64URL.to_string(),
                audience: RemoteControlClientEnrollmentAudience::RemoteControlClientEnrollment,
                challenge_id: "rch_123".to_string(),
                target_origin: "https://chatgpt.com".to_string(),
                target_path: "/wham/remote/control/client/enroll".to_string(),
                account_user_id: "account-user-1".to_string(),
                client_id: "cli_123".to_string(),
                device_identity_sha256_base64url: TEST_TOKEN_SHA256_BASE64URL.to_string(),
                challenge_expires_at: 1_700_000_060,
            },
        );

        let bytes = device_key_signing_payload_bytes(&payload).expect("payload should serialize");

        assert_eq!(
            String::from_utf8(bytes).expect("payload should be utf-8"),
            concat!(
                "{\"domain\":\"codex-device-key-sign-payload/v1\",",
                "\"payload\":{\"accountUserId\":\"account-user-1\",",
                "\"audience\":\"remote_control_client_enrollment\",",
                "\"challengeExpiresAt\":1700000060,",
                "\"challengeId\":\"rch_123\",",
                "\"clientId\":\"cli_123\",",
                "\"deviceIdentitySha256Base64url\":\"47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU\",",
                "\"nonce\":\"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\",",
                "\"targetOrigin\":\"https://chatgpt.com\",",
                "\"targetPath\":\"/wham/remote/control/client/enroll\",",
                "\"type\":\"remoteControlClientEnrollment\"}}"
            )
        );
    }

    #[test]
    fn sign_rejects_malformed_token_hash() {
        let store = store(DeviceKeyProtectionClass::HardwareTpm);
        let info = store
            .create(create_request(DeviceKeyProtectionPolicy::HardwareOnly))
            .expect("create should succeed");
        let mut payload = remote_control_client_connection_payload();
        match &mut payload {
            DeviceKeySignPayload::RemoteControlClientConnection(connection_payload) => {
                connection_payload.token_sha256_base64url = "not-a-sha256".to_string();
            }
            DeviceKeySignPayload::RemoteControlClientEnrollment(_) => unreachable!(),
        }

        let err = store
            .sign(DeviceKeySignRequest {
                key_id: info.key_id,
                payload,
            })
            .expect_err("malformed token hash should fail");

        assert!(
            matches!(
                err,
                DeviceKeyError::InvalidPayload(
                    "tokenSha256Base64url must be a SHA-256 digest encoded as unpadded base64url"
                )
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn sign_rejects_unexpected_scopes() {
        let store = store(DeviceKeyProtectionClass::HardwareTpm);
        let info = store
            .create(create_request(DeviceKeyProtectionPolicy::HardwareOnly))
            .expect("create should succeed");
        let mut payload = remote_control_client_connection_payload();
        match &mut payload {
            DeviceKeySignPayload::RemoteControlClientConnection(connection_payload) => {
                connection_payload.scopes = vec!["other_scope".to_string()];
            }
            DeviceKeySignPayload::RemoteControlClientEnrollment(_) => unreachable!(),
        }

        let err = store
            .sign(DeviceKeySignRequest {
                key_id: info.key_id,
                payload,
            })
            .expect_err("unexpected scope should fail");

        assert!(
            matches!(
                err,
                DeviceKeyError::InvalidPayload(
                    "scopes must contain exactly remote_control_controller_websocket"
                )
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn sign_rejects_malformed_enrollment_identity_hash() {
        let store = store(DeviceKeyProtectionClass::HardwareTpm);
        let info = store
            .create(create_request(DeviceKeyProtectionPolicy::HardwareOnly))
            .expect("create should succeed");
        let mut payload = remote_control_client_enrollment_payload();
        match &mut payload {
            DeviceKeySignPayload::RemoteControlClientEnrollment(enrollment_payload) => {
                enrollment_payload.device_identity_sha256_base64url = "not-a-sha256".to_string();
            }
            DeviceKeySignPayload::RemoteControlClientConnection(_) => unreachable!(),
        }

        let err = store
            .sign(DeviceKeySignRequest {
                key_id: info.key_id,
                payload,
            })
            .expect_err("malformed device identity hash should fail");

        assert!(
            matches!(
                err,
                DeviceKeyError::InvalidPayload(
                    "deviceIdentitySha256Base64url must be a SHA-256 digest encoded as unpadded base64url"
                )
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn sign_rejects_empty_target_binding() {
        let store = store(DeviceKeyProtectionClass::HardwareTpm);
        let info = store
            .create(create_request(DeviceKeyProtectionPolicy::HardwareOnly))
            .expect("create should succeed");
        let mut payload = remote_control_client_connection_payload();
        match &mut payload {
            DeviceKeySignPayload::RemoteControlClientConnection(connection_payload) => {
                connection_payload.target_origin.clear();
            }
            DeviceKeySignPayload::RemoteControlClientEnrollment(_) => unreachable!(),
        }

        let err = store
            .sign(DeviceKeySignRequest {
                key_id: info.key_id,
                payload,
            })
            .expect_err("empty target origin should fail");

        assert!(
            matches!(
                err,
                DeviceKeyError::InvalidPayload(
                    "targetOrigin must be an allowed remote-control backend origin"
                )
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn sign_rejects_remote_control_paths_for_other_payload_shapes() {
        let store = store(DeviceKeyProtectionClass::HardwareTpm);
        let info = store
            .create(create_request(DeviceKeyProtectionPolicy::HardwareOnly))
            .expect("create should succeed");
        let mut connection_payload = remote_control_client_connection_payload();
        match &mut connection_payload {
            DeviceKeySignPayload::RemoteControlClientConnection(payload) => {
                payload.target_path = "/api/codex/remote/control/client/enroll".to_string();
            }
            DeviceKeySignPayload::RemoteControlClientEnrollment(_) => unreachable!(),
        }

        let err = store
            .sign(DeviceKeySignRequest {
                key_id: info.key_id.clone(),
                payload: connection_payload,
            })
            .expect_err("connection payload should reject enrollment path");
        assert!(
            matches!(
                err,
                DeviceKeyError::InvalidPayload(
                    "targetPath must match the signed payload type's remote-control endpoint"
                )
            ),
            "unexpected connection path error: {err:?}"
        );

        let mut enrollment_payload = remote_control_client_enrollment_payload();
        match &mut enrollment_payload {
            DeviceKeySignPayload::RemoteControlClientEnrollment(payload) => {
                payload.target_path = "/wham/remote/control/client".to_string();
            }
            DeviceKeySignPayload::RemoteControlClientConnection(_) => unreachable!(),
        }

        let err = store
            .sign(DeviceKeySignRequest {
                key_id: info.key_id,
                payload: enrollment_payload,
            })
            .expect_err("enrollment payload should reject connection path");
        assert!(
            matches!(
                err,
                DeviceKeyError::InvalidPayload(
                    "targetPath must match the signed payload type's remote-control endpoint"
                )
            ),
            "unexpected enrollment path error: {err:?}"
        );
    }

    #[test]
    fn remote_control_origin_matches_remote_transport_allowlist() {
        for origin in [
            "https://chatgpt.com",
            "https://chatgpt-staging.com",
            "https://ab.chatgpt.com",
            "https://ab.chatgpt-staging.com",
            "http://localhost:8080",
            "https://localhost:8443",
            "http://127.0.0.1:8080",
            "http://[::1]:8080",
        ] {
            assert!(
                is_allowed_remote_control_origin(origin),
                "expected allowed origin: {origin}"
            );
        }

        for origin in [
            "http://chatgpt.com",
            "https://chat.openai.com",
            "https://api.openai.com",
            "https://chatgpt.com.evil.com",
            "https://evilchatgpt.com",
            "https://foo.localhost",
            "https://localhost.evil.com",
            "https://192.168.1.2",
            "https://chatgpt.com/backend-api",
            "https://chatgpt.com?query=1",
        ] {
            assert!(
                !is_allowed_remote_control_origin(origin),
                "expected rejected origin: {origin}"
            );
        }
    }

    #[test]
    fn sign_rejects_empty_session_binding() {
        let store = store(DeviceKeyProtectionClass::HardwareTpm);
        let info = store
            .create(create_request(DeviceKeyProtectionPolicy::HardwareOnly))
            .expect("create should succeed");
        let mut payload = remote_control_client_connection_payload();
        match &mut payload {
            DeviceKeySignPayload::RemoteControlClientConnection(connection_payload) => {
                connection_payload.session_id.clear();
            }
            DeviceKeySignPayload::RemoteControlClientEnrollment(_) => unreachable!(),
        }

        let err = store
            .sign(DeviceKeySignRequest {
                key_id: info.key_id,
                payload,
            })
            .expect_err("empty session id should fail");

        assert!(
            matches!(
                err,
                DeviceKeyError::InvalidPayload("sessionId must not be empty")
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn sign_rejects_empty_client_id() {
        let store = store(DeviceKeyProtectionClass::HardwareTpm);
        let info = store
            .create(create_request(DeviceKeyProtectionPolicy::HardwareOnly))
            .expect("create should succeed");
        let mut payload = remote_control_client_connection_payload();
        match &mut payload {
            DeviceKeySignPayload::RemoteControlClientConnection(connection_payload) => {
                connection_payload.client_id.clear();
            }
            DeviceKeySignPayload::RemoteControlClientEnrollment(_) => unreachable!(),
        }

        let err = store
            .sign(DeviceKeySignRequest {
                key_id: info.key_id,
                payload,
            })
            .expect_err("empty client id should fail");

        assert!(
            matches!(
                err,
                DeviceKeyError::InvalidPayload("clientId must not be empty")
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn sign_rejects_mismatched_binding() {
        let store = store(DeviceKeyProtectionClass::HardwareTpm);
        let info = store
            .create(create_request(DeviceKeyProtectionPolicy::HardwareOnly))
            .expect("create should succeed");
        let mut payload = remote_control_client_connection_payload();
        match &mut payload {
            DeviceKeySignPayload::RemoteControlClientConnection(connection_payload) => {
                connection_payload.account_user_id = "other-account-user".to_string();
            }
            DeviceKeySignPayload::RemoteControlClientEnrollment(_) => unreachable!(),
        }

        let err = store
            .sign(DeviceKeySignRequest {
                key_id: info.key_id,
                payload,
            })
            .expect_err("mismatched binding should fail");

        assert!(
            matches!(
                err,
                DeviceKeyError::InvalidPayload(
                    "payload accountUserId/clientId does not match device key binding"
                )
            ),
            "unexpected error: {err:?}"
        );
    }
}
