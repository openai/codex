//! The PKIX [`AlgorithmIdentifier`] type, and common values.
//!
//! If you need to use an [`AlgorithmIdentifier`] not defined here,
//! you can define it locally.

use core::fmt;
use core::ops::Deref;

// See src/data/README.md.

/// AlgorithmIdentifier for `id-ml-dsa-44`.
///
/// This is:
///
/// ```text
/// OBJECT_IDENTIFIER { 2.16.840.1.101.3.4.3.17 }
/// ```
///
/// <https://www.ietf.org/archive/id/draft-ietf-lamps-dilithium-certificates-07.html#name-identifiers>
pub const ML_DSA_44: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-ml-dsa-44.der"));

/// AlgorithmIdentifier for `id-ml-dsa-65`.
///
/// This is:
///
/// ```text
/// OBJECT_IDENTIFIER { 2.16.840.1.101.3.4.3.18 }
/// ```
///
/// <https://www.ietf.org/archive/id/draft-ietf-lamps-dilithium-certificates-07.html#name-identifiers>
pub const ML_DSA_65: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-ml-dsa-65.der"));

/// AlgorithmIdentifier for `id-ml-dsa-87`.
///
/// This is:
///
/// ```text
/// OBJECT_IDENTIFIER { 2.16.840.1.101.3.4.3.19 }
/// ```
///
/// <https://www.ietf.org/archive/id/draft-ietf-lamps-dilithium-certificates-07.html#name-identifiers>
pub const ML_DSA_87: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-ml-dsa-87.der"));

/// AlgorithmIdentifier for `id-ecPublicKey` with named curve `secp256r1`.
///
/// This is:
///
/// ```text
/// # ecPublicKey
/// OBJECT_IDENTIFIER { 1.2.840.10045.2.1 }
/// # secp256r1
/// OBJECT_IDENTIFIER { 1.2.840.10045.3.1.7 }
/// ```
pub const ECDSA_P256: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-ecdsa-p256.der"));

/// AlgorithmIdentifier for `id-ecPublicKey` with named curve `secp384r1`.
///
/// This is:
///
/// ```text
/// # ecPublicKey
/// OBJECT_IDENTIFIER { 1.2.840.10045.2.1 }
/// # secp384r1
/// OBJECT_IDENTIFIER { 1.3.132.0.34 }
/// ```
pub const ECDSA_P384: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-ecdsa-p384.der"));

/// AlgorithmIdentifier for `id-ecPublicKey` with named curve `secp521r1`.
///
/// This is:
///
/// ```text
/// # ecPublicKey
/// OBJECT_IDENTIFIER { 1.2.840.10045.2.1 }
/// # secp521r1
/// OBJECT_IDENTIFIER { 1.3.132.0.35 }
/// ```
pub const ECDSA_P521: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-ecdsa-p521.der"));

/// AlgorithmIdentifier for `ecdsa-with-SHA256`.
///
/// This is:
///
/// ```text
/// # ecdsa-with-SHA256
/// OBJECT_IDENTIFIER { 1.2.840.10045.4.3.2 }
/// ```
pub const ECDSA_SHA256: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-ecdsa-sha256.der"));

/// AlgorithmIdentifier for `ecdsa-with-SHA384`.
///
/// This is:
///
/// ```text
/// # ecdsa-with-SHA384
/// OBJECT_IDENTIFIER { 1.2.840.10045.4.3.3 }
/// ```
pub const ECDSA_SHA384: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-ecdsa-sha384.der"));

/// AlgorithmIdentifier for `ecdsa-with-SHA512`.
///
/// This is:
///
/// ```text
/// # ecdsa-with-SHA512
/// OBJECT_IDENTIFIER { 1.2.840.10045.4.3.4 }
/// ```
pub const ECDSA_SHA512: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-ecdsa-sha512.der"));

/// AlgorithmIdentifier for `rsaEncryption`.
///
/// This is:
///
/// ```text
/// # rsaEncryption
/// OBJECT_IDENTIFIER { 1.2.840.113549.1.1.1 }
/// NULL {}
/// ```
pub const RSA_ENCRYPTION: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-rsa-encryption.der"));

/// AlgorithmIdentifier for `sha256WithRSAEncryption`.
///
/// This is:
///
/// ```text
/// # sha256WithRSAEncryption
/// OBJECT_IDENTIFIER { 1.2.840.113549.1.1.11 }
/// NULL {}
/// ```
pub const RSA_PKCS1_SHA256: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-rsa-pkcs1-sha256.der"));

/// AlgorithmIdentifier for `sha384WithRSAEncryption`.
///
/// This is:
///
/// ```text
/// # sha384WithRSAEncryption
/// OBJECT_IDENTIFIER { 1.2.840.113549.1.1.12 }
/// NULL {}
/// ```
pub const RSA_PKCS1_SHA384: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-rsa-pkcs1-sha384.der"));

/// AlgorithmIdentifier for `sha512WithRSAEncryption`.
///
/// This is:
///
/// ```text
/// # sha512WithRSAEncryption
/// OBJECT_IDENTIFIER { 1.2.840.113549.1.1.13 }
/// NULL {}
/// ```
pub const RSA_PKCS1_SHA512: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-rsa-pkcs1-sha512.der"));

/// AlgorithmIdentifier for `rsassaPss` with:
///
/// - hashAlgorithm: sha256
/// - maskGenAlgorithm: mgf1 with sha256
/// - saltLength: 32
///
/// This is:
///
/// ```text
/// # rsassa-pss
/// OBJECT_IDENTIFIER { 1.2.840.113549.1.1.10 }
/// SEQUENCE {
///   # hashAlgorithm:
///   [0] {
///     SEQUENCE {
///       # sha256
///       OBJECT_IDENTIFIER { 2.16.840.1.101.3.4.2.1 }
///       NULL {}
///     }
///   }
///   # maskGenAlgorithm:
///   [1] {
///     SEQUENCE {
///       # mgf1
///       OBJECT_IDENTIFIER { 1.2.840.113549.1.1.8 }
///       SEQUENCE {
///         # sha256
///         OBJECT_IDENTIFIER { 2.16.840.1.101.3.4.2.1 }
///         NULL {}
///       }
///     }
///   }
///   # saltLength:
///   [2] {
///     INTEGER { 32 }
///   }
/// }
/// ```
///
/// See <https://datatracker.ietf.org/doc/html/rfc4055#section-3.1> for
/// the meaning of the context-specific tags.
pub const RSA_PSS_SHA256: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-rsa-pss-sha256.der"));

/// AlgorithmIdentifier for `rsassaPss` with:
///
/// - hashAlgorithm: sha384
/// - maskGenAlgorithm: mgf1 with sha384
/// - saltLength: 48
///
/// This is:
///
/// ```text
/// # rsassa-pss
/// OBJECT_IDENTIFIER { 1.2.840.113549.1.1.10 }
/// SEQUENCE {
///   # hashAlgorithm:
///   [0] {
///     SEQUENCE {
///       # sha384
///       OBJECT_IDENTIFIER { 2.16.840.1.101.3.4.2.2 }
///       NULL {}
///     }
///   }
///   # maskGenAlgorithm:
///   [1] {
///     SEQUENCE {
///       # mgf1
///       OBJECT_IDENTIFIER { 1.2.840.113549.1.1.8 }
///       SEQUENCE {
///         # sha384
///         OBJECT_IDENTIFIER { 2.16.840.1.101.3.4.2.2 }
///         NULL {}
///       }
///     }
///   }
///   # saltLength:
///   [2] {
///     INTEGER { 48 }
///   }
/// }
/// ```
///
/// See <https://datatracker.ietf.org/doc/html/rfc4055#section-3.1> for
/// the meaning of the context-specific tags.
pub const RSA_PSS_SHA384: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-rsa-pss-sha384.der"));

/// AlgorithmIdentifier for `rsassaPss` with:
///
/// - hashAlgorithm: sha512
/// - maskGenAlgorithm: mgf1 with sha512
/// - saltLength: 64
///
/// This is:
///
/// ```text
/// # rsassa-pss
/// OBJECT_IDENTIFIER { 1.2.840.113549.1.1.10 }
/// SEQUENCE {
///   # hashAlgorithm:
///   [0] {
///     SEQUENCE {
///       # sha512
///       OBJECT_IDENTIFIER { 2.16.840.1.101.3.4.2.3 }
///       NULL {}
///     }
///   }
///   # maskGenAlgorithm:
///   [1] {
///     SEQUENCE {
///       # mgf1
///       OBJECT_IDENTIFIER { 1.2.840.113549.1.1.8 }
///       SEQUENCE {
///         # sha512
///         OBJECT_IDENTIFIER { 2.16.840.1.101.3.4.2.3 }
///         NULL {}
///       }
///     }
///   }
///   # saltLength:
///   [2] {
///     INTEGER { 64 }
///   }
/// }
/// ```
///
/// See <https://datatracker.ietf.org/doc/html/rfc4055#section-3.1> for
/// the meaning of the context-specific tags.
pub const RSA_PSS_SHA512: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-rsa-pss-sha512.der"));

/// AlgorithmIdentifier for `ED25519`.
///
/// This is:
///
/// ```text
/// # ed25519
/// OBJECT_IDENTIFIER { 1.3.101.112 }
/// ```
pub const ED25519: AlgorithmIdentifier =
    AlgorithmIdentifier::from_slice(include_bytes!("data/alg-ed25519.der"));

/// A DER encoding of the PKIX AlgorithmIdentifier type:
///
/// ```ASN.1
/// AlgorithmIdentifier  ::=  SEQUENCE  {
///     algorithm               OBJECT IDENTIFIER,
///     parameters              ANY DEFINED BY algorithm OPTIONAL  }
///                                -- contains a value of the type
///                                -- registered for use with the
///                                -- algorithm object identifier value
/// ```
/// (from <https://www.rfc-editor.org/rfc/rfc5280#section-4.1.1.2>)
///
/// The outer sequence encoding is *not included*, so this is the DER encoding
/// of an OID for `algorithm` plus the `parameters` value.
///
/// For example, this is the `rsaEncryption` algorithm (but prefer to use the constant
/// [`RSA_ENCRYPTION`] instead):
///
/// ```
/// let rsa_encryption = rustls_pki_types::AlgorithmIdentifier::from_slice(
///     &[
///         // algorithm: 1.2.840.113549.1.1.1
///         0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01,
///         // parameters: NULL
///         0x05, 0x00
///     ]
/// );
/// assert_eq!(rustls_pki_types::alg_id::RSA_ENCRYPTION, rsa_encryption);
/// ```
///
/// Common values for this type are provided in this module.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AlgorithmIdentifier(&'static [u8]);

impl AlgorithmIdentifier {
    /// Makes a new `AlgorithmIdentifier` from a static octet slice.
    ///
    /// This does not validate the contents of the slice.
    pub const fn from_slice(bytes: &'static [u8]) -> Self {
        Self(bytes)
    }
}

impl AsRef<[u8]> for AlgorithmIdentifier {
    fn as_ref(&self) -> &[u8] {
        self.0
    }
}

impl fmt::Debug for AlgorithmIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        super::hex(f, self.0)
    }
}

impl Deref for AlgorithmIdentifier {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}
