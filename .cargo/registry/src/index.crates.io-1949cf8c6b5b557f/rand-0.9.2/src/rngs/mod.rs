// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Random number generators and adapters
//!
//! This crate provides a small selection of non-[portable] generators.
//! See also [Types of generators] and [Our RNGs] in the book.
//!
//! ## Generators
//!
//! This crate provides a small selection of non-[portable] random number generators:
//!
//! -   [`OsRng`] is a stateless interface over the operating system's random number
//!     source. This is typically secure with some form of periodic re-seeding.
//! -   [`ThreadRng`], provided by [`crate::rng()`], is a handle to a
//!     thread-local generator with periodic seeding from [`OsRng`]. Because this
//!     is local, it is typically much faster than [`OsRng`]. It should be
//!     secure, but see documentation on [`ThreadRng`].
//! -   [`StdRng`] is a CSPRNG chosen for good performance and trust of security
//!     (based on reviews, maturity and usage). The current algorithm is ChaCha12,
//!     which is well established and rigorously analysed.
//!     [`StdRng`] is the deterministic generator used by [`ThreadRng`] but
//!     without the periodic reseeding or thread-local management.
//! -   [`SmallRng`] is a relatively simple, insecure generator designed to be
//!     fast, use little memory, and pass various statistical tests of
//!     randomness quality.
//!
//! The algorithms selected for [`StdRng`] and [`SmallRng`] may change in any
//! release and may be platform-dependent, therefore they are not
//! [reproducible][portable].
//!
//! ### Additional generators
//!
//! -   The [`rdrand`] crate provides an interface to the RDRAND and RDSEED
//!     instructions available in modern Intel and AMD CPUs.
//! -   The [`rand_jitter`] crate provides a user-space implementation of
//!     entropy harvesting from CPU timer jitter, but is very slow and has
//!     [security issues](https://github.com/rust-random/rand/issues/699).
//! -   The [`rand_chacha`] crate provides [portable] implementations of
//!     generators derived from the [ChaCha] family of stream ciphers
//! -   The [`rand_pcg`] crate provides [portable] implementations of a subset
//!     of the [PCG] family of small, insecure generators
//! -   The [`rand_xoshiro`] crate provides [portable] implementations of the
//!     [xoshiro] family of small, insecure generators
//!
//! For more, search [crates with the `rng` tag].
//!
//! ## Traits and functionality
//!
//! All generators implement [`RngCore`] and thus also [`Rng`][crate::Rng].
//! See also the [Random Values] chapter in the book.
//!
//! Secure RNGs may additionally implement the [`CryptoRng`] trait.
//!
//! Use the [`rand_core`] crate when implementing your own RNGs.
//!
//! [portable]: https://rust-random.github.io/book/crate-reprod.html
//! [Types of generators]: https://rust-random.github.io/book/guide-gen.html
//! [Our RNGs]: https://rust-random.github.io/book/guide-rngs.html
//! [Random Values]: https://rust-random.github.io/book/guide-values.html
//! [`Rng`]: crate::Rng
//! [`RngCore`]: crate::RngCore
//! [`CryptoRng`]: crate::CryptoRng
//! [`SeedableRng`]: crate::SeedableRng
//! [`rdrand`]: https://crates.io/crates/rdrand
//! [`rand_jitter`]: https://crates.io/crates/rand_jitter
//! [`rand_chacha`]: https://crates.io/crates/rand_chacha
//! [`rand_pcg`]: https://crates.io/crates/rand_pcg
//! [`rand_xoshiro`]: https://crates.io/crates/rand_xoshiro
//! [crates with the `rng` tag]: https://crates.io/keywords/rng
//! [chacha]: https://cr.yp.to/chacha.html
//! [PCG]: https://www.pcg-random.org/
//! [xoshiro]: https://prng.di.unimi.it/

mod reseeding;
pub use reseeding::ReseedingRng;

#[deprecated(since = "0.9.2")]
pub mod mock; // Public so we don't export `StepRng` directly, making it a bit
              // more clear it is intended for testing.

#[cfg(feature = "small_rng")]
mod small;
#[cfg(all(
    feature = "small_rng",
    any(target_pointer_width = "32", target_pointer_width = "16")
))]
mod xoshiro128plusplus;
#[cfg(all(feature = "small_rng", target_pointer_width = "64"))]
mod xoshiro256plusplus;

#[cfg(feature = "std_rng")]
mod std;
#[cfg(feature = "thread_rng")]
pub(crate) mod thread;

#[cfg(feature = "small_rng")]
pub use self::small::SmallRng;
#[cfg(feature = "std_rng")]
pub use self::std::StdRng;
#[cfg(feature = "thread_rng")]
pub use self::thread::ThreadRng;

#[cfg(feature = "os_rng")]
pub use rand_core::OsRng;
