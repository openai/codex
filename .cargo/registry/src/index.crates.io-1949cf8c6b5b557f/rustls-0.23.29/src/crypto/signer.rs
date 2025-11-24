use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt::Debug;

use pki_types::{AlgorithmIdentifier, CertificateDer, PrivateKeyDer, SubjectPublicKeyInfoDer};

use super::CryptoProvider;
use crate::client::ResolvesClientCert;
use crate::enums::{SignatureAlgorithm, SignatureScheme};
use crate::error::{Error, InconsistentKeys};
use crate::server::{ClientHello, ParsedCertificate, ResolvesServerCert};
use crate::sync::Arc;
use crate::x509;

/// An abstract signing key.
///
/// This interface is used by rustls to use a private signing key
/// for authentication.  This includes server and client authentication.
///
/// Objects of this type are always used within Rustls as
/// `Arc<dyn SigningKey>`. There are no concrete public structs in Rustls
/// that implement this trait.
///
/// There are two main ways to get a signing key:
///
///  - [`KeyProvider::load_private_key()`], or
///  - some other method outside of the `KeyProvider` extension trait,
///    for instance:
///    - [`crypto::ring::sign::any_ecdsa_type()`]
///    - [`crypto::ring::sign::any_eddsa_type()`]
///    - [`crypto::ring::sign::any_supported_type()`]
///    - [`crypto::aws_lc_rs::sign::any_ecdsa_type()`]
///    - [`crypto::aws_lc_rs::sign::any_eddsa_type()`]
///    - [`crypto::aws_lc_rs::sign::any_supported_type()`]
///
/// The `KeyProvider` method `load_private_key()` is called under the hood by
/// [`ConfigBuilder::with_single_cert()`],
/// [`ConfigBuilder::with_client_auth_cert()`], and
/// [`ConfigBuilder::with_single_cert_with_ocsp()`].
///
/// A signing key created outside of the `KeyProvider` extension trait can be used
/// to create a [`CertifiedKey`], which in turn can be used to create a
/// [`ResolvesServerCertUsingSni`]. Alternately, a `CertifiedKey` can be returned from a
/// custom implementation of the [`ResolvesServerCert`] or [`ResolvesClientCert`] traits.
///
/// [`KeyProvider::load_private_key()`]: crate::crypto::KeyProvider::load_private_key
/// [`ConfigBuilder::with_single_cert()`]: crate::ConfigBuilder::with_single_cert
/// [`ConfigBuilder::with_single_cert_with_ocsp()`]: crate::ConfigBuilder::with_single_cert_with_ocsp
/// [`ConfigBuilder::with_client_auth_cert()`]: crate::ConfigBuilder::with_client_auth_cert
/// [`crypto::ring::sign::any_ecdsa_type()`]: crate::crypto::ring::sign::any_ecdsa_type
/// [`crypto::ring::sign::any_eddsa_type()`]: crate::crypto::ring::sign::any_eddsa_type
/// [`crypto::ring::sign::any_supported_type()`]: crate::crypto::ring::sign::any_supported_type
/// [`crypto::aws_lc_rs::sign::any_ecdsa_type()`]: crate::crypto::aws_lc_rs::sign::any_ecdsa_type
/// [`crypto::aws_lc_rs::sign::any_eddsa_type()`]: crate::crypto::aws_lc_rs::sign::any_eddsa_type
/// [`crypto::aws_lc_rs::sign::any_supported_type()`]: crate::crypto::aws_lc_rs::sign::any_supported_type
/// [`ResolvesServerCertUsingSni`]: crate::server::ResolvesServerCertUsingSni
/// [`ResolvesServerCert`]: crate::server::ResolvesServerCert
/// [`ResolvesClientCert`]: crate::client::ResolvesClientCert
pub trait SigningKey: Debug + Send + Sync {
    /// Choose a `SignatureScheme` from those offered.
    ///
    /// Expresses the choice by returning something that implements `Signer`,
    /// using the chosen scheme.
    fn choose_scheme(&self, offered: &[SignatureScheme]) -> Option<Box<dyn Signer>>;

    /// Get the RFC 5280-compliant SubjectPublicKeyInfo (SPKI) of this [`SigningKey`] if available.
    fn public_key(&self) -> Option<SubjectPublicKeyInfoDer<'_>> {
        // Opt-out by default
        None
    }

    /// What kind of key we have.
    fn algorithm(&self) -> SignatureAlgorithm;
}

/// A thing that can sign a message.
pub trait Signer: Debug + Send + Sync {
    /// Signs `message` using the selected scheme.
    ///
    /// `message` is not hashed; the implementer must hash it using the hash function
    /// implicit in [`Self::scheme()`].
    ///
    /// The returned signature format is also defined by [`Self::scheme()`].
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, Error>;

    /// Reveals which scheme will be used when you call [`Self::sign()`].
    fn scheme(&self) -> SignatureScheme;
}

/// Server certificate resolver which always resolves to the same certificate and key.
///
/// For use with [`ConfigBuilder::with_cert_resolver()`].
///
/// [`ConfigBuilder::with_cert_resolver()`]: crate::ConfigBuilder::with_cert_resolver
#[derive(Debug)]
pub struct SingleCertAndKey(Arc<CertifiedKey>);

impl From<CertifiedKey> for SingleCertAndKey {
    fn from(certified_key: CertifiedKey) -> Self {
        Self(Arc::new(certified_key))
    }
}

impl From<Arc<CertifiedKey>> for SingleCertAndKey {
    fn from(certified_key: Arc<CertifiedKey>) -> Self {
        Self(certified_key)
    }
}

impl ResolvesClientCert for SingleCertAndKey {
    fn resolve(
        &self,
        _root_hint_subjects: &[&[u8]],
        _sigschemes: &[SignatureScheme],
    ) -> Option<Arc<CertifiedKey>> {
        Some(self.0.clone())
    }

    fn has_certs(&self) -> bool {
        true
    }
}

impl ResolvesServerCert for SingleCertAndKey {
    fn resolve(&self, _client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        Some(self.0.clone())
    }
}

/// A packaged-together certificate chain, matching `SigningKey` and
/// optional stapled OCSP response.
///
/// Note: this struct is also used to represent an [RFC 7250] raw public key,
/// when the client/server is configured to use raw public keys instead of
/// certificates.
///
/// [RFC 7250]: https://tools.ietf.org/html/rfc7250
#[derive(Clone, Debug)]
pub struct CertifiedKey {
    /// The certificate chain or raw public key.
    pub cert: Vec<CertificateDer<'static>>,

    /// The certified key.
    pub key: Arc<dyn SigningKey>,

    /// An optional OCSP response from the certificate issuer,
    /// attesting to its continued validity.
    pub ocsp: Option<Vec<u8>>,
}

impl CertifiedKey {
    /// Create a new `CertifiedKey` from a certificate chain and DER-encoded private key.
    ///
    /// Attempt to parse the private key with the given [`CryptoProvider`]'s [`KeyProvider`] and
    /// verify that it matches the public key in the first certificate of the `cert_chain`
    /// if possible.
    ///
    /// [`KeyProvider`]: crate::crypto::KeyProvider
    pub fn from_der(
        cert_chain: Vec<CertificateDer<'static>>,
        key: PrivateKeyDer<'static>,
        provider: &CryptoProvider,
    ) -> Result<Self, Error> {
        let private_key = provider
            .key_provider
            .load_private_key(key)?;

        let certified_key = Self::new(cert_chain, private_key);
        match certified_key.keys_match() {
            // Don't treat unknown consistency as an error
            Ok(()) | Err(Error::InconsistentKeys(InconsistentKeys::Unknown)) => Ok(certified_key),
            Err(err) => Err(err),
        }
    }

    /// Make a new CertifiedKey, with the given chain and key.
    ///
    /// The cert chain must not be empty. The first certificate in the chain
    /// must be the end-entity certificate.
    pub fn new(cert: Vec<CertificateDer<'static>>, key: Arc<dyn SigningKey>) -> Self {
        Self {
            cert,
            key,
            ocsp: None,
        }
    }

    /// Verify the consistency of this [`CertifiedKey`]'s public and private keys.
    /// This is done by performing a comparison of SubjectPublicKeyInfo bytes.
    pub fn keys_match(&self) -> Result<(), Error> {
        let Some(key_spki) = self.key.public_key() else {
            return Err(InconsistentKeys::Unknown.into());
        };

        let cert = ParsedCertificate::try_from(self.end_entity_cert()?)?;
        match key_spki == cert.subject_public_key_info() {
            true => Ok(()),
            false => Err(InconsistentKeys::KeyMismatch.into()),
        }
    }

    /// The end-entity certificate.
    pub fn end_entity_cert(&self) -> Result<&CertificateDer<'_>, Error> {
        self.cert
            .first()
            .ok_or(Error::NoCertificatesPresented)
    }
}

#[cfg_attr(not(any(feature = "aws_lc_rs", feature = "ring")), allow(dead_code))]
pub(crate) fn public_key_to_spki(
    alg_id: &AlgorithmIdentifier,
    public_key: impl AsRef<[u8]>,
) -> SubjectPublicKeyInfoDer<'static> {
    // SubjectPublicKeyInfo  ::=  SEQUENCE  {
    //    algorithm            AlgorithmIdentifier,
    //    subjectPublicKey     BIT STRING  }
    //
    // AlgorithmIdentifier  ::=  SEQUENCE  {
    //    algorithm               OBJECT IDENTIFIER,
    //    parameters              ANY DEFINED BY algorithm OPTIONAL  }
    //
    // note that the `pki_types::AlgorithmIdentifier` type is the
    // concatenation of `algorithm` and `parameters`, but misses the
    // outer `Sequence`.

    let mut spki_inner = x509::wrap_in_sequence(alg_id.as_ref());
    spki_inner.extend(&x509::wrap_in_bit_string(public_key.as_ref()));

    let spki = x509::wrap_in_sequence(&spki_inner);

    SubjectPublicKeyInfoDer::from(spki)
}
