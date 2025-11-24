// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! See [`VarZeroVec`](crate::VarZeroVec) for details

pub(crate) mod components;
pub(crate) mod error;
pub(crate) mod lengthless;
#[cfg(feature = "alloc")]
pub(crate) mod owned;
pub(crate) mod slice;
pub(crate) mod vec;

#[cfg(feature = "databake")]
mod databake;

#[cfg(feature = "serde")]
mod serde;

pub use crate::{VarZeroSlice, VarZeroVec};

#[doc(hidden)]
pub use components::VarZeroVecComponents;

pub use components::{Index16, Index32, Index8, VarZeroSliceIter, VarZeroVecFormat};

#[cfg(feature = "alloc")]
pub use owned::VarZeroVecOwned;

pub use error::VarZeroVecFormatError;
