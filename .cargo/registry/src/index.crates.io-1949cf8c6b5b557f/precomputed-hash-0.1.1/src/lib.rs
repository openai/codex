//! A base trait to expose a precomputed hash for a type.

/// A trait to expose a precomputed hash for a type.
pub trait PrecomputedHash {
    // TODO(emilio): Perhaps an associated type would be on point here.

    /// Return the precomputed hash for this item.
    fn precomputed_hash(&self) -> u32;
}

// These are equivalent to the `std::Hash` impls.
impl<'a, T: PrecomputedHash> PrecomputedHash for &'a T {
    fn precomputed_hash(&self) -> u32 {
        (**self).precomputed_hash()
    }
}

impl<'a, T: PrecomputedHash> PrecomputedHash for &'a mut T {
    fn precomputed_hash(&self) -> u32 {
        (**self).precomputed_hash()
    }
}
