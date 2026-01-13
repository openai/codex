use aws_lc_rs::{
    digest::{Digest, SHA256, digest},
    pkcs8::Document,
    rand::SystemRandom,
    signature::{
        self, ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, EcdsaSigningAlgorithm,
        EcdsaVerificationAlgorithm, KeyPair, Signature,
    },
};
use base64::{Engine as _, prelude::BASE64_URL_SAFE_NO_PAD};
use rama_core::error::{ErrorContext, OpaqueError};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};

use crate::jose::{JWA, Signer};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
/// [`JWK`] or JSON Web Key as defined in [`rfc7517`]
///
/// [`rfc7517`]: https://datatracker.ietf.org/doc/html/rfc7517
pub struct JWK {
    /// Algorithm intended for use with this key
    pub alg: JWA,
    #[serde(flatten)]
    /// Key type (e.g., RSA, EC, oct)
    pub key_type: JWKType,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Intended use (e.g., "sig" for signature, "enc" for encryption)
    pub r#use: Option<JWKUse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Operations this key supports (e.g., ["sign", "verify"])
    pub key_ops: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// X.509 certificate chain
    pub x5c: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// X.509 certificate SHA-1 thumbprint (base64url-encoded)
    pub x5t: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "x5t#S256")]
    /// X.509 certificate SHA-256 thumbprint (base64url-encoded)
    pub x5t_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kty")]
/// The "kty" (key type) parameter identifies the cryptographic algorithm family used with the key, such as "RSA", "EC", or "OCT"
pub enum JWKType {
    RSA {
        n: String,
        e: String,
    },
    /// Elleptic curve
    EC {
        crv: JWKEllipticCurves,
        x: String,
        y: String,
    },
    /// an octet sequence key, which represents a symmetric key
    OCT {
        k: String,
    },
}

impl Serialize for JWKType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Order here is important as this output will be used to generate jwk thumb
        match &self {
            Self::EC { crv, x, y } => {
                let mut state = serializer.serialize_struct("JWKType", 4)?;
                state.serialize_field("crv", crv)?;
                state.serialize_field("kty", "EC")?;
                state.serialize_field("x", x)?;
                state.serialize_field("y", y)?;
                state.end()
            }
            Self::RSA { n, e } => {
                let mut state = serializer.serialize_struct("JWKType", 3)?;
                state.serialize_field("e", e)?;
                state.serialize_field("kty", "RSA")?;
                state.serialize_field("n", n)?;
                state.end()
            }
            Self::OCT { k } => {
                let mut state = serializer.serialize_struct("JWKType", 2)?;
                state.serialize_field("k", k)?;
                state.serialize_field("kty", "OCT")?;
                state.end()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum JWKEllipticCurves {
    #[serde(rename = "P-256")]
    P256,
    #[serde(rename = "P-384")]
    P384,
    #[serde(rename = "P-521")]
    P521,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
/// [`JWKUse`] identifies the intended use of the public key
pub enum JWKUse {
    #[serde(rename = "sig")]
    Signature,
    #[serde(rename = "enc")]
    Encryption,
}

impl JWK {
    /// Create a [`JWK`] for the given [`EcdsaKeyPair`]
    ///
    /// Warning: make sure to specify the correct algorithm.
    /// If `https://github.com/aws/aws-lc-rs/pull/834` gets merged this won't be needed anymore
    fn new_from_escdsa_keypair(key: &EcdsaKeyPair, alg: JWA) -> Result<Self, OpaqueError> {
        let curve = alg.try_into()?;
        // 0x04 prefix + x + y
        let pub_key = key.public_key().as_ref();
        let middle = (pub_key.len() - 1) / 2;
        let (x, y) = key.public_key().as_ref()[1..].split_at(middle);

        Ok(Self {
            alg,
            key_type: JWKType::EC {
                crv: curve,
                x: BASE64_URL_SAFE_NO_PAD.encode(x),
                y: BASE64_URL_SAFE_NO_PAD.encode(y),
            },
            r#use: Some(JWKUse::Signature),
            key_ops: None,
            x5c: None,
            x5t: None,
            x5t_sha256: None,
        })
    }

    /// [`JWKThumb`] as defined in [`rfc7638`] is url safe identifier for a [`JWK`]
    ///
    /// [`rfc7638`]: https://datatracker.ietf.org/doc/html/rfc7638
    pub fn thumb_sha256(&self) -> Result<Digest, OpaqueError> {
        Ok(digest(
            &SHA256,
            &serde_json::to_vec(&self.key_type).context("failed to serialise JWK")?,
        ))
    }

    /// Convert this [`JWK`] to an unparsed public key which can be used to verify signatures
    ///
    /// Warning no verification is done on this key until `.verify()` is called
    pub fn unparsed_public_key(
        &self,
    ) -> Result<signature::UnparsedPublicKey<Vec<u8>>, OpaqueError> {
        match &self.key_type {
            JWKType::RSA { .. } => Err(OpaqueError::from_display("currently not supported")),
            JWKType::OCT { .. } => Err(OpaqueError::from_display(
                "Symmetric key cannot be converted to public key",
            )),
            JWKType::EC { crv, x, y } => {
                let alg: &'static EcdsaVerificationAlgorithm =
                    JWA::from(crv.to_owned()).try_into()?;

                let x_bytes = BASE64_URL_SAFE_NO_PAD
                    .decode(x)
                    .context("decode ec curve x point")?;
                let y_bytes = BASE64_URL_SAFE_NO_PAD
                    .decode(y)
                    .context("decode ec curve y point")?;

                let mut point_bytes = Vec::with_capacity(1 + x_bytes.len() + y_bytes.len());
                point_bytes.push(0x04);
                point_bytes.extend_from_slice(&x_bytes);
                point_bytes.extend_from_slice(&y_bytes);

                Ok(signature::UnparsedPublicKey::new(alg, point_bytes))
            }
        }
    }
}

/// [`EcdsaKey`] which is used to identify and authenticate our requests
///
/// This contains the private and public key we will be using for JWS
pub struct EcdsaKey {
    rng: SystemRandom,
    alg: JWA,
    inner: EcdsaKeyPair,
}

impl EcdsaKey {
    /// Create a new [`EcdsaKey`] from the given [`EcdsaKeyPair`]
    pub fn new(key_pair: EcdsaKeyPair, alg: JWA, rng: SystemRandom) -> Result<Self, OpaqueError> {
        // Check if passed algorithm is a correct elliptic curve one
        let _curve = JWKEllipticCurves::try_from(alg)?;
        Ok(Self {
            rng,
            alg,
            inner: key_pair,
        })
    }

    /// Generate a new [`EcdsaKey`] from a newly generated [`EcdsaKeyPair`] using P-256 EC
    pub fn generate() -> Result<Self, OpaqueError> {
        let key_pair = EcdsaKeyPair::generate(&ECDSA_P256_SHA256_FIXED_SIGNING)
            .context("generate EcdsaKeyPair")?;

        Self::new(key_pair, JWA::ES256, SystemRandom::new())
    }

    /// Generate a new [`EcdsaKey`] from the given pkcs8 der
    pub fn from_pkcs8_der(
        pkcs8_der: &[u8],
        alg: JWA,
        rng: SystemRandom,
    ) -> Result<Self, OpaqueError> {
        let ec_alg: &'static EcdsaSigningAlgorithm = alg.try_into()?;
        let key_pair = EcdsaKeyPair::from_pkcs8(ec_alg, pkcs8_der)
            .context("create EcdsaKeyPair from pkcs8")?;

        Self::new(key_pair, alg, rng)
    }

    /// Create pkcs8 der for the current [`EcdsaKeyPair`]
    pub fn pkcs8_der(&self) -> Result<(JWA, Document), OpaqueError> {
        let doc = self
            .inner
            .to_pkcs8v1()
            .context("create pkcs8 der from keypair")?;
        Ok((self.alg, doc))
    }

    /// Create a [`JWK`] for this [`EcdsaKey`]
    #[must_use]
    pub fn create_jwk(&self) -> JWK {
        // `expect` because `new_for_escdsa_keypair`` can only fail if curve is not elliptic but we already check that in `new`
        JWK::new_from_escdsa_keypair(&self.inner, self.alg).expect("create JWK from escdsa keypair")
    }

    #[must_use]
    pub fn rng(&self) -> &SystemRandom {
        &self.rng
    }

    #[must_use]
    pub fn alg(&self) -> JWA {
        self.alg
    }
}

#[derive(Serialize)]
struct EcdsaKeySigningHeaders<'a> {
    alg: JWA,
    jwk: &'a JWK,
}

impl Signer for EcdsaKey {
    type Signature = Signature;
    type Error = OpaqueError;

    fn set_headers(
        &self,
        protected_headers: &mut super::jws::Headers,
        _unprotected_headers: &mut super::jws::Headers,
    ) -> Result<(), Self::Error> {
        let jwk = self.create_jwk();
        protected_headers.try_set_headers(EcdsaKeySigningHeaders {
            alg: jwk.alg,
            jwk: &jwk,
        })?;
        Ok(())
    }

    fn sign(&self, data: &str) -> Result<Self::Signature, Self::Error> {
        let sig = self
            .inner
            .sign(self.rng(), data.as_bytes())
            .context("sign protected data")?;

        Ok(sig)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jwk_thumb_order_is_correct() {
        let jwk_type = JWKType::EC {
            crv: JWKEllipticCurves::P256,
            x: "x".into(),
            y: "y".into(),
        };
        let output = serde_json::to_string(&jwk_type).unwrap();
        let expected_output = r##"{"crv":"P-256","kty":"EC","x":"x","y":"y"}"##;
        assert_eq!(&output, expected_output);

        let jwk_type = JWKType::RSA {
            n: "n".into(),
            e: "e".into(),
        };
        let output = serde_json::to_string(&jwk_type).unwrap();
        let expected_output = r##"{"e":"e","kty":"RSA","n":"n"}"##;
        assert_eq!(&output, expected_output);

        let jwk_type = JWKType::OCT { k: "k".into() };
        let output = serde_json::to_string(&jwk_type).unwrap();
        let expected_output = r##"{"k":"k","kty":"OCT"}"##;
        assert_eq!(&output, expected_output);
    }

    #[test]
    fn can_generate_and_reuse_keys() {
        let key = EcdsaKey::generate().unwrap();
        let stored = key.pkcs8_der().unwrap();
        let recreated_key =
            EcdsaKey::from_pkcs8_der(stored.1.as_ref(), stored.0, SystemRandom::new()).unwrap();

        assert_eq!(key.create_jwk(), recreated_key.create_jwk())
    }
}
