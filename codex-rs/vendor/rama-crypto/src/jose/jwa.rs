use std::ops::Deref;

use aws_lc_rs::signature::{
    ECDSA_P256_SHA256_FIXED_SIGNING, ECDSA_P384_SHA384_FIXED_SIGNING, EcdsaSigningAlgorithm,
    EcdsaVerificationAlgorithm,
};
use rama_core::error::OpaqueError;
use serde::{Deserialize, Serialize};

use crate::jose::JWKEllipticCurves;

#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
/// [`JWA`] or JSON Web Algorithms as defined in [`rfc7518`]
///
/// Some algorithms are required to be implemented when supporting
/// JWA, while others are recommended or optional.
///
/// TODO support all algorithms: https://github.com/plabayo/rama/issues/621
///
/// [`rfc7518`]: https://datatracker.ietf.org/doc/html/rfc7518
pub enum JWA {
    /// HMAC using SHA-256 (Required)
    HS256,
    /// HMAC using SHA-384 (Optional)
    HS384,
    /// HMAC using SHA-512 (Optional)
    HS512,
    /// RSASSA-PKCS1-v1_5 using SHA-256 (Recommended)
    RS256,
    /// RSASSA-PKCS1-v1_5 using SHA-384 (Optional)
    RS384,
    /// RSASSA-PKCS1-v1_5 using SHA-512 (Optional)
    RS512,
    /// ECDSA using P-256 and SHA-256 (Recommended+)
    ES256,
    /// ECDSA using P-384 and SHA-384 (Optional)
    ES384,
    /// ECDSA using P-521 and SHA-512 (Optional)
    ES512,
    /// RSASSA-PSS using SHA-256 and MGF1 with SHA-256 (Optional)
    PS256,
    /// RSASSA-PSS using SHA-384 and MGF1 with SHA-384 (Optional)
    PS384,
    /// RSASSA-PSS using SHA-512 and MGF1 with SHA-512 (Optional)
    PS512,
}

impl From<JWKEllipticCurves> for JWA {
    fn from(value: JWKEllipticCurves) -> Self {
        match value {
            JWKEllipticCurves::P256 => Self::ES256,
            JWKEllipticCurves::P384 => Self::ES384,
            JWKEllipticCurves::P521 => Self::ES512,
        }
    }
}

impl TryFrom<JWA> for JWKEllipticCurves {
    type Error = OpaqueError;

    fn try_from(value: JWA) -> Result<Self, Self::Error> {
        match value {
            JWA::ES256 => Ok(Self::P256),
            JWA::ES384 => Ok(Self::P384),
            JWA::ES512 => Ok(Self::P521),
            JWA::HS256 | JWA::HS384 | JWA::HS512 => Err(OpaqueError::from_display(
                "Hmac cannot be converted to elliptic curve",
            )),
            JWA::RS256 | JWA::RS384 | JWA::RS512 | JWA::PS256 | JWA::PS384 | JWA::PS512 => Err(
                OpaqueError::from_display("RSA cannot be converted to elliptic curve"),
            ),
        }
    }
}

impl TryFrom<JWA> for &'static EcdsaSigningAlgorithm {
    type Error = OpaqueError;

    fn try_from(value: JWA) -> Result<Self, Self::Error> {
        match value {
            JWA::ES256 | JWA::ES512 => Ok(&ECDSA_P256_SHA256_FIXED_SIGNING),
            JWA::ES384 => Ok(&ECDSA_P384_SHA384_FIXED_SIGNING),
            JWA::HS256 | JWA::HS384 | JWA::HS512 => Err(OpaqueError::from_display(
                "Hmac cannot be converted to elliptic curve",
            )),
            JWA::RS256 | JWA::RS384 | JWA::RS512 | JWA::PS256 | JWA::PS384 | JWA::PS512 => Err(
                OpaqueError::from_display("RSA cannot be converted to elliptic curve"),
            ),
        }
    }
}

impl TryFrom<JWA> for &'static EcdsaVerificationAlgorithm {
    type Error = OpaqueError;

    fn try_from(value: JWA) -> Result<Self, Self::Error> {
        let signing_algo: &'static EcdsaSigningAlgorithm = value.try_into()?;
        Ok(signing_algo.deref())
    }
}
