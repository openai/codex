use alloc::boxed::Box;
use alloc::vec::Vec;

use zeroize::Zeroize;

use super::{ActiveKeyExchange, hmac};
use crate::error::Error;
use crate::version::TLS13;

/// Implementation of `HkdfExpander` via `hmac::Key`.
pub struct HkdfExpanderUsingHmac(Box<dyn hmac::Key>);

impl HkdfExpanderUsingHmac {
    fn expand_unchecked(&self, info: &[&[u8]], output: &mut [u8]) {
        let mut term = hmac::Tag::new(b"");

        for (n, chunk) in output
            .chunks_mut(self.0.tag_len())
            .enumerate()
        {
            term = self
                .0
                .sign_concat(term.as_ref(), info, &[(n + 1) as u8]);
            chunk.copy_from_slice(&term.as_ref()[..chunk.len()]);
        }
    }
}

impl HkdfExpander for HkdfExpanderUsingHmac {
    fn expand_slice(&self, info: &[&[u8]], output: &mut [u8]) -> Result<(), OutputLengthError> {
        if output.len() > 255 * self.0.tag_len() {
            return Err(OutputLengthError);
        }

        self.expand_unchecked(info, output);
        Ok(())
    }

    fn expand_block(&self, info: &[&[u8]]) -> OkmBlock {
        let mut tag = [0u8; hmac::Tag::MAX_LEN];
        let reduced_tag = &mut tag[..self.0.tag_len()];
        self.expand_unchecked(info, reduced_tag);
        OkmBlock::new(reduced_tag)
    }

    fn hash_len(&self) -> usize {
        self.0.tag_len()
    }
}

/// Implementation of `Hkdf` (and thence `HkdfExpander`) via `hmac::Hmac`.
pub struct HkdfUsingHmac<'a>(pub &'a dyn hmac::Hmac);

impl Hkdf for HkdfUsingHmac<'_> {
    fn extract_from_zero_ikm(&self, salt: Option<&[u8]>) -> Box<dyn HkdfExpander> {
        let zeroes = [0u8; hmac::Tag::MAX_LEN];
        Box::new(HkdfExpanderUsingHmac(self.0.with_key(
            &self.extract_prk_from_secret(salt, &zeroes[..self.0.hash_output_len()]),
        )))
    }

    fn extract_from_secret(&self, salt: Option<&[u8]>, secret: &[u8]) -> Box<dyn HkdfExpander> {
        Box::new(HkdfExpanderUsingHmac(
            self.0
                .with_key(&self.extract_prk_from_secret(salt, secret)),
        ))
    }

    fn expander_for_okm(&self, okm: &OkmBlock) -> Box<dyn HkdfExpander> {
        Box::new(HkdfExpanderUsingHmac(self.0.with_key(okm.as_ref())))
    }

    fn hmac_sign(&self, key: &OkmBlock, message: &[u8]) -> hmac::Tag {
        self.0
            .with_key(key.as_ref())
            .sign(&[message])
    }
}

impl HkdfPrkExtract for HkdfUsingHmac<'_> {
    fn extract_prk_from_secret(&self, salt: Option<&[u8]>, secret: &[u8]) -> Vec<u8> {
        let zeroes = [0u8; hmac::Tag::MAX_LEN];
        let salt = match salt {
            Some(salt) => salt,
            None => &zeroes[..self.0.hash_output_len()],
        };
        self.0
            .with_key(salt)
            .sign(&[secret])
            .as_ref()
            .to_vec()
    }
}

/// Implementation of `HKDF-Expand` with an implicitly stored and immutable `PRK`.
pub trait HkdfExpander: Send + Sync {
    /// `HKDF-Expand(PRK, info, L)` into a slice.
    ///
    /// Where:
    ///
    /// - `PRK` is the implicit key material represented by this instance.
    /// - `L` is `output.len()`.
    /// - `info` is a slice of byte slices, which should be processed sequentially
    ///   (or concatenated if that is not possible).
    ///
    /// Returns `Err(OutputLengthError)` if `L` is larger than `255 * HashLen`.
    /// Otherwise, writes to `output`.
    fn expand_slice(&self, info: &[&[u8]], output: &mut [u8]) -> Result<(), OutputLengthError>;

    /// `HKDF-Expand(PRK, info, L=HashLen)` returned as a value.
    ///
    /// - `PRK` is the implicit key material represented by this instance.
    /// - `L := HashLen`.
    /// - `info` is a slice of byte slices, which should be processed sequentially
    ///   (or concatenated if that is not possible).
    ///
    /// This is infallible, because by definition `OkmBlock` is always exactly
    /// `HashLen` bytes long.
    fn expand_block(&self, info: &[&[u8]]) -> OkmBlock;

    /// Return what `HashLen` is for this instance.
    ///
    /// This must be no larger than [`OkmBlock::MAX_LEN`].
    fn hash_len(&self) -> usize;
}

/// A HKDF implementation oriented to the needs of TLS1.3.
///
/// See [RFC5869](https://datatracker.ietf.org/doc/html/rfc5869) for the terminology
/// used in this definition.
///
/// You can use [`HkdfUsingHmac`] which implements this trait on top of an implementation
/// of [`hmac::Hmac`].
pub trait Hkdf: Send + Sync {
    /// `HKDF-Extract(salt, 0_HashLen)`
    ///
    /// `0_HashLen` is a string of `HashLen` zero bytes.
    ///
    /// A `salt` of `None` should be treated as a sequence of `HashLen` zero bytes.
    fn extract_from_zero_ikm(&self, salt: Option<&[u8]>) -> Box<dyn HkdfExpander>;

    /// `HKDF-Extract(salt, secret)`
    ///
    /// A `salt` of `None` should be treated as a sequence of `HashLen` zero bytes.
    fn extract_from_secret(&self, salt: Option<&[u8]>, secret: &[u8]) -> Box<dyn HkdfExpander>;

    /// `HKDF-Extract(salt, shared_secret)` where `shared_secret` is the result of a key exchange.
    ///
    /// Custom implementations should complete the key exchange by calling
    /// `kx.complete(peer_pub_key)` and then using this as the input keying material to
    /// `HKDF-Extract`.
    ///
    /// A `salt` of `None` should be treated as a sequence of `HashLen` zero bytes.
    fn extract_from_kx_shared_secret(
        &self,
        salt: Option<&[u8]>,
        kx: Box<dyn ActiveKeyExchange>,
        peer_pub_key: &[u8],
    ) -> Result<Box<dyn HkdfExpander>, Error> {
        Ok(self.extract_from_secret(
            salt,
            kx.complete_for_tls_version(peer_pub_key, &TLS13)?
                .secret_bytes(),
        ))
    }

    /// Build a `HkdfExpander` using `okm` as the secret PRK.
    fn expander_for_okm(&self, okm: &OkmBlock) -> Box<dyn HkdfExpander>;

    /// Signs `message` using `key` viewed as a HMAC key.
    ///
    /// This should use the same hash function as the HKDF functions in this
    /// trait.
    ///
    /// See [RFC2104](https://datatracker.ietf.org/doc/html/rfc2104) for the
    /// definition of HMAC.
    fn hmac_sign(&self, key: &OkmBlock, message: &[u8]) -> hmac::Tag;

    /// Return `true` if this is backed by a FIPS-approved implementation.
    fn fips(&self) -> bool {
        false
    }
}

/// An extended HKDF implementation that supports directly extracting a pseudo-random key (PRK).
///
/// The base [`Hkdf`] trait is tailored to the needs of TLS 1.3, where all extracted PRKs
/// are expanded as-is, and so can be safely encapsulated without exposing the caller
/// to the key material.
///
/// In other contexts (for example, hybrid public key encryption (HPKE)) it may be necessary
/// to use the extracted PRK directly for purposes other than an immediate expansion.
/// This trait can be implemented to offer this functionality when it is required.
pub(crate) trait HkdfPrkExtract: Hkdf {
    /// `HKDF-Extract(salt, secret)`
    ///
    /// A `salt` of `None` should be treated as a sequence of `HashLen` zero bytes.
    ///
    /// In most cases you should prefer [`Hkdf::extract_from_secret`] and using the
    /// returned [HkdfExpander] instead of handling the PRK directly.
    fn extract_prk_from_secret(&self, salt: Option<&[u8]>, secret: &[u8]) -> Vec<u8>;
}

/// `HKDF-Expand(PRK, info, L)` to construct any type from a byte array.
///
/// - `PRK` is the implicit key material represented by this instance.
/// - `L := N`; N is the size of the byte array.
/// - `info` is a slice of byte slices, which should be processed sequentially
///   (or concatenated if that is not possible).
///
/// This is infallible, because the set of types (and therefore their length) is known
/// at compile time.
pub fn expand<T, const N: usize>(expander: &dyn HkdfExpander, info: &[&[u8]]) -> T
where
    T: From<[u8; N]>,
{
    let mut output = [0u8; N];
    expander
        .expand_slice(info, &mut output)
        .expect("expand type parameter T is too large");
    T::from(output)
}

/// Output key material from HKDF, as a value type.
#[derive(Clone)]
pub struct OkmBlock {
    buf: [u8; Self::MAX_LEN],
    used: usize,
}

impl OkmBlock {
    /// Build a single OKM block by copying a byte slice.
    ///
    /// The slice can be up to [`OkmBlock::MAX_LEN`] bytes in length.
    pub fn new(bytes: &[u8]) -> Self {
        let mut tag = Self {
            buf: [0u8; Self::MAX_LEN],
            used: bytes.len(),
        };
        tag.buf[..bytes.len()].copy_from_slice(bytes);
        tag
    }

    /// Maximum supported HMAC tag size: supports up to SHA512.
    pub const MAX_LEN: usize = 64;
}

impl Drop for OkmBlock {
    fn drop(&mut self) {
        self.buf.zeroize();
    }
}

impl AsRef<[u8]> for OkmBlock {
    fn as_ref(&self) -> &[u8] {
        &self.buf[..self.used]
    }
}

/// An error type used for `HkdfExpander::expand_slice` when
/// the slice exceeds the maximum HKDF output length.
#[derive(Debug)]
pub struct OutputLengthError;

#[cfg(all(test, feature = "ring"))]
mod tests {
    use std::prelude::v1::*;

    use super::{Hkdf, HkdfUsingHmac, expand};
    // nb: crypto::aws_lc_rs provider doesn't provide (or need) hmac,
    // so cannot be used for this test.
    use crate::crypto::ring::hmac;

    struct ByteArray<const N: usize>([u8; N]);

    impl<const N: usize> From<[u8; N]> for ByteArray<N> {
        fn from(array: [u8; N]) -> Self {
            Self(array)
        }
    }

    /// Test cases from appendix A in the RFC, minus cases requiring SHA1.

    #[test]
    fn test_case_1() {
        let hkdf = HkdfUsingHmac(&hmac::HMAC_SHA256);
        let ikm = &[0x0b; 22];
        let salt = &[
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
        ];
        let info: &[&[u8]] = &[
            &[0xf0, 0xf1, 0xf2],
            &[0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9],
        ];

        let output: ByteArray<42> = expand(
            hkdf.extract_from_secret(Some(salt), ikm)
                .as_ref(),
            info,
        );

        assert_eq!(
            &output.0,
            &[
                0x3c, 0xb2, 0x5f, 0x25, 0xfa, 0xac, 0xd5, 0x7a, 0x90, 0x43, 0x4f, 0x64, 0xd0, 0x36,
                0x2f, 0x2a, 0x2d, 0x2d, 0x0a, 0x90, 0xcf, 0x1a, 0x5a, 0x4c, 0x5d, 0xb0, 0x2d, 0x56,
                0xec, 0xc4, 0xc5, 0xbf, 0x34, 0x00, 0x72, 0x08, 0xd5, 0xb8, 0x87, 0x18, 0x58, 0x65
            ]
        );
    }

    #[test]
    fn test_case_2() {
        let hkdf = HkdfUsingHmac(&hmac::HMAC_SHA256);
        let ikm: Vec<u8> = (0x00u8..=0x4f).collect();
        let salt: Vec<u8> = (0x60u8..=0xaf).collect();
        let info: Vec<u8> = (0xb0u8..=0xff).collect();

        let output: ByteArray<82> = expand(
            hkdf.extract_from_secret(Some(&salt), &ikm)
                .as_ref(),
            &[&info],
        );

        assert_eq!(
            &output.0,
            &[
                0xb1, 0x1e, 0x39, 0x8d, 0xc8, 0x03, 0x27, 0xa1, 0xc8, 0xe7, 0xf7, 0x8c, 0x59, 0x6a,
                0x49, 0x34, 0x4f, 0x01, 0x2e, 0xda, 0x2d, 0x4e, 0xfa, 0xd8, 0xa0, 0x50, 0xcc, 0x4c,
                0x19, 0xaf, 0xa9, 0x7c, 0x59, 0x04, 0x5a, 0x99, 0xca, 0xc7, 0x82, 0x72, 0x71, 0xcb,
                0x41, 0xc6, 0x5e, 0x59, 0x0e, 0x09, 0xda, 0x32, 0x75, 0x60, 0x0c, 0x2f, 0x09, 0xb8,
                0x36, 0x77, 0x93, 0xa9, 0xac, 0xa3, 0xdb, 0x71, 0xcc, 0x30, 0xc5, 0x81, 0x79, 0xec,
                0x3e, 0x87, 0xc1, 0x4c, 0x01, 0xd5, 0xc1, 0xf3, 0x43, 0x4f, 0x1d, 0x87
            ]
        );
    }

    #[test]
    fn test_case_3() {
        let hkdf = HkdfUsingHmac(&hmac::HMAC_SHA256);
        let ikm = &[0x0b; 22];
        let salt = &[];
        let info = &[];

        let output: ByteArray<42> = expand(
            hkdf.extract_from_secret(Some(salt), ikm)
                .as_ref(),
            info,
        );

        assert_eq!(
            &output.0,
            &[
                0x8d, 0xa4, 0xe7, 0x75, 0xa5, 0x63, 0xc1, 0x8f, 0x71, 0x5f, 0x80, 0x2a, 0x06, 0x3c,
                0x5a, 0x31, 0xb8, 0xa1, 0x1f, 0x5c, 0x5e, 0xe1, 0x87, 0x9e, 0xc3, 0x45, 0x4e, 0x5f,
                0x3c, 0x73, 0x8d, 0x2d, 0x9d, 0x20, 0x13, 0x95, 0xfa, 0xa4, 0xb6, 0x1a, 0x96, 0xc8
            ]
        );
    }

    #[test]
    fn test_salt_not_provided() {
        // can't use test case 7, because we don't have (or want) SHA1.
        //
        // this output is generated with cryptography.io:
        //
        // >>> hkdf.HKDF(algorithm=hashes.SHA384(), length=96, salt=None, info=b"hello").derive(b"\x0b" * 40)

        let hkdf = HkdfUsingHmac(&hmac::HMAC_SHA384);
        let ikm = &[0x0b; 40];
        let info = &[&b"hel"[..], &b"lo"[..]];

        let output: ByteArray<96> = expand(
            hkdf.extract_from_secret(None, ikm)
                .as_ref(),
            info,
        );

        assert_eq!(
            &output.0,
            &[
                0xd5, 0x45, 0xdd, 0x3a, 0xff, 0x5b, 0x19, 0x46, 0xd4, 0x86, 0xfd, 0xb8, 0xd8, 0x88,
                0x2e, 0xe0, 0x1c, 0xc1, 0xa5, 0x48, 0xb6, 0x05, 0x75, 0xe4, 0xd7, 0x5d, 0x0f, 0x5f,
                0x23, 0x40, 0xee, 0x6c, 0x9e, 0x7c, 0x65, 0xd0, 0xee, 0x79, 0xdb, 0xb2, 0x07, 0x1d,
                0x66, 0xa5, 0x50, 0xc4, 0x8a, 0xa3, 0x93, 0x86, 0x8b, 0x7c, 0x69, 0x41, 0x6b, 0x3e,
                0x61, 0x44, 0x98, 0xb8, 0xc2, 0xfc, 0x82, 0x82, 0xae, 0xcd, 0x46, 0xcf, 0xb1, 0x47,
                0xdc, 0xd0, 0x69, 0x0d, 0x19, 0xad, 0xe6, 0x6c, 0x70, 0xfe, 0x87, 0x92, 0x04, 0xb6,
                0x82, 0x2d, 0x97, 0x7e, 0x46, 0x80, 0x4c, 0xe5, 0x76, 0x72, 0xb4, 0xb8
            ]
        );
    }

    #[test]
    fn test_output_length_bounds() {
        let hkdf = HkdfUsingHmac(&hmac::HMAC_SHA256);
        let ikm = &[];
        let info = &[];

        let mut output = [0u8; 32 * 255 + 1];
        assert!(
            hkdf.extract_from_secret(None, ikm)
                .expand_slice(info, &mut output)
                .is_err()
        );
    }
}
