// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Convenience re-export of common members
//!
//! Like the standard library's prelude, this module simplifies importing of
//! common items. Unlike the standard prelude, the contents of this module must
//! be imported manually:
//!
//! ```
//! use rand::prelude::*;
//! # let mut r = StdRng::from_rng(&mut rand::rng());
//! # let _: f32 = r.random();
//! ```

#[doc(no_inline)]
pub use crate::distr::Distribution;
#[cfg(feature = "small_rng")]
#[doc(no_inline)]
pub use crate::rngs::SmallRng;
#[cfg(feature = "std_rng")]
#[doc(no_inline)]
pub use crate::rngs::StdRng;
#[doc(no_inline)]
#[cfg(feature = "thread_rng")]
pub use crate::rngs::ThreadRng;
#[doc(no_inline)]
pub use crate::seq::{IndexedMutRandom, IndexedRandom, IteratorRandom, SliceRandom};
#[doc(no_inline)]
pub use crate::{CryptoRng, Rng, RngCore, SeedableRng};
