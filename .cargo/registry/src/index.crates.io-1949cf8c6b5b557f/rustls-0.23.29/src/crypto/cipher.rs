use alloc::boxed::Box;
use alloc::string::ToString;
use core::fmt;

use zeroize::Zeroize;

use crate::enums::{ContentType, ProtocolVersion};
use crate::error::Error;
use crate::msgs::codec;
pub use crate::msgs::message::{
    BorrowedPayload, InboundOpaqueMessage, InboundPlainMessage, OutboundChunks,
    OutboundOpaqueMessage, OutboundPlainMessage, PlainMessage, PrefixedPayload,
};
use crate::suites::ConnectionTrafficSecrets;

/// Factory trait for building `MessageEncrypter` and `MessageDecrypter` for a TLS1.3 cipher suite.
pub trait Tls13AeadAlgorithm: Send + Sync {
    /// Build a `MessageEncrypter` for the given key/iv.
    fn encrypter(&self, key: AeadKey, iv: Iv) -> Box<dyn MessageEncrypter>;

    /// Build a `MessageDecrypter` for the given key/iv.
    fn decrypter(&self, key: AeadKey, iv: Iv) -> Box<dyn MessageDecrypter>;

    /// The length of key in bytes required by `encrypter()` and `decrypter()`.
    fn key_len(&self) -> usize;

    /// Convert the key material from `key`/`iv`, into a `ConnectionTrafficSecrets` item.
    ///
    /// May return [`UnsupportedOperationError`] if the AEAD algorithm is not a supported
    /// variant of `ConnectionTrafficSecrets`.
    fn extract_keys(
        &self,
        key: AeadKey,
        iv: Iv,
    ) -> Result<ConnectionTrafficSecrets, UnsupportedOperationError>;

    /// Return `true` if this is backed by a FIPS-approved implementation.
    fn fips(&self) -> bool {
        false
    }
}

/// Factory trait for building `MessageEncrypter` and `MessageDecrypter` for a TLS1.2 cipher suite.
pub trait Tls12AeadAlgorithm: Send + Sync + 'static {
    /// Build a `MessageEncrypter` for the given key/iv and extra key block (which can be used for
    /// improving explicit nonce size security, if needed).
    ///
    /// The length of `key` is set by [`KeyBlockShape::enc_key_len`].
    ///
    /// The length of `iv` is set by [`KeyBlockShape::fixed_iv_len`].
    ///
    /// The length of `extra` is set by [`KeyBlockShape::explicit_nonce_len`].
    fn encrypter(&self, key: AeadKey, iv: &[u8], extra: &[u8]) -> Box<dyn MessageEncrypter>;

    /// Build a `MessageDecrypter` for the given key/iv.
    ///
    /// The length of `key` is set by [`KeyBlockShape::enc_key_len`].
    ///
    /// The length of `iv` is set by [`KeyBlockShape::fixed_iv_len`].
    fn decrypter(&self, key: AeadKey, iv: &[u8]) -> Box<dyn MessageDecrypter>;

    /// Return a `KeyBlockShape` that defines how large the `key_block` is and how it
    /// is split up prior to calling `encrypter()`, `decrypter()` and/or `extract_keys()`.
    fn key_block_shape(&self) -> KeyBlockShape;

    /// Convert the key material from `key`/`iv`, into a `ConnectionTrafficSecrets` item.
    ///
    /// The length of `key` is set by [`KeyBlockShape::enc_key_len`].
    ///
    /// The length of `iv` is set by [`KeyBlockShape::fixed_iv_len`].
    ///
    /// The length of `extra` is set by [`KeyBlockShape::explicit_nonce_len`].
    ///
    /// May return [`UnsupportedOperationError`] if the AEAD algorithm is not a supported
    /// variant of `ConnectionTrafficSecrets`.
    fn extract_keys(
        &self,
        key: AeadKey,
        iv: &[u8],
        explicit: &[u8],
    ) -> Result<ConnectionTrafficSecrets, UnsupportedOperationError>;

    /// Return `true` if this is backed by a FIPS-approved implementation.
    fn fips(&self) -> bool {
        false
    }
}

/// An error indicating that the AEAD algorithm does not support the requested operation.
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct UnsupportedOperationError;

impl From<UnsupportedOperationError> for Error {
    fn from(value: UnsupportedOperationError) -> Self {
        Self::General(value.to_string())
    }
}

impl fmt::Display for UnsupportedOperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "operation not supported")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for UnsupportedOperationError {}

/// How a TLS1.2 `key_block` is partitioned.
///
/// Note: ciphersuites with non-zero `mac_key_length` are  not currently supported.
pub struct KeyBlockShape {
    /// How long keys are.
    ///
    /// `enc_key_length` terminology is from the standard ([RFC5246 A.6]).
    ///
    /// [RFC5246 A.6]: <https://www.rfc-editor.org/rfc/rfc5246#appendix-A.6>
    pub enc_key_len: usize,

    /// How long the fixed part of the 'IV' is.
    ///
    /// `fixed_iv_length` terminology is from the standard ([RFC5246 A.6]).
    ///
    /// This isn't usually an IV, but we continue the
    /// terminology misuse to match the standard.
    ///
    /// [RFC5246 A.6]: <https://www.rfc-editor.org/rfc/rfc5246#appendix-A.6>
    pub fixed_iv_len: usize,

    /// This is a non-standard extension which extends the
    /// key block to provide an initial explicit nonce offset,
    /// in a deterministic and safe way.  GCM needs this,
    /// chacha20poly1305 works this way by design.
    pub explicit_nonce_len: usize,
}

/// Objects with this trait can decrypt TLS messages.
pub trait MessageDecrypter: Send + Sync {
    /// Decrypt the given TLS message `msg`, using the sequence number
    /// `seq` which can be used to derive a unique [`Nonce`].
    fn decrypt<'a>(
        &mut self,
        msg: InboundOpaqueMessage<'a>,
        seq: u64,
    ) -> Result<InboundPlainMessage<'a>, Error>;
}

/// Objects with this trait can encrypt TLS messages.
pub trait MessageEncrypter: Send + Sync {
    /// Encrypt the given TLS message `msg`, using the sequence number
    /// `seq` which can be used to derive a unique [`Nonce`].
    fn encrypt(
        &mut self,
        msg: OutboundPlainMessage<'_>,
        seq: u64,
    ) -> Result<OutboundOpaqueMessage, Error>;

    /// Return the length of the ciphertext that results from encrypting plaintext of
    /// length `payload_len`
    fn encrypted_payload_len(&self, payload_len: usize) -> usize;
}

impl dyn MessageEncrypter {
    pub(crate) fn invalid() -> Box<dyn MessageEncrypter> {
        Box::new(InvalidMessageEncrypter {})
    }
}

impl dyn MessageDecrypter {
    pub(crate) fn invalid() -> Box<dyn MessageDecrypter> {
        Box::new(InvalidMessageDecrypter {})
    }
}

/// A write or read IV.
#[derive(Default)]
pub struct Iv([u8; NONCE_LEN]);

impl Iv {
    /// Create a new `Iv` from a byte array, of precisely `NONCE_LEN` bytes.
    #[cfg(feature = "tls12")]
    pub fn new(value: [u8; NONCE_LEN]) -> Self {
        Self(value)
    }

    /// Create a new `Iv` from a byte slice, of precisely `NONCE_LEN` bytes.
    #[cfg(feature = "tls12")]
    pub fn copy(value: &[u8]) -> Self {
        debug_assert_eq!(value.len(), NONCE_LEN);
        let mut iv = Self::new(Default::default());
        iv.0.copy_from_slice(value);
        iv
    }
}

impl From<[u8; NONCE_LEN]> for Iv {
    fn from(bytes: [u8; NONCE_LEN]) -> Self {
        Self(bytes)
    }
}

impl AsRef<[u8]> for Iv {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

/// A nonce.  This is unique for all messages on a connection.
pub struct Nonce(pub [u8; NONCE_LEN]);

impl Nonce {
    /// Combine an `Iv` and sequence number to produce a unique nonce.
    ///
    /// This is `iv ^ seq` where `seq` is encoded as a 96-bit big-endian integer.
    #[inline]
    pub fn new(iv: &Iv, seq: u64) -> Self {
        let mut nonce = Self([0u8; NONCE_LEN]);
        codec::put_u64(seq, &mut nonce.0[4..]);

        nonce
            .0
            .iter_mut()
            .zip(iv.0.iter())
            .for_each(|(nonce, iv)| {
                *nonce ^= *iv;
            });

        nonce
    }
}

/// Size of TLS nonces (incorrectly termed "IV" in standard) for all supported ciphersuites
/// (AES-GCM, Chacha20Poly1305)
pub const NONCE_LEN: usize = 12;

/// Returns a TLS1.3 `additional_data` encoding.
///
/// See RFC8446 s5.2 for the `additional_data` definition.
#[inline]
pub fn make_tls13_aad(payload_len: usize) -> [u8; 5] {
    let version = ProtocolVersion::TLSv1_2.to_array();
    [
        ContentType::ApplicationData.into(),
        // Note: this is `legacy_record_version`, i.e. TLS1.2 even for TLS1.3.
        version[0],
        version[1],
        (payload_len >> 8) as u8,
        (payload_len & 0xff) as u8,
    ]
}

/// Returns a TLS1.2 `additional_data` encoding.
///
/// See RFC5246 s6.2.3.3 for the `additional_data` definition.
#[inline]
pub fn make_tls12_aad(
    seq: u64,
    typ: ContentType,
    vers: ProtocolVersion,
    len: usize,
) -> [u8; TLS12_AAD_SIZE] {
    let mut out = [0; TLS12_AAD_SIZE];
    codec::put_u64(seq, &mut out[0..]);
    out[8] = typ.into();
    codec::put_u16(vers.into(), &mut out[9..]);
    codec::put_u16(len as u16, &mut out[11..]);
    out
}

const TLS12_AAD_SIZE: usize = 8 + 1 + 2 + 2;

/// A key for an AEAD algorithm.
///
/// This is a value type for a byte string up to `AeadKey::MAX_LEN` bytes in length.
pub struct AeadKey {
    buf: [u8; Self::MAX_LEN],
    used: usize,
}

impl AeadKey {
    #[cfg(feature = "tls12")]
    pub(crate) fn new(buf: &[u8]) -> Self {
        debug_assert!(buf.len() <= Self::MAX_LEN);
        let mut key = Self::from([0u8; Self::MAX_LEN]);
        key.buf[..buf.len()].copy_from_slice(buf);
        key.used = buf.len();
        key
    }

    pub(crate) fn with_length(self, len: usize) -> Self {
        assert!(len <= self.used);
        Self {
            buf: self.buf,
            used: len,
        }
    }

    /// Largest possible AEAD key in the ciphersuites we support.
    pub(crate) const MAX_LEN: usize = 32;
}

impl Drop for AeadKey {
    fn drop(&mut self) {
        self.buf.zeroize();
    }
}

impl AsRef<[u8]> for AeadKey {
    fn as_ref(&self) -> &[u8] {
        &self.buf[..self.used]
    }
}

impl From<[u8; Self::MAX_LEN]> for AeadKey {
    fn from(bytes: [u8; Self::MAX_LEN]) -> Self {
        Self {
            buf: bytes,
            used: Self::MAX_LEN,
        }
    }
}

/// A `MessageEncrypter` which doesn't work.
struct InvalidMessageEncrypter {}

impl MessageEncrypter for InvalidMessageEncrypter {
    fn encrypt(
        &mut self,
        _m: OutboundPlainMessage<'_>,
        _seq: u64,
    ) -> Result<OutboundOpaqueMessage, Error> {
        Err(Error::EncryptError)
    }

    fn encrypted_payload_len(&self, payload_len: usize) -> usize {
        payload_len
    }
}

/// A `MessageDecrypter` which doesn't work.
struct InvalidMessageDecrypter {}

impl MessageDecrypter for InvalidMessageDecrypter {
    fn decrypt<'a>(
        &mut self,
        _m: InboundOpaqueMessage<'a>,
        _seq: u64,
    ) -> Result<InboundPlainMessage<'a>, Error> {
        Err(Error::DecryptError)
    }
}
