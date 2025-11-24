//! Simple stack-allocated vector.

#![cfg(not(feature = "alloc"))]
#![doc(hidden)]

use crate::bigint;
use core::{cmp, mem, ops, ptr, slice};

/// Simple stack vector implementation.
#[derive(Clone)]
pub struct StackVec {
    /// The raw buffer for the elements.
    data: [mem::MaybeUninit<bigint::Limb>; bigint::BIGINT_LIMBS],
    /// The number of elements in the array (we never need more than u16::MAX).
    length: u16,
}

#[allow(clippy::new_without_default)]
impl StackVec {
    /// Construct an empty vector.
    #[inline]
    pub const fn new() -> Self {
        Self {
            length: 0,
            data: [mem::MaybeUninit::uninit(); bigint::BIGINT_LIMBS],
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
    /// Safe as long as `len` is less than `BIGINT_LIMBS`.
    #[inline]
    pub unsafe fn set_len(&mut self, len: usize) {
        // Constant is `u16::MAX` for older Rustc versions.
        debug_assert!(len <= 0xffff);
        debug_assert!(len <= bigint::BIGINT_LIMBS);
        self.length = len as u16;
    }

    /// The number of elements stored in the vector.
    #[inline]
    pub const fn len(&self) -> usize {
        self.length as usize
    }

    /// If the vector is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The number of items the vector can hold.
    #[inline]
    pub const fn capacity(&self) -> usize {
        bigint::BIGINT_LIMBS as usize
    }

    /// Append an item to the vector, without bounds checking.
    ///
    /// # Safety
    ///
    /// Safe if `self.len() < self.capacity()`.
    #[inline]
    pub unsafe fn push_unchecked(&mut self, value: bigint::Limb) {
        debug_assert!(self.len() < self.capacity());
        // SAFETY: safe, capacity is less than the current size.
        unsafe {
            ptr::write(self.as_mut_ptr().add(self.len()), value);
            self.length += 1;
        }
    }

    /// Append an item to the vector.
    #[inline]
    pub fn try_push(&mut self, value: bigint::Limb) -> Option<()> {
        if self.len() < self.capacity() {
            // SAFETY: safe, capacity is less than the current size.
            unsafe { self.push_unchecked(value) };
            Some(())
        } else {
            None
        }
    }

    /// Remove an item from the end of a vector, without bounds checking.
    ///
    /// # Safety
    ///
    /// Safe if `self.len() > 0`.
    #[inline]
    pub unsafe fn pop_unchecked(&mut self) -> bigint::Limb {
        debug_assert!(!self.is_empty());
        // SAFETY: safe if `self.length > 0`.
        // We have a trivial drop and copy, so this is safe.
        self.length -= 1;
        unsafe { ptr::read(self.as_mut_ptr().add(self.len())) }
    }

    /// Remove an item from the end of the vector and return it, or None if empty.
    #[inline]
    pub fn pop(&mut self) -> Option<bigint::Limb> {
        if self.is_empty() {
            None
        } else {
            // SAFETY: safe, since `self.len() > 0`.
            unsafe { Some(self.pop_unchecked()) }
        }
    }

    /// Add items from a slice to the vector, without bounds checking.
    ///
    /// # Safety
    ///
    /// Safe if `self.len() + slc.len() <= self.capacity()`.
    #[inline]
    pub unsafe fn extend_unchecked(&mut self, slc: &[bigint::Limb]) {
        let index = self.len();
        let new_len = index + slc.len();
        debug_assert!(self.len() + slc.len() <= self.capacity());
        let src = slc.as_ptr();
        // SAFETY: safe if `self.len() + slc.len() <= self.capacity()`.
        unsafe {
            let dst = self.as_mut_ptr().add(index);
            ptr::copy_nonoverlapping(src, dst, slc.len());
            self.set_len(new_len);
        }
    }

    /// Copy elements from a slice and append them to the vector.
    #[inline]
    pub fn try_extend(&mut self, slc: &[bigint::Limb]) -> Option<()> {
        if self.len() + slc.len() <= self.capacity() {
            // SAFETY: safe, since `self.len() + slc.len() <= self.capacity()`.
            unsafe { self.extend_unchecked(slc) };
            Some(())
        } else {
            None
        }
    }

    /// Truncate vector to new length, dropping any items after `len`.
    ///
    /// # Safety
    ///
    /// Safe as long as `len <= self.capacity()`.
    unsafe fn truncate_unchecked(&mut self, len: usize) {
        debug_assert!(len <= self.capacity());
        self.length = len as u16;
    }

    /// Resize the buffer, without bounds checking.
    ///
    /// # Safety
    ///
    /// Safe as long as `len <= self.capacity()`.
    #[inline]
    pub unsafe fn resize_unchecked(&mut self, len: usize, value: bigint::Limb) {
        debug_assert!(len <= self.capacity());
        let old_len = self.len();
        if len > old_len {
            // We have a trivial drop, so there's no worry here.
            // Just, don't set the length until all values have been written,
            // so we don't accidentally read uninitialized memory.

            // SAFETY: safe if `len < self.capacity()`.
            let count = len - old_len;
            for index in 0..count {
                unsafe {
                    let dst = self.as_mut_ptr().add(old_len + index);
                    ptr::write(dst, value);
                }
            }
            self.length = len as u16;
        } else {
            // SAFETY: safe since `len < self.len()`.
            unsafe { self.truncate_unchecked(len) };
        }
    }

    /// Try to resize the buffer.
    ///
    /// If the new length is smaller than the current length, truncate
    /// the input. If it's larger, then append elements to the buffer.
    #[inline]
    pub fn try_resize(&mut self, len: usize, value: bigint::Limb) -> Option<()> {
        if len > self.capacity() {
            None
        } else {
            // SAFETY: safe, since `len <= self.capacity()`.
            unsafe { self.resize_unchecked(len, value) };
            Some(())
        }
    }

    // HI

    /// Get the high 64 bits from the vector.
    #[inline(always)]
    pub fn hi64(&self) -> (u64, bool) {
        bigint::hi64(self)
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

impl PartialEq for StackVec {
    #[inline]
    #[allow(clippy::op_ref)]
    fn eq(&self, other: &Self) -> bool {
        use core::ops::Deref;
        self.len() == other.len() && self.deref() == other.deref()
    }
}

impl Eq for StackVec {
}

impl cmp::PartialOrd for StackVec {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(bigint::compare(self, other))
    }
}

impl cmp::Ord for StackVec {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        bigint::compare(self, other)
    }
}

impl ops::Deref for StackVec {
    type Target = [bigint::Limb];
    #[inline]
    fn deref(&self) -> &[bigint::Limb] {
        // SAFETY: safe since `self.data[..self.len()]` must be initialized
        // and `self.len() <= self.capacity()`.
        unsafe {
            let ptr = self.data.as_ptr() as *const bigint::Limb;
            slice::from_raw_parts(ptr, self.len())
        }
    }
}

impl ops::DerefMut for StackVec {
    #[inline]
    fn deref_mut(&mut self) -> &mut [bigint::Limb] {
        // SAFETY: safe since `self.data[..self.len()]` must be initialized
        // and `self.len() <= self.capacity()`.
        unsafe {
            let ptr = self.data.as_mut_ptr() as *mut bigint::Limb;
            slice::from_raw_parts_mut(ptr, self.len())
        }
    }
}

impl ops::MulAssign<&[bigint::Limb]> for StackVec {
    #[inline]
    fn mul_assign(&mut self, rhs: &[bigint::Limb]) {
        bigint::large_mul(self, rhs).unwrap();
    }
}
