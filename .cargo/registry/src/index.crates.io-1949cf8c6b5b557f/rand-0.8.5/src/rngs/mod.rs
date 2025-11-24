// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Random number generators and adapters
//!
//! ## Background: Random number generators (RNGs)
//!
//! Computers cannot produce random numbers from nowhere. We classify
//! random number generators as follows:
//!
//! -   "True" random number generators (TRNGs) use hard-to-predict data sources
//!     (e.g. the high-resolution parts of event timings and sensor jitter) to
//!     harvest random bit-sequences, apply algorithms to remove bias and
//!     estimate available entropy, then combine these bits into a byte-sequence
//!     or an entropy pool. This job is usually done by the operating system or
//!     a hardware generator (HRNG).
//! -   "Pseudo"-random number generators (PRNGs) use algorithms to transform a
//!     seed into a sequence of pseudo-random numbers. These generators can be
//!     fast and produce well-distributed unpredictable random numbers (or not).
//!     They are usually deterministic: given algorithm and seed, the output
//!     sequence can be reproduced. They have finite period and eventually loop;
//!     with many algorithms this period is fixed and can be proven sufficiently
//!     long, while others are chaotic and the period depends on the seed.
//! -   "Cryptographically secure" pseudo-random number generators (CSPRNGs)
//!     are the sub-set of PRNGs which are secure. Security of the generator
//!     relies both on hiding the internal state and using a strong algorithm.
//!
//! ## Traits and functionality
//!
//! All RNGs implement the [`RngCore`] trait, as a consequence of which the
//! [`Rng`] extension trait is automatically implemented. Secure RNGs may
//! additionally implement the [`CryptoRng`] trait.
//!
//! All PRNGs require a seed to produce their random number sequence. The
//! [`SeedableRng`] trait provides three ways of constructing PRNGs:
//!
//! -   `from_seed` accepts a type specific to the PRNG
//! -   `from_rng` allows a PRNG to be seeded from any other RNG
//! -   `seed_from_u64` allows any PRNG to be seeded from a `u64` insecurely
//! -   `from_entropy` securely seeds a PRNG from fresh entropy
//!
//! Use the [`rand_core`] crate when implementing your own RNGs.
//!
//! ## Our generators
//!
//! This crate provides several random number generators:
//!
//! -   [`OsRng`] is an interface to the operating system's random number
//!     source. Typically the operating system uses a CSPRNG with entropy
//!     provided by a TRNG and some type of on-going re-seeding.
//! -   [`ThreadRng`], provided by the [`thread_rng`] function, is a handle to a
//!     thread-local CSPRNG with periodic seeding from [`OsRng`]. Because this
//!     is local, it is typically much faster than [`OsRng`]. It should be
//!     secure, though the paranoid may prefer [`OsRng`].
//! -   [`StdRng`] is a CSPRNG chosen for good performance and trust of security
//!     (based on reviews, maturity and usage). The current algorithm is ChaCha12,
//!     which is well established and rigorously analysed.
//!     [`StdRng`] provides the algorithm used by [`ThreadRng`] but without
//!     periodic reseeding.
//! -   [`SmallRng`] is an **insecure** PRNG designed to be fast, simple, require
//!     little memory, and have good output quality.
//!
//! The algorithms selected for [`StdRng`] and [`SmallRng`] may change in any
//! release and may be platform-dependent, therefore they should be considered
//! **not reproducible**.
//!
//! ## Additional generators
//!
//! **TRNGs**: The [`rdrand`] crate provides an interface to the RDRAND and
//! RDSEED instructions available in modern Intel and AMD CPUs.
//! The [`rand_jitter`] crate provides a user-space implementation of
//! entropy harvesting from CPU timer jitter, but is very slow and has
//! [security issues](https://github.com/rust-random/rand/issues/699).
//!
//! **PRNGs**: Several companion crates are available, providing individual or
//! families of PRNG algorithms. These provide the implementations behind
//! [`StdRng`] and [`SmallRng`] but can also be used directly, indeed *should*
//! be used directly when **reproducibility** matters.
//! Some suggestions are: [`rand_chacha`], [`rand_pcg`], [`rand_xoshiro`].
//! A full list can be found by searching for crates with the [`rng` tag].
//!
//! [`Rng`]: crate::Rng
//! [`RngCore`]: crate::RngCore
//! [`CryptoRng`]: crate::CryptoRng
//! [`SeedableRng`]: crate::SeedableRng
//! [`thread_rng`]: crate::thread_rng
//! [`rdrand`]: https://crates.io/crates/rdrand
//! [`rand_jitter`]: https://crates.io/crates/rand_jitter
//! [`rand_chacha`]: https://crates.io/crates/rand_chacha
//! [`rand_pcg`]: https://crates.io/crates/rand_pcg
//! [`rand_xoshiro`]: https://crates.io/crates/rand_xoshiro
//! [`rng` tag]: https://crates.io/keywords/rng

#[cfg_attr(doc_cfg, doc(cfg(feature = "std")))]
#[cfg(feature = "std")] pub mod adapter;

pub mod mock; // Public so we don't export `StepRng` directly, making it a bit
              // more clear it is intended for testing.

#[cfg(all(feature = "small_rng", target_pointer_width = "64"))]
mod xoshiro256plusplus;
#[cfg(all(feature = "small_rng", not(target_pointer_width = "64")))]
mod xoshiro128plusplus;
#[cfg(feature = "small_rng")] mod small;

#[cfg(feature = "std_rng")] mod std;
#[cfg(all(feature = "std", feature = "std_rng"))] pub(crate) mod thread;

#[cfg(feature = "small_rng")] pub use self::small::SmallRng;
#[cfg(feature = "std_rng")] pub use self::std::StdRng;
#[cfg(all(feature = "std", feature = "std_rng"))] pub use self::thread::ThreadRng;

#[cfg_attr(doc_cfg, doc(cfg(feature = "getrandom")))]
#[cfg(feature = "getrandom")] pub use rand_core::OsRng;
