//! Generic implementation of Hash-based Message Authentication Code (HMAC).
//!
//! To use it you will need a cryptographic hash function implementation which
//! implements the [`digest`] crate traits. You can find compatible crates
//! (e.g. [`sha2`]) in the [`RustCrypto/hashes`] repository.
//!
//! This crate provides two HMAC implementation [`Hmac`] and [`SimpleHmac`].
//! The first one is a buffered wrapper around block-level [`HmacCore`].
//! Internally it uses efficient state representation, but works only with
//! hash functions which expose block-level API and consume blocks eagerly
//! (e.g. it will not work with the BLAKE2 family of  hash functions).
//! On the other hand, [`SimpleHmac`] is a bit less efficient memory-wise,
//! but works with all hash functions which implement the [`Digest`] trait.
//!
//! # Examples
//! Let us demonstrate how to use HMAC using the SHA-256 hash function.
//!
//! In the following examples [`Hmac`] is interchangeable with [`SimpleHmac`].
//!
//! To get authentication code:
//!
//! ```rust
//! use sha2::Sha256;
//! use hmac::{Hmac, Mac};
//! use hex_literal::hex;
//!
//! // Create alias for HMAC-SHA256
//! type HmacSha256 = Hmac<Sha256>;
//!
//! let mut mac = HmacSha256::new_from_slice(b"my secret and secure key")
//!     .expect("HMAC can take key of any size");
//! mac.update(b"input message");
//!
//! // `result` has type `CtOutput` which is a thin wrapper around array of
//! // bytes for providing constant time equality check
//! let result = mac.finalize();
//! // To get underlying array use `into_bytes`, but be careful, since
//! // incorrect use of the code value may permit timing attacks which defeats
//! // the security provided by the `CtOutput`
//! let code_bytes = result.into_bytes();
//! let expected = hex!("
//!     97d2a569059bbcd8ead4444ff99071f4
//!     c01d005bcefe0d3567e1be628e5fdcd9
//! ");
//! assert_eq!(code_bytes[..], expected[..]);
//! ```
//!
//! To verify the message:
//!
//! ```rust
//! # use sha2::Sha256;
//! # use hmac::{Hmac, Mac};
//! # use hex_literal::hex;
//! # type HmacSha256 = Hmac<Sha256>;
//! let mut mac = HmacSha256::new_from_slice(b"my secret and secure key")
//!     .expect("HMAC can take key of any size");
//!
//! mac.update(b"input message");
//!
//! let code_bytes = hex!("
//!     97d2a569059bbcd8ead4444ff99071f4
//!     c01d005bcefe0d3567e1be628e5fdcd9
//! ");
//! // `verify_slice` will return `Ok(())` if code is correct, `Err(MacError)` otherwise
//! mac.verify_slice(&code_bytes[..]).unwrap();
//! ```
//!
//! # Block and input sizes
//! Usually it is assumed that block size is larger than output size. Due to the
//! generic nature of the implementation, this edge case must be handled as well
//! to remove potential panic. This is done by truncating hash output to the hash
//! block size if needed.
//!
//! [`digest`]: https://docs.rs/digest
//! [`sha2`]: https://docs.rs/sha2
//! [`RustCrypto/hashes`]: https://github.com/RustCrypto/hashes

#![no_std]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/media/26acc39f/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/media/26acc39f/logo.svg",
    html_root_url = "https://docs.rs/hmac/0.12.1"
)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs, rust_2018_idioms)]

#[cfg(feature = "std")]
extern crate std;

pub use digest;
pub use digest::Mac;

use digest::{
    core_api::{Block, BlockSizeUser},
    Digest,
};

mod optim;
mod simple;

pub use optim::{Hmac, HmacCore};
pub use simple::SimpleHmac;

const IPAD: u8 = 0x36;
const OPAD: u8 = 0x5C;

fn get_der_key<D: Digest + BlockSizeUser>(key: &[u8]) -> Block<D> {
    let mut der_key = Block::<D>::default();
    // The key that HMAC processes must be the same as the block size of the
    // underlying hash function. If the provided key is smaller than that,
    // we just pad it with zeros. If its larger, we hash it and then pad it
    // with zeros.
    if key.len() <= der_key.len() {
        der_key[..key.len()].copy_from_slice(key);
    } else {
        let hash = D::digest(key);
        // All commonly used hash functions have block size bigger
        // than output hash size, but to be extra rigorous we
        // handle the potential uncommon cases as well.
        // The condition is calcualted at compile time, so this
        // branch gets removed from the final binary.
        if hash.len() <= der_key.len() {
            der_key[..hash.len()].copy_from_slice(&hash);
        } else {
            let n = der_key.len();
            der_key.copy_from_slice(&hash[..n]);
        }
    }
    der_key
}
