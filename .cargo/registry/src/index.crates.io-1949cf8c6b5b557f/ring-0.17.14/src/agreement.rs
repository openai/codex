// Copyright 2015-2017 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY
// SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION
// OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN
// CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

//! Key Agreement: ECDH, including X25519.
//!
//! # Example
//!
//! Note that this example uses X25519, but ECDH using NIST P-256/P-384 is done
//! exactly the same way, just substituting
//! `agreement::ECDH_P256`/`agreement::ECDH_P384` for `agreement::X25519`.
//!
//! ```
//! use ring::{agreement, rand};
//!
//! let rng = rand::SystemRandom::new();
//!
//! let my_private_key = agreement::EphemeralPrivateKey::generate(&agreement::X25519, &rng)?;
//!
//! // Make `my_public_key` a byte slice containing my public key. In a real
//! // application, this would be sent to the peer in an encoded protocol
//! // message.
//! let my_public_key = my_private_key.compute_public_key()?;
//!
//! let peer_public_key_bytes = {
//!     // In a real application, the peer public key would be parsed out of a
//!     // protocol message. Here we just generate one.
//!     let peer_private_key =
//!         agreement::EphemeralPrivateKey::generate(&agreement::X25519, &rng)?;
//!     peer_private_key.compute_public_key()?
//! };
//!
//! let peer_public_key = agreement::UnparsedPublicKey::new(
//!     &agreement::X25519,
//!     peer_public_key_bytes);
//!
//! agreement::agree_ephemeral(
//!     my_private_key,
//!     &peer_public_key,
//!     |_key_material| {
//!         // In a real application, we'd apply a KDF to the key material and the
//!         // public keys (as recommended in RFC 7748) and then derive session
//!         // keys from the result. We omit all that here.
//!     },
//! )?;
//!
//! # Ok::<(), ring::error::Unspecified>(())
//! ```

// The "NSA Guide" steps here are from from section 3.1, "Ephemeral Unified
// Model."

use crate::{cpu, debug, ec, error, rand};

pub use crate::ec::{
    curve25519::x25519::X25519,
    suite_b::ecdh::{ECDH_P256, ECDH_P384},
};

/// A key agreement algorithm.
pub struct Algorithm {
    pub(crate) curve: &'static ec::Curve,
    pub(crate) ecdh: fn(
        out: &mut [u8],
        private_key: &ec::Seed,
        peer_public_key: untrusted::Input,
        cpu: cpu::Features,
    ) -> Result<(), error::Unspecified>,
}

derive_debug_via_field!(Algorithm, curve);

impl Eq for Algorithm {}
impl PartialEq for Algorithm {
    fn eq(&self, other: &Self) -> bool {
        self.curve.id == other.curve.id
    }
}

/// An ephemeral private key for use (only) with `agree_ephemeral`. The
/// signature of `agree_ephemeral` ensures that an `EphemeralPrivateKey` can be
/// used for at most one key agreement.
pub struct EphemeralPrivateKey {
    private_key: ec::Seed,
    algorithm: &'static Algorithm,
}

derive_debug_via_field!(
    EphemeralPrivateKey,
    stringify!(EphemeralPrivateKey),
    algorithm
);

impl EphemeralPrivateKey {
    /// Generate a new ephemeral private key for the given algorithm.
    pub fn generate(
        alg: &'static Algorithm,
        rng: &dyn rand::SecureRandom,
    ) -> Result<Self, error::Unspecified> {
        let cpu_features = cpu::features();

        // NSA Guide Step 1.
        //
        // This only handles the key generation part of step 1. The rest of
        // step one is done by `compute_public_key()`.
        let private_key = ec::Seed::generate(alg.curve, rng, cpu_features)?;
        Ok(Self {
            private_key,
            algorithm: alg,
        })
    }

    /// Computes the public key from the private key.
    #[inline(always)]
    pub fn compute_public_key(&self) -> Result<PublicKey, error::Unspecified> {
        // NSA Guide Step 1.
        //
        // Obviously, this only handles the part of Step 1 between the private
        // key generation and the sending of the public key to the peer. `out`
        // is what should be sent to the peer.
        self.private_key
            .compute_public_key(cpu::features())
            .map(|public_key| PublicKey {
                algorithm: self.algorithm,
                bytes: public_key,
            })
    }

    /// The algorithm for the private key.
    #[inline]
    pub fn algorithm(&self) -> &'static Algorithm {
        self.algorithm
    }

    /// Do not use.
    #[deprecated]
    #[cfg(test)]
    pub fn bytes(&self) -> &[u8] {
        self.bytes_for_test()
    }

    #[cfg(test)]
    pub(super) fn bytes_for_test(&self) -> &[u8] {
        self.private_key.bytes_less_safe()
    }
}

/// A public key for key agreement.
#[derive(Clone)]
pub struct PublicKey {
    algorithm: &'static Algorithm,
    bytes: ec::PublicKey,
}

impl AsRef<[u8]> for PublicKey {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_ref()
    }
}

impl core::fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> Result<(), core::fmt::Error> {
        f.debug_struct("PublicKey")
            .field("algorithm", &self.algorithm)
            .field("bytes", &debug::HexStr(self.as_ref()))
            .finish()
    }
}

impl PublicKey {
    /// The algorithm for the public key.
    #[inline]
    pub fn algorithm(&self) -> &'static Algorithm {
        self.algorithm
    }
}

/// An unparsed, possibly malformed, public key for key agreement.
#[derive(Clone, Copy)]
pub struct UnparsedPublicKey<B> {
    algorithm: &'static Algorithm,
    bytes: B,
}

impl<B> AsRef<[u8]> for UnparsedPublicKey<B>
where
    B: AsRef<[u8]>,
{
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_ref()
    }
}

impl<B: core::fmt::Debug> core::fmt::Debug for UnparsedPublicKey<B>
where
    B: AsRef<[u8]>,
{
    fn fmt(&self, f: &mut core::fmt::Formatter) -> Result<(), core::fmt::Error> {
        f.debug_struct("UnparsedPublicKey")
            .field("algorithm", &self.algorithm)
            .field("bytes", &debug::HexStr(self.bytes.as_ref()))
            .finish()
    }
}

impl<B> UnparsedPublicKey<B> {
    /// Constructs a new `UnparsedPublicKey`.
    pub fn new(algorithm: &'static Algorithm, bytes: B) -> Self {
        Self { algorithm, bytes }
    }

    /// The algorithm for the public key.
    #[inline]
    pub fn algorithm(&self) -> &'static Algorithm {
        self.algorithm
    }

    /// TODO: doc
    #[inline]
    pub fn bytes(&self) -> &B {
        &self.bytes
    }
}

/// Performs a key agreement with an ephemeral private key and the given public
/// key.
///
/// `my_private_key` is the ephemeral private key to use. Since it is moved, it
/// will not be usable after calling `agree_ephemeral`, thus guaranteeing that
/// the key is used for only one key agreement.
///
/// `peer_public_key` is the peer's public key. `agree_ephemeral` will return
/// `Err(error_value)` if it does not match `my_private_key's` algorithm/curve.
/// `agree_ephemeral` verifies that it is encoded in the standard form for the
/// algorithm and that the key is *valid*; see the algorithm's documentation for
/// details on how keys are to be encoded and what constitutes a valid key for
/// that algorithm.
///
/// After the key agreement is done, `agree_ephemeral` calls `kdf` with the raw
/// key material from the key agreement operation and then returns what `kdf`
/// returns.
#[inline]
pub fn agree_ephemeral<B: AsRef<[u8]>, R>(
    my_private_key: EphemeralPrivateKey,
    peer_public_key: &UnparsedPublicKey<B>,
    kdf: impl FnOnce(&[u8]) -> R,
) -> Result<R, error::Unspecified> {
    let peer_public_key = UnparsedPublicKey {
        algorithm: peer_public_key.algorithm,
        bytes: peer_public_key.bytes.as_ref(),
    };
    agree_ephemeral_(my_private_key, peer_public_key, kdf, cpu::features())
}

fn agree_ephemeral_<R>(
    my_private_key: EphemeralPrivateKey,
    peer_public_key: UnparsedPublicKey<&[u8]>,
    kdf: impl FnOnce(&[u8]) -> R,
    cpu: cpu::Features,
) -> Result<R, error::Unspecified> {
    // NSA Guide Prerequisite 1.
    //
    // The domain parameters are hard-coded. This check verifies that the
    // peer's public key's domain parameters match the domain parameters of
    // this private key.
    if peer_public_key.algorithm != my_private_key.algorithm {
        return Err(error::Unspecified);
    }

    let alg = &my_private_key.algorithm;

    // NSA Guide Prerequisite 2, regarding which KDFs are allowed, is delegated
    // to the caller.

    // NSA Guide Prerequisite 3, "Prior to or during the key-agreement process,
    // each party shall obtain the identifier associated with the other party
    // during the key-agreement scheme," is delegated to the caller.

    // NSA Guide Step 1 is handled by `EphemeralPrivateKey::generate()` and
    // `EphemeralPrivateKey::compute_public_key()`.

    let mut shared_key = [0u8; ec::ELEM_MAX_BYTES];
    let shared_key = &mut shared_key[..alg.curve.elem_scalar_seed_len];

    // NSA Guide Steps 2, 3, and 4.
    //
    // We have a pretty liberal interpretation of the NIST's spec's "Destroy"
    // that doesn't meet the NSA requirement to "zeroize."
    (alg.ecdh)(
        shared_key,
        &my_private_key.private_key,
        untrusted::Input::from(peer_public_key.bytes),
        cpu,
    )?;

    // NSA Guide Steps 5 and 6.
    //
    // Again, we have a pretty liberal interpretation of the NIST's spec's
    // "Destroy" that doesn't meet the NSA requirement to "zeroize."
    Ok(kdf(shared_key))
}
