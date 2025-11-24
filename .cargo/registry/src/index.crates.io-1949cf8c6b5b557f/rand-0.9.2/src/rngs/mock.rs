// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Mock random number generator

#![allow(deprecated)]

use rand_core::{impls, RngCore};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A mock generator yielding very predictable output
///
/// This generates an arithmetic sequence (i.e. adds a constant each step)
/// over a `u64` number, using wrapping arithmetic. If the increment is 0
/// the generator yields a constant.
///
/// Other integer types (64-bit and smaller) are produced via cast from `u64`.
///
/// Other types are produced via their implementation of [`Rng`](crate::Rng) or
/// [`Distribution`](crate::distr::Distribution).
/// Output values may not be intuitive and may change in future releases but
/// are considered
/// [portable](https://rust-random.github.io/book/portability.html).
/// (`bool` output is true when bit `1u64 << 31` is set.)
///
/// # Example
///
/// ```
/// # #![allow(deprecated)]
/// use rand::Rng;
/// use rand::rngs::mock::StepRng;
///
/// let mut my_rng = StepRng::new(2, 1);
/// let sample: [u64; 3] = my_rng.random();
/// assert_eq!(sample, [2, 3, 4]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[deprecated(since = "0.9.2", note = "Deprecated without replacement")]
pub struct StepRng {
    v: u64,
    a: u64,
}

impl StepRng {
    /// Create a `StepRng`, yielding an arithmetic sequence starting with
    /// `initial` and incremented by `increment` each time.
    pub fn new(initial: u64, increment: u64) -> Self {
        StepRng {
            v: initial,
            a: increment,
        }
    }
}

impl RngCore for StepRng {
    #[inline]
    fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }

    #[inline]
    fn next_u64(&mut self) -> u64 {
        let res = self.v;
        self.v = self.v.wrapping_add(self.a);
        res
    }

    #[inline]
    fn fill_bytes(&mut self, dst: &mut [u8]) {
        impls::fill_bytes_via_next(self, dst)
    }
}
