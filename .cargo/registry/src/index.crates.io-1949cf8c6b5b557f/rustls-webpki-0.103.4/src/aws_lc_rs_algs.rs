#[cfg(all(feature = "aws-lc-rs-unstable", not(feature = "aws-lc-rs-fips")))]
use aws_lc_rs::unstable;
use aws_lc_rs::{signature, try_fips_mode};
use pki_types::{AlgorithmIdentifier, InvalidSignature, SignatureVerificationAlgorithm, alg_id};

// nb. aws-lc-rs has an API that is broadly compatible with *ring*,
// so this is very similar to ring_algs.rs.

/// A `SignatureVerificationAlgorithm` implemented using aws-lc-rs.
#[derive(Debug)]
struct AwsLcRsAlgorithm {
    public_key_alg_id: AlgorithmIdentifier,
    signature_alg_id: AlgorithmIdentifier,
    verification_alg: &'static dyn signature::VerificationAlgorithm,
    in_fips_submission: bool,
}

impl SignatureVerificationAlgorithm for AwsLcRsAlgorithm {
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
        if matches!(
            self.public_key_alg_id,
            alg_id::ECDSA_P256 | alg_id::ECDSA_P384 | alg_id::ECDSA_P521
        ) {
            // Restrict the allowed encodings of EC public keys.
            //
            // "The first octet of the OCTET STRING indicates whether the key is
            //  compressed or uncompressed.  The uncompressed form is indicated
            //  by 0x04 and the compressed form is indicated by either 0x02 or
            //  0x03 (see 2.3.3 in [SEC1]).  The public key MUST be rejected if
            //  any other value is included in the first octet."
            // -- <https://datatracker.ietf.org/doc/html/rfc5480#section-2.2>
            match public_key.first() {
                Some(0x04) | Some(0x02) | Some(0x03) => {}
                _ => return Err(InvalidSignature),
            };
        }
        signature::UnparsedPublicKey::new(self.verification_alg, public_key)
            .verify(message, signature)
            .map_err(|_| InvalidSignature)
    }

    fn fips(&self) -> bool {
        self.in_fips_submission && try_fips_mode().is_ok()
    }
}

/// ML-DSA signatures using the [4, 4] matrix (security strength category 2).
#[cfg(all(feature = "aws-lc-rs-unstable", not(feature = "aws-lc-rs-fips")))]
pub static ML_DSA_44: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::ML_DSA_44,
    signature_alg_id: alg_id::ML_DSA_44,
    verification_alg: &unstable::signature::MLDSA_44,
    // Not included in AWS-LC-FIPS 3.0 FIPS scope
    in_fips_submission: false,
};

/// ML-DSA signatures using the [6, 5] matrix (security strength category 3).
#[cfg(all(feature = "aws-lc-rs-unstable", not(feature = "aws-lc-rs-fips")))]
pub static ML_DSA_65: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::ML_DSA_65,
    signature_alg_id: alg_id::ML_DSA_65,
    verification_alg: &unstable::signature::MLDSA_65,
    // Not included in AWS-LC-FIPS 3.0 FIPS scope
    in_fips_submission: false,
};

/// ML-DSA signatures using the [8. 7] matrix (security strength category 5).
#[cfg(all(feature = "aws-lc-rs-unstable", not(feature = "aws-lc-rs-fips")))]
pub static ML_DSA_87: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::ML_DSA_87,
    signature_alg_id: alg_id::ML_DSA_87,
    verification_alg: &unstable::signature::MLDSA_87,
    // Not included in AWS-LC-FIPS 3.0 FIPS scope
    in_fips_submission: false,
};

/// ECDSA signatures using the P-256 curve and SHA-256.
pub static ECDSA_P256_SHA256: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::ECDSA_P256,
    signature_alg_id: alg_id::ECDSA_SHA256,
    verification_alg: &signature::ECDSA_P256_SHA256_ASN1,
    in_fips_submission: true,
};

/// ECDSA signatures using the P-256 curve and SHA-384. Deprecated.
pub static ECDSA_P256_SHA384: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::ECDSA_P256,
    signature_alg_id: alg_id::ECDSA_SHA384,
    verification_alg: &signature::ECDSA_P256_SHA384_ASN1,
    in_fips_submission: true,
};

/// ECDSA signatures using the P-384 curve and SHA-256. Deprecated.
pub static ECDSA_P384_SHA256: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::ECDSA_P384,
    signature_alg_id: alg_id::ECDSA_SHA256,
    verification_alg: &signature::ECDSA_P384_SHA256_ASN1,
    in_fips_submission: true,
};

/// ECDSA signatures using the P-384 curve and SHA-384.
pub static ECDSA_P384_SHA384: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::ECDSA_P384,
    signature_alg_id: alg_id::ECDSA_SHA384,
    verification_alg: &signature::ECDSA_P384_SHA384_ASN1,
    in_fips_submission: true,
};

/// ECDSA signatures using the P-521 curve and SHA-256.
pub static ECDSA_P521_SHA256: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::ECDSA_P521,
    signature_alg_id: alg_id::ECDSA_SHA256,
    verification_alg: &signature::ECDSA_P521_SHA256_ASN1,
    in_fips_submission: true,
};

/// ECDSA signatures using the P-521 curve and SHA-384.
pub static ECDSA_P521_SHA384: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::ECDSA_P521,
    signature_alg_id: alg_id::ECDSA_SHA384,
    verification_alg: &signature::ECDSA_P521_SHA384_ASN1,
    in_fips_submission: true,
};

/// ECDSA signatures using the P-521 curve and SHA-512.
pub static ECDSA_P521_SHA512: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::ECDSA_P521,
    signature_alg_id: alg_id::ECDSA_SHA512,
    verification_alg: &signature::ECDSA_P521_SHA512_ASN1,
    in_fips_submission: true,
};

/// RSA PKCS#1 1.5 signatures using SHA-256 for keys of 2048-8192 bits.
pub static RSA_PKCS1_2048_8192_SHA256: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::RSA_ENCRYPTION,
    signature_alg_id: alg_id::RSA_PKCS1_SHA256,
    verification_alg: &signature::RSA_PKCS1_2048_8192_SHA256,
    in_fips_submission: true,
};

/// RSA PKCS#1 1.5 signatures using SHA-384 for keys of 2048-8192 bits.
pub static RSA_PKCS1_2048_8192_SHA384: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::RSA_ENCRYPTION,
    signature_alg_id: alg_id::RSA_PKCS1_SHA384,
    verification_alg: &signature::RSA_PKCS1_2048_8192_SHA384,
    in_fips_submission: true,
};

/// RSA PKCS#1 1.5 signatures using SHA-512 for keys of 2048-8192 bits.
pub static RSA_PKCS1_2048_8192_SHA512: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::RSA_ENCRYPTION,
    signature_alg_id: alg_id::RSA_PKCS1_SHA512,
    verification_alg: &signature::RSA_PKCS1_2048_8192_SHA512,
    in_fips_submission: true,
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
pub static RSA_PKCS1_2048_8192_SHA256_ABSENT_PARAMS: &dyn SignatureVerificationAlgorithm =
    &AwsLcRsAlgorithm {
        public_key_alg_id: alg_id::RSA_ENCRYPTION,
        signature_alg_id: alg_id::AlgorithmIdentifier::from_slice(include_bytes!(
            "data/alg-rsa-pkcs1-sha256-absent-params.der"
        )),
        verification_alg: &signature::RSA_PKCS1_2048_8192_SHA256,
        in_fips_submission: true,
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
pub static RSA_PKCS1_2048_8192_SHA384_ABSENT_PARAMS: &dyn SignatureVerificationAlgorithm =
    &AwsLcRsAlgorithm {
        public_key_alg_id: alg_id::RSA_ENCRYPTION,
        signature_alg_id: alg_id::AlgorithmIdentifier::from_slice(include_bytes!(
            "data/alg-rsa-pkcs1-sha384-absent-params.der"
        )),
        verification_alg: &signature::RSA_PKCS1_2048_8192_SHA384,
        in_fips_submission: true,
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
pub static RSA_PKCS1_2048_8192_SHA512_ABSENT_PARAMS: &dyn SignatureVerificationAlgorithm =
    &AwsLcRsAlgorithm {
        public_key_alg_id: alg_id::RSA_ENCRYPTION,
        signature_alg_id: alg_id::AlgorithmIdentifier::from_slice(include_bytes!(
            "data/alg-rsa-pkcs1-sha512-absent-params.der"
        )),
        verification_alg: &signature::RSA_PKCS1_2048_8192_SHA512,
        in_fips_submission: true,
    };

/// RSA PKCS#1 1.5 signatures using SHA-384 for keys of 3072-8192 bits.
pub static RSA_PKCS1_3072_8192_SHA384: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::RSA_ENCRYPTION,
    signature_alg_id: alg_id::RSA_PKCS1_SHA384,
    verification_alg: &signature::RSA_PKCS1_3072_8192_SHA384,
    in_fips_submission: true,
};

/// RSA PSS signatures using SHA-256 for keys of 2048-8192 bits and of
/// type rsaEncryption; see [RFC 4055 Section 1.2].
///
/// [RFC 4055 Section 1.2]: https://tools.ietf.org/html/rfc4055#section-1.2
pub static RSA_PSS_2048_8192_SHA256_LEGACY_KEY: &dyn SignatureVerificationAlgorithm =
    &AwsLcRsAlgorithm {
        public_key_alg_id: alg_id::RSA_ENCRYPTION,
        signature_alg_id: alg_id::RSA_PSS_SHA256,
        verification_alg: &signature::RSA_PSS_2048_8192_SHA256,
        in_fips_submission: true,
    };

/// RSA PSS signatures using SHA-384 for keys of 2048-8192 bits and of
/// type rsaEncryption; see [RFC 4055 Section 1.2].
///
/// [RFC 4055 Section 1.2]: https://tools.ietf.org/html/rfc4055#section-1.2
pub static RSA_PSS_2048_8192_SHA384_LEGACY_KEY: &dyn SignatureVerificationAlgorithm =
    &AwsLcRsAlgorithm {
        public_key_alg_id: alg_id::RSA_ENCRYPTION,
        signature_alg_id: alg_id::RSA_PSS_SHA384,
        verification_alg: &signature::RSA_PSS_2048_8192_SHA384,
        in_fips_submission: true,
    };

/// RSA PSS signatures using SHA-512 for keys of 2048-8192 bits and of
/// type rsaEncryption; see [RFC 4055 Section 1.2].
///
/// [RFC 4055 Section 1.2]: https://tools.ietf.org/html/rfc4055#section-1.2
pub static RSA_PSS_2048_8192_SHA512_LEGACY_KEY: &dyn SignatureVerificationAlgorithm =
    &AwsLcRsAlgorithm {
        public_key_alg_id: alg_id::RSA_ENCRYPTION,
        signature_alg_id: alg_id::RSA_PSS_SHA512,
        verification_alg: &signature::RSA_PSS_2048_8192_SHA512,
        in_fips_submission: true,
    };

/// ED25519 signatures according to RFC 8410
pub static ED25519: &dyn SignatureVerificationAlgorithm = &AwsLcRsAlgorithm {
    public_key_alg_id: alg_id::ED25519,
    signature_alg_id: alg_id::ED25519,
    verification_alg: &signature::ED25519,
    in_fips_submission: true,
};

#[cfg(test)]
#[path = "."]
mod tests {
    use crate::error::{
        Error, UnsupportedSignatureAlgorithmContext,
        UnsupportedSignatureAlgorithmForPublicKeyContext,
    };

    static SUPPORTED_ALGORITHMS_IN_TESTS: &[&dyn super::SignatureVerificationAlgorithm] = &[
        // Reasonable algorithms.
        super::ECDSA_P256_SHA256,
        super::ECDSA_P384_SHA384,
        super::ECDSA_P521_SHA256,
        super::ECDSA_P521_SHA384,
        super::ECDSA_P521_SHA512,
        super::ED25519,
        super::RSA_PKCS1_2048_8192_SHA256,
        super::RSA_PKCS1_2048_8192_SHA384,
        super::RSA_PKCS1_2048_8192_SHA512,
        super::RSA_PKCS1_3072_8192_SHA384,
        super::RSA_PSS_2048_8192_SHA256_LEGACY_KEY,
        super::RSA_PSS_2048_8192_SHA384_LEGACY_KEY,
        super::RSA_PSS_2048_8192_SHA512_LEGACY_KEY,
        // Algorithms deprecated because they are nonsensical combinations.
        super::ECDSA_P256_SHA384, // Truncates digest.
        super::ECDSA_P384_SHA256, // Digest is unnecessarily short.
        #[cfg(all(feature = "aws-lc-rs-unstable", not(feature = "aws-lc-rs-fips")))]
        super::ML_DSA_44,
        #[cfg(all(feature = "aws-lc-rs-unstable", not(feature = "aws-lc-rs-fips")))]
        super::ML_DSA_65,
        #[cfg(all(feature = "aws-lc-rs-unstable", not(feature = "aws-lc-rs-fips")))]
        super::ML_DSA_87,
    ];

    const OK_IF_POINT_COMPRESSION_SUPPORTED: Result<(), Error> = Ok(());

    #[path = "alg_tests.rs"]
    mod alg_tests;

    fn maybe_rsa() -> Result<(), Error> {
        Ok(())
    }

    fn unsupported_for_rsa(_sig_alg_id: &[u8], _public_key_alg_id: &[u8]) -> Error {
        Error::UnsupportedSignatureAlgorithmForPublicKeyContext(
            UnsupportedSignatureAlgorithmForPublicKeyContext {
                #[cfg(feature = "alloc")]
                signature_algorithm_id: _sig_alg_id.to_vec(),
                #[cfg(feature = "alloc")]
                public_key_algorithm_id: _public_key_alg_id.to_vec(),
            },
        )
    }

    fn invalid_rsa_signature() -> Error {
        Error::InvalidSignatureForPublicKey
    }

    fn unsupported_for_ecdsa(_sig_alg_id: &[u8], _public_key_alg_id: &[u8]) -> Error {
        Error::UnsupportedSignatureAlgorithmForPublicKeyContext(
            UnsupportedSignatureAlgorithmForPublicKeyContext {
                #[cfg(feature = "alloc")]
                signature_algorithm_id: _sig_alg_id.to_vec(),
                #[cfg(feature = "alloc")]
                public_key_algorithm_id: _public_key_alg_id.to_vec(),
            },
        )
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
