// Copyright 2018 Developers of the Rand project.
// Copyright 2013-2017 The Rust Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! [`Rng`] trait

use crate::distr::uniform::{SampleRange, SampleUniform};
use crate::distr::{self, Distribution, StandardUniform};
use core::num::Wrapping;
use core::{mem, slice};
use rand_core::RngCore;

/// User-level interface for RNGs
///
/// [`RngCore`] is the `dyn`-safe implementation-level interface for Random
/// (Number) Generators. This trait, `Rng`, provides a user-level interface on
/// RNGs. It is implemented automatically for any `R: RngCore`.
///
/// This trait must usually be brought into scope via `use rand::Rng;` or
/// `use rand::prelude::*;`.
///
/// # Generic usage
///
/// The basic pattern is `fn foo<R: Rng + ?Sized>(rng: &mut R)`. Some
/// things are worth noting here:
///
/// - Since `Rng: RngCore` and every `RngCore` implements `Rng`, it makes no
///   difference whether we use `R: Rng` or `R: RngCore`.
/// - The `+ ?Sized` un-bounding allows functions to be called directly on
///   type-erased references; i.e. `foo(r)` where `r: &mut dyn RngCore`. Without
///   this it would be necessary to write `foo(&mut r)`.
///
/// An alternative pattern is possible: `fn foo<R: Rng>(rng: R)`. This has some
/// trade-offs. It allows the argument to be consumed directly without a `&mut`
/// (which is how `from_rng(rand::rng())` works); also it still works directly
/// on references (including type-erased references). Unfortunately within the
/// function `foo` it is not known whether `rng` is a reference type or not,
/// hence many uses of `rng` require an extra reference, either explicitly
/// (`distr.sample(&mut rng)`) or implicitly (`rng.random()`); one may hope the
/// optimiser can remove redundant references later.
///
/// Example:
///
/// ```
/// use rand::Rng;
///
/// fn foo<R: Rng + ?Sized>(rng: &mut R) -> f32 {
///     rng.random()
/// }
///
/// # let v = foo(&mut rand::rng());
/// ```
pub trait Rng: RngCore {
    /// Return a random value via the [`StandardUniform`] distribution.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::Rng;
    ///
    /// let mut rng = rand::rng();
    /// let x: u32 = rng.random();
    /// println!("{}", x);
    /// println!("{:?}", rng.random::<(f64, bool)>());
    /// ```
    ///
    /// # Arrays and tuples
    ///
    /// The `rng.random()` method is able to generate arrays
    /// and tuples (up to 12 elements), so long as all element types can be
    /// generated.
    ///
    /// For arrays of integers, especially for those with small element types
    /// (< 64 bit), it will likely be faster to instead use [`Rng::fill`],
    /// though note that generated values will differ.
    ///
    /// ```
    /// use rand::Rng;
    ///
    /// let mut rng = rand::rng();
    /// let tuple: (u8, i32, char) = rng.random(); // arbitrary tuple support
    ///
    /// let arr1: [f32; 32] = rng.random();        // array construction
    /// let mut arr2 = [0u8; 128];
    /// rng.fill(&mut arr2);                    // array fill
    /// ```
    ///
    /// [`StandardUniform`]: distr::StandardUniform
    #[inline]
    fn random<T>(&mut self) -> T
    where
        StandardUniform: Distribution<T>,
    {
        StandardUniform.sample(self)
    }

    /// Return an iterator over [`random`](Self::random) variates
    ///
    /// This is a just a wrapper over [`Rng::sample_iter`] using
    /// [`distr::StandardUniform`].
    ///
    /// Note: this method consumes its argument. Use
    /// `(&mut rng).random_iter()` to avoid consuming the RNG.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::{rngs::SmallRng, Rng, SeedableRng};
    ///
    /// let rng = SmallRng::seed_from_u64(0);
    /// let v: Vec<i32> = rng.random_iter().take(5).collect();
    /// assert_eq!(v.len(), 5);
    /// ```
    #[inline]
    fn random_iter<T>(self) -> distr::Iter<StandardUniform, Self, T>
    where
        Self: Sized,
        StandardUniform: Distribution<T>,
    {
        StandardUniform.sample_iter(self)
    }

    /// Generate a random value in the given range.
    ///
    /// This function is optimised for the case that only a single sample is
    /// made from the given range. See also the [`Uniform`] distribution
    /// type which may be faster if sampling from the same range repeatedly.
    ///
    /// All types support `low..high_exclusive` and `low..=high` range syntax.
    /// Unsigned integer types also support `..high_exclusive` and `..=high` syntax.
    ///
    /// # Panics
    ///
    /// Panics if the range is empty, or if `high - low` overflows for floats.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::Rng;
    ///
    /// let mut rng = rand::rng();
    ///
    /// // Exclusive range
    /// let n: u32 = rng.random_range(..10);
    /// println!("{}", n);
    /// let m: f64 = rng.random_range(-40.0..1.3e5);
    /// println!("{}", m);
    ///
    /// // Inclusive range
    /// let n: u32 = rng.random_range(..=10);
    /// println!("{}", n);
    /// ```
    ///
    /// [`Uniform`]: distr::uniform::Uniform
    #[track_caller]
    fn random_range<T, R>(&mut self, range: R) -> T
    where
        T: SampleUniform,
        R: SampleRange<T>,
    {
        assert!(!range.is_empty(), "cannot sample empty range");
        range.sample_single(self).unwrap()
    }

    /// Return a bool with a probability `p` of being true.
    ///
    /// See also the [`Bernoulli`] distribution, which may be faster if
    /// sampling from the same probability repeatedly.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::Rng;
    ///
    /// let mut rng = rand::rng();
    /// println!("{}", rng.random_bool(1.0 / 3.0));
    /// ```
    ///
    /// # Panics
    ///
    /// If `p < 0` or `p > 1`.
    ///
    /// [`Bernoulli`]: distr::Bernoulli
    #[inline]
    #[track_caller]
    fn random_bool(&mut self, p: f64) -> bool {
        match distr::Bernoulli::new(p) {
            Ok(d) => self.sample(d),
            Err(_) => panic!("p={:?} is outside range [0.0, 1.0]", p),
        }
    }

    /// Return a bool with a probability of `numerator/denominator` of being
    /// true.
    ///
    /// That is, `random_ratio(2, 3)` has chance of 2 in 3, or about 67%, of
    /// returning true. If `numerator == denominator`, then the returned value
    /// is guaranteed to be `true`. If `numerator == 0`, then the returned
    /// value is guaranteed to be `false`.
    ///
    /// See also the [`Bernoulli`] distribution, which may be faster if
    /// sampling from the same `numerator` and `denominator` repeatedly.
    ///
    /// # Panics
    ///
    /// If `denominator == 0` or `numerator > denominator`.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::Rng;
    ///
    /// let mut rng = rand::rng();
    /// println!("{}", rng.random_ratio(2, 3));
    /// ```
    ///
    /// [`Bernoulli`]: distr::Bernoulli
    #[inline]
    #[track_caller]
    fn random_ratio(&mut self, numerator: u32, denominator: u32) -> bool {
        match distr::Bernoulli::from_ratio(numerator, denominator) {
            Ok(d) => self.sample(d),
            Err(_) => panic!(
                "p={}/{} is outside range [0.0, 1.0]",
                numerator, denominator
            ),
        }
    }

    /// Sample a new value, using the given distribution.
    ///
    /// ### Example
    ///
    /// ```
    /// use rand::Rng;
    /// use rand::distr::Uniform;
    ///
    /// let mut rng = rand::rng();
    /// let x = rng.sample(Uniform::new(10u32, 15).unwrap());
    /// // Type annotation requires two types, the type and distribution; the
    /// // distribution can be inferred.
    /// let y = rng.sample::<u16, _>(Uniform::new(10, 15).unwrap());
    /// ```
    fn sample<T, D: Distribution<T>>(&mut self, distr: D) -> T {
        distr.sample(self)
    }

    /// Create an iterator that generates values using the given distribution.
    ///
    /// Note: this method consumes its arguments. Use
    /// `(&mut rng).sample_iter(..)` to avoid consuming the RNG.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::Rng;
    /// use rand::distr::{Alphanumeric, Uniform, StandardUniform};
    ///
    /// let mut rng = rand::rng();
    ///
    /// // Vec of 16 x f32:
    /// let v: Vec<f32> = (&mut rng).sample_iter(StandardUniform).take(16).collect();
    ///
    /// // String:
    /// let s: String = (&mut rng).sample_iter(Alphanumeric)
    ///     .take(7)
    ///     .map(char::from)
    ///     .collect();
    ///
    /// // Combined values
    /// println!("{:?}", (&mut rng).sample_iter(StandardUniform).take(5)
    ///                              .collect::<Vec<(f64, bool)>>());
    ///
    /// // Dice-rolling:
    /// let die_range = Uniform::new_inclusive(1, 6).unwrap();
    /// let mut roll_die = (&mut rng).sample_iter(die_range);
    /// while roll_die.next().unwrap() != 6 {
    ///     println!("Not a 6; rolling again!");
    /// }
    /// ```
    fn sample_iter<T, D>(self, distr: D) -> distr::Iter<D, Self, T>
    where
        D: Distribution<T>,
        Self: Sized,
    {
        distr.sample_iter(self)
    }

    /// Fill any type implementing [`Fill`] with random data
    ///
    /// This method is implemented for types which may be safely reinterpreted
    /// as an (aligned) `[u8]` slice then filled with random data. It is often
    /// faster than using [`Rng::random`] but not value-equivalent.
    ///
    /// The distribution is expected to be uniform with portable results, but
    /// this cannot be guaranteed for third-party implementations.
    ///
    /// # Example
    ///
    /// ```
    /// use rand::Rng;
    ///
    /// let mut arr = [0i8; 20];
    /// rand::rng().fill(&mut arr[..]);
    /// ```
    ///
    /// [`fill_bytes`]: RngCore::fill_bytes
    #[track_caller]
    fn fill<T: Fill + ?Sized>(&mut self, dest: &mut T) {
        dest.fill(self)
    }

    /// Alias for [`Rng::random`].
    #[inline]
    #[deprecated(
        since = "0.9.0",
        note = "Renamed to `random` to avoid conflict with the new `gen` keyword in Rust 2024."
    )]
    fn r#gen<T>(&mut self) -> T
    where
        StandardUniform: Distribution<T>,
    {
        self.random()
    }

    /// Alias for [`Rng::random_range`].
    #[inline]
    #[deprecated(since = "0.9.0", note = "Renamed to `random_range`")]
    fn gen_range<T, R>(&mut self, range: R) -> T
    where
        T: SampleUniform,
        R: SampleRange<T>,
    {
        self.random_range(range)
    }

    /// Alias for [`Rng::random_bool`].
    #[inline]
    #[deprecated(since = "0.9.0", note = "Renamed to `random_bool`")]
    fn gen_bool(&mut self, p: f64) -> bool {
        self.random_bool(p)
    }

    /// Alias for [`Rng::random_ratio`].
    #[inline]
    #[deprecated(since = "0.9.0", note = "Renamed to `random_ratio`")]
    fn gen_ratio(&mut self, numerator: u32, denominator: u32) -> bool {
        self.random_ratio(numerator, denominator)
    }
}

impl<R: RngCore + ?Sized> Rng for R {}

/// Types which may be filled with random data
///
/// This trait allows arrays to be efficiently filled with random data.
///
/// Implementations are expected to be portable across machines unless
/// clearly documented otherwise (see the
/// [Chapter on Portability](https://rust-random.github.io/book/portability.html)).
pub trait Fill {
    /// Fill self with random data
    fn fill<R: Rng + ?Sized>(&mut self, rng: &mut R);
}

macro_rules! impl_fill_each {
    () => {};
    ($t:ty) => {
        impl Fill for [$t] {
            fn fill<R: Rng + ?Sized>(&mut self, rng: &mut R) {
                for elt in self.iter_mut() {
                    *elt = rng.random();
                }
            }
        }
    };
    ($t:ty, $($tt:ty,)*) => {
        impl_fill_each!($t);
        impl_fill_each!($($tt,)*);
    };
}

impl_fill_each!(bool, char, f32, f64,);

impl Fill for [u8] {
    fn fill<R: Rng + ?Sized>(&mut self, rng: &mut R) {
        rng.fill_bytes(self)
    }
}

/// Call target for unsafe macros
const unsafe fn __unsafe() {}

/// Implement `Fill` for given type `$t`.
///
/// # Safety
/// All bit patterns of `[u8; size_of::<$t>()]` must represent values of `$t`.
macro_rules! impl_fill {
    () => {};
    ($t:ty) => {{
        // Force caller to wrap with an `unsafe` block
        __unsafe();

        impl Fill for [$t] {
            fn fill<R: Rng + ?Sized>(&mut self, rng: &mut R) {
                if self.len() > 0 {
                    let size = mem::size_of_val(self);
                    rng.fill_bytes(
                        // SAFETY: `self` non-null and valid for reads and writes within its `size`
                        // bytes. `self` meets the alignment requirements of `&mut [u8]`.
                        // The contents of `self` are initialized. Both `[u8]` and `[$t]` are valid
                        // for all bit-patterns of their contents (note that the SAFETY requirement
                        // on callers of this macro). `self` is not borrowed.
                        unsafe {
                            slice::from_raw_parts_mut(self.as_mut_ptr()
                                as *mut u8,
                                size
                            )
                        }
                    );
                    for x in self {
                        *x = x.to_le();
                    }
                }
            }
        }

        impl Fill for [Wrapping<$t>] {
            fn fill<R: Rng + ?Sized>(&mut self, rng: &mut R) {
                if self.len() > 0 {
                    let size = self.len() * mem::size_of::<$t>();
                    rng.fill_bytes(
                        // SAFETY: `self` non-null and valid for reads and writes within its `size`
                        // bytes. `self` meets the alignment requirements of `&mut [u8]`.
                        // The contents of `self` are initialized. Both `[u8]` and `[$t]` are valid
                        // for all bit-patterns of their contents (note that the SAFETY requirement
                        // on callers of this macro). `self` is not borrowed.
                        unsafe {
                            slice::from_raw_parts_mut(self.as_mut_ptr()
                                as *mut u8,
                                size
                            )
                        }
                    );
                    for x in self {
                        *x = Wrapping(x.0.to_le());
                    }
                }
            }
        }}
    };
    ($t:ty, $($tt:ty,)*) => {{
        impl_fill!($t);
        // TODO: this could replace above impl once Rust #32463 is fixed
        // impl_fill!(Wrapping<$t>);
        impl_fill!($($tt,)*);
    }}
}

// SAFETY: All bit patterns of `[u8; size_of::<$t>()]` represent values of `u*`.
const _: () = unsafe { impl_fill!(u16, u32, u64, u128,) };
// SAFETY: All bit patterns of `[u8; size_of::<$t>()]` represent values of `i*`.
const _: () = unsafe { impl_fill!(i8, i16, i32, i64, i128,) };

impl<T, const N: usize> Fill for [T; N]
where
    [T]: Fill,
{
    fn fill<R: Rng + ?Sized>(&mut self, rng: &mut R) {
        <[T] as Fill>::fill(self, rng)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::{const_rng, rng};
    #[cfg(feature = "alloc")]
    use alloc::boxed::Box;

    #[test]
    fn test_fill_bytes_default() {
        let mut r = const_rng(0x11_22_33_44_55_66_77_88);

        // check every remainder mod 8, both in small and big vectors.
        let lengths = [0, 1, 2, 3, 4, 5, 6, 7, 80, 81, 82, 83, 84, 85, 86, 87];
        for &n in lengths.iter() {
            let mut buffer = [0u8; 87];
            let v = &mut buffer[0..n];
            r.fill_bytes(v);

            // use this to get nicer error messages.
            for (i, &byte) in v.iter().enumerate() {
                if byte == 0 {
                    panic!("byte {} of {} is zero", i, n)
                }
            }
        }
    }

    #[test]
    fn test_fill() {
        let x = 9041086907909331047; // a random u64
        let mut rng = const_rng(x);

        // Convert to byte sequence and back to u64; byte-swap twice if BE.
        let mut array = [0u64; 2];
        rng.fill(&mut array[..]);
        assert_eq!(array, [x, x]);
        assert_eq!(rng.next_u64(), x);

        // Convert to bytes then u32 in LE order
        let mut array = [0u32; 2];
        rng.fill(&mut array[..]);
        assert_eq!(array, [x as u32, (x >> 32) as u32]);
        assert_eq!(rng.next_u32(), x as u32);

        // Check equivalence using wrapped arrays
        let mut warray = [Wrapping(0u32); 2];
        rng.fill(&mut warray[..]);
        assert_eq!(array[0], warray[0].0);
        assert_eq!(array[1], warray[1].0);

        // Check equivalence for generated floats
        let mut array = [0f32; 2];
        rng.fill(&mut array);
        let arr2: [f32; 2] = rng.random();
        assert_eq!(array, arr2);
    }

    #[test]
    fn test_fill_empty() {
        let mut array = [0u32; 0];
        let mut rng = rng(1);
        rng.fill(&mut array);
        rng.fill(&mut array[..]);
    }

    #[test]
    fn test_random_range_int() {
        let mut r = rng(101);
        for _ in 0..1000 {
            let a = r.random_range(-4711..17);
            assert!((-4711..17).contains(&a));
            let a: i8 = r.random_range(-3..42);
            assert!((-3..42).contains(&a));
            let a: u16 = r.random_range(10..99);
            assert!((10..99).contains(&a));
            let a: i32 = r.random_range(-100..2000);
            assert!((-100..2000).contains(&a));
            let a: u32 = r.random_range(12..=24);
            assert!((12..=24).contains(&a));

            assert_eq!(r.random_range(..1u32), 0u32);
            assert_eq!(r.random_range(-12i64..-11), -12i64);
            assert_eq!(r.random_range(3_000_000..3_000_001), 3_000_000);
        }
    }

    #[test]
    fn test_random_range_float() {
        let mut r = rng(101);
        for _ in 0..1000 {
            let a = r.random_range(-4.5..1.7);
            assert!((-4.5..1.7).contains(&a));
            let a = r.random_range(-1.1..=-0.3);
            assert!((-1.1..=-0.3).contains(&a));

            assert_eq!(r.random_range(0.0f32..=0.0), 0.);
            assert_eq!(r.random_range(-11.0..=-11.0), -11.);
            assert_eq!(r.random_range(3_000_000.0..=3_000_000.0), 3_000_000.);
        }
    }

    #[test]
    #[should_panic]
    #[allow(clippy::reversed_empty_ranges)]
    fn test_random_range_panic_int() {
        let mut r = rng(102);
        r.random_range(5..-2);
    }

    #[test]
    #[should_panic]
    #[allow(clippy::reversed_empty_ranges)]
    fn test_random_range_panic_usize() {
        let mut r = rng(103);
        r.random_range(5..2);
    }

    #[test]
    #[allow(clippy::bool_assert_comparison)]
    fn test_random_bool() {
        let mut r = rng(105);
        for _ in 0..5 {
            assert_eq!(r.random_bool(0.0), false);
            assert_eq!(r.random_bool(1.0), true);
        }
    }

    #[test]
    fn test_rng_mut_ref() {
        fn use_rng(mut r: impl Rng) {
            let _ = r.next_u32();
        }

        let mut rng = rng(109);
        use_rng(&mut rng);
    }

    #[test]
    fn test_rng_trait_object() {
        use crate::distr::{Distribution, StandardUniform};
        let mut rng = rng(109);
        let mut r = &mut rng as &mut dyn RngCore;
        r.next_u32();
        r.random::<i32>();
        assert_eq!(r.random_range(0..1), 0);
        let _c: u8 = StandardUniform.sample(&mut r);
    }

    #[test]
    #[cfg(feature = "alloc")]
    fn test_rng_boxed_trait() {
        use crate::distr::{Distribution, StandardUniform};
        let rng = rng(110);
        let mut r = Box::new(rng) as Box<dyn RngCore>;
        r.next_u32();
        r.random::<i32>();
        assert_eq!(r.random_range(0..1), 0);
        let _c: u8 = StandardUniform.sample(&mut r);
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_gen_ratio_average() {
        const NUM: u32 = 3;
        const DENOM: u32 = 10;
        const N: u32 = 100_000;

        let mut sum: u32 = 0;
        let mut rng = rng(111);
        for _ in 0..N {
            if rng.random_ratio(NUM, DENOM) {
                sum += 1;
            }
        }
        // Have Binomial(N, NUM/DENOM) distribution
        let expected = (NUM * N) / DENOM; // exact integer
        assert!(((sum - expected) as i32).abs() < 500);
    }
}
