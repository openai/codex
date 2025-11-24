//! Randomization of big integers
#![cfg(feature = "rand")]
#![cfg_attr(docsrs, doc(cfg(feature = "rand")))]

use rand::distributions::uniform::{SampleBorrow, SampleUniform, UniformSampler};
use rand::prelude::*;

use crate::BigInt;
use crate::BigUint;
use crate::Sign::*;

use crate::biguint::biguint_from_vec;

use num_integer::Integer;
use num_traits::{ToPrimitive, Zero};

/// A trait for sampling random big integers.
///
/// The `rand` feature must be enabled to use this. See crate-level documentation for details.
pub trait RandBigInt {
    /// Generate a random [`BigUint`] of the given bit size.
    fn gen_biguint(&mut self, bit_size: u64) -> BigUint;

    /// Generate a random [ BigInt`] of the given bit size.
    fn gen_bigint(&mut self, bit_size: u64) -> BigInt;

    /// Generate a random [`BigUint`] less than the given bound. Fails
    /// when the bound is zero.
    fn gen_biguint_below(&mut self, bound: &BigUint) -> BigUint;

    /// Generate a random [`BigUint`] within the given range. The lower
    /// bound is inclusive; the upper bound is exclusive. Fails when
    /// the upper bound is not greater than the lower bound.
    fn gen_biguint_range(&mut self, lbound: &BigUint, ubound: &BigUint) -> BigUint;

    /// Generate a random [`BigInt`] within the given range. The lower
    /// bound is inclusive; the upper bound is exclusive. Fails when
    /// the upper bound is not greater than the lower bound.
    fn gen_bigint_range(&mut self, lbound: &BigInt, ubound: &BigInt) -> BigInt;
}

fn gen_bits<R: Rng + ?Sized>(rng: &mut R, data: &mut [u32], rem: u64) {
    // `fill` is faster than many `gen::<u32>` calls
    rng.fill(data);
    if rem > 0 {
        let last = data.len() - 1;
        data[last] >>= 32 - rem;
    }
}

impl<R: Rng + ?Sized> RandBigInt for R {
    cfg_digit!(
        fn gen_biguint(&mut self, bit_size: u64) -> BigUint {
            let (digits, rem) = bit_size.div_rem(&32);
            let len = (digits + (rem > 0) as u64)
                .to_usize()
                .expect("capacity overflow");
            let mut data = vec![0u32; len];
            gen_bits(self, &mut data, rem);
            biguint_from_vec(data)
        }

        fn gen_biguint(&mut self, bit_size: u64) -> BigUint {
            use core::slice;

            let (digits, rem) = bit_size.div_rem(&32);
            let len = (digits + (rem > 0) as u64)
                .to_usize()
                .expect("capacity overflow");
            let native_digits = Integer::div_ceil(&bit_size, &64);
            let native_len = native_digits.to_usize().expect("capacity overflow");
            let mut data = vec![0u64; native_len];
            unsafe {
                // Generate bits in a `&mut [u32]` slice for value stability
                let ptr = data.as_mut_ptr() as *mut u32;
                debug_assert!(native_len * 2 >= len);
                let data = slice::from_raw_parts_mut(ptr, len);
                gen_bits(self, data, rem);
            }
            #[cfg(target_endian = "big")]
            for digit in &mut data {
                // swap u32 digits into u64 endianness
                *digit = (*digit << 32) | (*digit >> 32);
            }
            biguint_from_vec(data)
        }
    );

    fn gen_bigint(&mut self, bit_size: u64) -> BigInt {
        loop {
            // Generate a random BigUint...
            let biguint = self.gen_biguint(bit_size);
            // ...and then randomly assign it a Sign...
            let sign = if biguint.is_zero() {
                // ...except that if the BigUint is zero, we need to try
                // again with probability 0.5. This is because otherwise,
                // the probability of generating a zero BigInt would be
                // double that of any other number.
                if self.gen() {
                    continue;
                } else {
                    NoSign
                }
            } else if self.gen() {
                Plus
            } else {
                Minus
            };
            return BigInt::from_biguint(sign, biguint);
        }
    }

    fn gen_biguint_below(&mut self, bound: &BigUint) -> BigUint {
        assert!(!bound.is_zero());
        let bits = bound.bits();
        loop {
            let n = self.gen_biguint(bits);
            if n < *bound {
                return n;
            }
        }
    }

    fn gen_biguint_range(&mut self, lbound: &BigUint, ubound: &BigUint) -> BigUint {
        assert!(*lbound < *ubound);
        if lbound.is_zero() {
            self.gen_biguint_below(ubound)
        } else {
            lbound + self.gen_biguint_below(&(ubound - lbound))
        }
    }

    fn gen_bigint_range(&mut self, lbound: &BigInt, ubound: &BigInt) -> BigInt {
        assert!(*lbound < *ubound);
        if lbound.is_zero() {
            BigInt::from(self.gen_biguint_below(ubound.magnitude()))
        } else if ubound.is_zero() {
            lbound + BigInt::from(self.gen_biguint_below(lbound.magnitude()))
        } else {
            let delta = ubound - lbound;
            lbound + BigInt::from(self.gen_biguint_below(delta.magnitude()))
        }
    }
}

/// The back-end implementing rand's [`UniformSampler`] for [`BigUint`].
#[derive(Clone, Debug)]
pub struct UniformBigUint {
    base: BigUint,
    len: BigUint,
}

impl UniformSampler for UniformBigUint {
    type X = BigUint;

    #[inline]
    fn new<B1, B2>(low_b: B1, high_b: B2) -> Self
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        let low = low_b.borrow();
        let high = high_b.borrow();
        assert!(low < high);
        UniformBigUint {
            len: high - low,
            base: low.clone(),
        }
    }

    #[inline]
    fn new_inclusive<B1, B2>(low_b: B1, high_b: B2) -> Self
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        let low = low_b.borrow();
        let high = high_b.borrow();
        assert!(low <= high);
        Self::new(low, high + 1u32)
    }

    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Self::X {
        &self.base + rng.gen_biguint_below(&self.len)
    }

    #[inline]
    fn sample_single<R: Rng + ?Sized, B1, B2>(low: B1, high: B2, rng: &mut R) -> Self::X
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        rng.gen_biguint_range(low.borrow(), high.borrow())
    }
}

impl SampleUniform for BigUint {
    type Sampler = UniformBigUint;
}

/// The back-end implementing rand's [`UniformSampler`] for [`BigInt`].
#[derive(Clone, Debug)]
pub struct UniformBigInt {
    base: BigInt,
    len: BigUint,
}

impl UniformSampler for UniformBigInt {
    type X = BigInt;

    #[inline]
    fn new<B1, B2>(low_b: B1, high_b: B2) -> Self
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        let low = low_b.borrow();
        let high = high_b.borrow();
        assert!(low < high);
        UniformBigInt {
            len: (high - low).into_parts().1,
            base: low.clone(),
        }
    }

    #[inline]
    fn new_inclusive<B1, B2>(low_b: B1, high_b: B2) -> Self
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        let low = low_b.borrow();
        let high = high_b.borrow();
        assert!(low <= high);
        Self::new(low, high + 1u32)
    }

    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Self::X {
        &self.base + BigInt::from(rng.gen_biguint_below(&self.len))
    }

    #[inline]
    fn sample_single<R: Rng + ?Sized, B1, B2>(low: B1, high: B2, rng: &mut R) -> Self::X
    where
        B1: SampleBorrow<Self::X> + Sized,
        B2: SampleBorrow<Self::X> + Sized,
    {
        rng.gen_bigint_range(low.borrow(), high.borrow())
    }
}

impl SampleUniform for BigInt {
    type Sampler = UniformBigInt;
}

/// A random distribution for [`BigUint`] and [`BigInt`] values of a particular bit size.
///
/// The `rand` feature must be enabled to use this. See crate-level documentation for details.
#[derive(Clone, Copy, Debug)]
pub struct RandomBits {
    bits: u64,
}

impl RandomBits {
    #[inline]
    pub fn new(bits: u64) -> RandomBits {
        RandomBits { bits }
    }
}

impl Distribution<BigUint> for RandomBits {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> BigUint {
        rng.gen_biguint(self.bits)
    }
}

impl Distribution<BigInt> for RandomBits {
    #[inline]
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> BigInt {
        rng.gen_bigint(self.bits)
    }
}
