// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The Bernoulli distribution `Bernoulli(p)`.

use crate::distr::Distribution;
use crate::Rng;
use core::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// The [Bernoulli distribution](https://en.wikipedia.org/wiki/Bernoulli_distribution) `Bernoulli(p)`.
///
/// This distribution describes a single boolean random variable, which is true
/// with probability `p` and false with probability `1 - p`.
/// It is a special case of the Binomial distribution with `n = 1`.
///
/// # Plot
///
/// The following plot shows the Bernoulli distribution with `p = 0.1`,
/// `p = 0.5`, and `p = 0.9`.
///
/// ![Bernoulli distribution](https://raw.githubusercontent.com/rust-random/charts/main/charts/bernoulli.svg)
///
/// # Example
///
/// ```rust
/// use rand::distr::{Bernoulli, Distribution};
///
/// let d = Bernoulli::new(0.3).unwrap();
/// let v = d.sample(&mut rand::rng());
/// println!("{} is from a Bernoulli distribution", v);
/// ```
///
/// # Precision
///
/// This `Bernoulli` distribution uses 64 bits from the RNG (a `u64`),
/// so only probabilities that are multiples of 2<sup>-64</sup> can be
/// represented.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Bernoulli {
    /// Probability of success, relative to the maximal integer.
    p_int: u64,
}

// To sample from the Bernoulli distribution we use a method that compares a
// random `u64` value `v < (p * 2^64)`.
//
// If `p == 1.0`, the integer `v` to compare against can not represented as a
// `u64`. We manually set it to `u64::MAX` instead (2^64 - 1 instead of 2^64).
// Note that  value of `p < 1.0` can never result in `u64::MAX`, because an
// `f64` only has 53 bits of precision, and the next largest value of `p` will
// result in `2^64 - 2048`.
//
// Also there is a 100% theoretical concern: if someone consistently wants to
// generate `true` using the Bernoulli distribution (i.e. by using a probability
// of `1.0`), just using `u64::MAX` is not enough. On average it would return
// false once every 2^64 iterations. Some people apparently care about this
// case.
//
// That is why we special-case `u64::MAX` to always return `true`, without using
// the RNG, and pay the performance price for all uses that *are* reasonable.
// Luckily, if `new()` and `sample` are close, the compiler can optimize out the
// extra check.
const ALWAYS_TRUE: u64 = u64::MAX;

// This is just `2.0.powi(64)`, but written this way because it is not available
// in `no_std` mode.
const SCALE: f64 = 2.0 * (1u64 << 63) as f64;

/// Error type returned from [`Bernoulli::new`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BernoulliError {
    /// `p < 0` or `p > 1`.
    InvalidProbability,
}

impl fmt::Display for BernoulliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            BernoulliError::InvalidProbability => "p is outside [0, 1] in Bernoulli distribution",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for BernoulliError {}

impl Bernoulli {
    /// Construct a new `Bernoulli` with the given probability of success `p`.
    ///
    /// # Precision
    ///
    /// For `p = 1.0`, the resulting distribution will always generate true.
    /// For `p = 0.0`, the resulting distribution will always generate false.
    ///
    /// This method is accurate for any input `p` in the range `[0, 1]` which is
    /// a multiple of 2<sup>-64</sup>. (Note that not all multiples of
    /// 2<sup>-64</sup> in `[0, 1]` can be represented as a `f64`.)
    #[inline]
    pub fn new(p: f64) -> Result<Bernoulli, BernoulliError> {
        if !(0.0..1.0).contains(&p) {
            if p == 1.0 {
                return Ok(Bernoulli { p_int: ALWAYS_TRUE });
            }
            return Err(BernoulliError::InvalidProbability);
        }
        Ok(Bernoulli {
            p_int: (p * SCALE) as u64,
        })
    }

    /// Construct a new `Bernoulli` with the probability of success of
    /// `numerator`-in-`denominator`. I.e. `new_ratio(2, 3)` will return
    /// a `Bernoulli` with a 2-in-3 chance, or about 67%, of returning `true`.
    ///
    /// return `true`. If `numerator == 0` it will always return `false`.
    /// For `numerator > denominator` and `denominator == 0`, this returns an
    /// error. Otherwise, for `numerator == denominator`, samples are always
    /// true; for `numerator == 0` samples are always false.
    #[inline]
    pub fn from_ratio(numerator: u32, denominator: u32) -> Result<Bernoulli, BernoulliError> {
        if numerator > denominator || denominator == 0 {
            return Err(BernoulliError::InvalidProbability);
        }
        if numerator == denominator {
            return Ok(Bernoulli { p_int: ALWAYS_TRUE });
        }
        let p_int = ((f64::from(numerator) / f64::from(denominator)) * SCALE) as u64;
        Ok(Bernoulli { p_int })
    }

    #[inline]
    /// Returns the probability (`p`) of the distribution.
    ///
    /// This value may differ slightly from the input due to loss of precision.
    pub fn p(&self) -> f64 {
        if self.p_int == ALWAYS_TRUE {
            1.0
        } else {
            (self.p_int as f64) / SCALE
        }
    }
}

impl Distribution<bool> for Bernoulli {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> bool {
        // Make sure to always return true for p = 1.0.
        if self.p_int == ALWAYS_TRUE {
            return true;
        }
        let v: u64 = rng.random();
        v < self.p_int
    }
}

#[cfg(test)]
mod test {
    use super::Bernoulli;
    use crate::distr::Distribution;
    use crate::Rng;

    #[test]
    #[cfg(feature = "serde")]
    fn test_serializing_deserializing_bernoulli() {
        let coin_flip = Bernoulli::new(0.5).unwrap();
        let de_coin_flip: Bernoulli =
            bincode::deserialize(&bincode::serialize(&coin_flip).unwrap()).unwrap();

        assert_eq!(coin_flip.p_int, de_coin_flip.p_int);
    }

    #[test]
    fn test_trivial() {
        // We prefer to be explicit here.
        #![allow(clippy::bool_assert_comparison)]

        let mut r = crate::test::rng(1);
        let always_false = Bernoulli::new(0.0).unwrap();
        let always_true = Bernoulli::new(1.0).unwrap();
        for _ in 0..5 {
            assert_eq!(r.sample::<bool, _>(&always_false), false);
            assert_eq!(r.sample::<bool, _>(&always_true), true);
            assert_eq!(Distribution::<bool>::sample(&always_false, &mut r), false);
            assert_eq!(Distribution::<bool>::sample(&always_true, &mut r), true);
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)] // Miri is too slow
    fn test_average() {
        const P: f64 = 0.3;
        const NUM: u32 = 3;
        const DENOM: u32 = 10;
        let d1 = Bernoulli::new(P).unwrap();
        let d2 = Bernoulli::from_ratio(NUM, DENOM).unwrap();
        const N: u32 = 100_000;

        let mut sum1: u32 = 0;
        let mut sum2: u32 = 0;
        let mut rng = crate::test::rng(2);
        for _ in 0..N {
            if d1.sample(&mut rng) {
                sum1 += 1;
            }
            if d2.sample(&mut rng) {
                sum2 += 1;
            }
        }
        let avg1 = (sum1 as f64) / (N as f64);
        assert!((avg1 - P).abs() < 5e-3);

        let avg2 = (sum2 as f64) / (N as f64);
        assert!((avg2 - (NUM as f64) / (DENOM as f64)).abs() < 5e-3);
    }

    #[test]
    fn value_stability() {
        let mut rng = crate::test::rng(3);
        let distr = Bernoulli::new(0.4532).unwrap();
        let mut buf = [false; 10];
        for x in &mut buf {
            *x = rng.sample(distr);
        }
        assert_eq!(
            buf,
            [true, false, false, true, false, false, true, true, true, true]
        );
    }

    #[test]
    fn bernoulli_distributions_can_be_compared() {
        assert_eq!(Bernoulli::new(1.0), Bernoulli::new(1.0));
    }
}
