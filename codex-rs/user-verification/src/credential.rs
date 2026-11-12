//! Public credential encoding and results; private keys never cross this boundary.

use crate::UserVerificationError;
use crate::UserVerificationFailureReason;
use crate::UserVerificationUnavailableReason;
use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use p256::pkcs8::EncodePublicKey as _;
use sha2::Digest as _;
use sha2::Sha256;

/// Validated display text and exact challenge bytes supplied by the trusted calling UI.
#[derive(Clone)]
pub struct UserVerificationRequest {
    pub challenge: Vec<u8>,
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserVerificationKeyInfo {
    pub credential_id: String,
    pub algorithm: String,
    /// Unpadded base64url of the P-256 SubjectPublicKeyInfo DER encoding.
    pub public_key: String,
}

impl UserVerificationKeyInfo {
    pub fn from_sec1_public_key(bytes: &[u8]) -> Result<Self, UserVerificationError> {
        let public_key = p256::PublicKey::from_sec1_bytes(bytes)
            .map_err(|_| invalid_public_key())?
            .to_public_key_der()
            .map_err(|_| invalid_public_key())?;
        let bytes = public_key.as_bytes();
        Ok(Self {
            credential_id: URL_SAFE_NO_PAD.encode(Sha256::digest(bytes)),
            algorithm: "ecdsaP256Sha256X962".to_string(),
            public_key: URL_SAFE_NO_PAD.encode(bytes),
        })
    }
}

fn invalid_public_key() -> UserVerificationError {
    UserVerificationError::Failed {
        reason: UserVerificationFailureReason::ProviderError,
        message: "could not encode the user-verification public key".to_string(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserVerificationKeyCreation {
    pub created: bool,
    pub credential: UserVerificationKeyInfo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserVerificationKeyDeletion {
    pub deleted_credential_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserVerificationStatus {
    pub credential: Option<UserVerificationKeyInfo>,
    pub unavailable_reason: Option<UserVerificationUnavailableReason>,
    pub unavailable_message: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct UserVerificationProof {
    pub credential_id: String,
    /// Unpadded base64url of an ASN.1 DER ECDSA signature over the challenge, hashed once.
    pub signature: String,
    /// Method that authorized this signing operation, when the provider can establish it.
    pub biometric_type: Option<UserVerificationBiometricType>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserVerificationBiometricType {
    Fingerprint,
}

impl std::fmt::Debug for UserVerificationProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserVerificationProof")
            .field("credential_id", &"[REDACTED]")
            .field("signature", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
#[path = "credential_tests.rs"]
mod tests;
