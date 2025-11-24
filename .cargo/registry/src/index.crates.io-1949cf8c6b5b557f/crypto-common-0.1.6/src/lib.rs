//! Common cryptographic traits.

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/media/6ee8e381/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/media/6ee8e381/logo.svg"
)]
#![forbid(unsafe_code)]
#![warn(missing_docs, rust_2018_idioms)]

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "rand_core")]
pub use rand_core;

pub use generic_array;
pub use generic_array::typenum;

use core::fmt;
use generic_array::{typenum::Unsigned, ArrayLength, GenericArray};
#[cfg(feature = "rand_core")]
use rand_core::{CryptoRng, RngCore};

/// Block on which [`BlockSizeUser`] implementors operate.
pub type Block<B> = GenericArray<u8, <B as BlockSizeUser>::BlockSize>;

/// Parallel blocks on which [`ParBlocksSizeUser`] implementors operate.
pub type ParBlocks<T> = GenericArray<Block<T>, <T as ParBlocksSizeUser>::ParBlocksSize>;

/// Output array of [`OutputSizeUser`] implementors.
pub type Output<T> = GenericArray<u8, <T as OutputSizeUser>::OutputSize>;

/// Key used by [`KeySizeUser`] implementors.
pub type Key<B> = GenericArray<u8, <B as KeySizeUser>::KeySize>;

/// Initialization vector (nonce) used by [`IvSizeUser`] implementors.
pub type Iv<B> = GenericArray<u8, <B as IvSizeUser>::IvSize>;

/// Types which process data in blocks.
pub trait BlockSizeUser {
    /// Size of the block in bytes.
    type BlockSize: ArrayLength<u8> + 'static;

    /// Return block size in bytes.
    fn block_size() -> usize {
        Self::BlockSize::USIZE
    }
}

impl<T: BlockSizeUser> BlockSizeUser for &T {
    type BlockSize = T::BlockSize;
}

impl<T: BlockSizeUser> BlockSizeUser for &mut T {
    type BlockSize = T::BlockSize;
}

/// Types which can process blocks in parallel.
pub trait ParBlocksSizeUser: BlockSizeUser {
    /// Number of blocks which can be processed in parallel.
    type ParBlocksSize: ArrayLength<Block<Self>>;
}

/// Types which return data with the given size.
pub trait OutputSizeUser {
    /// Size of the output in bytes.
    type OutputSize: ArrayLength<u8> + 'static;

    /// Return output size in bytes.
    fn output_size() -> usize {
        Self::OutputSize::USIZE
    }
}

/// Types which use key for initialization.
///
/// Generally it's used indirectly via [`KeyInit`] or [`KeyIvInit`].
pub trait KeySizeUser {
    /// Key size in bytes.
    type KeySize: ArrayLength<u8> + 'static;

    /// Return key size in bytes.
    fn key_size() -> usize {
        Self::KeySize::USIZE
    }
}

/// Types which use initialization vector (nonce) for initialization.
///
/// Generally it's used indirectly via [`KeyIvInit`] or [`InnerIvInit`].
pub trait IvSizeUser {
    /// Initialization vector size in bytes.
    type IvSize: ArrayLength<u8> + 'static;

    /// Return IV size in bytes.
    fn iv_size() -> usize {
        Self::IvSize::USIZE
    }
}

/// Types which use another type for initialization.
///
/// Generally it's used indirectly via [`InnerInit`] or [`InnerIvInit`].
pub trait InnerUser {
    /// Inner type.
    type Inner;
}

/// Resettable types.
pub trait Reset {
    /// Reset state to its initial value.
    fn reset(&mut self);
}

/// Trait which stores algorithm name constant, used in `Debug` implementations.
pub trait AlgorithmName {
    /// Write algorithm name into `f`.
    fn write_alg_name(f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

/// Types which can be initialized from key.
pub trait KeyInit: KeySizeUser + Sized {
    /// Create new value from fixed size key.
    fn new(key: &Key<Self>) -> Self;

    /// Create new value from variable size key.
    fn new_from_slice(key: &[u8]) -> Result<Self, InvalidLength> {
        if key.len() != Self::KeySize::to_usize() {
            Err(InvalidLength)
        } else {
            Ok(Self::new(Key::<Self>::from_slice(key)))
        }
    }

    /// Generate random key using the provided [`CryptoRng`].
    #[cfg(feature = "rand_core")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rand_core")))]
    #[inline]
    fn generate_key(mut rng: impl CryptoRng + RngCore) -> Key<Self> {
        let mut key = Key::<Self>::default();
        rng.fill_bytes(&mut key);
        key
    }
}

/// Types which can be initialized from key and initialization vector (nonce).
pub trait KeyIvInit: KeySizeUser + IvSizeUser + Sized {
    /// Create new value from fixed length key and nonce.
    fn new(key: &Key<Self>, iv: &Iv<Self>) -> Self;

    /// Create new value from variable length key and nonce.
    #[inline]
    fn new_from_slices(key: &[u8], iv: &[u8]) -> Result<Self, InvalidLength> {
        let key_len = Self::KeySize::USIZE;
        let iv_len = Self::IvSize::USIZE;
        if key.len() != key_len || iv.len() != iv_len {
            Err(InvalidLength)
        } else {
            Ok(Self::new(
                Key::<Self>::from_slice(key),
                Iv::<Self>::from_slice(iv),
            ))
        }
    }

    /// Generate random key using the provided [`CryptoRng`].
    #[cfg(feature = "rand_core")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rand_core")))]
    #[inline]
    fn generate_key(mut rng: impl CryptoRng + RngCore) -> Key<Self> {
        let mut key = Key::<Self>::default();
        rng.fill_bytes(&mut key);
        key
    }

    /// Generate random IV using the provided [`CryptoRng`].
    #[cfg(feature = "rand_core")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rand_core")))]
    #[inline]
    fn generate_iv(mut rng: impl CryptoRng + RngCore) -> Iv<Self> {
        let mut iv = Iv::<Self>::default();
        rng.fill_bytes(&mut iv);
        iv
    }

    /// Generate random key and nonce using the provided [`CryptoRng`].
    #[cfg(feature = "rand_core")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rand_core")))]
    #[inline]
    fn generate_key_iv(mut rng: impl CryptoRng + RngCore) -> (Key<Self>, Iv<Self>) {
        (Self::generate_key(&mut rng), Self::generate_iv(&mut rng))
    }
}

/// Types which can be initialized from another type (usually block ciphers).
///
/// Usually used for initializing types from block ciphers.
pub trait InnerInit: InnerUser + Sized {
    /// Initialize value from the `inner`.
    fn inner_init(inner: Self::Inner) -> Self;
}

/// Types which can be initialized from another type and additional initialization
/// vector/nonce.
///
/// Usually used for initializing types from block ciphers.
pub trait InnerIvInit: InnerUser + IvSizeUser + Sized {
    /// Initialize value using `inner` and `iv` array.
    fn inner_iv_init(inner: Self::Inner, iv: &Iv<Self>) -> Self;

    /// Initialize value using `inner` and `iv` slice.
    fn inner_iv_slice_init(inner: Self::Inner, iv: &[u8]) -> Result<Self, InvalidLength> {
        if iv.len() != Self::IvSize::to_usize() {
            Err(InvalidLength)
        } else {
            Ok(Self::inner_iv_init(inner, Iv::<Self>::from_slice(iv)))
        }
    }

    /// Generate random IV using the provided [`CryptoRng`].
    #[cfg(feature = "rand_core")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rand_core")))]
    #[inline]
    fn generate_iv(mut rng: impl CryptoRng + RngCore) -> Iv<Self> {
        let mut iv = Iv::<Self>::default();
        rng.fill_bytes(&mut iv);
        iv
    }
}

impl<T> KeySizeUser for T
where
    T: InnerUser,
    T::Inner: KeySizeUser,
{
    type KeySize = <T::Inner as KeySizeUser>::KeySize;
}

impl<T> KeyIvInit for T
where
    T: InnerIvInit,
    T::Inner: KeyInit,
{
    #[inline]
    fn new(key: &Key<Self>, iv: &Iv<Self>) -> Self {
        Self::inner_iv_init(T::Inner::new(key), iv)
    }

    #[inline]
    fn new_from_slices(key: &[u8], iv: &[u8]) -> Result<Self, InvalidLength> {
        T::Inner::new_from_slice(key).and_then(|i| T::inner_iv_slice_init(i, iv))
    }
}

impl<T> KeyInit for T
where
    T: InnerInit,
    T::Inner: KeyInit,
{
    #[inline]
    fn new(key: &Key<Self>) -> Self {
        Self::inner_init(T::Inner::new(key))
    }

    #[inline]
    fn new_from_slice(key: &[u8]) -> Result<Self, InvalidLength> {
        T::Inner::new_from_slice(key)
            .map_err(|_| InvalidLength)
            .map(Self::inner_init)
    }
}

// Unfortunately this blanket impl is impossible without mutually
// exclusive traits, see: https://github.com/rust-lang/rfcs/issues/1053
// or at the very least without: https://github.com/rust-lang/rust/issues/20400
/*
impl<T> KeyIvInit for T
where
    T: InnerInit,
    T::Inner: KeyIvInit,
{
    #[inline]
    fn new(key: &Key<Self>, iv: &Iv<Self>) -> Self {
        Self::inner_init(T::Inner::new(key, iv))
    }

    #[inline]
    fn new_from_slices(key: &[u8], iv: &[u8]) -> Result<Self, InvalidLength> {
        T::Inner::new_from_slice(key)
            .map_err(|_| InvalidLength)
            .map(Self::inner_init)
    }
}
*/

/// The error type returned when key and/or IV used in the [`KeyInit`],
/// [`KeyIvInit`], and [`InnerIvInit`] slice-based methods had
/// an invalid length.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct InvalidLength;

impl fmt::Display for InvalidLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.write_str("Invalid Length")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for InvalidLength {}
