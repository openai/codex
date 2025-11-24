// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! See [`ZeroMap`](crate::ZeroMap) for details.

mod borrowed;
mod kv;
#[expect(clippy::module_inception)] // module is purely internal
pub(crate) mod map;
mod vecs;

#[cfg(feature = "databake")]
mod databake;
#[cfg(feature = "serde")]
mod serde;
#[cfg(feature = "serde")]
mod serde_helpers;

pub use crate::ZeroMap;
pub use borrowed::ZeroMapBorrowed;
pub use kv::ZeroMapKV;
pub use vecs::{MutableZeroVecLike, ZeroVecLike};
