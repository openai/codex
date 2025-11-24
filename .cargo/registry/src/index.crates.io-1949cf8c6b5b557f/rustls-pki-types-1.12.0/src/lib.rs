//! This crate provides types for representing X.509 certificates, keys and other types as
//! commonly used in the rustls ecosystem. It is intended to be used by crates that need to work
//! with such X.509 types, such as [rustls](https://crates.io/crates/rustls),
//! [rustls-webpki](https://crates.io/crates/rustls-webpki),
//! [rustls-pemfile](https://crates.io/crates/rustls-pemfile), and others.
//!
//! Some of these crates used to define their own trivial wrappers around DER-encoded bytes.
//! However, in order to avoid inconvenient dependency edges, these were all disconnected. By
//! using a common low-level crate of types with long-term stable API, we hope to avoid the
//! downsides of unnecessary dependency edges while providing good interoperability between crates.
//!
//! ## DER and PEM
//!
//! Many of the types defined in this crate represent DER-encoded data. DER is a binary encoding of
//! the ASN.1 format commonly used in web PKI specifications. It is a binary encoding, so it is
//! relatively compact when stored in memory. However, as a binary format, it is not very easy to
//! work with for humans and in contexts where binary data is inconvenient. For this reason,
//! many tools and protocols use a ASCII-based encoding of DER, called PEM. In addition to the
//! base64-encoded DER, PEM objects are delimited by header and footer lines which indicate the type
//! of object contained in the PEM blob.
//!
//! Types here can be created from:
//!
//! - DER using (for example) [`PrivatePkcs8KeyDer::from()`].
//! - PEM using (for example) [`pem::PemObject::from_pem_slice()`].
//!
//! The [`pem::PemObject`] trait contains the full selection of ways to construct
//! these types from PEM encodings.  That includes ways to open and read from a file,
//! from a slice, or from an `std::io` stream.
//!
//! There is also a lower-level API that allows a given PEM file to be fully consumed
//! in one pass, even if it contains different data types: see the implementation of
//! the [`pem::PemObject`] trait on the `(pem::SectionKind, Vec<u8>)` tuple.
//!
//! ## Creating new certificates and keys
//!
//! This crate does not provide any functionality for creating new certificates or keys. However,
//! the [rcgen](https://docs.rs/rcgen) crate can be used to create new certificates and keys.
//!
//! ## Cloning private keys
//!
//! This crate intentionally **does not** implement `Clone` on private key types in
//! order to minimize the exposure of private key data in memory.
//!
//! If you want to extend the lifetime of a `PrivateKeyDer<'_>`, consider [`PrivateKeyDer::clone_key()`].
//! Alternatively  since these types are immutable, consider wrapping the `PrivateKeyDer<'_>` in a [`Rc`]
//! or an [`Arc`].
//!
//! [`Rc`]: https://doc.rust-lang.org/std/rc/struct.Rc.html
//! [`Arc`]: https://doc.rust-lang.org/std/sync/struct.Arc.html
//! [`PrivateKeyDer::clone_key()`]: https://docs.rs/rustls-pki-types/latest/rustls_pki_types/enum.PrivateKeyDer.html#method.clone_key
//!
//! ## Target `wasm32-unknown-unknown` with the `web` feature
//!
//! [`std::time::SystemTime`](https://doc.rust-lang.org/std/time/struct.SystemTime.html)
//! is unavailable in `wasm32-unknown-unknown` targets, so calls to
//! [`UnixTime::now()`](https://docs.rs/rustls-pki-types/latest/rustls_pki_types/struct.UnixTime.html#method.now),
//! otherwise enabled by the [`std`](https://docs.rs/crate/rustls-pki-types/latest/features#std) feature,
//! require building instead with the [`web`](https://docs.rs/crate/rustls-pki-types/latest/features#web)
//! feature. It gets time by calling [`Date.now()`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Date/now)
//! in the browser.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unreachable_pub, clippy::use_self)]
#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;
use core::fmt;
use core::ops::Deref;
use core::time::Duration;
#[cfg(feature = "alloc")]
use pem::{PemObject, PemObjectFilter, SectionKind};
#[cfg(all(
    feature = "std",
    not(all(target_family = "wasm", target_os = "unknown"))
))]
use std::time::SystemTime;
#[cfg(all(target_family = "wasm", target_os = "unknown", feature = "web"))]
use web_time::SystemTime;

pub mod alg_id;
mod base64;
mod server_name;

/// Low-level PEM decoding APIs.
///
/// These APIs allow decoding PEM format in an iterator, which means you
/// can load multiple different types of PEM section from a file in a single
/// pass.
#[cfg(feature = "alloc")]
pub mod pem;

pub use alg_id::AlgorithmIdentifier;
pub use server_name::{
    AddrParseError, DnsName, InvalidDnsNameError, IpAddr, Ipv4Addr, Ipv6Addr, ServerName,
};

/// A DER-encoded X.509 private key, in one of several formats
///
/// See variant inner types for more detailed information.
///
/// This can load several types of PEM-encoded private key, and then reveal
/// which types were found:
///
/// ```rust
/// # #[cfg(all(feature = "alloc", feature = "std"))] {
/// use rustls_pki_types::{PrivateKeyDer, pem::PemObject};
///
/// // load from a PEM file
/// let pkcs8 = PrivateKeyDer::from_pem_file("tests/data/nistp256key.pkcs8.pem").unwrap();
/// let pkcs1 = PrivateKeyDer::from_pem_file("tests/data/rsa1024.pkcs1.pem").unwrap();
/// let sec1 = PrivateKeyDer::from_pem_file("tests/data/nistp256key.pem").unwrap();
/// assert!(matches!(pkcs8, PrivateKeyDer::Pkcs8(_)));
/// assert!(matches!(pkcs1, PrivateKeyDer::Pkcs1(_)));
/// assert!(matches!(sec1, PrivateKeyDer::Sec1(_)));
/// # }
/// ```
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq)]
pub enum PrivateKeyDer<'a> {
    /// An RSA private key
    Pkcs1(PrivatePkcs1KeyDer<'a>),
    /// A Sec1 private key
    Sec1(PrivateSec1KeyDer<'a>),
    /// A PKCS#8 private key
    Pkcs8(PrivatePkcs8KeyDer<'a>),
}

#[cfg(feature = "alloc")]
impl zeroize::Zeroize for PrivateKeyDer<'static> {
    fn zeroize(&mut self) {
        match self {
            Self::Pkcs1(key) => key.zeroize(),
            Self::Sec1(key) => key.zeroize(),
            Self::Pkcs8(key) => key.zeroize(),
        }
    }
}

impl PrivateKeyDer<'_> {
    /// Clone the private key to a `'static` value
    #[cfg(feature = "alloc")]
    pub fn clone_key(&self) -> PrivateKeyDer<'static> {
        use PrivateKeyDer::*;
        match self {
            Pkcs1(key) => Pkcs1(key.clone_key()),
            Sec1(key) => Sec1(key.clone_key()),
            Pkcs8(key) => Pkcs8(key.clone_key()),
        }
    }

    /// Yield the DER-encoded bytes of the private key
    pub fn secret_der(&self) -> &[u8] {
        match self {
            PrivateKeyDer::Pkcs1(key) => key.secret_pkcs1_der(),
            PrivateKeyDer::Sec1(key) => key.secret_sec1_der(),
            PrivateKeyDer::Pkcs8(key) => key.secret_pkcs8_der(),
        }
    }
}

#[cfg(feature = "alloc")]
impl PemObject for PrivateKeyDer<'static> {
    fn from_pem(kind: SectionKind, value: Vec<u8>) -> Option<Self> {
        match kind {
            SectionKind::RsaPrivateKey => Some(Self::Pkcs1(value.into())),
            SectionKind::EcPrivateKey => Some(Self::Sec1(value.into())),
            SectionKind::PrivateKey => Some(Self::Pkcs8(value.into())),
            _ => None,
        }
    }
}

impl<'a> From<PrivatePkcs1KeyDer<'a>> for PrivateKeyDer<'a> {
    fn from(key: PrivatePkcs1KeyDer<'a>) -> Self {
        Self::Pkcs1(key)
    }
}

impl<'a> From<PrivateSec1KeyDer<'a>> for PrivateKeyDer<'a> {
    fn from(key: PrivateSec1KeyDer<'a>) -> Self {
        Self::Sec1(key)
    }
}

impl<'a> From<PrivatePkcs8KeyDer<'a>> for PrivateKeyDer<'a> {
    fn from(key: PrivatePkcs8KeyDer<'a>) -> Self {
        Self::Pkcs8(key)
    }
}

impl<'a> TryFrom<&'a [u8]> for PrivateKeyDer<'a> {
    type Error = &'static str;

    fn try_from(key: &'a [u8]) -> Result<Self, Self::Error> {
        const SHORT_FORM_LEN_MAX: u8 = 128;
        const TAG_SEQUENCE: u8 = 0x30;
        const TAG_INTEGER: u8 = 0x02;

        // We expect all key formats to begin with a SEQUENCE, which requires at least 2 bytes
        // in the short length encoding.
        if key.first() != Some(&TAG_SEQUENCE) || key.len() < 2 {
            return Err(INVALID_KEY_DER_ERR);
        }

        // The length of the SEQUENCE is encoded in the second byte. We must skip this many bytes.
        let skip_len = match key[1] >= SHORT_FORM_LEN_MAX {
            // 1 byte for SEQUENCE tag, 1 byte for short-form len
            false => 2,
            // 1 byte for SEQUENCE tag, 1 byte for start of len, remaining bytes encoded
            // in key[1].
            true => 2 + (key[1] - SHORT_FORM_LEN_MAX) as usize,
        };
        let key_bytes = key.get(skip_len..).ok_or(INVALID_KEY_DER_ERR)?;

        // PKCS#8 (https://www.rfc-editor.org/rfc/rfc5208) describes the PrivateKeyInfo
        // structure as:
        //   PrivateKeyInfo ::= SEQUENCE {
        //      version Version,
        //      privateKeyAlgorithm AlgorithmIdentifier {{PrivateKeyAlgorithms}},
        //      privateKey PrivateKey,
        //      attributes [0] Attributes OPTIONAL
        //   }
        // PKCS#5 (https://www.rfc-editor.org/rfc/rfc8018) describes the AlgorithmIdentifier
        // as a SEQUENCE.
        //
        // Therefore, we consider the outer SEQUENCE, a version number, and the start of
        // an AlgorithmIdentifier to be enough to identify a PKCS#8 key. If it were PKCS#1 or SEC1
        // the version would not be followed by a SEQUENCE.
        if matches!(key_bytes, [TAG_INTEGER, 0x01, _, TAG_SEQUENCE, ..]) {
            return Ok(Self::Pkcs8(key.into()));
        }

        // PKCS#1 (https://www.rfc-editor.org/rfc/rfc8017) describes the RSAPrivateKey structure
        // as:
        //  RSAPrivateKey ::= SEQUENCE {
        //              version           Version,
        //              modulus           INTEGER,  -- n
        //              publicExponent    INTEGER,  -- e
        //              privateExponent   INTEGER,  -- d
        //              prime1            INTEGER,  -- p
        //              prime2            INTEGER,  -- q
        //              exponent1         INTEGER,  -- d mod (p-1)
        //              exponent2         INTEGER,  -- d mod (q-1)
        //              coefficient       INTEGER,  -- (inverse of q) mod p
        //              otherPrimeInfos   OtherPrimeInfos OPTIONAL
        //          }
        //
        // Therefore, we consider the outer SEQUENCE and a Version of 0 to be enough to identify
        // a PKCS#1 key. If it were PKCS#8, the version would be followed by a SEQUENCE. If it
        // were SEC1, the VERSION would have been 1.
        if key_bytes.starts_with(&[TAG_INTEGER, 0x01, 0x00]) {
            return Ok(Self::Pkcs1(key.into()));
        }

        // SEC1 (https://www.rfc-editor.org/rfc/rfc5915) describes the ECPrivateKey structure as:
        //   ECPrivateKey ::= SEQUENCE {
        //      version        INTEGER { ecPrivkeyVer1(1) } (ecPrivkeyVer1),
        //      privateKey     OCTET STRING,
        //      parameters [0] ECParameters {{ NamedCurve }} OPTIONAL,
        //      publicKey  [1] BIT STRING OPTIONAL
        //   }
        //
        // Therefore, we consider the outer SEQUENCE and an INTEGER of 1 to be enough to
        // identify a SEC1 key. If it were PKCS#8 or PKCS#1, the version would have been 0.
        if key_bytes.starts_with(&[TAG_INTEGER, 0x01, 0x01]) {
            return Ok(Self::Sec1(key.into()));
        }

        Err(INVALID_KEY_DER_ERR)
    }
}

static INVALID_KEY_DER_ERR: &str = "unknown or invalid key format";

#[cfg(feature = "alloc")]
impl TryFrom<Vec<u8>> for PrivateKeyDer<'_> {
    type Error = &'static str;

    fn try_from(key: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(match PrivateKeyDer::try_from(&key[..])? {
            PrivateKeyDer::Pkcs1(_) => Self::Pkcs1(key.into()),
            PrivateKeyDer::Sec1(_) => Self::Sec1(key.into()),
            PrivateKeyDer::Pkcs8(_) => Self::Pkcs8(key.into()),
        })
    }
}

/// A DER-encoded plaintext RSA private key; as specified in PKCS#1/RFC 3447
///
/// RSA private keys are identified in PEM context as `RSA PRIVATE KEY` and when stored in a
/// file usually use a `.pem` or `.key` extension.
///
/// ```rust
/// # #[cfg(all(feature = "alloc", feature = "std"))] {
/// use rustls_pki_types::{PrivatePkcs1KeyDer, pem::PemObject};
///
/// // load from a PEM file
/// PrivatePkcs1KeyDer::from_pem_file("tests/data/rsa1024.pkcs1.pem").unwrap();
///
/// // or from a PEM byte slice...
/// # let byte_slice = include_bytes!("../tests/data/rsa1024.pkcs1.pem");
/// PrivatePkcs1KeyDer::from_pem_slice(byte_slice).unwrap();
/// # }
/// ```
#[derive(PartialEq, Eq)]
pub struct PrivatePkcs1KeyDer<'a>(Der<'a>);

impl PrivatePkcs1KeyDer<'_> {
    /// Clone the private key to a `'static` value
    #[cfg(feature = "alloc")]
    pub fn clone_key(&self) -> PrivatePkcs1KeyDer<'static> {
        PrivatePkcs1KeyDer::from(self.0.as_ref().to_vec())
    }

    /// Yield the DER-encoded bytes of the private key
    pub fn secret_pkcs1_der(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[cfg(feature = "alloc")]
impl zeroize::Zeroize for PrivatePkcs1KeyDer<'static> {
    fn zeroize(&mut self) {
        self.0.0.zeroize()
    }
}

#[cfg(feature = "alloc")]
impl PemObjectFilter for PrivatePkcs1KeyDer<'static> {
    const KIND: SectionKind = SectionKind::RsaPrivateKey;
}

impl<'a> From<&'a [u8]> for PrivatePkcs1KeyDer<'a> {
    fn from(slice: &'a [u8]) -> Self {
        Self(Der(BytesInner::Borrowed(slice)))
    }
}

#[cfg(feature = "alloc")]
impl From<Vec<u8>> for PrivatePkcs1KeyDer<'_> {
    fn from(vec: Vec<u8>) -> Self {
        Self(Der(BytesInner::Owned(vec)))
    }
}

impl fmt::Debug for PrivatePkcs1KeyDer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PrivatePkcs1KeyDer")
            .field(&"[secret key elided]")
            .finish()
    }
}

/// A Sec1-encoded plaintext private key; as specified in RFC 5915
///
/// Sec1 private keys are identified in PEM context as `EC PRIVATE KEY` and when stored in a
/// file usually use a `.pem` or `.key` extension. For more on PEM files, refer to the crate
/// documentation.
///
/// ```rust
/// # #[cfg(all(feature = "alloc", feature = "std"))] {
/// use rustls_pki_types::{PrivateSec1KeyDer, pem::PemObject};
///
/// // load from a PEM file
/// PrivateSec1KeyDer::from_pem_file("tests/data/nistp256key.pem").unwrap();
///
/// // or from a PEM byte slice...
/// # let byte_slice = include_bytes!("../tests/data/nistp256key.pem");
/// PrivateSec1KeyDer::from_pem_slice(byte_slice).unwrap();
/// # }
/// ```
#[derive(PartialEq, Eq)]
pub struct PrivateSec1KeyDer<'a>(Der<'a>);

impl PrivateSec1KeyDer<'_> {
    /// Clone the private key to a `'static` value
    #[cfg(feature = "alloc")]
    pub fn clone_key(&self) -> PrivateSec1KeyDer<'static> {
        PrivateSec1KeyDer::from(self.0.as_ref().to_vec())
    }

    /// Yield the DER-encoded bytes of the private key
    pub fn secret_sec1_der(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[cfg(feature = "alloc")]
impl zeroize::Zeroize for PrivateSec1KeyDer<'static> {
    fn zeroize(&mut self) {
        self.0.0.zeroize()
    }
}

#[cfg(feature = "alloc")]
impl PemObjectFilter for PrivateSec1KeyDer<'static> {
    const KIND: SectionKind = SectionKind::EcPrivateKey;
}

impl<'a> From<&'a [u8]> for PrivateSec1KeyDer<'a> {
    fn from(slice: &'a [u8]) -> Self {
        Self(Der(BytesInner::Borrowed(slice)))
    }
}

#[cfg(feature = "alloc")]
impl From<Vec<u8>> for PrivateSec1KeyDer<'_> {
    fn from(vec: Vec<u8>) -> Self {
        Self(Der(BytesInner::Owned(vec)))
    }
}

impl fmt::Debug for PrivateSec1KeyDer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PrivateSec1KeyDer")
            .field(&"[secret key elided]")
            .finish()
    }
}

/// A DER-encoded plaintext private key; as specified in PKCS#8/RFC 5958
///
/// PKCS#8 private keys are identified in PEM context as `PRIVATE KEY` and when stored in a
/// file usually use a `.pem` or `.key` extension. For more on PEM files, refer to the crate
/// documentation.
///
/// ```rust
/// # #[cfg(all(feature = "alloc", feature = "std"))] {
/// use rustls_pki_types::{PrivatePkcs8KeyDer, pem::PemObject};
///
/// // load from a PEM file
/// PrivatePkcs8KeyDer::from_pem_file("tests/data/nistp256key.pkcs8.pem").unwrap();
/// PrivatePkcs8KeyDer::from_pem_file("tests/data/rsa1024.pkcs8.pem").unwrap();
///
/// // or from a PEM byte slice...
/// # let byte_slice = include_bytes!("../tests/data/nistp256key.pkcs8.pem");
/// PrivatePkcs8KeyDer::from_pem_slice(byte_slice).unwrap();
/// # }
/// ```
#[derive(PartialEq, Eq)]
pub struct PrivatePkcs8KeyDer<'a>(Der<'a>);

impl PrivatePkcs8KeyDer<'_> {
    /// Clone the private key to a `'static` value
    #[cfg(feature = "alloc")]
    pub fn clone_key(&self) -> PrivatePkcs8KeyDer<'static> {
        PrivatePkcs8KeyDer::from(self.0.as_ref().to_vec())
    }

    /// Yield the DER-encoded bytes of the private key
    pub fn secret_pkcs8_der(&self) -> &[u8] {
        self.0.as_ref()
    }
}

#[cfg(feature = "alloc")]
impl zeroize::Zeroize for PrivatePkcs8KeyDer<'static> {
    fn zeroize(&mut self) {
        self.0.0.zeroize()
    }
}

#[cfg(feature = "alloc")]
impl PemObjectFilter for PrivatePkcs8KeyDer<'static> {
    const KIND: SectionKind = SectionKind::PrivateKey;
}

impl<'a> From<&'a [u8]> for PrivatePkcs8KeyDer<'a> {
    fn from(slice: &'a [u8]) -> Self {
        Self(Der(BytesInner::Borrowed(slice)))
    }
}

#[cfg(feature = "alloc")]
impl From<Vec<u8>> for PrivatePkcs8KeyDer<'_> {
    fn from(vec: Vec<u8>) -> Self {
        Self(Der(BytesInner::Owned(vec)))
    }
}

impl fmt::Debug for PrivatePkcs8KeyDer<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PrivatePkcs8KeyDer")
            .field(&"[secret key elided]")
            .finish()
    }
}

/// A trust anchor (a.k.a. root CA)
///
/// Traditionally, certificate verification libraries have represented trust anchors as full X.509
/// root certificates. However, those certificates contain a lot more data than is needed for
/// verifying certificates. The [`TrustAnchor`] representation allows an application to store
/// just the essential elements of trust anchors.
///
/// The most common way to get one of these is to call [`rustls_webpki::anchor_from_trusted_cert()`].
///
/// [`rustls_webpki::anchor_from_trusted_cert()`]: https://docs.rs/rustls-webpki/latest/webpki/fn.anchor_from_trusted_cert.html
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustAnchor<'a> {
    /// Value of the `subject` field of the trust anchor
    pub subject: Der<'a>,
    /// Value of the `subjectPublicKeyInfo` field of the trust anchor
    pub subject_public_key_info: Der<'a>,
    /// Value of DER-encoded `NameConstraints`, containing name constraints to the trust anchor, if any
    pub name_constraints: Option<Der<'a>>,
}

impl TrustAnchor<'_> {
    /// Yield a `'static` lifetime of the `TrustAnchor` by allocating owned `Der` variants
    #[cfg(feature = "alloc")]
    pub fn to_owned(&self) -> TrustAnchor<'static> {
        #[cfg(not(feature = "std"))]
        use alloc::borrow::ToOwned;
        TrustAnchor {
            subject: self.subject.as_ref().to_owned().into(),
            subject_public_key_info: self.subject_public_key_info.as_ref().to_owned().into(),
            name_constraints: self
                .name_constraints
                .as_ref()
                .map(|nc| nc.as_ref().to_owned().into()),
        }
    }
}

/// A Certificate Revocation List; as specified in RFC 5280
///
/// Certificate revocation lists are identified in PEM context as `X509 CRL` and when stored in a
/// file usually use a `.crl` extension. For more on PEM files, refer to the crate documentation.
///
/// ```rust
/// # #[cfg(all(feature = "alloc", feature = "std"))] {
/// use rustls_pki_types::{CertificateRevocationListDer, pem::PemObject};
///
/// // load several from a PEM file
/// let crls: Vec<_> = CertificateRevocationListDer::pem_file_iter("tests/data/crl.pem")
///     .unwrap()
///     .collect();
/// assert!(crls.len() >= 1);
///
/// // or one from a PEM byte slice...
/// # let byte_slice = include_bytes!("../tests/data/crl.pem");
/// CertificateRevocationListDer::from_pem_slice(byte_slice).unwrap();
///
/// // or several from a PEM byte slice
/// let crls: Vec<_> = CertificateRevocationListDer::pem_slice_iter(byte_slice)
///     .collect();
/// assert!(crls.len() >= 1);
/// # }
/// ```

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertificateRevocationListDer<'a>(Der<'a>);

#[cfg(feature = "alloc")]
impl PemObjectFilter for CertificateRevocationListDer<'static> {
    const KIND: SectionKind = SectionKind::Crl;
}

impl AsRef<[u8]> for CertificateRevocationListDer<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Deref for CertificateRevocationListDer<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<'a> From<&'a [u8]> for CertificateRevocationListDer<'a> {
    fn from(slice: &'a [u8]) -> Self {
        Self(Der::from(slice))
    }
}

#[cfg(feature = "alloc")]
impl From<Vec<u8>> for CertificateRevocationListDer<'_> {
    fn from(vec: Vec<u8>) -> Self {
        Self(Der::from(vec))
    }
}

/// A Certificate Signing Request; as specified in RFC 2986
///
/// Certificate signing requests are identified in PEM context as `CERTIFICATE REQUEST` and when stored in a
/// file usually use a `.csr` extension. For more on PEM files, refer to the crate documentation.
///
/// ```rust
/// # #[cfg(all(feature = "alloc", feature = "std"))] {
/// use rustls_pki_types::{CertificateSigningRequestDer, pem::PemObject};
///
/// // load from a PEM file
/// CertificateSigningRequestDer::from_pem_file("tests/data/csr.pem").unwrap();
///
/// // or from a PEM byte slice...
/// # let byte_slice = include_bytes!("../tests/data/csr.pem");
/// CertificateSigningRequestDer::from_pem_slice(byte_slice).unwrap();
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertificateSigningRequestDer<'a>(Der<'a>);

#[cfg(feature = "alloc")]
impl PemObjectFilter for CertificateSigningRequestDer<'static> {
    const KIND: SectionKind = SectionKind::Csr;
}

impl AsRef<[u8]> for CertificateSigningRequestDer<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Deref for CertificateSigningRequestDer<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<'a> From<&'a [u8]> for CertificateSigningRequestDer<'a> {
    fn from(slice: &'a [u8]) -> Self {
        Self(Der::from(slice))
    }
}

#[cfg(feature = "alloc")]
impl From<Vec<u8>> for CertificateSigningRequestDer<'_> {
    fn from(vec: Vec<u8>) -> Self {
        Self(Der::from(vec))
    }
}

/// A DER-encoded X.509 certificate; as specified in RFC 5280
///
/// Certificates are identified in PEM context as `CERTIFICATE` and when stored in a
/// file usually use a `.pem`, `.cer` or `.crt` extension. For more on PEM files, refer to the
/// crate documentation.
///
/// ```rust
/// # #[cfg(all(feature = "alloc", feature = "std"))] {
/// use rustls_pki_types::{CertificateDer, pem::PemObject};
///
/// // load several from a PEM file
/// let certs: Vec<_> = CertificateDer::pem_file_iter("tests/data/certificate.chain.pem")
///     .unwrap()
///     .collect();
/// assert_eq!(certs.len(), 3);
///
/// // or one from a PEM byte slice...
/// # let byte_slice = include_bytes!("../tests/data/certificate.chain.pem");
/// CertificateDer::from_pem_slice(byte_slice).unwrap();
///
/// // or several from a PEM byte slice
/// let certs: Vec<_> = CertificateDer::pem_slice_iter(byte_slice)
///     .collect();
/// assert_eq!(certs.len(), 3);
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CertificateDer<'a>(Der<'a>);

impl<'a> CertificateDer<'a> {
    /// A const constructor to create a `CertificateDer` from a slice of DER.
    pub const fn from_slice(bytes: &'a [u8]) -> Self {
        Self(Der::from_slice(bytes))
    }
}

#[cfg(feature = "alloc")]
impl PemObjectFilter for CertificateDer<'static> {
    const KIND: SectionKind = SectionKind::Certificate;
}

impl AsRef<[u8]> for CertificateDer<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Deref for CertificateDer<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<'a> From<&'a [u8]> for CertificateDer<'a> {
    fn from(slice: &'a [u8]) -> Self {
        Self(Der::from(slice))
    }
}

#[cfg(feature = "alloc")]
impl From<Vec<u8>> for CertificateDer<'_> {
    fn from(vec: Vec<u8>) -> Self {
        Self(Der::from(vec))
    }
}

impl CertificateDer<'_> {
    /// Converts this certificate into its owned variant, unfreezing borrowed content (if any)
    #[cfg(feature = "alloc")]
    pub fn into_owned(self) -> CertificateDer<'static> {
        CertificateDer(Der(self.0.0.into_owned()))
    }
}

/// A DER-encoded SubjectPublicKeyInfo (SPKI), as specified in RFC 5280.
#[deprecated(since = "1.7.0", note = "Prefer `SubjectPublicKeyInfoDer` instead")]
pub type SubjectPublicKeyInfo<'a> = SubjectPublicKeyInfoDer<'a>;

/// A DER-encoded SubjectPublicKeyInfo (SPKI), as specified in RFC 5280.
///
/// Public keys are identified in PEM context as a `PUBLIC KEY`.
///
/// ```rust
/// # #[cfg(all(feature = "alloc", feature = "std"))] {
/// use rustls_pki_types::{SubjectPublicKeyInfoDer, pem::PemObject};
///
/// // load from a PEM file
/// SubjectPublicKeyInfoDer::from_pem_file("tests/data/spki.pem").unwrap();
///
/// // or from a PEM byte slice...
/// # let byte_slice = include_bytes!("../tests/data/spki.pem");
/// SubjectPublicKeyInfoDer::from_pem_slice(byte_slice).unwrap();
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubjectPublicKeyInfoDer<'a>(Der<'a>);

#[cfg(feature = "alloc")]
impl PemObjectFilter for SubjectPublicKeyInfoDer<'static> {
    const KIND: SectionKind = SectionKind::PublicKey;
}

impl AsRef<[u8]> for SubjectPublicKeyInfoDer<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Deref for SubjectPublicKeyInfoDer<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<'a> From<&'a [u8]> for SubjectPublicKeyInfoDer<'a> {
    fn from(slice: &'a [u8]) -> Self {
        Self(Der::from(slice))
    }
}

#[cfg(feature = "alloc")]
impl From<Vec<u8>> for SubjectPublicKeyInfoDer<'_> {
    fn from(vec: Vec<u8>) -> Self {
        Self(Der::from(vec))
    }
}

impl SubjectPublicKeyInfoDer<'_> {
    /// Converts this SubjectPublicKeyInfo into its owned variant, unfreezing borrowed content (if any)
    #[cfg(feature = "alloc")]
    pub fn into_owned(self) -> SubjectPublicKeyInfoDer<'static> {
        SubjectPublicKeyInfoDer(Der(self.0.0.into_owned()))
    }
}

/// A TLS-encoded Encrypted Client Hello (ECH) configuration list (`ECHConfigList`); as specified in
/// [draft-ietf-tls-esni-18 §4](https://datatracker.ietf.org/doc/html/draft-ietf-tls-esni-18#section-4)
#[derive(Clone, Eq, PartialEq)]
pub struct EchConfigListBytes<'a>(BytesInner<'a>);

impl EchConfigListBytes<'_> {
    /// Converts this config into its owned variant, unfreezing borrowed content (if any)
    #[cfg(feature = "alloc")]
    pub fn into_owned(self) -> EchConfigListBytes<'static> {
        EchConfigListBytes(self.0.into_owned())
    }
}

#[cfg(feature = "alloc")]
impl EchConfigListBytes<'static> {
    /// Convert an iterator over PEM items into an `EchConfigListBytes` and private key.
    ///
    /// This handles the "ECHConfig file" format specified in
    /// <https://www.ietf.org/archive/id/draft-farrell-tls-pemesni-05.html#name-echconfig-file>
    ///
    /// Use it like:
    ///
    /// ```rust
    /// # #[cfg(all(feature = "alloc", feature = "std"))] {
    /// # use rustls_pki_types::{EchConfigListBytes, pem::PemObject};
    /// let (config, key) = EchConfigListBytes::config_and_key_from_iter(
    ///     PemObject::pem_file_iter("tests/data/ech.pem").unwrap()
    /// ).unwrap();
    /// # }
    /// ```
    pub fn config_and_key_from_iter(
        iter: impl Iterator<Item = Result<(SectionKind, Vec<u8>), pem::Error>>,
    ) -> Result<(Self, PrivatePkcs8KeyDer<'static>), pem::Error> {
        let mut key = None;
        let mut config = None;

        for item in iter {
            let (kind, data) = item?;
            match kind {
                SectionKind::PrivateKey => {
                    key = PrivatePkcs8KeyDer::from_pem(kind, data);
                }
                SectionKind::EchConfigList => {
                    config = Self::from_pem(kind, data);
                }
                _ => continue,
            };

            if let (Some(_key), Some(_config)) = (&key, &config) {
                return Ok((config.take().unwrap(), key.take().unwrap()));
            }
        }

        Err(pem::Error::NoItemsFound)
    }
}

#[cfg(feature = "alloc")]
impl PemObjectFilter for EchConfigListBytes<'static> {
    const KIND: SectionKind = SectionKind::EchConfigList;
}

impl fmt::Debug for EchConfigListBytes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        hex(f, self.as_ref())
    }
}

impl AsRef<[u8]> for EchConfigListBytes<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Deref for EchConfigListBytes<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<'a> From<&'a [u8]> for EchConfigListBytes<'a> {
    fn from(slice: &'a [u8]) -> Self {
        Self(BytesInner::Borrowed(slice))
    }
}

#[cfg(feature = "alloc")]
impl From<Vec<u8>> for EchConfigListBytes<'_> {
    fn from(vec: Vec<u8>) -> Self {
        Self(BytesInner::Owned(vec))
    }
}

/// An abstract signature verification algorithm.
///
/// One of these is needed per supported pair of public key type (identified
/// with `public_key_alg_id()`) and `signatureAlgorithm` (identified with
/// `signature_alg_id()`).  Note that both of these `AlgorithmIdentifier`s include
/// the parameters encoding, so separate `SignatureVerificationAlgorithm`s are needed
/// for each possible public key or signature parameters.
///
/// Debug implementations should list the public key algorithm identifier and
/// signature algorithm identifier in human friendly form (i.e. not encoded bytes),
/// along with the name of the implementing library (to distinguish different
/// implementations of the same algorithms).
pub trait SignatureVerificationAlgorithm: Send + Sync + fmt::Debug {
    /// Verify a signature.
    ///
    /// `public_key` is the `subjectPublicKey` value from a `SubjectPublicKeyInfo` encoding
    /// and is untrusted.  The key's `subjectPublicKeyInfo` matches the [`AlgorithmIdentifier`]
    /// returned by `public_key_alg_id()`.
    ///
    /// `message` is the data over which the signature was allegedly computed.
    /// It is not hashed; implementations of this trait function must do hashing
    /// if that is required by the algorithm they implement.
    ///
    /// `signature` is the signature allegedly over `message`.
    ///
    /// Return `Ok(())` only if `signature` is a valid signature on `message`.
    ///
    /// Return `Err(InvalidSignature)` if the signature is invalid, including if the `public_key`
    /// encoding is invalid.  There is no need or opportunity to produce errors
    /// that are more specific than this.
    fn verify_signature(
        &self,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> Result<(), InvalidSignature>;

    /// Return the `AlgorithmIdentifier` that must equal a public key's
    /// `subjectPublicKeyInfo` value for this `SignatureVerificationAlgorithm`
    /// to be used for signature verification.
    fn public_key_alg_id(&self) -> AlgorithmIdentifier;

    /// Return the `AlgorithmIdentifier` that must equal the `signatureAlgorithm` value
    /// on the data to be verified for this `SignatureVerificationAlgorithm` to be used
    /// for signature verification.
    fn signature_alg_id(&self) -> AlgorithmIdentifier;

    /// Return `true` if this is backed by a FIPS-approved implementation.
    fn fips(&self) -> bool {
        false
    }
}

/// A detail-less error when a signature is not valid.
#[derive(Debug, Copy, Clone)]
pub struct InvalidSignature;

/// A timestamp, tracking the number of non-leap seconds since the Unix epoch.
///
/// The Unix epoch is defined January 1, 1970 00:00:00 UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnixTime(u64);

impl UnixTime {
    /// The current time, as a `UnixTime`
    #[cfg(any(
        all(
            feature = "std",
            not(all(target_family = "wasm", target_os = "unknown"))
        ),
        all(target_family = "wasm", target_os = "unknown", feature = "web")
    ))]
    pub fn now() -> Self {
        Self::since_unix_epoch(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap(), // Safe: this code did not exist before 1970.
        )
    }

    /// Convert a `Duration` since the start of 1970 to a `UnixTime`
    ///
    /// The `duration` must be relative to the Unix epoch.
    pub fn since_unix_epoch(duration: Duration) -> Self {
        Self(duration.as_secs())
    }

    /// Number of seconds since the Unix epoch
    pub fn as_secs(&self) -> u64 {
        self.0
    }
}

/// DER-encoded data, either owned or borrowed
///
/// This wrapper type is used to represent DER-encoded data in a way that is agnostic to whether
/// the data is owned (by a `Vec<u8>`) or borrowed (by a `&[u8]`). Support for the owned
/// variant is only available when the `alloc` feature is enabled.
#[derive(Clone, Eq, PartialEq)]
pub struct Der<'a>(BytesInner<'a>);

impl<'a> Der<'a> {
    /// A const constructor to create a `Der` from a borrowed slice
    pub const fn from_slice(der: &'a [u8]) -> Self {
        Self(BytesInner::Borrowed(der))
    }
}

impl AsRef<[u8]> for Der<'_> {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Deref for Der<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<'a> From<&'a [u8]> for Der<'a> {
    fn from(slice: &'a [u8]) -> Self {
        Self(BytesInner::Borrowed(slice))
    }
}

#[cfg(feature = "alloc")]
impl From<Vec<u8>> for Der<'static> {
    fn from(vec: Vec<u8>) -> Self {
        Self(BytesInner::Owned(vec))
    }
}

impl fmt::Debug for Der<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        hex(f, self.as_ref())
    }
}

#[derive(Debug, Clone)]
enum BytesInner<'a> {
    #[cfg(feature = "alloc")]
    Owned(Vec<u8>),
    Borrowed(&'a [u8]),
}

#[cfg(feature = "alloc")]
impl BytesInner<'_> {
    fn into_owned(self) -> BytesInner<'static> {
        BytesInner::Owned(match self {
            Self::Owned(vec) => vec,
            Self::Borrowed(slice) => slice.to_vec(),
        })
    }
}

#[cfg(feature = "alloc")]
impl zeroize::Zeroize for BytesInner<'static> {
    fn zeroize(&mut self) {
        match self {
            BytesInner::Owned(vec) => vec.zeroize(),
            BytesInner::Borrowed(_) => (),
        }
    }
}

impl AsRef<[u8]> for BytesInner<'_> {
    fn as_ref(&self) -> &[u8] {
        match &self {
            #[cfg(feature = "alloc")]
            BytesInner::Owned(vec) => vec.as_ref(),
            BytesInner::Borrowed(slice) => slice,
        }
    }
}

impl PartialEq for BytesInner<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ref() == other.as_ref()
    }
}

impl Eq for BytesInner<'_> {}

// Format an iterator of u8 into a hex string
fn hex<'a>(f: &mut fmt::Formatter<'_>, payload: impl IntoIterator<Item = &'a u8>) -> fmt::Result {
    for (i, b) in payload.into_iter().enumerate() {
        if i == 0 {
            write!(f, "0x")?;
        }
        write!(f, "{:02x}", b)?;
    }
    Ok(())
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn der_debug() {
        let der = Der::from_slice(&[0x01, 0x02, 0x03]);
        assert_eq!(format!("{:?}", der), "0x010203");
    }

    #[test]
    fn alg_id_debug() {
        let alg_id = AlgorithmIdentifier::from_slice(&[0x01, 0x02, 0x03]);
        assert_eq!(format!("{:?}", alg_id), "0x010203");
    }

    #[test]
    fn bytes_inner_equality() {
        let owned_a = BytesInner::Owned(vec![1, 2, 3]);
        let owned_b = BytesInner::Owned(vec![4, 5]);
        let borrowed_a = BytesInner::Borrowed(&[1, 2, 3]);
        let borrowed_b = BytesInner::Borrowed(&[99]);

        // Self-equality.
        assert_eq!(owned_a, owned_a);
        assert_eq!(owned_b, owned_b);
        assert_eq!(borrowed_a, borrowed_a);
        assert_eq!(borrowed_b, borrowed_b);

        // Borrowed vs Owned equality
        assert_eq!(owned_a, borrowed_a);
        assert_eq!(borrowed_a, owned_a);

        // Owned inequality
        assert_ne!(owned_a, owned_b);
        assert_ne!(owned_b, owned_a);

        // Borrowed inequality
        assert_ne!(borrowed_a, borrowed_b);
        assert_ne!(borrowed_b, borrowed_a);

        // Borrowed vs Owned inequality
        assert_ne!(owned_a, borrowed_b);
        assert_ne!(borrowed_b, owned_a);
    }
}
