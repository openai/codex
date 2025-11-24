//! An implementation of the [SHA-2][1] cryptographic hash algorithms.
//!
//! There are 6 standard algorithms specified in the SHA-2 standard: [`Sha224`],
//! [`Sha256`], [`Sha512_224`], [`Sha512_256`], [`Sha384`], and [`Sha512`].
//!
//! Algorithmically, there are only 2 core algorithms: SHA-256 and SHA-512.
//! All other algorithms are just applications of these with different initial
//! hash values, and truncated to different digest bit lengths. The first two
//! algorithms in the list are based on SHA-256, while the last four are based
//! on SHA-512.
//!
//! # Usage
//!
//! ## One-shot API
//!
//! ```rust
//! use hex_literal::hex;
//! use sha2::{Sha256, Digest};
//!
//! let result = Sha256::digest(b"hello world");
//! assert_eq!(result[..], hex!("
//!     b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
//! ")[..]);
//! ```
//!
//! ## Incremental API
//!
//! ```rust
//! use hex_literal::hex;
//! use sha2::{Sha256, Sha512, Digest};
//!
//! // create a Sha256 object
//! let mut hasher = Sha256::new();
//!
//! // write input message
//! hasher.update(b"hello world");
//!
//! // read hash digest and consume hasher
//! let result = hasher.finalize();
//!
//! assert_eq!(result[..], hex!("
//!     b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
//! ")[..]);
//!
//! // same for Sha512
//! let mut hasher = Sha512::new();
//! hasher.update(b"hello world");
//! let result = hasher.finalize();
//!
//! assert_eq!(result[..], hex!("
//!     309ecc489c12d6eb4cc40f50c902f2b4d0ed77ee511a7c7a9bcd3ca86d4cd86f
//!     989dd35bc5ff499670da34255b45b0cfd830e81f605dcf7dc5542e93ae9cd76f
//! ")[..]);
//! ```
//!
//! Also see [RustCrypto/hashes][2] readme.
//!
//! [1]: https://en.wikipedia.org/wiki/SHA-2
//! [2]: https://github.com/RustCrypto/hashes

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/RustCrypto/media/6ee8e381/logo.svg",
    html_favicon_url = "https://raw.githubusercontent.com/RustCrypto/media/6ee8e381/logo.svg"
)]
#![warn(missing_docs, rust_2018_idioms)]

pub use digest::{self, Digest};

#[cfg(feature = "oid")]
use digest::const_oid::{AssociatedOid, ObjectIdentifier};
use digest::{
    consts::{U28, U32, U48, U64},
    core_api::{CoreWrapper, CtVariableCoreWrapper},
    impl_oid_carrier,
};

#[rustfmt::skip]
mod consts;
mod core_api;
mod sha256;
mod sha512;

#[cfg(feature = "compress")]
pub use sha256::compress256;
#[cfg(feature = "compress")]
pub use sha512::compress512;

pub use core_api::{Sha256VarCore, Sha512VarCore};

impl_oid_carrier!(OidSha256, "2.16.840.1.101.3.4.2.1");
impl_oid_carrier!(OidSha384, "2.16.840.1.101.3.4.2.2");
impl_oid_carrier!(OidSha512, "2.16.840.1.101.3.4.2.3");
impl_oid_carrier!(OidSha224, "2.16.840.1.101.3.4.2.4");
impl_oid_carrier!(OidSha512_224, "2.16.840.1.101.3.4.2.5");
impl_oid_carrier!(OidSha512_256, "2.16.840.1.101.3.4.2.6");

/// SHA-224 hasher.
pub type Sha224 = CoreWrapper<CtVariableCoreWrapper<Sha256VarCore, U28, OidSha224>>;
/// SHA-256 hasher.
pub type Sha256 = CoreWrapper<CtVariableCoreWrapper<Sha256VarCore, U32, OidSha256>>;
/// SHA-512/224 hasher.
pub type Sha512_224 = CoreWrapper<CtVariableCoreWrapper<Sha512VarCore, U28, OidSha512_224>>;
/// SHA-512/256 hasher.
pub type Sha512_256 = CoreWrapper<CtVariableCoreWrapper<Sha512VarCore, U32, OidSha512_256>>;
/// SHA-384 hasher.
pub type Sha384 = CoreWrapper<CtVariableCoreWrapper<Sha512VarCore, U48, OidSha384>>;
/// SHA-512 hasher.
pub type Sha512 = CoreWrapper<CtVariableCoreWrapper<Sha512VarCore, U64, OidSha512>>;
