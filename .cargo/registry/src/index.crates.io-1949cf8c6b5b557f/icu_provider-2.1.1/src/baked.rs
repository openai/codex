// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

//! This module contains scaffolding for baked providers, typically generated using
//! databake.
//!
//! It can be glob-imported, and includes the icu_provider prelude.
//!
//! This needs the `"baked"` feature to be enabled.

pub mod zerotrie;
use crate::prelude::{DataIdentifierBorrowed, DataMarker, DataPayload};

/// A backing store for baked data
pub trait DataStore<M: DataMarker>: private::Sealed {
    /// Get the value for a key
    fn get(
        &self,
        req: DataIdentifierBorrowed,
        attributes_prefix_match: bool,
    ) -> Option<DataPayload<M>>;

    /// The type returned by the iterator
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    type IterReturn: Iterator<Item = crate::prelude::DataIdentifierCow<'static>>;
    /// Iterate over all data
    ///
    /// ✨ *Enabled with the `alloc` Cargo feature.*
    #[cfg(feature = "alloc")]
    fn iter(&'static self) -> Self::IterReturn;
}

pub(crate) mod private {
    pub trait Sealed {}
}
