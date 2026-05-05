use std::fmt;
use std::future::Future;
use std::sync::Arc;

use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use codex_app_server_protocol::ClientResponsePayload;
use codex_app_server_protocol::DeviceKeyAlgorithm;
use codex_app_server_protocol::DeviceKeyCreateParams;
use codex_app_server_protocol::DeviceKeyCreateResponse;
use codex_app_server_protocol::DeviceKeyProtectionClass;
use codex_app_server_protocol::DeviceKeyPublicParams;
use codex_app_server_protocol::DeviceKeyPublicResponse;
use codex_app_server_protocol::DeviceKeySignParams;
use codex_app_server_protocol::DeviceKeySignPayload;
use codex_app_server_protocol::DeviceKeySignResponse;
use codex_app_server_protocol::JSONRPCErrorError;
use codex_device_key::DeviceKeyBinding;
use codex_device_key::DeviceKeyBindingStore;
use codex_device_key::DeviceKeyCreateRequest;
use codex_device_key::DeviceKeyError;
use codex_device_key::DeviceKeyGetPublicRequest;
use codex_device_key::DeviceKeyInfo;
use codex_device_key::DeviceKeyProtectionPolicy;
use codex_device_key::DeviceKeySignRequest;
use codex_device_key::DeviceKeyStore;
use codex_device_key::RemoteControlClientConnectionAudience;
use codex_device_key::RemoteControlClientConnectionSignPayload;
use codex_device_key::RemoteControlClientEnrollmentAudience;
use codex_device_key::RemoteControlClientEnrollmentSignPayload;
use codex_rollout::state_db::StateDbHandle;
use codex_state::DeviceKeyBindingRecord;

#[derive(Clone)]
pub(crate) struct DeviceKeyRequestProcessor {
    outgoing: Arc<OutgoingMessageSender>,
    store: DeviceKeyStore,
}

impl DeviceKeyRequestProcessor {
    pub(crate) fn new(outgoing: Arc<OutgoingMessageSender>, state_db: StateDbHandle) -> Self {
        Self {
            outgoing,
            store: DeviceKeyStore::new(Arc::new(StateDeviceKeyBindingStore::new(state_db))),
        }
    }

    pub(crate) fn create(
        &self,
        request_id: ConnectionRequestId,
        params: DeviceKeyCreateParams,
        device_key_requests_allowed: bool,
    ) {
        self.spawn_request(
            request_id,
            "device/key/create",
            device_key_requests_allowed,
            move |store| async move { create_device_key(store, params).await },
        );
    }

    pub(crate) fn public(
        &self,
        request_id: ConnectionRequestId,
        params: DeviceKeyPublicParams,
        device_key_requests_allowed: bool,
    ) {
        self.spawn_request(
            request_id,
            "device/key/public",
            device_key_requests_allowed,
            move |store| async move { public_device_key(store, params).await },
        );
    }

    pub(crate) fn sign(
        &self,
        request_id: ConnectionRequestId,
        params: DeviceKeySignParams,
        device_key_requests_allowed: bool,
    ) {
        self.spawn_request(
            request_id,
            "device/key/sign",
            device_key_requests_allowed,
            move |store| async move { sign_device_key(store, params).await },
        );
    }

    fn spawn_request<R, F, Fut>(
        &self,
        request_id: ConnectionRequestId,
        method: &'static str,
        device_key_requests_allowed: bool,
        run_request: F,
    ) where
        R: Into<ClientResponsePayload> + Send + 'static,
        F: FnOnce(DeviceKeyStore) -> Fut + Send + 'static,
        Fut: Future<Output = Result<R, JSONRPCErrorError>> + Send + 'static,
    {
        let store = self.store.clone();
        let outgoing = Arc::clone(&self.outgoing);
        tokio::spawn(async move {
            let result = if !device_key_requests_allowed {
                Err(invalid_request(format!(
                    "{method} is not available over remote transports"
                )))
            } else {
                run_request(store).await
            };
            outgoing.send_result(request_id, result).await;
        });
    }
}

async fn create_device_key(
    store: DeviceKeyStore,
    params: DeviceKeyCreateParams,
) -> Result<DeviceKeyCreateResponse, JSONRPCErrorError> {
    let info = store
        .create(DeviceKeyCreateRequest {
            protection_policy: protection_policy_from_params(params.protection_policy),
            binding: DeviceKeyBinding {
                account_user_id: params.account_user_id,
                client_id: params.client_id,
            },
        })
        .await
        .map_err(map_device_key_error)?;
    Ok(create_response_from_info(info))
}

async fn public_device_key(
    store: DeviceKeyStore,
    params: DeviceKeyPublicParams,
) -> Result<DeviceKeyPublicResponse, JSONRPCErrorError> {
    let info = store
        .get_public(DeviceKeyGetPublicRequest {
            key_id: params.key_id,
        })
        .await
        .map_err(map_device_key_error)?;
    Ok(public_response_from_info(info))
}

async fn sign_device_key(
    store: DeviceKeyStore,
    params: DeviceKeySignParams,
) -> Result<DeviceKeySignResponse, JSONRPCErrorError> {
    let signature = store
        .sign(DeviceKeySignRequest {
            key_id: params.key_id,
            payload: payload_from_params(params.payload),
        })
        .await
        .map_err(map_device_key_error)?;
    Ok(DeviceKeySignResponse {
        signature_der_base64: STANDARD.encode(signature.signature_der),
        signed_payload_base64: STANDARD.encode(signature.signed_payload),
        algorithm: algorithm_from_store(signature.algorithm),
    })
}

struct StateDeviceKeyBindingStore {
    state_db: StateDbHandle,
}

impl StateDeviceKeyBindingStore {
    fn new(state_db: StateDbHandle) -> Self {
        Self { state_db }
    }
}

impl fmt::Debug for StateDeviceKeyBindingStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StateDeviceKeyBindingStore")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl DeviceKeyBindingStore for StateDeviceKeyBindingStore {
    async fn get_binding(&self, key_id: &str) -> Result<Option<DeviceKeyBinding>, DeviceKeyError> {
        let state_db = self.state_db.clone();
        state_db
            .get_device_key_binding(key_id)
            .await
            .map(|record| {
                record.map(|record| DeviceKeyBinding {
                    account_user_id: record.account_user_id,
                    client_id: record.client_id,
                })
            })
            .map_err(|err| DeviceKeyError::Platform(err.to_string()))
    }

    async fn put_binding(
        &self,
        key_id: &str,
        binding: &DeviceKeyBinding,
    ) -> Result<(), DeviceKeyError> {
        let state_db = self.state_db.clone();
        state_db
            .upsert_device_key_binding(&DeviceKeyBindingRecord {
                key_id: key_id.to_string(),
                account_user_id: binding.account_user_id.clone(),
                client_id: binding.client_id.clone(),
            })
            .await
            .map_err(|err| DeviceKeyError::Platform(err.to_string()))
    }
}

fn create_response_from_info(info: DeviceKeyInfo) -> DeviceKeyCreateResponse {
    DeviceKeyCreateResponse {
        key_id: info.key_id,
        public_key_spki_der_base64: STANDARD.encode(info.public_key_spki_der),
        algorithm: algorithm_from_store(info.algorithm),
        protection_class: protection_class_from_store(info.protection_class),
    }
}

fn public_response_from_info(info: DeviceKeyInfo) -> DeviceKeyPublicResponse {
    DeviceKeyPublicResponse {
        key_id: info.key_id,
        public_key_spki_der_base64: STANDARD.encode(info.public_key_spki_der),
        algorithm: algorithm_from_store(info.algorithm),
        protection_class: protection_class_from_store(info.protection_class),
    }
}

fn protection_policy_from_params(
    protection_policy: Option<codex_app_server_protocol::DeviceKeyProtectionPolicy>,
) -> DeviceKeyProtectionPolicy {
    match protection_policy
        .unwrap_or(codex_app_server_protocol::DeviceKeyProtectionPolicy::HardwareOnly)
    {
        codex_app_server_protocol::DeviceKeyProtectionPolicy::HardwareOnly => {
            DeviceKeyProtectionPolicy::HardwareOnly
        }
        codex_app_server_protocol::DeviceKeyProtectionPolicy::AllowOsProtectedNonextractable => {
            DeviceKeyProtectionPolicy::AllowOsProtectedNonextractable
        }
    }
}

fn payload_from_params(payload: DeviceKeySignPayload) -> codex_device_key::DeviceKeySignPayload {
    match payload {
        DeviceKeySignPayload::RemoteControlClientConnection {
            nonce,
            audience,
            session_id,
            target_origin,
            target_path,
            account_user_id,
            client_id,
            token_sha256_base64url,
            token_expires_at,
            scopes,
        } => codex_device_key::DeviceKeySignPayload::RemoteControlClientConnection(
            RemoteControlClientConnectionSignPayload {
                nonce,
                audience: remote_control_client_connection_audience_from_protocol(audience),
                session_id,
                target_origin,
                target_path,
                account_user_id,
                client_id,
                token_sha256_base64url,
                token_expires_at,
                scopes,
            },
        ),
        DeviceKeySignPayload::RemoteControlClientEnrollment {
            nonce,
            audience,
            challenge_id,
            target_origin,
            target_path,
            account_user_id,
            client_id,
            device_identity_sha256_base64url,
            challenge_expires_at,
        } => codex_device_key::DeviceKeySignPayload::RemoteControlClientEnrollment(
            RemoteControlClientEnrollmentSignPayload {
                nonce,
                audience: remote_control_client_enrollment_audience_from_protocol(audience),
                challenge_id,
                target_origin,
                target_path,
                account_user_id,
                client_id,
                device_identity_sha256_base64url,
                challenge_expires_at,
            },
        ),
    }
}

fn remote_control_client_connection_audience_from_protocol(
    audience: codex_app_server_protocol::RemoteControlClientConnectionAudience,
) -> RemoteControlClientConnectionAudience {
    match audience {
        codex_app_server_protocol::RemoteControlClientConnectionAudience::RemoteControlClientWebsocket => {
            RemoteControlClientConnectionAudience::RemoteControlClientWebsocket
        }
    }
}

fn remote_control_client_enrollment_audience_from_protocol(
    audience: codex_app_server_protocol::RemoteControlClientEnrollmentAudience,
) -> RemoteControlClientEnrollmentAudience {
    match audience {
        codex_app_server_protocol::RemoteControlClientEnrollmentAudience::RemoteControlClientEnrollment => {
            RemoteControlClientEnrollmentAudience::RemoteControlClientEnrollment
        }
    }
}

fn algorithm_from_store(algorithm: codex_device_key::DeviceKeyAlgorithm) -> DeviceKeyAlgorithm {
    match algorithm {
        codex_device_key::DeviceKeyAlgorithm::EcdsaP256Sha256 => {
            DeviceKeyAlgorithm::EcdsaP256Sha256
        }
    }
}

fn protection_class_from_store(
    protection_class: codex_device_key::DeviceKeyProtectionClass,
) -> DeviceKeyProtectionClass {
    match protection_class {
        codex_device_key::DeviceKeyProtectionClass::HardwareSecureEnclave => {
            DeviceKeyProtectionClass::HardwareSecureEnclave
        }
        codex_device_key::DeviceKeyProtectionClass::HardwareTpm => {
            DeviceKeyProtectionClass::HardwareTpm
        }
        codex_device_key::DeviceKeyProtectionClass::OsProtectedNonextractable => {
            DeviceKeyProtectionClass::OsProtectedNonextractable
        }
    }
}

fn map_device_key_error(error: DeviceKeyError) -> JSONRPCErrorError {
    match &error {
        DeviceKeyError::DegradedProtectionNotAllowed { .. }
        | DeviceKeyError::HardwareBackedKeysUnavailable
        | DeviceKeyError::KeyNotFound
        | DeviceKeyError::InvalidPayload(_) => invalid_request(error.to_string()),
        DeviceKeyError::Platform(_) | DeviceKeyError::Crypto(_) => {
            internal_error(error.to_string())
        }
    }
}
