#![allow(clippy::duplicate_mod)]

use alloc::boxed::Box;
use core::fmt;

use super::ring_like::agreement;
use super::ring_like::rand::SystemRandom;
use crate::crypto::{ActiveKeyExchange, FfdheGroup, SharedSecret, SupportedKxGroup};
use crate::error::{Error, PeerMisbehaved};
use crate::msgs::enums::NamedGroup;
use crate::rand::GetRandomFailed;

/// A key-exchange group supported by *ring*.
struct KxGroup {
    /// The IANA "TLS Supported Groups" name of the group
    name: NamedGroup,

    /// The corresponding ring agreement::Algorithm
    agreement_algorithm: &'static agreement::Algorithm,

    /// Whether the algorithm is allowed by FIPS
    ///
    /// `SupportedKxGroup::fips()` is true if and only if the algorithm is allowed,
    /// _and_ the implementation is FIPS-validated.
    fips_allowed: bool,

    /// aws-lc-rs 1.9 and later accepts more formats of public keys than
    /// just uncompressed.
    ///
    /// That is not compatible with TLS:
    /// - TLS1.3 outlaws other encodings,
    /// - TLS1.2 negotiates other encodings (we only offer uncompressed), and
    ///   defaults to uncompressed if negotiation is not done.
    ///
    /// This function should return `true` if the basic shape of its argument
    /// is consistent with an uncompressed point encoding.  It does not need
    /// to verify that the point is on the curve (if the curve requires that
    /// for security); aws-lc-rs/ring must do that.
    pub_key_validator: fn(&[u8]) -> bool,
}

impl SupportedKxGroup for KxGroup {
    fn start(&self) -> Result<Box<dyn ActiveKeyExchange>, Error> {
        let rng = SystemRandom::new();
        let priv_key = agreement::EphemeralPrivateKey::generate(self.agreement_algorithm, &rng)
            .map_err(|_| GetRandomFailed)?;

        let pub_key = priv_key
            .compute_public_key()
            .map_err(|_| GetRandomFailed)?;

        Ok(Box::new(KeyExchange {
            name: self.name,
            agreement_algorithm: self.agreement_algorithm,
            priv_key,
            pub_key,
            pub_key_validator: self.pub_key_validator,
        }))
    }

    fn ffdhe_group(&self) -> Option<FfdheGroup<'static>> {
        None
    }

    fn name(&self) -> NamedGroup {
        self.name
    }

    fn fips(&self) -> bool {
        self.fips_allowed && super::fips()
    }
}

impl fmt::Debug for KxGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.name.fmt(f)
    }
}

/// Ephemeral ECDH on curve25519 (see RFC7748)
pub static X25519: &dyn SupportedKxGroup = &KxGroup {
    name: NamedGroup::X25519,
    agreement_algorithm: &agreement::X25519,

    // "Curves that are included in SP 800-186 but not included in SP 800-56Arev3 are
    //  not approved for key agreement. E.g., the ECDH X25519 and X448 key agreement
    //  schemes (defined in RFC 7748) that use Curve25519 and Curve448, respectively,
    //  are not compliant to SP 800-56Arev3."
    // -- <https://csrc.nist.gov/csrc/media/Projects/cryptographic-module-validation-program/documents/fips%20140-3/FIPS%20140-3%20IG.pdf>
    fips_allowed: false,

    pub_key_validator: |point: &[u8]| point.len() == 32,
};

/// Ephemeral ECDH on secp256r1 (aka NIST-P256)
pub static SECP256R1: &dyn SupportedKxGroup = &KxGroup {
    name: NamedGroup::secp256r1,
    agreement_algorithm: &agreement::ECDH_P256,
    fips_allowed: true,
    pub_key_validator: uncompressed_point,
};

/// Ephemeral ECDH on secp384r1 (aka NIST-P384)
pub static SECP384R1: &dyn SupportedKxGroup = &KxGroup {
    name: NamedGroup::secp384r1,
    agreement_algorithm: &agreement::ECDH_P384,
    fips_allowed: true,
    pub_key_validator: uncompressed_point,
};

fn uncompressed_point(point: &[u8]) -> bool {
    // See `UncompressedPointRepresentation`, which is a retelling of
    // SEC1 section 2.3.3 "Elliptic-Curve-Point-to-Octet-String Conversion"
    // <https://datatracker.ietf.org/doc/html/rfc8446#section-4.2.8.2>
    matches!(point.first(), Some(0x04))
}

/// An in-progress key exchange.  This has the algorithm,
/// our private key, and our public key.
struct KeyExchange {
    name: NamedGroup,
    agreement_algorithm: &'static agreement::Algorithm,
    priv_key: agreement::EphemeralPrivateKey,
    pub_key: agreement::PublicKey,
    pub_key_validator: fn(&[u8]) -> bool,
}

impl ActiveKeyExchange for KeyExchange {
    /// Completes the key exchange, given the peer's public key.
    fn complete(self: Box<Self>, peer: &[u8]) -> Result<SharedSecret, Error> {
        if !(self.pub_key_validator)(peer) {
            return Err(PeerMisbehaved::InvalidKeyShare.into());
        }
        let peer_key = agreement::UnparsedPublicKey::new(self.agreement_algorithm, peer);
        super::ring_shim::agree_ephemeral(self.priv_key, &peer_key)
            .map_err(|_| PeerMisbehaved::InvalidKeyShare.into())
    }

    fn ffdhe_group(&self) -> Option<FfdheGroup<'static>> {
        None
    }

    /// Return the group being used.
    fn group(&self) -> NamedGroup {
        self.name
    }

    /// Return the public key being used.
    fn pub_key(&self) -> &[u8] {
        self.pub_key.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use std::format;

    #[test]
    fn kxgroup_fmt_yields_name() {
        assert_eq!("X25519", format!("{:?}", super::X25519));
    }
}

#[cfg(bench)]
mod benchmarks {
    #[bench]
    fn bench_x25519(b: &mut test::Bencher) {
        bench_any(b, super::X25519);
    }

    #[bench]
    fn bench_ecdh_p256(b: &mut test::Bencher) {
        bench_any(b, super::SECP256R1);
    }

    #[bench]
    fn bench_ecdh_p384(b: &mut test::Bencher) {
        bench_any(b, super::SECP384R1);
    }

    fn bench_any(b: &mut test::Bencher, kxg: &dyn super::SupportedKxGroup) {
        b.iter(|| {
            let akx = kxg.start().unwrap();
            let pub_key = akx.pub_key().to_vec();
            test::black_box(akx.complete(&pub_key).unwrap());
        });
    }
}
