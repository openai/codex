// Copyright 2018 Developers of the Rand project.
// Copyright 2017-2018 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Random number generation traits
//!
//! This crate is mainly of interest to crates publishing implementations of
//! [`RngCore`]. Other users are encouraged to use the [`rand`] crate instead
//! which re-exports the main traits and error types.
//!
//! [`RngCore`] is the core trait implemented by algorithmic pseudo-random number
//! generators and external random-number sources.
//!
//! [`SeedableRng`] is an extension trait for construction from fixed seeds and
//! other random number generators.
//!
//! [`Error`] is provided for error-handling. It is safe to use in `no_std`
//! environments.
//!
//! The [`impls`] and [`le`] sub-modules include a few small functions to assist
//! implementation of [`RngCore`].
//!
//! [`rand`]: https://docs.rs/rand

#![doc(
    html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk.png",
    html_favicon_url = "https://www.rust-lang.org/favicon.ico",
    html_root_url = "https://rust-random.github.io/rand/"
)]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![doc(test(attr(allow(unused_variables), deny(warnings))))]
#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![no_std]

use core::convert::AsMut;
use core::default::Default;

#[cfg(feature = "std")] extern crate std;
#[cfg(feature = "alloc")] extern crate alloc;
#[cfg(feature = "alloc")] use alloc::boxed::Box;

pub use error::Error;
#[cfg(feature = "getrandom")] pub use os::OsRng;


pub mod block;
mod error;
pub mod impls;
pub mod le;
#[cfg(feature = "getrandom")] mod os;


/// The core of a random number generator.
///
/// This trait encapsulates the low-level functionality common to all
/// generators, and is the "back end", to be implemented by generators.
/// End users should normally use the `Rng` trait from the [`rand`] crate,
/// which is automatically implemented for every type implementing `RngCore`.
///
/// Three different methods for generating random data are provided since the
/// optimal implementation of each is dependent on the type of generator. There
/// is no required relationship between the output of each; e.g. many
/// implementations of [`fill_bytes`] consume a whole number of `u32` or `u64`
/// values and drop any remaining unused bytes. The same can happen with the
/// [`next_u32`] and [`next_u64`] methods, implementations may discard some
/// random bits for efficiency.
///
/// The [`try_fill_bytes`] method is a variant of [`fill_bytes`] allowing error
/// handling; it is not deemed sufficiently useful to add equivalents for
/// [`next_u32`] or [`next_u64`] since the latter methods are almost always used
/// with algorithmic generators (PRNGs), which are normally infallible.
///
/// Implementers should produce bits uniformly. Pathological RNGs (e.g. always
/// returning the same value, or never setting certain bits) can break rejection
/// sampling used by random distributions, and also break other RNGs when
/// seeding them via [`SeedableRng::from_rng`].
///
/// Algorithmic generators implementing [`SeedableRng`] should normally have
/// *portable, reproducible* output, i.e. fix Endianness when converting values
/// to avoid platform differences, and avoid making any changes which affect
/// output (except by communicating that the release has breaking changes).
///
/// Typically an RNG will implement only one of the methods available
/// in this trait directly, then use the helper functions from the
/// [`impls`] module to implement the other methods.
///
/// It is recommended that implementations also implement:
///
/// - `Debug` with a custom implementation which *does not* print any internal
///   state (at least, [`CryptoRng`]s should not risk leaking state through
///   `Debug`).
/// - `Serialize` and `Deserialize` (from Serde), preferably making Serde
///   support optional at the crate level in PRNG libs.
/// - `Clone`, if possible.
/// - *never* implement `Copy` (accidental copies may cause repeated values).
/// - *do not* implement `Default` for pseudorandom generators, but instead
///   implement [`SeedableRng`], to guide users towards proper seeding.
///   External / hardware RNGs can choose to implement `Default`.
/// - `Eq` and `PartialEq` could be implemented, but are probably not useful.
///
/// # Example
///
/// A simple example, obviously not generating very *random* output:
///
/// ```
/// #![allow(dead_code)]
/// use rand_core::{RngCore, Error, impls};
///
/// struct CountingRng(u64);
///
/// impl RngCore for CountingRng {
///     fn next_u32(&mut self) -> u32 {
///         self.next_u64() as u32
///     }
///
///     fn next_u64(&mut self) -> u64 {
///         self.0 += 1;
///         self.0
///     }
///
///     fn fill_bytes(&mut self, dest: &mut [u8]) {
///         impls::fill_bytes_via_next(self, dest)
///     }
///
///     fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
///         Ok(self.fill_bytes(dest))
///     }
/// }
/// ```
///
/// [`rand`]: https://docs.rs/rand
/// [`try_fill_bytes`]: RngCore::try_fill_bytes
/// [`fill_bytes`]: RngCore::fill_bytes
/// [`next_u32`]: RngCore::next_u32
/// [`next_u64`]: RngCore::next_u64
pub trait RngCore {
    /// Return the next random `u32`.
    ///
    /// RNGs must implement at least one method from this trait directly. In
    /// the case this method is not implemented directly, it can be implemented
    /// using `self.next_u64() as u32` or via [`impls::next_u32_via_fill`].
    fn next_u32(&mut self) -> u32;

    /// Return the next random `u64`.
    ///
    /// RNGs must implement at least one method from this trait directly. In
    /// the case this method is not implemented directly, it can be implemented
    /// via [`impls::next_u64_via_u32`] or via [`impls::next_u64_via_fill`].
    fn next_u64(&mut self) -> u64;

    /// Fill `dest` with random data.
    ///
    /// RNGs must implement at least one method from this trait directly. In
    /// the case this method is not implemented directly, it can be implemented
    /// via [`impls::fill_bytes_via_next`] or
    /// via [`RngCore::try_fill_bytes`]; if this generator can
    /// fail the implementation must choose how best to handle errors here
    /// (e.g. panic with a descriptive message or log a warning and retry a few
    /// times).
    ///
    /// This method should guarantee that `dest` is entirely filled
    /// with new data, and may panic if this is impossible
    /// (e.g. reading past the end of a file that is being used as the
    /// source of randomness).
    fn fill_bytes(&mut self, dest: &mut [u8]);

    /// Fill `dest` entirely with random data.
    ///
    /// This is the only method which allows an RNG to report errors while
    /// generating random data thus making this the primary method implemented
    /// by external (true) RNGs (e.g. `OsRng`) which can fail. It may be used
    /// directly to generate keys and to seed (infallible) PRNGs.
    ///
    /// Other than error handling, this method is identical to [`RngCore::fill_bytes`];
    /// thus this may be implemented using `Ok(self.fill_bytes(dest))` or
    /// `fill_bytes` may be implemented with
    /// `self.try_fill_bytes(dest).unwrap()` or more specific error handling.
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error>;
}

/// A marker trait used to indicate that an [`RngCore`] or [`BlockRngCore`]
/// implementation is supposed to be cryptographically secure.
///
/// *Cryptographically secure generators*, also known as *CSPRNGs*, should
/// satisfy an additional properties over other generators: given the first
/// *k* bits of an algorithm's output
/// sequence, it should not be possible using polynomial-time algorithms to
/// predict the next bit with probability significantly greater than 50%.
///
/// Some generators may satisfy an additional property, however this is not
/// required by this trait: if the CSPRNG's state is revealed, it should not be
/// computationally-feasible to reconstruct output prior to this. Some other
/// generators allow backwards-computation and are considered *reversible*.
///
/// Note that this trait is provided for guidance only and cannot guarantee
/// suitability for cryptographic applications. In general it should only be
/// implemented for well-reviewed code implementing well-regarded algorithms.
///
/// Note also that use of a `CryptoRng` does not protect against other
/// weaknesses such as seeding from a weak entropy source or leaking state.
///
/// [`BlockRngCore`]: block::BlockRngCore
pub trait CryptoRng {}

/// An extension trait that is automatically implemented for any type
/// implementing [`RngCore`] and [`CryptoRng`].
///
/// It may be used as a trait object, and supports upcasting to [`RngCore`] via
/// the [`CryptoRngCore::as_rngcore`] method.
///
/// # Example
///
/// ```
/// use rand_core::CryptoRngCore;
///
/// #[allow(unused)]
/// fn make_token(rng: &mut dyn CryptoRngCore) -> [u8; 32] {
///     let mut buf = [0u8; 32];
///     rng.fill_bytes(&mut buf);
///     buf
/// }
/// ```
pub trait CryptoRngCore: CryptoRng + RngCore {
    /// Upcast to an [`RngCore`] trait object.
    fn as_rngcore(&mut self) -> &mut dyn RngCore;
}

impl<T: CryptoRng + RngCore> CryptoRngCore for T {
    fn as_rngcore(&mut self) -> &mut dyn RngCore {
        self
    }
}

/// A random number generator that can be explicitly seeded.
///
/// This trait encapsulates the low-level functionality common to all
/// pseudo-random number generators (PRNGs, or algorithmic generators).
///
/// [`rand`]: https://docs.rs/rand
pub trait SeedableRng: Sized {
    /// Seed type, which is restricted to types mutably-dereferenceable as `u8`
    /// arrays (we recommend `[u8; N]` for some `N`).
    ///
    /// It is recommended to seed PRNGs with a seed of at least circa 100 bits,
    /// which means an array of `[u8; 12]` or greater to avoid picking RNGs with
    /// partially overlapping periods.
    ///
    /// For cryptographic RNG's a seed of 256 bits is recommended, `[u8; 32]`.
    ///
    ///
    /// # Implementing `SeedableRng` for RNGs with large seeds
    ///
    /// Note that the required traits `core::default::Default` and
    /// `core::convert::AsMut<u8>` are not implemented for large arrays
    /// `[u8; N]` with `N` > 32. To be able to implement the traits required by
    /// `SeedableRng` for RNGs with such large seeds, the newtype pattern can be
    /// used:
    ///
    /// ```
    /// use rand_core::SeedableRng;
    ///
    /// const N: usize = 64;
    /// pub struct MyRngSeed(pub [u8; N]);
    /// pub struct MyRng(MyRngSeed);
    ///
    /// impl Default for MyRngSeed {
    ///     fn default() -> MyRngSeed {
    ///         MyRngSeed([0; N])
    ///     }
    /// }
    ///
    /// impl AsMut<[u8]> for MyRngSeed {
    ///     fn as_mut(&mut self) -> &mut [u8] {
    ///         &mut self.0
    ///     }
    /// }
    ///
    /// impl SeedableRng for MyRng {
    ///     type Seed = MyRngSeed;
    ///
    ///     fn from_seed(seed: MyRngSeed) -> MyRng {
    ///         MyRng(seed)
    ///     }
    /// }
    /// ```
    type Seed: Sized + Default + AsMut<[u8]>;

    /// Create a new PRNG using the given seed.
    ///
    /// PRNG implementations are allowed to assume that bits in the seed are
    /// well distributed. That means usually that the number of one and zero
    /// bits are roughly equal, and values like 0, 1 and (size - 1) are unlikely.
    /// Note that many non-cryptographic PRNGs will show poor quality output
    /// if this is not adhered to. If you wish to seed from simple numbers, use
    /// `seed_from_u64` instead.
    ///
    /// All PRNG implementations should be reproducible unless otherwise noted:
    /// given a fixed `seed`, the same sequence of output should be produced
    /// on all runs, library versions and architectures (e.g. check endianness).
    /// Any "value-breaking" changes to the generator should require bumping at
    /// least the minor version and documentation of the change.
    ///
    /// It is not required that this function yield the same state as a
    /// reference implementation of the PRNG given equivalent seed; if necessary
    /// another constructor replicating behaviour from a reference
    /// implementation can be added.
    ///
    /// PRNG implementations should make sure `from_seed` never panics. In the
    /// case that some special values (like an all zero seed) are not viable
    /// seeds it is preferable to map these to alternative constant value(s),
    /// for example `0xBAD5EEDu32` or `0x0DDB1A5E5BAD5EEDu64` ("odd biases? bad
    /// seed"). This is assuming only a small number of values must be rejected.
    fn from_seed(seed: Self::Seed) -> Self;

    /// Create a new PRNG using a `u64` seed.
    ///
    /// This is a convenience-wrapper around `from_seed` to allow construction
    /// of any `SeedableRng` from a simple `u64` value. It is designed such that
    /// low Hamming Weight numbers like 0 and 1 can be used and should still
    /// result in good, independent seeds to the PRNG which is returned.
    ///
    /// This **is not suitable for cryptography**, as should be clear given that
    /// the input size is only 64 bits.
    ///
    /// Implementations for PRNGs *may* provide their own implementations of
    /// this function, but the default implementation should be good enough for
    /// all purposes. *Changing* the implementation of this function should be
    /// considered a value-breaking change.
    fn seed_from_u64(mut state: u64) -> Self {
        // We use PCG32 to generate a u32 sequence, and copy to the seed
        fn pcg32(state: &mut u64) -> [u8; 4] {
            const MUL: u64 = 6364136223846793005;
            const INC: u64 = 11634580027462260723;

            // We advance the state first (to get away from the input value,
            // in case it has low Hamming Weight).
            *state = state.wrapping_mul(MUL).wrapping_add(INC);
            let state = *state;

            // Use PCG output function with to_le to generate x:
            let xorshifted = (((state >> 18) ^ state) >> 27) as u32;
            let rot = (state >> 59) as u32;
            let x = xorshifted.rotate_right(rot);
            x.to_le_bytes()
        }

        let mut seed = Self::Seed::default();
        let mut iter = seed.as_mut().chunks_exact_mut(4);
        for chunk in &mut iter {
            chunk.copy_from_slice(&pcg32(&mut state));
        }
        let rem = iter.into_remainder();
        if !rem.is_empty() {
            rem.copy_from_slice(&pcg32(&mut state)[..rem.len()]);
        }

        Self::from_seed(seed)
    }

    /// Create a new PRNG seeded from another `Rng`.
    ///
    /// This may be useful when needing to rapidly seed many PRNGs from a master
    /// PRNG, and to allow forking of PRNGs. It may be considered deterministic.
    ///
    /// The master PRNG should be at least as high quality as the child PRNGs.
    /// When seeding non-cryptographic child PRNGs, we recommend using a
    /// different algorithm for the master PRNG (ideally a CSPRNG) to avoid
    /// correlations between the child PRNGs. If this is not possible (e.g.
    /// forking using small non-crypto PRNGs) ensure that your PRNG has a good
    /// mixing function on the output or consider use of a hash function with
    /// `from_seed`.
    ///
    /// Note that seeding `XorShiftRng` from another `XorShiftRng` provides an
    /// extreme example of what can go wrong: the new PRNG will be a clone
    /// of the parent.
    ///
    /// PRNG implementations are allowed to assume that a good RNG is provided
    /// for seeding, and that it is cryptographically secure when appropriate.
    /// As of `rand` 0.7 / `rand_core` 0.5, implementations overriding this
    /// method should ensure the implementation satisfies reproducibility
    /// (in prior versions this was not required).
    ///
    /// [`rand`]: https://docs.rs/rand
    fn from_rng<R: RngCore>(mut rng: R) -> Result<Self, Error> {
        let mut seed = Self::Seed::default();
        rng.try_fill_bytes(seed.as_mut())?;
        Ok(Self::from_seed(seed))
    }

    /// Creates a new instance of the RNG seeded via [`getrandom`].
    ///
    /// This method is the recommended way to construct non-deterministic PRNGs
    /// since it is convenient and secure.
    ///
    /// In case the overhead of using [`getrandom`] to seed *many* PRNGs is an
    /// issue, one may prefer to seed from a local PRNG, e.g.
    /// `from_rng(thread_rng()).unwrap()`.
    ///
    /// # Panics
    ///
    /// If [`getrandom`] is unable to provide secure entropy this method will panic.
    ///
    /// [`getrandom`]: https://docs.rs/getrandom
    #[cfg(feature = "getrandom")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "getrandom")))]
    fn from_entropy() -> Self {
        let mut seed = Self::Seed::default();
        if let Err(err) = getrandom::getrandom(seed.as_mut()) {
            panic!("from_entropy failed: {}", err);
        }
        Self::from_seed(seed)
    }
}

// Implement `RngCore` for references to an `RngCore`.
// Force inlining all functions, so that it is up to the `RngCore`
// implementation and the optimizer to decide on inlining.
impl<'a, R: RngCore + ?Sized> RngCore for &'a mut R {
    #[inline(always)]
    fn next_u32(&mut self) -> u32 {
        (**self).next_u32()
    }

    #[inline(always)]
    fn next_u64(&mut self) -> u64 {
        (**self).next_u64()
    }

    #[inline(always)]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        (**self).fill_bytes(dest)
    }

    #[inline(always)]
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        (**self).try_fill_bytes(dest)
    }
}

// Implement `RngCore` for boxed references to an `RngCore`.
// Force inlining all functions, so that it is up to the `RngCore`
// implementation and the optimizer to decide on inlining.
#[cfg(feature = "alloc")]
impl<R: RngCore + ?Sized> RngCore for Box<R> {
    #[inline(always)]
    fn next_u32(&mut self) -> u32 {
        (**self).next_u32()
    }

    #[inline(always)]
    fn next_u64(&mut self) -> u64 {
        (**self).next_u64()
    }

    #[inline(always)]
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        (**self).fill_bytes(dest)
    }

    #[inline(always)]
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        (**self).try_fill_bytes(dest)
    }
}

#[cfg(feature = "std")]
impl std::io::Read for dyn RngCore {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        self.try_fill_bytes(buf)?;
        Ok(buf.len())
    }
}

// Implement `CryptoRng` for references to a `CryptoRng`.
impl<'a, R: CryptoRng + ?Sized> CryptoRng for &'a mut R {}

// Implement `CryptoRng` for boxed references to a `CryptoRng`.
#[cfg(feature = "alloc")]
impl<R: CryptoRng + ?Sized> CryptoRng for Box<R> {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_seed_from_u64() {
        struct SeedableNum(u64);
        impl SeedableRng for SeedableNum {
            type Seed = [u8; 8];

            fn from_seed(seed: Self::Seed) -> Self {
                let mut x = [0u64; 1];
                le::read_u64_into(&seed, &mut x);
                SeedableNum(x[0])
            }
        }

        const N: usize = 8;
        const SEEDS: [u64; N] = [0u64, 1, 2, 3, 4, 8, 16, -1i64 as u64];
        let mut results = [0u64; N];
        for (i, seed) in SEEDS.iter().enumerate() {
            let SeedableNum(x) = SeedableNum::seed_from_u64(*seed);
            results[i] = x;
        }

        for (i1, r1) in results.iter().enumerate() {
            let weight = r1.count_ones();
            // This is the binomial distribution B(64, 0.5), so chance of
            // weight < 20 is binocdf(19, 64, 0.5) = 7.8e-4, and same for
            // weight > 44.
            assert!((20..=44).contains(&weight));

            for (i2, r2) in results.iter().enumerate() {
                if i1 == i2 {
                    continue;
                }
                let diff_weight = (r1 ^ r2).count_ones();
                assert!(diff_weight >= 20);
            }
        }

        // value-breakage test:
        assert_eq!(results[0], 5029875928683246316);
    }
}
