//! Bridges AWS-LC's ML-KEM-768 implementation into Clatter's `Kem` trait.
//!
//! Clatter owns the hybrid Noise state machine, while AWS-LC owns the concrete
//! post-quantum operations and their randomness. This module is intentionally
//! limited to converting between the two libraries' fixed-size key, ciphertext,
//! and shared-secret representations; it does not add protocol behavior.

use aws_lc_rs::kem::Ciphertext;
use aws_lc_rs::kem::DecapsulationKey;
use aws_lc_rs::kem::EncapsulationKey;
use aws_lc_rs::kem::ML_KEM_768;
use clatter::KeyPair;
use clatter::bytearray::ByteArray;
use clatter::bytearray::HeapArray;
use clatter::bytearray::SensitiveByteArray;
use clatter::error::KemError;
use clatter::error::KemResult;
use clatter::traits::CryptoComponent;
use clatter::traits::Kem;
use clatter::traits::Rng;

pub(super) const PUBLIC_KEY_LEN: usize = 1184;
const SECRET_KEY_LEN: usize = 2400;
const CIPHERTEXT_LEN: usize = 1088;
const SHARED_SECRET_LEN: usize = 32;

/// ML-KEM-768 implementation backed by AWS-LC through `aws-lc-rs`.
#[derive(Clone)]
pub(super) struct AwsLcMlKem768;

impl CryptoComponent for AwsLcMlKem768 {
    fn name() -> &'static str {
        "MLKEM768"
    }
}

impl Kem for AwsLcMlKem768 {
    type SecretKey = SensitiveByteArray<HeapArray<SECRET_KEY_LEN>>;
    type PubKey = HeapArray<PUBLIC_KEY_LEN>;
    type Ct = HeapArray<CIPHERTEXT_LEN>;
    type Ss = SensitiveByteArray<[u8; SHARED_SECRET_LEN]>;

    // Generate the long-lived static KEM keypair used by hybrid IK. AWS-LC
    // returns serialized key bytes; copying them into Clatter's fixed-size
    // containers gives the handshake implementation the exact lengths encoded
    // by this suite and keeps the secret half in a zeroizing container.
    fn genkey_rng<R: Rng>(_rng: &mut R) -> KemResult<KeyPair<Self::PubKey, Self::SecretKey>> {
        // AWS-LC owns ML-KEM key-generation randomness internally, so
        // Clatter's injectable RNG cannot be plumbed through this provider.
        let decapsulation_key =
            DecapsulationKey::generate(&ML_KEM_768).map_err(|_| KemError::KeyGeneration)?;
        let encapsulation_key = decapsulation_key
            .encapsulation_key()
            .map_err(|_| KemError::KeyGeneration)?;
        let public = encapsulation_key
            .key_bytes()
            .map_err(|_| KemError::KeyGeneration)?;
        let secret = decapsulation_key
            .key_bytes()
            .map_err(|_| KemError::KeyGeneration)?;

        Ok(KeyPair {
            public: Self::PubKey::from_slice(public.as_ref()),
            secret: Self::SecretKey::from_slice(secret.as_ref()),
        })
    }

    // Parse the peer's serialized public key at the provider boundary, then
    // return the ciphertext and shared secret in Clatter's fixed-size types.
    // AWS-LC performs both public-key validation and encapsulation randomness.
    fn encapsulate<R: Rng>(pk: &[u8], _rng: &mut R) -> KemResult<(Self::Ct, Self::Ss)> {
        let encapsulation_key =
            EncapsulationKey::new(&ML_KEM_768, pk).map_err(|_| KemError::Input)?;
        let (ciphertext, shared_secret) = encapsulation_key
            .encapsulate()
            .map_err(|_| KemError::Encapsulation)?;

        Ok((
            Self::Ct::from_slice(ciphertext.as_ref()),
            Self::Ss::from_slice(shared_secret.as_ref()),
        ))
    }

    // Reconstruct the provider key from process-local secret bytes and recover
    // the same shared secret the peer mixed into the hybrid handshake.
    fn decapsulate(ct: &[u8], sk: &[u8]) -> KemResult<Self::Ss> {
        // Reject the length before constructing AWS-LC's ciphertext wrapper.
        // This keeps malformed wire input classified as an input error rather
        // than relying on provider-specific decapsulation behavior.
        if ct.len() != CIPHERTEXT_LEN {
            return Err(KemError::Input);
        }

        let decapsulation_key =
            DecapsulationKey::new(&ML_KEM_768, sk).map_err(|_| KemError::Input)?;
        let shared_secret = decapsulation_key
            .decapsulate(Ciphertext::from(ct))
            .map_err(|_| KemError::Decapsulation)?;

        Ok(Self::Ss::from_slice(shared_secret.as_ref()))
    }
}

#[cfg(test)]
#[path = "aws_lc_ml_kem_tests.rs"]
mod tests;
