// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Weighted index sampling
//!
//! This module is deprecated. Use [`crate::distributions::WeightedIndex`] and
//! [`crate::distributions::WeightedError`] instead.

pub use super::{WeightedIndex, WeightedError};

#[allow(missing_docs)]
#[deprecated(since = "0.8.0", note = "moved to rand_distr crate")]
pub mod alias_method {
    // This module exists to provide a deprecation warning which minimises
    // compile errors, but still fails to compile if ever used.
    use core::marker::PhantomData;
    use alloc::vec::Vec;
    use super::WeightedError;

    #[derive(Debug)]
    pub struct WeightedIndex<W: Weight> {
        _phantom: PhantomData<W>,
    }
    impl<W: Weight> WeightedIndex<W> {
        pub fn new(_weights: Vec<W>) -> Result<Self, WeightedError> {
            Err(WeightedError::NoItem)
        }
    }

    pub trait Weight {}
    macro_rules! impl_weight {
        () => {};
        ($T:ident, $($more:ident,)*) => {
            impl Weight for $T {}
            impl_weight!($($more,)*);
        };
    }
    impl_weight!(f64, f32,);
    impl_weight!(u8, u16, u32, u64, usize,);
    impl_weight!(i8, i16, i32, i64, isize,);
    impl_weight!(u128, i128,);
}
