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

//! Public key signatures: signing and verification.
//!
//! Use the `verify` function to verify signatures, passing a reference to the
//! algorithm that identifies the algorithm. See the documentation for `verify`
//! for examples.
//!
//! For signature verification, this API treats each combination of parameters
//! as a separate algorithm. For example, instead of having a single "RSA"
//! algorithm with a verification function that takes a bunch of parameters,
//! there are `RSA_PKCS1_2048_8192_SHA256`, `RSA_PKCS1_2048_8192_SHA384`, etc.,
//! which encode sets of parameter choices into objects. This is designed to
//! reduce the risks of algorithm agility and to provide consistency with ECDSA
//! and EdDSA.
//!
//! Currently this module does not support digesting the message to be signed
//! separately from the public key operation, as it is currently being
//! optimized for Ed25519 and for the implementation of protocols that do not
//! requiring signing large messages. An interface for efficiently supporting
//! larger messages may be added later.
//!
//!
//! # Algorithm Details
//!
//! ## `ECDSA_*_ASN1` Details: ASN.1-encoded ECDSA Signatures
//!
//! The signature is a ASN.1 DER-encoded `Ecdsa-Sig-Value` as described in
//! [RFC 3279 Section 2.2.3]. This is the form of ECDSA signature used in
//! X.509-related structures and in TLS's `ServerKeyExchange` messages.
//!
//! The public key is encoding in uncompressed form using the
//! Octet-String-to-Elliptic-Curve-Point algorithm in
//! [SEC 1: Elliptic Curve Cryptography, Version 2.0].
//!
//! During verification, the public key is validated using the ECC Partial
//! Public-Key Validation Routine from Section 5.6.2.3.3 of
//! [NIST Special Publication 800-56A, revision 2] and Appendix A.3 of the
//! NSA's [Suite B implementer's guide to FIPS 186-3]. Note that, as explained
//! in the NSA guide, ECC Partial Public-Key Validation is equivalent to ECC
//! Full Public-Key Validation for prime-order curves like this one.
//!
//! ## `ECDSA_*_FIXED` Details: Fixed-length (PKCS#11-style) ECDSA Signatures
//!
//! The signature is *r*||*s*, where || denotes concatenation, and where both
//! *r* and *s* are both big-endian-encoded values that are left-padded to the
//! maximum length. A P-256 signature will be 64 bytes long (two 32-byte
//! components) and a P-384 signature will be 96 bytes long (two 48-byte
//! components). This is the form of ECDSA signature used PKCS#11 and DNSSEC.
//!
//! The public key is encoding in uncompressed form using the
//! Octet-String-to-Elliptic-Curve-Point algorithm in
//! [SEC 1: Elliptic Curve Cryptography, Version 2.0].
//!
//! During verification, the public key is validated using the ECC Partial
//! Public-Key Validation Routine from Section 5.6.2.3.3 of
//! [NIST Special Publication 800-56A, revision 2] and Appendix A.3 of the
//! NSA's [Suite B implementer's guide to FIPS 186-3]. Note that, as explained
//! in the NSA guide, ECC Partial Public-Key Validation is equivalent to ECC
//! Full Public-Key Validation for prime-order curves like this one.
//!
//! ## `RSA_PKCS1_*` Details: RSA PKCS#1 1.5 Signatures
//!
//! The signature is an RSASSA-PKCS1-v1_5 signature as described in
//! [RFC 3447 Section 8.2].
//!
//! The public key is encoded as an ASN.1 `RSAPublicKey` as described in
//! [RFC 3447 Appendix-A.1.1]. The public key modulus length, rounded *up* to
//! the nearest (larger) multiple of 8 bits, must be in the range given in the
//! name of the algorithm. The public exponent must be an odd integer of 2-33
//! bits, inclusive.
//!
//!
//! ## `RSA_PSS_*` Details: RSA PSS Signatures
//!
//! The signature is an RSASSA-PSS signature as described in
//! [RFC 3447 Section 8.1].
//!
//! The public key is encoded as an ASN.1 `RSAPublicKey` as described in
//! [RFC 3447 Appendix-A.1.1]. The public key modulus length, rounded *up* to
//! the nearest (larger) multiple of 8 bits, must be in the range given in the
//! name of the algorithm. The public exponent must be an odd integer of 2-33
//! bits, inclusive.
//!
//! During verification, signatures will only be accepted if the MGF1 digest
//! algorithm is the same as the message digest algorithm and if the salt
//! length is the same length as the message digest. This matches the
//! requirements in TLS 1.3 and other recent specifications.
//!
//! During signing, the message digest algorithm will be used as the MGF1
//! digest algorithm. The salt will be the same length as the message digest.
//! This matches the requirements in TLS 1.3 and other recent specifications.
//! Additionally, the entire salt is randomly generated separately for each
//! signature using the secure random number generator passed to `sign()`.
//!
//!
//! [SEC 1: Elliptic Curve Cryptography, Version 2.0]:
//!     http://www.secg.org/sec1-v2.pdf
//! [NIST Special Publication 800-56A, revision 2]:
//!     http://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-56Ar2.pdf
//! [Suite B implementer's guide to FIPS 186-3]:
//!     https://github.com/briansmith/ring/blob/main/doc/ecdsa.pdf
//! [RFC 3279 Section 2.2.3]:
//!     https://tools.ietf.org/html/rfc3279#section-2.2.3
//! [RFC 3447 Section 8.2]:
//!     https://tools.ietf.org/html/rfc3447#section-7.2
//! [RFC 3447 Section 8.1]:
//!     https://tools.ietf.org/html/rfc3447#section-8.1
//! [RFC 3447 Appendix-A.1.1]:
//!     https://tools.ietf.org/html/rfc3447#appendix-A.1.1
//!
//!
//! # Examples
//!
//! ## Signing and verifying with Ed25519
//!
//! ```
//! use ring::{
//!     rand,
//!     signature::{self, KeyPair},
//! };
//!
//! # fn main() -> Result<(), ring::error::Unspecified> {
//! // Generate a key pair in PKCS#8 (v2) format.
//! let rng = rand::SystemRandom::new();
//! let pkcs8_bytes = signature::Ed25519KeyPair::generate_pkcs8(&rng)?;
//!
//! // Normally the application would store the PKCS#8 file persistently. Later
//! // it would read the PKCS#8 file from persistent storage to use it.
//!
//! let key_pair = signature::Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref())?;
//!
//! // Sign the message "hello, world".
//! const MESSAGE: &[u8] = b"hello, world";
//! let sig = key_pair.sign(MESSAGE);
//!
//! // Normally an application would extract the bytes of the signature and
//! // send them in a protocol message to the peer(s). Here we just get the
//! // public key key directly from the key pair.
//! let peer_public_key_bytes = key_pair.public_key().as_ref();
//!
//! // Verify the signature of the message using the public key. Normally the
//! // verifier of the message would parse the inputs to this code out of the
//! // protocol message(s) sent by the signer.
//! let peer_public_key =
//!     signature::UnparsedPublicKey::new(&signature::ED25519, peer_public_key_bytes);
//! peer_public_key.verify(MESSAGE, sig.as_ref())?;
//!
//! # Ok(())
//! # }
//! ```
//!
//! ## Signing and verifying with RSA (PKCS#1 1.5 padding)
//!
//! By default OpenSSL writes RSA public keys in SubjectPublicKeyInfo format,
//! not RSAPublicKey format, and Base64-encodes them (“PEM” format).
//!
//! To convert the PEM SubjectPublicKeyInfo format (“BEGIN PUBLIC KEY”) to the
//! binary RSAPublicKey format needed by `verify()`, use:
//!
//! ```sh
//! openssl rsa -pubin \
//!             -in public_key.pem \
//!             -inform PEM \
//!             -RSAPublicKey_out \
//!             -outform DER \
//!             -out public_key.der
//! ```
//!
//! To extract the RSAPublicKey-formatted public key from an ASN.1 (binary)
//! DER-encoded RSAPrivateKey format private key file, use:
//!
//! ```sh
//! openssl rsa -in private_key.der \
//!             -inform DER \
//!             -RSAPublicKey_out \
//!             -outform DER \
//!             -out public_key.der
//! ```
//!
//! ```
//! # #[cfg(feature = "std")]
//! use ring::{rand, rsa, signature};
//!
//! # #[cfg(feature = "std")]
//! fn sign_and_verify_rsa(private_key_path: &std::path::Path,
//!                        public_key_path: &std::path::Path)
//!                        -> Result<(), MyError> {
//! // Create an RSA keypair from the DER-encoded bytes. This example uses
//! // a 2048-bit key, but larger keys are also supported.
//! let private_key_der = read_file(private_key_path)?;
//! let key_pair = rsa::KeyPair::from_der(&private_key_der)
//!     .map_err(|_| MyError::BadPrivateKey)?;
//!
//! // Sign the message "hello, world", using PKCS#1 v1.5 padding and the
//! // SHA256 digest algorithm.
//! const MESSAGE: &'static [u8] = b"hello, world";
//! let rng = rand::SystemRandom::new();
//! let mut signature = vec![0; key_pair.public().modulus_len()];
//! key_pair.sign(&signature::RSA_PKCS1_SHA256, &rng, MESSAGE, &mut signature)
//!     .map_err(|_| MyError::OOM)?;
//!
//! // Verify the signature.
//! let public_key =
//!     signature::UnparsedPublicKey::new(&signature::RSA_PKCS1_2048_8192_SHA256,
//!                                       read_file(public_key_path)?);
//! public_key.verify(MESSAGE, &signature)
//!     .map_err(|_| MyError::BadSignature)
//! }
//!
//! #[derive(Debug)]
//! enum MyError {
//! #  #[cfg(feature = "std")]
//!    IO(std::io::Error),
//!    BadPrivateKey,
//!    OOM,
//!    BadSignature,
//! }
//!
//! # #[cfg(feature = "std")]
//! fn read_file(path: &std::path::Path) -> Result<Vec<u8>, MyError> {
//!     use std::io::Read;
//!
//!     let mut file = std::fs::File::open(path).map_err(|e| MyError::IO(e))?;
//!     let mut contents: Vec<u8> = Vec::new();
//!     file.read_to_end(&mut contents).map_err(|e| MyError::IO(e))?;
//!     Ok(contents)
//! }
//! #
//! # #[cfg(not(feature = "std"))]
//! # fn sign_and_verify_rsa(_private_key_path: &std::path::Path,
//! #                        _public_key_path: &std::path::Path)
//! #                        -> Result<(), ()> {
//! #     Ok(())
//! # }
//! #
//! # fn main() {
//! #     let private_key_path =
//! #         std::path::Path::new("src/rsa/signature_rsa_example_private_key.der");
//! #     let public_key_path =
//! #         std::path::Path::new("src/rsa/signature_rsa_example_public_key.der");
//! #     sign_and_verify_rsa(&private_key_path, &public_key_path).unwrap()
//! # }
//! ```

use crate::{cpu, debug, ec, error, sealed};

pub use crate::ec::{
    curve25519::ed25519::{
        signing::Ed25519KeyPair,
        verification::{EdDSAParameters, ED25519},
        ED25519_PUBLIC_KEY_LEN,
    },
    suite_b::ecdsa::{
        signing::{
            EcdsaKeyPair, EcdsaSigningAlgorithm, ECDSA_P256_SHA256_ASN1_SIGNING,
            ECDSA_P256_SHA256_FIXED_SIGNING, ECDSA_P384_SHA384_ASN1_SIGNING,
            ECDSA_P384_SHA384_FIXED_SIGNING,
        },
        verification::{
            EcdsaVerificationAlgorithm, ECDSA_P256_SHA256_ASN1, ECDSA_P256_SHA256_FIXED,
            ECDSA_P256_SHA384_ASN1, ECDSA_P384_SHA256_ASN1, ECDSA_P384_SHA384_ASN1,
            ECDSA_P384_SHA384_FIXED,
        },
    },
};

#[cfg(feature = "alloc")]
pub use crate::rsa::{
    padding::{
        RsaEncoding, RSA_PKCS1_SHA256, RSA_PKCS1_SHA384, RSA_PKCS1_SHA512, RSA_PSS_SHA256,
        RSA_PSS_SHA384, RSA_PSS_SHA512,
    },
    verification::{
        RsaPublicKeyComponents, RSA_PKCS1_1024_8192_SHA1_FOR_LEGACY_USE_ONLY,
        RSA_PKCS1_1024_8192_SHA256_FOR_LEGACY_USE_ONLY,
        RSA_PKCS1_1024_8192_SHA512_FOR_LEGACY_USE_ONLY,
        RSA_PKCS1_2048_8192_SHA1_FOR_LEGACY_USE_ONLY, RSA_PKCS1_2048_8192_SHA256,
        RSA_PKCS1_2048_8192_SHA384, RSA_PKCS1_2048_8192_SHA512, RSA_PKCS1_3072_8192_SHA384,
        RSA_PSS_2048_8192_SHA256, RSA_PSS_2048_8192_SHA384, RSA_PSS_2048_8192_SHA512,
    },
    RsaParameters,
};

/// An RSA key pair, used for signing.
#[cfg(feature = "alloc")]
pub type RsaKeyPair = crate::rsa::KeyPair;

/// A public key signature returned from a signing operation.
#[derive(Clone, Copy)]
pub struct Signature {
    value: [u8; MAX_LEN],
    len: usize,
}

impl Signature {
    // Panics if `value` is too long.
    pub(crate) fn new<F>(fill: F) -> Self
    where
        F: FnOnce(&mut [u8; MAX_LEN]) -> usize,
    {
        let mut r = Self {
            value: [0; MAX_LEN],
            len: 0,
        };
        r.len = fill(&mut r.value);
        r
    }
}

impl AsRef<[u8]> for Signature {
    fn as_ref(&self) -> &[u8] {
        &self.value[..self.len]
    }
}

/// Key pairs for signing messages (private key and public key).
pub trait KeyPair: core::fmt::Debug + Send + Sized + Sync {
    /// The type of the public key.
    type PublicKey: AsRef<[u8]> + core::fmt::Debug + Clone + Send + Sized + Sync;

    /// The public key for the key pair.
    fn public_key(&self) -> &Self::PublicKey;
}

/// The longest signature is an ASN.1 P-384 signature where *r* and *s* are of
/// maximum length with the leading high bit set on each. Then each component
/// will have a tag, a one-byte length, and a one-byte “I'm not negative”
/// prefix, and the outer sequence will have a two-byte length.
pub(crate) const MAX_LEN: usize = 1/*tag:SEQUENCE*/ + 2/*len*/ +
    (2 * (1/*tag:INTEGER*/ + 1/*len*/ + 1/*zero*/ + ec::SCALAR_MAX_BYTES));

/// A signature verification algorithm.
pub trait VerificationAlgorithm: core::fmt::Debug + Sync + sealed::Sealed {
    /// Verify the signature `signature` of message `msg` with the public key
    /// `public_key`.
    fn verify(
        &self,
        public_key: untrusted::Input,
        msg: untrusted::Input,
        signature: untrusted::Input,
    ) -> Result<(), error::Unspecified>;
}

/// An unparsed, possibly malformed, public key for signature verification.
#[derive(Clone, Copy)]
pub struct UnparsedPublicKey<B> {
    algorithm: &'static dyn VerificationAlgorithm,
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
    /// Construct a new `UnparsedPublicKey`.
    ///
    /// No validation of `bytes` is done until `verify()` is called.
    #[inline]
    pub fn new(algorithm: &'static dyn VerificationAlgorithm, bytes: B) -> Self {
        Self { algorithm, bytes }
    }

    /// Parses the public key and verifies `signature` is a valid signature of
    /// `message` using it.
    ///
    /// See the [crate::signature] module-level documentation for examples.
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), error::Unspecified>
    where
        B: AsRef<[u8]>,
    {
        let _ = cpu::features();
        self.algorithm.verify(
            untrusted::Input::from(self.bytes.as_ref()),
            untrusted::Input::from(message),
            untrusted::Input::from(signature),
        )
    }
}
