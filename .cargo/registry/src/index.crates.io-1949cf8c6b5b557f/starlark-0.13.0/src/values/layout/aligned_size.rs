/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::alloc::Layout;
use std::mem;
use std::ops::Add;
use std::ops::Mul;
use std::ops::Sub;
use std::ptr::NonNull;

use allocative::Allocative;
use dupe::Dupe;

use crate::values::layout::heap::repr::AValueHeader;

/// Allocations in Starlark are word-aligned, and this type represents the size of an allocation.
#[derive(
    Copy,
    Clone,
    Dupe,
    Default,
    Debug,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Allocative,
    derive_more::Display
)]
#[repr(transparent)]
pub(crate) struct AlignedSize {
    /// Starlark only supports objects smaller than 1<<32.
    bytes: u32,
}

impl AlignedSize {
    pub(crate) const ZERO: AlignedSize = AlignedSize::new_bytes(0);

    const MAX_SIZE: AlignedSize =
        AlignedSize::new_bytes(u32::MAX as usize - AValueHeader::ALIGN + 1);

    #[track_caller]
    #[inline]
    pub(crate) const fn new_bytes(bytes: usize) -> AlignedSize {
        assert!(
            bytes % AValueHeader::ALIGN == 0,
            "AlignedSize must be aligned"
        );
        assert!(
            bytes as u32 as usize == bytes,
            "AlignedSize must not exceed u32::MAX"
        );
        let bytes = bytes as u32;
        AlignedSize { bytes }
    }

    #[track_caller]
    #[inline]
    pub(crate) const fn align_up(bytes: usize) -> AlignedSize {
        assert!(
            bytes <= AlignedSize::MAX_SIZE.bytes() as usize,
            "AlignedSize must not exceed u32::MAX"
        );
        let bytes = (bytes + AValueHeader::ALIGN - 1) & !(AValueHeader::ALIGN - 1);
        let bytes = bytes as u32;
        AlignedSize { bytes }
    }

    #[inline]
    pub(crate) const fn bytes(self) -> u32 {
        self.bytes
    }

    #[inline]
    pub(crate) const fn of<T>() -> AlignedSize {
        AlignedSize::align_up(mem::size_of::<T>())
    }

    #[inline]
    pub(crate) const fn layout(self) -> Layout {
        match Layout::from_size_align(self.bytes as usize, AValueHeader::ALIGN) {
            Ok(layout) => layout,
            Err(_) => panic!("Layout::from_size_align failed"),
        }
    }

    #[inline]
    pub(crate) fn checked_next_power_of_two(self) -> Option<AlignedSize> {
        let bytes = self.bytes.checked_next_power_of_two()?;
        Some(AlignedSize::new_bytes(bytes as usize))
    }

    #[inline]
    pub(crate) fn unchecked_sub(self, rhs: AlignedSize) -> AlignedSize {
        debug_assert!(self.bytes >= rhs.bytes, "{:?} - {:?}", self, rhs);
        AlignedSize {
            bytes: self.bytes - rhs.bytes,
        }
    }

    #[inline]
    pub(crate) fn ptr_diff(begin: NonNull<usize>, end: NonNull<usize>) -> AlignedSize {
        unsafe { AlignedSize::new_bytes(end.as_ptr().byte_offset_from(begin.as_ptr()) as usize) }
    }
}

impl Add for AlignedSize {
    type Output = AlignedSize;

    #[track_caller]
    #[inline]
    fn add(self, rhs: AlignedSize) -> AlignedSize {
        let bytes = self.bytes.checked_add(rhs.bytes).unwrap();
        AlignedSize { bytes }
    }
}

impl Sub for AlignedSize {
    type Output = AlignedSize;

    #[track_caller]
    #[inline]
    fn sub(self, rhs: AlignedSize) -> AlignedSize {
        let bytes = self.bytes.checked_sub(rhs.bytes).unwrap();
        AlignedSize { bytes }
    }
}

impl Mul<u32> for AlignedSize {
    type Output = AlignedSize;

    #[track_caller]
    #[inline]
    fn mul(self, rhs: u32) -> Self::Output {
        let bytes = self.bytes.checked_mul(rhs).unwrap();
        AlignedSize { bytes }
    }
}

#[cfg(test)]
mod tests {
    use crate::values::layout::aligned_size::AlignedSize;
    use crate::values::layout::heap::repr::AValueHeader;

    #[test]
    fn test_checked_next_power_of_two() {
        assert_eq!(
            AlignedSize::new_bytes(AValueHeader::ALIGN),
            AlignedSize::new_bytes(AValueHeader::ALIGN)
                .checked_next_power_of_two()
                .unwrap()
        );
        assert_eq!(
            AlignedSize::new_bytes(2 * AValueHeader::ALIGN),
            AlignedSize::new_bytes(2 * AValueHeader::ALIGN)
                .checked_next_power_of_two()
                .unwrap()
        );
        assert_eq!(
            AlignedSize::new_bytes(4 * AValueHeader::ALIGN),
            AlignedSize::new_bytes(3 * AValueHeader::ALIGN)
                .checked_next_power_of_two()
                .unwrap()
        );
        assert_eq!(
            AlignedSize::new_bytes(8 * AValueHeader::ALIGN),
            AlignedSize::new_bytes(5 * AValueHeader::ALIGN)
                .checked_next_power_of_two()
                .unwrap()
        );
    }

    #[test]
    fn test_sub() {
        assert_eq!(
            AlignedSize::new_bytes(2 * AValueHeader::ALIGN),
            AlignedSize::new_bytes(5 * AValueHeader::ALIGN)
                - AlignedSize::new_bytes(3 * AValueHeader::ALIGN)
        );
    }
}
