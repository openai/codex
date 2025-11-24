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
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![no_std]

#[cfg(feature = "std")]
extern crate std;

use core::{fmt, ops::DerefMut};

pub mod block;
pub mod impls;
pub mod le;
#[cfg(feature = "os_rng")]
mod os;

#[cfg(feature = "os_rng")]
pub use os::{OsError, OsRng};

/// Implementation-level interface for RNGs
///
/// This trait encapsulates the low-level functionality common to all
/// generators, and is the "back end", to be implemented by generators.
/// End users should normally use the [`rand::Rng`] trait
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
/// Note that implementors of [`RngCore`] also automatically implement
/// the [`TryRngCore`] trait with the `Error` associated type being
/// equal to [`Infallible`].
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
/// use rand_core::{RngCore, impls};
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
///     fn fill_bytes(&mut self, dst: &mut [u8]) {
///         impls::fill_bytes_via_next(self, dst)
///     }
/// }
/// ```
///
/// [`rand::Rng`]: https://docs.rs/rand/latest/rand/trait.Rng.html
/// [`fill_bytes`]: RngCore::fill_bytes
/// [`next_u32`]: RngCore::next_u32
/// [`next_u64`]: RngCore::next_u64
/// [`Infallible`]: core::convert::Infallible
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
    /// via [`impls::fill_bytes_via_next`].
    ///
    /// This method should guarantee that `dest` is entirely filled
    /// with new data, and may panic if this is impossible
    /// (e.g. reading past the end of a file that is being used as the
    /// source of randomness).
    fn fill_bytes(&mut self, dst: &mut [u8]);
}

impl<T: DerefMut> RngCore for T
where
    T::Target: RngCore,
{
    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.deref_mut().next_u32()
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.deref_mut().next_u64()
    }

    #[inline]
    fn fill_bytes(&mut self, dst: &mut [u8]) {
        self.deref_mut().fill_bytes(dst);
    }
}

/// A marker trait over [`RngCore`] for securely unpredictable RNGs
///
/// This marker trait indicates that the implementing generator is intended,
/// when correctly seeded and protected from side-channel attacks such as a
/// leaking of state, to be a cryptographically secure generator. This trait is
/// provided as a tool to aid review of cryptographic code, but does not by
/// itself guarantee suitability for cryptographic applications.
///
/// Implementors of `CryptoRng` automatically implement the [`TryCryptoRng`]
/// trait.
///
/// Implementors of `CryptoRng` should only implement [`Default`] if the
/// `default()` instances are themselves secure generators: for example if the
/// implementing type is a stateless interface over a secure external generator
/// (like [`OsRng`]) or if the `default()` instance uses a strong, fresh seed.
///
/// Formally, a CSPRNG (Cryptographically Secure Pseudo-Random Number Generator)
/// should satisfy an additional property over other generators: assuming that
/// the generator has been appropriately seeded and has unknown state, then
/// given the first *k* bits of an algorithm's output
/// sequence, it should not be possible using polynomial-time algorithms to
/// predict the next bit with probability significantly greater than 50%.
///
/// An optional property of CSPRNGs is backtracking resistance: if the CSPRNG's
/// state is revealed, it will not be computationally-feasible to reconstruct
/// prior output values. This property is not required by `CryptoRng`.
pub trait CryptoRng: RngCore {}

impl<T: DerefMut> CryptoRng for T where T::Target: CryptoRng {}

/// A potentially fallible variant of [`RngCore`]
///
/// This trait is a generalization of [`RngCore`] to support potentially-
/// fallible IO-based generators such as [`OsRng`].
///
/// All implementations of [`RngCore`] automatically support this `TryRngCore`
/// trait, using [`Infallible`][core::convert::Infallible] as the associated
/// `Error` type.
///
/// An implementation of this trait may be made compatible with code requiring
/// an [`RngCore`] through [`TryRngCore::unwrap_err`]. The resulting RNG will
/// panic in case the underlying fallible RNG yields an error.
pub trait TryRngCore {
    /// The type returned in the event of a RNG error.
    type Error: fmt::Debug + fmt::Display;

    /// Return the next random `u32`.
    fn try_next_u32(&mut self) -> Result<u32, Self::Error>;
    /// Return the next random `u64`.
    fn try_next_u64(&mut self) -> Result<u64, Self::Error>;
    /// Fill `dest` entirely with random data.
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error>;

    /// Wrap RNG with the [`UnwrapErr`] wrapper.
    fn unwrap_err(self) -> UnwrapErr<Self>
    where
        Self: Sized,
    {
        UnwrapErr(self)
    }

    /// Wrap RNG with the [`UnwrapMut`] wrapper.
    fn unwrap_mut(&mut self) -> UnwrapMut<'_, Self> {
        UnwrapMut(self)
    }

    /// Convert an [`RngCore`] to a [`RngReadAdapter`].
    #[cfg(feature = "std")]
    fn read_adapter(&mut self) -> RngReadAdapter<'_, Self>
    where
        Self: Sized,
    {
        RngReadAdapter { inner: self }
    }
}

// Note that, unfortunately, this blanket impl prevents us from implementing
// `TryRngCore` for types which can be dereferenced to `TryRngCore`, i.e. `TryRngCore`
// will not be automatically implemented for `&mut R`, `Box<R>`, etc.
impl<R: RngCore + ?Sized> TryRngCore for R {
    type Error = core::convert::Infallible;

    #[inline]
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(self.next_u32())
    }

    #[inline]
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(self.next_u64())
    }

    #[inline]
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
        self.fill_bytes(dst);
        Ok(())
    }
}

/// A marker trait over [`TryRngCore`] for securely unpredictable RNGs
///
/// This trait is like [`CryptoRng`] but for the trait [`TryRngCore`].
///
/// This marker trait indicates that the implementing generator is intended,
/// when correctly seeded and protected from side-channel attacks such as a
/// leaking of state, to be a cryptographically secure generator. This trait is
/// provided as a tool to aid review of cryptographic code, but does not by
/// itself guarantee suitability for cryptographic applications.
///
/// Implementors of `TryCryptoRng` should only implement [`Default`] if the
/// `default()` instances are themselves secure generators: for example if the
/// implementing type is a stateless interface over a secure external generator
/// (like [`OsRng`]) or if the `default()` instance uses a strong, fresh seed.
pub trait TryCryptoRng: TryRngCore {}

impl<R: CryptoRng + ?Sized> TryCryptoRng for R {}

/// Wrapper around [`TryRngCore`] implementation which implements [`RngCore`]
/// by panicking on potential errors.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct UnwrapErr<R: TryRngCore>(pub R);

impl<R: TryRngCore> RngCore for UnwrapErr<R> {
    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.0.try_next_u32().unwrap()
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.0.try_next_u64().unwrap()
    }

    #[inline]
    fn fill_bytes(&mut self, dst: &mut [u8]) {
        self.0.try_fill_bytes(dst).unwrap()
    }
}

impl<R: TryCryptoRng> CryptoRng for UnwrapErr<R> {}

/// Wrapper around [`TryRngCore`] implementation which implements [`RngCore`]
/// by panicking on potential errors.
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct UnwrapMut<'r, R: TryRngCore + ?Sized>(pub &'r mut R);

impl<'r, R: TryRngCore + ?Sized> UnwrapMut<'r, R> {
    /// Reborrow with a new lifetime
    ///
    /// Rust allows references like `&T` or `&mut T` to be "reborrowed" through
    /// coercion: essentially, the pointer is copied under a new, shorter, lifetime.
    /// Until rfcs#1403 lands, reborrows on user types require a method call.
    #[inline(always)]
    pub fn re<'b>(&'b mut self) -> UnwrapMut<'b, R>
    where
        'r: 'b,
    {
        UnwrapMut(self.0)
    }
}

impl<R: TryRngCore + ?Sized> RngCore for UnwrapMut<'_, R> {
    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.0.try_next_u32().unwrap()
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.0.try_next_u64().unwrap()
    }

    #[inline]
    fn fill_bytes(&mut self, dst: &mut [u8]) {
        self.0.try_fill_bytes(dst).unwrap()
    }
}

impl<R: TryCryptoRng + ?Sized> CryptoRng for UnwrapMut<'_, R> {}

/// A random number generator that can be explicitly seeded.
///
/// This trait encapsulates the low-level functionality common to all
/// pseudo-random number generators (PRNGs, or algorithmic generators).
///
/// A generator implementing `SeedableRng` will usually be deterministic, but
/// beware that portability and reproducibility of results **is not implied**.
/// Refer to documentation of the generator, noting that generators named after
/// a specific algorithm are usually tested for reproducibility against a
/// reference vector, while `SmallRng` and `StdRng` specifically opt out of
/// reproducibility guarantees.
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
    /// Note that [`Default`] is not implemented for large arrays `[u8; N]` with
    /// `N` > 32. To be able to implement the traits required by `SeedableRng`
    /// for RNGs with such large seeds, the newtype pattern can be used:
    ///
    /// ```
    /// use rand_core::SeedableRng;
    ///
    /// const N: usize = 64;
    /// #[derive(Clone)]
    /// pub struct MyRngSeed(pub [u8; N]);
    /// # #[allow(dead_code)]
    /// pub struct MyRng(MyRngSeed);
    ///
    /// impl Default for MyRngSeed {
    ///     fn default() -> MyRngSeed {
    ///         MyRngSeed([0; N])
    ///     }
    /// }
    ///
    /// impl AsRef<[u8]> for MyRngSeed {
    ///     fn as_ref(&self) -> &[u8] {
    ///         &self.0
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
    type Seed: Clone + Default + AsRef<[u8]> + AsMut<[u8]>;

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

    /// Create a new PRNG seeded from an infallible `Rng`.
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
    fn from_rng(rng: &mut impl RngCore) -> Self {
        let mut seed = Self::Seed::default();
        rng.fill_bytes(seed.as_mut());
        Self::from_seed(seed)
    }

    /// Create a new PRNG seeded from a potentially fallible `Rng`.
    ///
    /// See [`from_rng`][SeedableRng::from_rng] docs for more information.
    fn try_from_rng<R: TryRngCore>(rng: &mut R) -> Result<Self, R::Error> {
        let mut seed = Self::Seed::default();
        rng.try_fill_bytes(seed.as_mut())?;
        Ok(Self::from_seed(seed))
    }

    /// Creates a new instance of the RNG seeded via [`getrandom`].
    ///
    /// This method is the recommended way to construct non-deterministic PRNGs
    /// since it is convenient and secure.
    ///
    /// Note that this method may panic on (extremely unlikely) [`getrandom`] errors.
    /// If it's not desirable, use the [`try_from_os_rng`] method instead.
    ///
    /// In case the overhead of using [`getrandom`] to seed *many* PRNGs is an
    /// issue, one may prefer to seed from a local PRNG, e.g.
    /// `from_rng(rand::rng()).unwrap()`.
    ///
    /// # Panics
    ///
    /// If [`getrandom`] is unable to provide secure entropy this method will panic.
    ///
    /// [`getrandom`]: https://docs.rs/getrandom
    /// [`try_from_os_rng`]: SeedableRng::try_from_os_rng
    #[cfg(feature = "os_rng")]
    fn from_os_rng() -> Self {
        match Self::try_from_os_rng() {
            Ok(res) => res,
            Err(err) => panic!("from_os_rng failed: {}", err),
        }
    }

    /// Creates a new instance of the RNG seeded via [`getrandom`] without unwrapping
    /// potential [`getrandom`] errors.
    ///
    /// In case the overhead of using [`getrandom`] to seed *many* PRNGs is an
    /// issue, one may prefer to seed from a local PRNG, e.g.
    /// `from_rng(&mut rand::rng()).unwrap()`.
    ///
    /// [`getrandom`]: https://docs.rs/getrandom
    #[cfg(feature = "os_rng")]
    fn try_from_os_rng() -> Result<Self, getrandom::Error> {
        let mut seed = Self::Seed::default();
        getrandom::fill(seed.as_mut())?;
        let res = Self::from_seed(seed);
        Ok(res)
    }
}

/// Adapter that enables reading through a [`io::Read`](std::io::Read) from a [`RngCore`].
///
/// # Examples
///
/// ```no_run
/// # use std::{io, io::Read};
/// # use std::fs::File;
/// # use rand_core::{OsRng, TryRngCore};
///
/// io::copy(&mut OsRng.read_adapter().take(100), &mut File::create("/tmp/random.bytes").unwrap()).unwrap();
/// ```
#[cfg(feature = "std")]
pub struct RngReadAdapter<'a, R: TryRngCore + ?Sized> {
    inner: &'a mut R,
}

#[cfg(feature = "std")]
impl<R: TryRngCore + ?Sized> std::io::Read for RngReadAdapter<'_, R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, std::io::Error> {
        self.inner.try_fill_bytes(buf).map_err(|err| {
            std::io::Error::new(std::io::ErrorKind::Other, std::format!("RNG error: {err}"))
        })?;
        Ok(buf.len())
    }
}

#[cfg(feature = "std")]
impl<R: TryRngCore + ?Sized> std::fmt::Debug for RngReadAdapter<'_, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReadAdapter").finish()
    }
}

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

    // A stub RNG.
    struct SomeRng;

    impl RngCore for SomeRng {
        fn next_u32(&mut self) -> u32 {
            unimplemented!()
        }
        fn next_u64(&mut self) -> u64 {
            unimplemented!()
        }
        fn fill_bytes(&mut self, _: &mut [u8]) {
            unimplemented!()
        }
    }

    impl CryptoRng for SomeRng {}

    #[test]
    fn dyn_rngcore_to_tryrngcore() {
        // Illustrates the need for `+ ?Sized` bound in `impl<R: RngCore> TryRngCore for R`.

        // A method in another crate taking a fallible RNG
        fn third_party_api(_rng: &mut (impl TryRngCore + ?Sized)) -> bool {
            true
        }

        // A method in our crate requiring an infallible RNG
        fn my_api(rng: &mut dyn RngCore) -> bool {
            // We want to call the method above
            third_party_api(rng)
        }

        assert!(my_api(&mut SomeRng));
    }

    #[test]
    fn dyn_cryptorng_to_trycryptorng() {
        // Illustrates the need for `+ ?Sized` bound in `impl<R: CryptoRng> TryCryptoRng for R`.

        // A method in another crate taking a fallible RNG
        fn third_party_api(_rng: &mut (impl TryCryptoRng + ?Sized)) -> bool {
            true
        }

        // A method in our crate requiring an infallible RNG
        fn my_api(rng: &mut dyn CryptoRng) -> bool {
            // We want to call the method above
            third_party_api(rng)
        }

        assert!(my_api(&mut SomeRng));
    }

    #[test]
    fn dyn_unwrap_mut_tryrngcore() {
        // Illustrates the need for `+ ?Sized` bound in
        // `impl<R: TryRngCore> RngCore for UnwrapMut<'_, R>`.

        fn third_party_api(_rng: &mut impl RngCore) -> bool {
            true
        }

        fn my_api(rng: &mut (impl TryRngCore + ?Sized)) -> bool {
            let mut infallible_rng = rng.unwrap_mut();
            third_party_api(&mut infallible_rng)
        }

        assert!(my_api(&mut SomeRng));
    }

    #[test]
    fn dyn_unwrap_mut_trycryptorng() {
        // Illustrates the need for `+ ?Sized` bound in
        // `impl<R: TryCryptoRng> CryptoRng for UnwrapMut<'_, R>`.

        fn third_party_api(_rng: &mut impl CryptoRng) -> bool {
            true
        }

        fn my_api(rng: &mut (impl TryCryptoRng + ?Sized)) -> bool {
            let mut infallible_rng = rng.unwrap_mut();
            third_party_api(&mut infallible_rng)
        }

        assert!(my_api(&mut SomeRng));
    }

    #[test]
    fn reborrow_unwrap_mut() {
        struct FourRng;

        impl TryRngCore for FourRng {
            type Error = core::convert::Infallible;
            fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
                Ok(4)
            }
            fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
                unimplemented!()
            }
            fn try_fill_bytes(&mut self, _: &mut [u8]) -> Result<(), Self::Error> {
                unimplemented!()
            }
        }

        let mut rng = FourRng;
        let mut rng = rng.unwrap_mut();

        assert_eq!(rng.next_u32(), 4);
        let mut rng2 = rng.re();
        assert_eq!(rng2.next_u32(), 4);
        drop(rng2);
        assert_eq!(rng.next_u32(), 4);
    }
}
