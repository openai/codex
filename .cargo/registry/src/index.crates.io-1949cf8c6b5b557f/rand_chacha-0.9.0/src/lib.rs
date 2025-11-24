// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The ChaCha random number generators.
//!
//! These are native Rust implementations of RNGs derived from the
//! [ChaCha stream ciphers] by D J Bernstein.
//!
//! ## Generators
//!
//! This crate provides 8-, 12- and 20-round variants of generators via a "core"
//! implementation (of [`BlockRngCore`]), each with an associated "RNG" type
//! (implementing [`RngCore`]).
//!
//! These generators are all deterministic and portable (see [Reproducibility]
//! in the book), with testing against reference vectors.
//!
//! ## Cryptographic (secure) usage
//!
//! Where secure unpredictable generators are required, it is suggested to use
//! [`ChaCha12Rng`] or [`ChaCha20Rng`] and to seed via
//! [`SeedableRng::from_os_rng`].
//!
//! See also the [Security] chapter in the rand book. The crate is provided
//! "as is", without any form of guarantee, and without a security audit.
//!
//! ## Seeding (construction)
//!
//! Generators implement the [`SeedableRng`] trait. Any method may be used,
//! but note that `seed_from_u64` is not suitable for usage where security is
//! important. Some suggestions:
//!
//! 1.  With a fresh seed, **direct from the OS** (implies a syscall):
//!     ```
//!     # use {rand_core::SeedableRng, rand_chacha::ChaCha12Rng};
//!     let rng = ChaCha12Rng::from_os_rng();
//!     # let _: ChaCha12Rng = rng;
//!     ```
//! 2.  **From a master generator.** This could be [`rand::rng`]
//!     (effectively a fresh seed without the need for a syscall on each usage)
//!     or a deterministic generator such as [`ChaCha20Rng`].
//!     Beware that should a weak master generator be used, correlations may be
//!     detectable between the outputs of its child generators.
//!     ```ignore
//!     let rng = ChaCha12Rng::from_rng(&mut rand::rng());
//!     ```
//!
//! See also [Seeding RNGs] in the book.
//!
//! ## Generation
//!
//! Generators implement [`RngCore`], whose methods may be used directly to
//! generate unbounded integer or byte values.
//! ```
//! use rand_core::{SeedableRng, RngCore};
//! use rand_chacha::ChaCha12Rng;
//!
//! let mut rng = ChaCha12Rng::from_seed(Default::default());
//! let x = rng.next_u64();
//! assert_eq!(x, 0x53f955076a9af49b);
//! ```
//!
//! It is often more convenient to use the [`rand::Rng`] trait, which provides
//! further functionality. See also the [Random Values] chapter in the book.
//!
//! [ChaCha stream ciphers]: https://cr.yp.to/chacha.html
//! [Reproducibility]: https://rust-random.github.io/book/crate-reprod.html
//! [Seeding RNGs]: https://rust-random.github.io/book/guide-seeding.html
//! [Security]: https://rust-random.github.io/book/guide-rngs.html#security
//! [Random Values]: https://rust-random.github.io/book/guide-values.html
//! [`BlockRngCore`]: rand_core::block::BlockRngCore
//! [`RngCore`]: rand_core::RngCore
//! [`SeedableRng`]: rand_core::SeedableRng
//! [`SeedableRng::from_os_rng`]: rand_core::SeedableRng::from_os_rng
//! [`rand::rng`]: https://docs.rs/rand/latest/rand/fn.rng.html
//! [`rand::Rng`]: https://docs.rs/rand/latest/rand/trait.Rng.html

#![doc(
    html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk.png",
    html_favicon_url = "https://www.rust-lang.org/favicon.ico",
    html_root_url = "https://rust-random.github.io/rand/"
)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![doc(test(attr(allow(unused_variables), deny(warnings))))]
#![cfg_attr(not(feature = "std"), no_std)]

pub use rand_core;

mod chacha;
mod guts;

pub use crate::chacha::{
    ChaCha12Core, ChaCha12Rng, ChaCha20Core, ChaCha20Rng, ChaCha8Core, ChaCha8Rng,
};

/// ChaCha with 20 rounds
pub type ChaChaRng = ChaCha20Rng;
/// ChaCha with 20 rounds, low-level interface
pub type ChaChaCore = ChaCha20Core;
