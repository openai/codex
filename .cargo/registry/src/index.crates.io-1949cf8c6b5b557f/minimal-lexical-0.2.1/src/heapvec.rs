//! Simple heap-allocated vector.

#![cfg(feature = "alloc")]
#![doc(hidden)]

use crate::bigint;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
use core::{cmp, ops};
#[cfg(feature = "std")]
use std::vec::Vec;

/// Simple heap vector implementation.
#[derive(Clone)]
pub struct HeapVec {
    /// The heap-allocated buffer for the elements.
    data: Vec<bigint::Limb>,
}

#[allow(clippy::new_without_default)]
impl HeapVec {
    /// Construct an empty vector.
    #[inline]
    pub fn new() -> Self {
        Self {
            data: Vec::with_capacity(bigint::BIGINT_LIMBS),
        }
    }

    /// Construct a vector from an existing slice.
    #[inline]
    pub fn try_from(x: &[bigint::Limb]) -> Option<Self> {
        let mut vec = Self::new();
        vec.try_extend(x)?;
        Some(vec)
    }

    /// Sets the length of a vector.
    ///
    /// This will explicitly set the size of the vector, without actually
    /// modifying its buffers, so it is up to the caller to ensure that the
    /// vector is actually the specified size.
    ///
    /// # Safety
    ///
    /// Safe as long as `len` is less than `self.capacity()` and has been initialized.
    #[inline]
    pub unsafe fn set_len(&mut self, len: usize) {
        debug_assert!(len <= bigint::BIGINT_LIMBS);
        unsafe { self.data.set_len(len) };
    }

    /// The number of elements stored in the vector.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// If the vector is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The number of items the vector can hold.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Append an item to the vector.
    #[inline]
    pub fn try_push(&mut self, value: bigint::Limb) -> Option<()> {
        self.data.push(value);
        Some(())
    }

    /// Remove an item from the end of the vector and return it, or None if empty.
    #[inline]
    pub fn pop(&mut self) -> Option<bigint::Limb> {
        self.data.pop()
    }

    /// Copy elements from a slice and append them to the vector.
    #[inline]
    pub fn try_extend(&mut self, slc: &[bigint::Limb]) -> Option<()> {
        self.data.extend_from_slice(slc);
        Some(())
    }

    /// Try to resize the buffer.
    ///
    /// If the new length is smaller than the current length, truncate
    /// the input. If it's larger, then append elements to the buffer.
    #[inline]
    pub fn try_resize(&mut self, len: usize, value: bigint::Limb) -> Option<()> {
        self.data.resize(len, value);
        Some(())
    }

    // HI

    /// Get the high 64 bits from the vector.
    #[inline(always)]
    pub fn hi64(&self) -> (u64, bool) {
        bigint::hi64(&self.data)
    }

    // FROM

    /// Create StackVec from u64 value.
    #[inline(always)]
    pub fn from_u64(x: u64) -> Self {
        bigint::from_u64(x)
    }

    // MATH

    /// Normalize the integer, so any leading zero values are removed.
    #[inline]
    pub fn normalize(&mut self) {
        bigint::normalize(self)
    }

    /// Get if the big integer is normalized.
    #[inline]
    pub fn is_normalized(&self) -> bool {
        bigint::is_normalized(self)
    }

    /// AddAssign small integer.
    #[inline]
    pub fn add_small(&mut self, y: bigint::Limb) -> Option<()> {
        bigint::small_add(self, y)
    }

    /// MulAssign small integer.
    #[inline]
    pub fn mul_small(&mut self, y: bigint::Limb) -> Option<()> {
        bigint::small_mul(self, y)
    }
}

impl PartialEq for HeapVec {
    #[inline]
    #[allow(clippy::op_ref)]
    fn eq(&self, other: &Self) -> bool {
        use core::ops::Deref;
        self.len() == other.len() && self.deref() == other.deref()
    }
}

impl Eq for HeapVec {
}

impl cmp::PartialOrd for HeapVec {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(bigint::compare(self, other))
    }
}

impl cmp::Ord for HeapVec {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        bigint::compare(self, other)
    }
}

impl ops::Deref for HeapVec {
    type Target = [bigint::Limb];
    #[inline]
    fn deref(&self) -> &[bigint::Limb] {
        &self.data
    }
}

impl ops::DerefMut for HeapVec {
    #[inline]
    fn deref_mut(&mut self) -> &mut [bigint::Limb] {
        &mut self.data
    }
}

impl ops::MulAssign<&[bigint::Limb]> for HeapVec {
    #[inline]
    fn mul_assign(&mut self, rhs: &[bigint::Limb]) {
        bigint::large_mul(self, rhs).unwrap();
    }
}
