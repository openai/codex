// Adapted from https://github.com/Alexhuszagh/rust-lexical.

//! Big integer type definition.

use super::math::*;
#[allow(unused_imports)]
use alloc::vec::Vec;

/// Storage for a big integer type.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Bigint {
    /// Internal storage for the Bigint, in little-endian order.
    pub(crate) data: Vec<Limb>,
}

impl Default for Bigint {
    fn default() -> Self {
        Bigint {
            data: Vec::with_capacity(20),
        }
    }
}

impl Math for Bigint {
    #[inline]
    fn data(&self) -> &Vec<Limb> {
        &self.data
    }

    #[inline]
    fn data_mut(&mut self) -> &mut Vec<Limb> {
        &mut self.data
    }
}
