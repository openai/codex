// Copyright 2015 Brian Smith.
//
// Permission to use, copy, modify, and/or distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHORS DISCLAIM ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHORS BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
// ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
// OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

use pki_types::{AlgorithmIdentifier, InvalidSignature, SignatureVerificationAlgorithm, alg_id};
use ring::signature;

/// A `SignatureVerificationAlgorithm` implemented using *ring*.
#[derive(Debug)]
struct RingAlgorithm {
    public_key_alg_id: AlgorithmIdentifier,
    signature_alg_id: AlgorithmIdentifier,
    verification_alg: &'static dyn signature::VerificationAlgorithm,
}

impl SignatureVerificationAlgorithm for RingAlgorithm {
    fn public_key_alg_id(&self) -> AlgorithmIdentifier {
        self.public_key_alg_id
    }

    fn signature_alg_id(&self) -> AlgorithmIdentifier {
        self.signature_alg_id
    }

    fn verify_signature(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), InvalidSignature> {
        signature::UnparsedPublicKey::new(self.verification_alg, public_key)
            .verify(message, signature)
            .map_err(|_| InvalidSignature)
    }
}

/// ECDSA signatures using the P-256 curve and SHA-256.
pub static ECDSA_P256_SHA256: &dyn SignatureVerificationAlgorithm = &RingAlgorithm {
    public_key_alg_id: alg_id::ECDSA_P256,
    signature_alg_id: alg_id::ECDSA_SHA256,
    verification_alg: &signature::ECDSA_P256_SHA256_ASN1,
};

/// ECDSA signatures using the P-256 curve and SHA-384. Deprecated.
pub static ECDSA_P256_SHA384: &dyn SignatureVerificationAlgorithm = &RingAlgorithm {
    public_key_alg_id: alg_id::ECDSA_P256,
    signature_alg_id: alg_id::ECDSA_SHA384,
    verification_alg: &signature::ECDSA_P256_SHA384_ASN1,
};

/// ECDSA signatures using the P-384 curve and SHA-256. Deprecated.
pub static ECDSA_P384_SHA256: &dyn SignatureVerificationAlgorithm = &RingAlgorithm {
    public_key_alg_id: alg_id::ECDSA_P384,
    signature_alg_id: alg_id::ECDSA_SHA256,
    verification_alg: &signature::ECDSA_P384_SHA256_ASN1,
};

/// ECDSA signatures using the P-384 curve and SHA-384.
pub static ECDSA_P384_SHA384: &dyn SignatureVerificationAlgorithm = &RingAlgorithm {
    public_key_alg_id: alg_id::ECDSA_P384,
    signature_alg_id: alg_id::ECDSA_SHA384,
    verification_alg: &signature::ECDSA_P384_SHA384_ASN1,
};

/// RSA PKCS#1 1.5 signatures using SHA-256 for keys of 2048-8192 bits.
#[cfg(feature = "alloc")]
pub static RSA_PKCS1_2048_8192_SHA256: &dyn SignatureVerificationAlgorithm = &RingAlgorithm {
    public_key_alg_id: alg_id::RSA_ENCRYPTION,
    signature_alg_id: alg_id::RSA_PKCS1_SHA256,
    verification_alg: &signature::RSA_PKCS1_2048_8192_SHA256,
};

/// RSA PKCS#1 1.5 signatures using SHA-384 for keys of 2048-8192 bits.
#[cfg(feature = "alloc")]
pub static RSA_PKCS1_2048_8192_SHA384: &dyn SignatureVerificationAlgorithm = &RingAlgorithm {
    public_key_alg_id: alg_id::RSA_ENCRYPTION,
    signature_alg_id: alg_id::RSA_PKCS1_SHA384,
    verification_alg: &signature::RSA_PKCS1_2048_8192_SHA384,
};

/// RSA PKCS#1 1.5 signatures using SHA-512 for keys of 2048-8192 bits.
#[cfg(feature = "alloc")]
pub static RSA_PKCS1_2048_8192_SHA512: &dyn SignatureVerificationAlgorithm = &RingAlgorithm {
    public_key_alg_id: alg_id::RSA_ENCRYPTION,
    signature_alg_id: alg_id::RSA_PKCS1_SHA512,
    verification_alg: &signature::RSA_PKCS1_2048_8192_SHA512,
};

/// RSA PKCS#1 1.5 signatures using SHA-256 for keys of 2048-8192 bits,
/// with illegally absent AlgorithmIdentifier parameters.
///
/// RFC4055 says on sha256WithRSAEncryption and company:
///
/// >   When any of these four object identifiers appears within an
/// >   AlgorithmIdentifier, the parameters MUST be NULL.  Implementations
/// >   MUST accept the parameters being absent as well as present.
///
/// This algorithm covers the absent case, [`RSA_PKCS1_2048_8192_SHA256`] covers
/// the present case.
#[cfg(feature = "alloc")]
pub static RSA_PKCS1_2048_8192_SHA256_ABSENT_PARAMS: &dyn SignatureVerificationAlgorithm =
    &RingAlgorithm {
        public_key_alg_id: alg_id::RSA_ENCRYPTION,
        signature_alg_id: alg_id::AlgorithmIdentifier::from_slice(include_bytes!(
            "data/alg-rsa-pkcs1-sha256-absent-params.der"
        )),
        verification_alg: &signature::RSA_PKCS1_2048_8192_SHA256,
    };

/// RSA PKCS#1 1.5 signatures using SHA-384 for keys of 2048-8192 bits,
/// with illegally absent AlgorithmIdentifier parameters.
///
/// RFC4055 says on sha256WithRSAEncryption and company:
///
/// >   When any of these four object identifiers appears within an
/// >   AlgorithmIdentifier, the parameters MUST be NULL.  Implementations
/// >   MUST accept the parameters being absent as well as present.
///
/// This algorithm covers the absent case, [`RSA_PKCS1_2048_8192_SHA384`] covers
/// the present case.
#[cfg(feature = "alloc")]
pub static RSA_PKCS1_2048_8192_SHA384_ABSENT_PARAMS: &dyn SignatureVerificationAlgorithm =
    &RingAlgorithm {
        public_key_alg_id: alg_id::RSA_ENCRYPTION,
        signature_alg_id: alg_id::AlgorithmIdentifier::from_slice(include_bytes!(
            "data/alg-rsa-pkcs1-sha384-absent-params.der"
        )),
        verification_alg: &signature::RSA_PKCS1_2048_8192_SHA384,
    };

/// RSA PKCS#1 1.5 signatures using SHA-512 for keys of 2048-8192 bits,
/// with illegally absent AlgorithmIdentifier parameters.
///
/// RFC4055 says on sha256WithRSAEncryption and company:
///
/// >   When any of these four object identifiers appears within an
/// >   AlgorithmIdentifier, the parameters MUST be NULL.  Implementations
/// >   MUST accept the parameters being absent as well as present.
///
/// This algorithm covers the absent case, [`RSA_PKCS1_2048_8192_SHA512`] covers
/// the present case.
#[cfg(feature = "alloc")]
pub static RSA_PKCS1_2048_8192_SHA512_ABSENT_PARAMS: &dyn SignatureVerificationAlgorithm =
    &RingAlgorithm {
        public_key_alg_id: alg_id::RSA_ENCRYPTION,
        signature_alg_id: alg_id::AlgorithmIdentifier::from_slice(include_bytes!(
            "data/alg-rsa-pkcs1-sha512-absent-params.der"
        )),
        verification_alg: &signature::RSA_PKCS1_2048_8192_SHA512,
    };

/// RSA PKCS#1 1.5 signatures using SHA-384 for keys of 3072-8192 bits.
#[cfg(feature = "alloc")]
pub static RSA_PKCS1_3072_8192_SHA384: &dyn SignatureVerificationAlgorithm = &RingAlgorithm {
    public_key_alg_id: alg_id::RSA_ENCRYPTION,
    signature_alg_id: alg_id::RSA_PKCS1_SHA384,
    verification_alg: &signature::RSA_PKCS1_3072_8192_SHA384,
};

/// RSA PSS signatures using SHA-256 for keys of 2048-8192 bits and of
/// type rsaEncryption; see [RFC 4055 Section 1.2].
///
/// [RFC 4055 Section 1.2]: https://tools.ietf.org/html/rfc4055#section-1.2
#[cfg(feature = "alloc")]
pub static RSA_PSS_2048_8192_SHA256_LEGACY_KEY: &dyn SignatureVerificationAlgorithm =
    &RingAlgorithm {
        public_key_alg_id: alg_id::RSA_ENCRYPTION,
        signature_alg_id: alg_id::RSA_PSS_SHA256,
        verification_alg: &signature::RSA_PSS_2048_8192_SHA256,
    };

/// RSA PSS signatures using SHA-384 for keys of 2048-8192 bits and of
/// type rsaEncryption; see [RFC 4055 Section 1.2].
///
/// [RFC 4055 Section 1.2]: https://tools.ietf.org/html/rfc4055#section-1.2
#[cfg(feature = "alloc")]
pub static RSA_PSS_2048_8192_SHA384_LEGACY_KEY: &dyn SignatureVerificationAlgorithm =
    &RingAlgorithm {
        public_key_alg_id: alg_id::RSA_ENCRYPTION,
        signature_alg_id: alg_id::RSA_PSS_SHA384,
        verification_alg: &signature::RSA_PSS_2048_8192_SHA384,
    };

/// RSA PSS signatures using SHA-512 for keys of 2048-8192 bits and of
/// type rsaEncryption; see [RFC 4055 Section 1.2].
///
/// [RFC 4055 Section 1.2]: https://tools.ietf.org/html/rfc4055#section-1.2
#[cfg(feature = "alloc")]
pub static RSA_PSS_2048_8192_SHA512_LEGACY_KEY: &dyn SignatureVerificationAlgorithm =
    &RingAlgorithm {
        public_key_alg_id: alg_id::RSA_ENCRYPTION,
        signature_alg_id: alg_id::RSA_PSS_SHA512,
        verification_alg: &signature::RSA_PSS_2048_8192_SHA512,
    };

/// ED25519 signatures according to RFC 8410
pub static ED25519: &dyn SignatureVerificationAlgorithm = &RingAlgorithm {
    public_key_alg_id: alg_id::ED25519,
    signature_alg_id: alg_id::ED25519,
    verification_alg: &signature::ED25519,
};

#[cfg(test)]
#[path = "."]
mod tests {
    #[cfg(feature = "alloc")]
    use crate::error::UnsupportedSignatureAlgorithmForPublicKeyContext;
    use crate::error::{Error, UnsupportedSignatureAlgorithmContext};

    static SUPPORTED_ALGORITHMS_IN_TESTS: &[&dyn super::SignatureVerificationAlgorithm] = &[
        // Reasonable algorithms.
        super::ECDSA_P256_SHA256,
        super::ECDSA_P384_SHA384,
        super::ED25519,
        #[cfg(feature = "alloc")]
        super::RSA_PKCS1_2048_8192_SHA256,
        #[cfg(feature = "alloc")]
        super::RSA_PKCS1_2048_8192_SHA384,
        #[cfg(feature = "alloc")]
        super::RSA_PKCS1_2048_8192_SHA512,
        #[cfg(feature = "alloc")]
        super::RSA_PKCS1_3072_8192_SHA384,
        #[cfg(feature = "alloc")]
        super::RSA_PSS_2048_8192_SHA256_LEGACY_KEY,
        #[cfg(feature = "alloc")]
        super::RSA_PSS_2048_8192_SHA384_LEGACY_KEY,
        #[cfg(feature = "alloc")]
        super::RSA_PSS_2048_8192_SHA512_LEGACY_KEY,
        // Algorithms deprecated because they are nonsensical combinations.
        super::ECDSA_P256_SHA384, // Truncates digest.
        super::ECDSA_P384_SHA256, // Digest is unnecessarily short.
    ];

    const OK_IF_POINT_COMPRESSION_SUPPORTED: Result<(), Error> =
        Err(Error::InvalidSignatureForPublicKey);

    #[path = "alg_tests.rs"]
    mod alg_tests;

    fn maybe_rsa() -> Result<(), Error> {
        #[cfg(feature = "alloc")]
        {
            Ok(())
        }
        #[cfg(not(feature = "alloc"))]
        {
            Err(unsupported(&[]))
        }
    }

    fn unsupported_for_rsa(sig_alg_id: &[u8], _public_key_alg_id: &[u8]) -> Error {
        #[cfg(feature = "alloc")]
        {
            Error::UnsupportedSignatureAlgorithmForPublicKeyContext(
                UnsupportedSignatureAlgorithmForPublicKeyContext {
                    signature_algorithm_id: sig_alg_id.to_vec(),
                    public_key_algorithm_id: _public_key_alg_id.to_vec(),
                },
            )
        }
        #[cfg(not(feature = "alloc"))]
        {
            unsupported(sig_alg_id)
        }
    }

    fn invalid_rsa_signature() -> Error {
        #[cfg(feature = "alloc")]
        {
            Error::InvalidSignatureForPublicKey
        }
        #[cfg(not(feature = "alloc"))]
        {
            unsupported(&[])
        }
    }

    fn unsupported_for_ecdsa(sig_alg_id: &[u8], _public_key_alg_id: &[u8]) -> Error {
        unsupported(sig_alg_id)
    }

    fn unsupported(_sig_alg_id: &[u8]) -> Error {
        Error::UnsupportedSignatureAlgorithmContext(UnsupportedSignatureAlgorithmContext {
            #[cfg(feature = "alloc")]
            signature_algorithm_id: _sig_alg_id.to_vec(),
            #[cfg(feature = "alloc")]
            supported_algorithms: SUPPORTED_ALGORITHMS_IN_TESTS
                .iter()
                .map(|&alg| alg.signature_alg_id())
                .collect(),
        })
    }
}
