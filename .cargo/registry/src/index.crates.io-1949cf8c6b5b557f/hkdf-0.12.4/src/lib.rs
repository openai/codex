//! An implementation of HKDF, the [HMAC-based Extract-and-Expand Key Derivation Function][1].
//!
//! # Usage
//!
//! The most common way to use HKDF is as follows: you provide the Initial Key
//! Material (IKM) and an optional salt, then you expand it (perhaps multiple times)
//! into some Output Key Material (OKM) bound to an "info" context string.
//!
//! There are two usage options for the salt:
//!
//! - [`None`] or static for domain separation in a private setting
//! -  guaranteed to be uniformly-distributed and unique in a public setting
//!
//! Other non fitting data should be added to the `IKM` or `info`.
//!
//! ```rust
//! use sha2::Sha256;
//! use hkdf::Hkdf;
//! use hex_literal::hex;
//!
//! let ikm = hex!("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b");
//! let salt = hex!("000102030405060708090a0b0c");
//! let info = hex!("f0f1f2f3f4f5f6f7f8f9");
//!
//! let hk = Hkdf::<Sha256>::new(Some(&salt[..]), &ikm);
//! let mut okm = [0u8; 42];
//! hk.expand(&info, &mut okm)
//!     .expect("42 is a valid length for Sha256 to output");
//!
//! let expected = hex!("
//!     3cb25f25faacd57a90434f64d0362f2a
//!     2d2d0a90cf1a5a4c5db02d56ecc4c5bf
//!     34007208d5b887185865
//! ");
//! assert_eq!(okm[..], expected[..]);
//! ```
//!
//! Normally the PRK (Pseudo-Random Key) remains hidden within the HKDF
//! object, but if you need to access it, use [`Hkdf::extract`] instead of
//! [`Hkdf::new`].
//!
//! ```rust
//! # use sha2::Sha256;
//! # use hkdf::Hkdf;
//! # use hex_literal::hex;
//! # let ikm = hex!("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b");
//! # let salt = hex!("000102030405060708090a0b0c");
//!
//! let (prk, hk) = Hkdf::<Sha256>::extract(Some(&salt[..]), &ikm);
//! let expected = hex!("
//!     077709362c2e32df0ddc3f0dc47bba63
//!     90b6c73bb50f9c3122ec844ad7c2b3e5
//! ");
//! assert_eq!(prk[..], expected[..]);
//! ```
//!
//! If you already have a strong key to work from (uniformly-distributed and
//! long enough), you can save a tiny amount of time by skipping the extract
//! step. In this case, you pass a Pseudo-Random Key (PRK) into the
//! [`Hkdf::from_prk`] constructor, then use the resulting [`Hkdf`] object
//! as usual.
//!
//! ```rust
//! # use sha2::Sha256;
//! # use hkdf::Hkdf;
//! # use hex_literal::hex;
//! # let salt = hex!("000102030405060708090a0b0c");
//! # let info = hex!("f0f1f2f3f4f5f6f7f8f9");
//! let prk = hex!("
//!     077709362c2e32df0ddc3f0dc47bba63
//!     90b6c73bb50f9c3122ec844ad7c2b3e5
//! ");
//!
//! let hk = Hkdf::<Sha256>::from_prk(&prk).expect("PRK should be large enough");
//! let mut okm = [0u8; 42];
//! hk.expand(&info, &mut okm)
//!     .expect("42 is a valid length for Sha256 to output");
//!
//! let expected = hex!("
//!     3cb25f25faacd57a90434f64d0362f2a
//!     2d2d0a90cf1a5a4c5db02d56ecc4c5bf
//!     34007208d5b887185865
//! ");
//! assert_eq!(okm[..], expected[..]);
//! ```
//!
//! [1]: https://tools.ietf.org/html/rfc5869

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/media/6ee8e381/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/media/6ee8e381/logo.svg"
)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

#[cfg(feature = "std")]
extern crate std;

pub use hmac;

use core::fmt;
use core::marker::PhantomData;
use hmac::digest::{
    crypto_common::AlgorithmName, generic_array::typenum::Unsigned, Output, OutputSizeUser,
};
use hmac::{Hmac, SimpleHmac};

mod errors;
mod sealed;

pub use errors::{InvalidLength, InvalidPrkLength};

/// [`HkdfExtract`] variant which uses [`SimpleHmac`] for underlying HMAC
/// implementation.
pub type SimpleHkdfExtract<H> = HkdfExtract<H, SimpleHmac<H>>;
/// [`Hkdf`] variant which uses [`SimpleHmac`] for underlying HMAC
/// implementation.
pub type SimpleHkdf<H> = Hkdf<H, SimpleHmac<H>>;

/// Structure representing the streaming context of an HKDF-Extract operation
/// ```rust
/// # use hkdf::{Hkdf, HkdfExtract};
/// # use sha2::Sha256;
/// let mut extract_ctx = HkdfExtract::<Sha256>::new(Some(b"mysalt"));
/// extract_ctx.input_ikm(b"hello");
/// extract_ctx.input_ikm(b" world");
/// let (streamed_res, _) = extract_ctx.finalize();
///
/// let (oneshot_res, _) = Hkdf::<Sha256>::extract(Some(b"mysalt"), b"hello world");
/// assert_eq!(streamed_res, oneshot_res);
/// ```
#[derive(Clone)]
pub struct HkdfExtract<H, I = Hmac<H>>
where
    H: OutputSizeUser,
    I: HmacImpl<H>,
{
    hmac: I,
    _pd: PhantomData<H>,
}

impl<H, I> HkdfExtract<H, I>
where
    H: OutputSizeUser,
    I: HmacImpl<H>,
{
    /// Initiates the HKDF-Extract context with the given optional salt
    pub fn new(salt: Option<&[u8]>) -> Self {
        let default_salt = Output::<H>::default();
        let salt = salt.unwrap_or(&default_salt);
        Self {
            hmac: I::new_from_slice(salt),
            _pd: PhantomData,
        }
    }

    /// Feeds in additional input key material to the HKDF-Extract context
    pub fn input_ikm(&mut self, ikm: &[u8]) {
        self.hmac.update(ikm);
    }

    /// Completes the HKDF-Extract operation, returning both the generated pseudorandom key and
    /// `Hkdf` struct for expanding.
    pub fn finalize(self) -> (Output<H>, Hkdf<H, I>) {
        let prk = self.hmac.finalize();
        let hkdf = Hkdf::from_prk(&prk).expect("PRK size is correct");
        (prk, hkdf)
    }
}

impl<H, I> fmt::Debug for HkdfExtract<H, I>
where
    H: OutputSizeUser,
    I: HmacImpl<H>,
    I::Core: AlgorithmName,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HkdfExtract<")?;
        <I::Core as AlgorithmName>::write_alg_name(f)?;
        f.write_str("> { ... }")
    }
}

/// Structure representing the HKDF, capable of HKDF-Expand and HKDF-Extract operations.
/// Recommendations for the correct usage of the parameters can be found in the
/// [crate root](index.html#usage).
#[derive(Clone)]
pub struct Hkdf<H: OutputSizeUser, I: HmacImpl<H> = Hmac<H>> {
    hmac: I::Core,
    _pd: PhantomData<H>,
}

impl<H: OutputSizeUser, I: HmacImpl<H>> Hkdf<H, I> {
    /// Convenience method for [`extract`][Hkdf::extract] when the generated
    /// pseudorandom key can be ignored and only HKDF-Expand operation is needed. This is the most
    /// common constructor.
    pub fn new(salt: Option<&[u8]>, ikm: &[u8]) -> Self {
        let (_, hkdf) = Self::extract(salt, ikm);
        hkdf
    }

    /// Create `Hkdf` from an already cryptographically strong pseudorandom key
    /// as per section 3.3 from RFC5869.
    pub fn from_prk(prk: &[u8]) -> Result<Self, InvalidPrkLength> {
        // section 2.3 specifies that prk must be "at least HashLen octets"
        if prk.len() < <H as OutputSizeUser>::OutputSize::to_usize() {
            return Err(InvalidPrkLength);
        }
        Ok(Self {
            hmac: I::new_core(prk),
            _pd: PhantomData,
        })
    }

    /// The RFC5869 HKDF-Extract operation returning both the generated
    /// pseudorandom key and `Hkdf` struct for expanding.
    pub fn extract(salt: Option<&[u8]>, ikm: &[u8]) -> (Output<H>, Self) {
        let mut extract_ctx = HkdfExtract::new(salt);
        extract_ctx.input_ikm(ikm);
        extract_ctx.finalize()
    }

    /// The RFC5869 HKDF-Expand operation. This is equivalent to calling
    /// [`expand`][Hkdf::extract] with the `info` argument set equal to the
    /// concatenation of all the elements of `info_components`.
    pub fn expand_multi_info(
        &self,
        info_components: &[&[u8]],
        okm: &mut [u8],
    ) -> Result<(), InvalidLength> {
        let mut prev: Option<Output<H>> = None;

        let chunk_len = <H as OutputSizeUser>::OutputSize::USIZE;
        if okm.len() > chunk_len * 255 {
            return Err(InvalidLength);
        }

        for (block_n, block) in okm.chunks_mut(chunk_len).enumerate() {
            let mut hmac = I::from_core(&self.hmac);

            if let Some(ref prev) = prev {
                hmac.update(prev)
            };

            // Feed in the info components in sequence. This is equivalent to feeding in the
            // concatenation of all the info components
            for info in info_components {
                hmac.update(info);
            }

            hmac.update(&[block_n as u8 + 1]);

            let output = hmac.finalize();

            let block_len = block.len();
            block.copy_from_slice(&output[..block_len]);

            prev = Some(output);
        }

        Ok(())
    }

    /// The RFC5869 HKDF-Expand operation
    ///
    /// If you don't have any `info` to pass, use an empty slice.
    pub fn expand(&self, info: &[u8], okm: &mut [u8]) -> Result<(), InvalidLength> {
        self.expand_multi_info(&[info], okm)
    }
}

impl<H, I> fmt::Debug for Hkdf<H, I>
where
    H: OutputSizeUser,
    I: HmacImpl<H>,
    I::Core: AlgorithmName,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Hkdf<")?;
        <I::Core as AlgorithmName>::write_alg_name(f)?;
        f.write_str("> { ... }")
    }
}

/// Sealed trait implemented for [`Hmac`] and [`SimpleHmac`].
pub trait HmacImpl<H: OutputSizeUser>: sealed::Sealed<H> {}

impl<H: OutputSizeUser, T: sealed::Sealed<H>> HmacImpl<H> for T {}
