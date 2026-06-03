//! Narrow, misuse-resistant wrapper around the Clatter primitives used by the
//! remote exec-server relay.
//!
//! The harness is the Noise hybrid-IK initiator and pins the registry-provided
//! exec-server static public key. The exec-server is the responder; after it
//! reads the first IK message, Noise exposes the authenticated harness static
//! public key for registry authorization. Application data is allowed only
//! after both the cryptographic handshake and that external authorization pass.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use clatter::HybridHandshake;
use clatter::HybridHandshakeParams;
use clatter::KeyPair;
use clatter::bytearray::ByteArray;
use clatter::constants::MAX_MESSAGE_LEN;
use clatter::crypto::cipher::AesGcm;
use clatter::crypto::dh::X25519;
use clatter::crypto::hash::Sha256;
use clatter::handshakepattern::noise_hybrid_ik;
use clatter::traits::Cipher;
use clatter::traits::Dh;
use clatter::traits::Handshaker;
use clatter::traits::Kem;
use clatter::transportstate::TransportState;
use serde::Deserialize;
use serde::Serialize;

use crate::aws_lc_ml_kem::AwsLcMlKem768;
use crate::aws_lc_ml_kem::PUBLIC_KEY_LEN as MLKEM768_PUBLIC_KEY_LEN;

pub const NOISE_CHANNEL_SUITE: &str = "Noise_hybridIK_X25519+MLKEM768_AESGCM_SHA256";

const X25519_PUBLIC_KEY_LEN: usize = 32;
const MAX_TRANSPORT_RECORDS_PER_DIRECTION: u64 = u32::MAX as u64;
const PROLOGUE_DOMAIN: &[u8] = b"codex-exec-server-relay-noise/v1";

type Handshake = HybridHandshake<X25519, AwsLcMlKem768, AwsLcMlKem768, AesGcm, Sha256>;
type Transport = TransportState<AesGcm, Sha256>;
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
    /// Validate suite selection, encoding, and exact key lengths.
    pub fn validate(&self) -> Result<(), NoiseChannelError> {
        let _ = self.decode()?;
        Ok(())
    }

    fn from_keypairs(dh: &DhKeyPair, kem: &KemKeyPair) -> Self {
        Self {
            suite: NOISE_CHANNEL_SUITE.to_string(),
            x25519_public_key: STANDARD.encode(dh.public),
            mlkem768_public_key: STANDARD.encode(kem.public.as_slice()),
        }
    }

    fn from_raw(dh: &<X25519 as Dh>::PubKey, kem: &<AwsLcMlKem768 as Kem>::PubKey) -> Self {
        Self {
            suite: NOISE_CHANNEL_SUITE.to_string(),
            x25519_public_key: STANDARD.encode(dh),
            mlkem768_public_key: STANDARD.encode(kem.as_slice()),
        }
    }

    fn decode(
        &self,
    ) -> Result<(<X25519 as Dh>::PubKey, <AwsLcMlKem768 as Kem>::PubKey), NoiseChannelError> {
        if self.suite != NOISE_CHANNEL_SUITE {
            return Err(NoiseChannelError::InvalidPublicKey(
                "unsupported Noise channel suite",
            ));
        }
        let dh = STANDARD
            .decode(&self.x25519_public_key)
            .map_err(|_| NoiseChannelError::InvalidPublicKey("invalid X25519 public key"))?;
        let dh: [u8; X25519_PUBLIC_KEY_LEN] = dh
            .try_into()
            .map_err(|_| NoiseChannelError::InvalidPublicKey("invalid X25519 public key length"))?;
        let kem = STANDARD
            .decode(&self.mlkem768_public_key)
            .map_err(|_| NoiseChannelError::InvalidPublicKey("invalid ML-KEM-768 public key"))?;
        if kem.len() != MLKEM768_PUBLIC_KEY_LEN {
            return Err(NoiseChannelError::InvalidPublicKey(
                "invalid ML-KEM-768 public key length",
            ));
        }

        Ok((
            dh,
            <AwsLcMlKem768 as Kem>::PubKey::from_slice(kem.as_slice()),
        ))
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

pub(crate) struct InitiatorHandshake {
    handshake: Handshake,
}

impl InitiatorHandshake {
    /// Start hybrid IK while pinning the expected responder static key.
    ///
    /// `payload` is encrypted by the first IK message. The relay uses it for
    /// the short-lived registry authorization naming this harness public key.
    pub(crate) fn start(
        identity: &NoiseChannelIdentity,
        responder_public_key: &NoiseChannelPublicKey,
        prologue: &[u8],
        payload: &[u8],
    ) -> Result<(Self, Vec<u8>), NoiseChannelError> {
        let (responder_dh, responder_kem) = responder_public_key.decode()?;

        // IK authenticates both static identities. Supplying both responder
        // components here is what makes a misrouted or impersonating
        // exec-server fail before any JSON-RPC plaintext is released.
        let params = HybridHandshakeParams::new(noise_hybrid_ik(), true)
            .with_prologue(prologue)
            .with_s(identity.dh.clone())
            .with_s_kem(identity.kem.clone())
            .with_rs(responder_dh)
            .with_rs_kem(responder_kem);
        let mut handshake = Handshake::new(params)?;
        let mut output = [0u8; MAX_MESSAGE_LEN];
        let output_len = handshake.write_message(payload, &mut output)?;
        Ok((Self { handshake }, output[..output_len].to_vec()))
    }

    /// Consume the responder message and enter transport mode.
    ///
    /// The responder message carries no application payload in v1. Rejecting
    /// one keeps future protocol additions from becoming an implicit channel.
    pub(crate) fn finish(mut self, response: &[u8]) -> Result<NoiseTransport, NoiseChannelError> {
        ensure_noise_frame_len(response.len(), "handshake response is too large")?;
        let mut payload = [0u8; MAX_MESSAGE_LEN];
        let payload_len = self.handshake.read_message(response, &mut payload)?;
        if payload_len != 0 {
            return Err(NoiseChannelError::InvalidMessage(
                "handshake response payload must be empty",
            ));
        }
        Ok(NoiseTransport {
            transport: self.handshake.finalize()?,
        })
    }
}

pub(crate) struct PendingResponderHandshake {
    handshake: Handshake,
    initiator_public_key: NoiseChannelPublicKey,
    payload: Vec<u8>,
}

impl PendingResponderHandshake {
    /// Authenticate and parse the first IK message without completing it.
    ///
    /// This split is intentional: callers must authorize `initiator_public_key`
    /// with the registry before calling [`Self::complete`].
    pub(crate) fn read_request(
        identity: &NoiseChannelIdentity,
        prologue: &[u8],
        request: &[u8],
    ) -> Result<Self, NoiseChannelError> {
        ensure_noise_frame_len(request.len(), "handshake request is too large")?;
        let params = HybridHandshakeParams::new(noise_hybrid_ik(), false)
            .with_prologue(prologue)
            .with_s(identity.dh.clone())
            .with_s_kem(identity.kem.clone());
        let mut handshake = Handshake::new(params)?;
        let mut payload = [0u8; MAX_MESSAGE_LEN];
        let payload_len = handshake.read_message(request, &mut payload)?;
        // Clatter exposes the initiator static key only after the first IK
        // message authenticates and decrypts successfully.
        let remote = handshake
            .get_remote_static()
            .ok_or(NoiseChannelError::InvalidMessage(
                "handshake request is missing initiator static key",
            ))?;
        Ok(Self {
            handshake,
            initiator_public_key: NoiseChannelPublicKey::from_raw(remote.dh(), remote.kem()),
            payload: payload[..payload_len].to_vec(),
        })
    }

    pub(crate) fn initiator_public_key(&self) -> &NoiseChannelPublicKey {
        &self.initiator_public_key
    }

    pub(crate) fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Finish the responder handshake after external harness authorization.
    pub(crate) fn complete(mut self) -> Result<(NoiseTransport, Vec<u8>), NoiseChannelError> {
        let mut response = [0u8; MAX_MESSAGE_LEN];
        let response_len = self.handshake.write_message(&[], &mut response)?;
        Ok((
            NoiseTransport {
                transport: self.handshake.finalize()?,
            },
            response[..response_len].to_vec(),
        ))
    }
}

pub(crate) struct NoiseTransport {
    transport: Transport,
}

impl NoiseTransport {
    /// Encrypt exactly one ordered transport record.
    ///
    /// The caller owns relay sequence assignment and must never encrypt the
    /// same logical record twice under different transport nonces.
    pub(crate) fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, NoiseChannelError> {
        if self.transport.sending_nonce() >= MAX_TRANSPORT_RECORDS_PER_DIRECTION {
            return Err(NoiseChannelError::InvalidState(
                "transport record nonce exhausted",
            ));
        }
        let frame_len = plaintext.len().checked_add(AesGcm::tag_len()).ok_or(
            NoiseChannelError::InvalidMessage("transport plaintext is too large"),
        )?;
        ensure_noise_frame_len(frame_len, "transport plaintext is too large")?;
        Ok(self.transport.send_vec(plaintext)?)
    }

    /// Decrypt exactly the next ordered transport record.
    ///
    /// Clatter advances the receiving nonce during this call, so callers must
    /// reorder and deduplicate relay frames before invoking it.
    pub(crate) fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, NoiseChannelError> {
        if self.transport.receiving_nonce() >= MAX_TRANSPORT_RECORDS_PER_DIRECTION {
            return Err(NoiseChannelError::InvalidState(
                "transport record nonce exhausted",
            ));
        }
        if ciphertext.len() < AesGcm::tag_len() {
            return Err(NoiseChannelError::InvalidMessage(
                "transport ciphertext is too short",
            ));
        }
        ensure_noise_frame_len(ciphertext.len(), "transport ciphertext is too large")?;
        Ok(self.transport.receive_vec(ciphertext)?)
    }
}

/// Build the transcript prologue that binds cryptographic identity to routing.
///
/// A handshake captured from one environment, exec-server registration, or
/// relay stream cannot be replayed into another because every participant must
/// construct the same prologue before the first Noise message is processed.
pub(crate) fn noise_channel_prologue(
    environment_id: &str,
    executor_registration_id: &str,
    stream_id: &str,
) -> Result<Vec<u8>, NoiseChannelError> {
    let mut prologue = Vec::new();
    append_prologue_part(&mut prologue, PROLOGUE_DOMAIN)?;
    append_prologue_part(&mut prologue, environment_id.as_bytes())?;
    append_prologue_part(&mut prologue, executor_registration_id.as_bytes())?;
    append_prologue_part(&mut prologue, stream_id.as_bytes())?;
    Ok(prologue)
}

fn append_prologue_part(prologue: &mut Vec<u8>, part: &[u8]) -> Result<(), NoiseChannelError> {
    // Length prefixes make component boundaries unambiguous. Raw concatenation
    // would allow different identifier tuples to produce the same prologue.
    let len = u32::try_from(part.len()).map_err(|_| {
        NoiseChannelError::InvalidMessage("Noise channel prologue part is too large")
    })?;
    prologue.extend_from_slice(&len.to_be_bytes());
    prologue.extend_from_slice(part);
    Ok(())
}

fn ensure_noise_frame_len(
    frame_len: usize,
    message: &'static str,
) -> Result<(), NoiseChannelError> {
    if frame_len > MAX_MESSAGE_LEN {
        return Err(NoiseChannelError::InvalidMessage(message));
    }
    Ok(())
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
