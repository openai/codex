use crate::error_code::INTERNAL_ERROR_CODE;
use crate::error_code::INVALID_REQUEST_ERROR_CODE;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
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

#[derive(Clone, Default)]
pub(crate) struct DeviceKeyApi {
    store: DeviceKeyStore,
}

impl DeviceKeyApi {
    pub(crate) fn create(
        &self,
        params: DeviceKeyCreateParams,
    ) -> Result<DeviceKeyCreateResponse, JSONRPCErrorError> {
        let info = self
            .store
            .create(DeviceKeyCreateRequest {
                protection_policy: protection_policy_from_params(params.protection_policy),
                binding: DeviceKeyBinding {
                    account_user_id: params.account_user_id,
                    client_id: params.client_id,
                },
            })
            .map_err(map_device_key_error)?;
        Ok(create_response_from_info(info))
    }

    pub(crate) fn public(
        &self,
        params: DeviceKeyPublicParams,
    ) -> Result<DeviceKeyPublicResponse, JSONRPCErrorError> {
        let info = self
            .store
            .get_public(DeviceKeyGetPublicRequest {
                key_id: params.key_id,
            })
            .map_err(map_device_key_error)?;
        Ok(public_response_from_info(info))
    }

    pub(crate) fn sign(
        &self,
        params: DeviceKeySignParams,
    ) -> Result<DeviceKeySignResponse, JSONRPCErrorError> {
        let signature = self
            .store
            .sign(DeviceKeySignRequest {
                key_id: params.key_id,
                payload: payload_from_params(params.payload),
            })
            .map_err(map_device_key_error)?;
        Ok(DeviceKeySignResponse {
            signature_der_base64: STANDARD.encode(signature.signature_der),
            signed_payload_base64: STANDARD.encode(signature.signed_payload),
            algorithm: algorithm_from_store(signature.algorithm),
        })
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
    let code = match error {
        DeviceKeyError::DegradedProtectionNotAllowed { .. }
        | DeviceKeyError::HardwareBackedKeysUnavailable
        | DeviceKeyError::KeyNotFound
        | DeviceKeyError::InvalidPayload(_) => INVALID_REQUEST_ERROR_CODE,
        DeviceKeyError::Platform(_) | DeviceKeyError::Crypto(_) => INTERNAL_ERROR_CODE,
    };
    JSONRPCErrorError {
        code,
        message: error.to_string(),
        data: None,
    }
}
