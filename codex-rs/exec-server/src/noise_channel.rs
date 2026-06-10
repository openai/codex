//! Narrow, misuse-resistant wrapper around the Clatter primitives used by the
//! remote exec-server relay.
//!
//! The harness initiates hybrid IK and pins the exec-server static key returned
//! by the registry. The first handshake message lets the exec-server authenticate
//! the harness static key; the exec-server then asks the registry whether that
//! key is authorized before completing the handshake.
//!
//! "Hybrid" means the session keys include both X25519 and ML-KEM-768 key
//! agreement. Once the two-message handshake finishes, AES-GCM protects the
//! ordered transport records carrying JSON-RPC.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use clatter::KeyPair;
use clatter::bytearray::ByteArray;
use clatter::crypto::dh::X25519;
use clatter::traits::Dh;
use clatter::traits::Kem;
use serde::Deserialize;
use serde::Serialize;

use crate::aws_lc_ml_kem::AwsLcMlKem768;

/// Identifies the handshake pattern and algorithms used by this channel.
pub const NOISE_CHANNEL_SUITE: &str = "Noise_hybridIK_X25519+MLKEM768_AESGCM_SHA256";

type DhKeyPair = KeyPair<<X25519 as Dh>::PubKey, <X25519 as Dh>::PrivateKey>;
type KemKeyPair = KeyPair<<AwsLcMlKem768 as Kem>::PubKey, <AwsLcMlKem768 as Kem>::SecretKey>;

/// Public key material for the exec-server Noise-over-relay suite.
///
/// The suite field is part of the serialized contract. A key from a different
/// suite must not be interpreted as compatible merely because one component has
/// a familiar byte length.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoiseChannelPublicKey {
    suite: String,
    x25519_public_key: String,
    mlkem768_public_key: String,
}

impl std::fmt::Debug for NoiseChannelPublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoiseChannelPublicKey")
            .field("suite", &self.suite)
            .field("x25519_public_key", &"<redacted>")
            .field("mlkem768_public_key", &"<redacted>")
            .finish()
    }
}

impl NoiseChannelPublicKey {
    /// Serialize both public components as one suite-tagged value.
    fn from_keypairs(dh: &DhKeyPair, kem: &KemKeyPair) -> Self {
        Self {
            suite: NOISE_CHANNEL_SUITE.to_string(),
            x25519_public_key: STANDARD.encode(dh.public),
            mlkem768_public_key: STANDARD.encode(kem.public.as_slice()),
        }
    }
}

/// Endpoint-local static identity for the exec-server Noise-over-relay suite.
///
/// Private components never cross the process boundary. Cloning is used only to
/// construct Clatter handshake state for reconnects within the same process.
#[derive(Clone)]
pub struct NoiseChannelIdentity {
    dh: DhKeyPair,
    kem: KemKeyPair,
}

impl std::fmt::Debug for NoiseChannelIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoiseChannelIdentity")
            .field("public_key", &self.public_key())
            .finish_non_exhaustive()
    }
}

impl NoiseChannelIdentity {
    /// Generate independent classical and post-quantum static keypairs.
    pub fn generate() -> Result<Self, NoiseChannelError> {
        let dh = X25519::genkey()
            .map_err(|error| NoiseChannelError::KeyGeneration(error.to_string()))?;
        let kem = AwsLcMlKem768::genkey()
            .map_err(|error| NoiseChannelError::KeyGeneration(error.to_string()))?;
        Ok(Self { dh, kem })
    }

    /// Return the distributable public half of this endpoint identity.
    pub fn public_key(&self) -> NoiseChannelPublicKey {
        NoiseChannelPublicKey::from_keypairs(&self.dh, &self.kem)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NoiseChannelError {
    #[error("Noise channel key generation failed: {0}")]
    KeyGeneration(String),
    #[error("invalid Noise channel public key: {0}")]
    InvalidPublicKey(&'static str),
    #[error("invalid Noise channel state: {0}")]
    InvalidState(&'static str),
    #[error("invalid Noise channel message: {0}")]
    InvalidMessage(&'static str),
    #[error("Noise channel handshake failed: {0}")]
    Handshake(String),
    #[error("Noise channel transport failed: {0}")]
    Transport(String),
}

impl From<clatter::error::HandshakeError> for NoiseChannelError {
    fn from(error: clatter::error::HandshakeError) -> Self {
        Self::Handshake(error.to_string())
    }
}

impl From<clatter::error::TransportError> for NoiseChannelError {
    fn from(error: clatter::error::TransportError) -> Self {
        Self::Transport(error.to_string())
    }
}

#[cfg(test)]
#[path = "noise_channel_tests.rs"]
mod tests;
