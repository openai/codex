// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::*;
use crate::error::ZeroTrieBuildError;
use alloc::collections::btree_map::Entry;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// Helper class for caching the results of multiple [`PerfectByteHashMap`] calculations.
pub struct PerfectByteHashMapCacheOwned {
    // Note: This should probably be a HashMap but that isn't in `alloc`
    data: BTreeMap<Vec<u8>, PerfectByteHashMap<Vec<u8>>>,
}

impl PerfectByteHashMapCacheOwned {
    /// Creates a new empty instance.
    pub fn new_empty() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }

    /// Gets the [`PerfectByteHashMap`] for the given bytes, calculating it if necessary.
    pub fn try_get_or_insert(
        &mut self,
        keys: Vec<u8>,
    ) -> Result<&PerfectByteHashMap<[u8]>, ZeroTrieBuildError> {
        let mut_phf = match self.data.entry(keys) {
            Entry::Vacant(entry) => {
                let value = PerfectByteHashMap::try_new(entry.key())?;
                entry.insert(value)
            }
            Entry::Occupied(entry) => entry.into_mut(),
        };
        Ok(mut_phf.as_borrowed())
    }
}
