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

//! Array type used in implementation of `List`.
//!
//! This object is used internally, and not visible outside of `starlark` crate.

use std::cell::UnsafeCell;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::ptr;
use std::slice;

use allocative::Allocative;
use serde::Serialize;
use starlark_derive::starlark_value;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::cast::transmute;
use crate::private::Private;
use crate::values::layout::avalue::alloc_static;
use crate::values::layout::avalue::AValue;
use crate::values::layout::avalue::AValueArray;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::types::list::value::display_list;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::Value;

/// Fixed-capacity list.
///
/// Mutation operations (like `insert`) panic if there's not enough remaining capacity.
#[derive(ProvidesStaticType, Allocative)]
#[repr(C)]
pub(crate) struct Array<'v> {
    // We use `u32` to save some space.
    // `UnsafeCell` is to make this type `Sync` to put an empty array instance into
    // a static variable.
    /// Current number of elements in the array.
    #[allocative(skip)]
    len: UnsafeCell<u32>,
    /// Fixed capacity.
    capacity: u32,
    /// Number of active iterators: when iterator count is non-zero, we cannot modify the array.
    /// Note we track the number of iterators here, but we don't prevent modification here
    /// while iterator count is non-zero: `List` does that.
    // TODO(nga): consider merging this field with `capacity` to save space:
    //   `iter_count_cap >= 0` means capacity
    //   `iter_count_cap < 0` means `-iter_count_cap` active iterators,
    //     and iterator object holds the capacity.
    #[allocative(skip)]
    iter_count: UnsafeCell<u32>,
    content: [Value<'v>; 0],
}

impl<'v> Debug for Array<'v> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Array")
            .field("len", &self.len())
            .field("capacity", &self.capacity)
            .field("iter_count", unsafe { &*self.iter_count.get() })
            .field("content", &self.content())
            .finish()
    }
}

/// `Array` is not `Sync`, so wrap it into this struct to store it in static variable.
/// Empty `Array` is logically `Sync`.
pub(crate) struct ValueEmptyArray(AValueRepr<AValueImpl<'static, AValueArray>>);
unsafe impl Sync for ValueEmptyArray {}

pub(crate) static VALUE_EMPTY_ARRAY: ValueEmptyArray =
    ValueEmptyArray(alloc_static(unsafe { Array::new(0, 0) }));

impl ValueEmptyArray {
    pub(crate) fn repr<'v>(
        &'static self,
    ) -> &'v AValueRepr<AValueImpl<'v, impl AValue<'v, StarlarkValue = Array<'v>>>> {
        // Cast lifetimes. Cannot use `crate::cast::ptr_lifetime` here
        // because type parameter of `AValue` also need to be casted.
        unsafe {
            transmute!(
                &AValueRepr<AValueImpl<AValueArray>>,
                &AValueRepr<AValueImpl<AValueArray>>,
                &self.0
            )
        }
    }
}

impl<'v> Array<'v> {
    pub(crate) fn offset_of_content() -> usize {
        memoffset::offset_of!(Self, content)
    }

    /// Create an array with specified length and capacity.
    /// This function is `unsafe` because it does not populate array content.
    pub(crate) const unsafe fn new(len: u32, capacity: u32) -> Array<'v> {
        debug_assert!(len <= capacity);
        Array {
            len: UnsafeCell::new(len),
            capacity,
            iter_count: UnsafeCell::new(0),
            content: [],
        }
    }

    pub(crate) fn len(&self) -> usize {
        unsafe { *self.len.get() as usize }
    }

    pub(crate) fn capacity(&self) -> usize {
        self.capacity as usize
    }

    fn is_statically_allocated(&self) -> bool {
        self.capacity == 0
    }

    /// Remaining capacity in the array.
    pub(crate) fn remaining_capacity(&self) -> usize {
        debug_assert!(self.capacity as usize >= self.len());
        // This function is called only when modifying.
        debug_assert!(!self.iter_count_is_non_zero());
        self.capacity as usize - self.len()
    }

    /// Get an array content.
    ///
    /// Note this function takes `&self` not `&mut self`, so it does not prevent
    /// modification of the array while holding content reference.
    /// This is memory-safe, because we never overwrite content with
    /// invalid `Value` values.
    pub(crate) fn content(&self) -> &[Value<'v>] {
        unsafe { slice::from_raw_parts(self.content.as_ptr(), self.len()) }
    }

    pub(crate) fn content_mut(&mut self) -> &mut [Value<'v>] {
        unsafe { slice::from_raw_parts_mut(self.content.as_mut_ptr(), self.len()) }
    }

    /// Pointer to an element at given offset.
    fn ptr_at(&self, index: usize) -> *const Value<'v> {
        unsafe { self.content.as_ptr().add(index) }
    }

    /// Pointer to an element at given offset.
    fn mut_ptr_at(&self, index: usize) -> *mut Value<'v> {
        self.ptr_at(index) as *mut Value
    }

    unsafe fn get_unchecked(&self, index: usize) -> Value<'v> {
        debug_assert!(index < self.len());
        *self.ptr_at(index)
    }

    pub(crate) fn set_at(&self, index: usize, value: Value<'v>) {
        debug_assert!(!self.iter_count_is_non_zero());
        assert!(index < self.len());
        unsafe {
            *self.mut_ptr_at(index) = value;
        }
    }

    /// Has at leave one iterator over the array.
    pub(crate) fn iter_count_is_non_zero(&self) -> bool {
        unsafe { *self.iter_count.get() != 0 }
    }

    pub(crate) fn inc_iter_count(&self) {
        // When array is statically allocated, `iter_count` variable
        // is shared between threads.
        if !self.is_statically_allocated() {
            unsafe {
                *self.iter_count.get() += 1;
            };
        }
    }

    pub(crate) fn dec_iter_count(&self) {
        unsafe {
            if !self.is_statically_allocated() {
                debug_assert!(*self.iter_count.get() >= 1);
                *self.iter_count.get() -= 1;
            } else {
                debug_assert!(*self.iter_count.get() == 0);
            }
        }
    }

    pub(crate) fn insert(&self, index: usize, value: Value<'v>) {
        assert!(self.remaining_capacity() >= 1);
        assert!(index <= self.len());
        unsafe {
            ptr::copy(
                self.ptr_at(index),
                self.mut_ptr_at(index + 1),
                self.len() - index,
            );
            *self.mut_ptr_at(index) = value;
            *self.len.get() += 1;
        }
    }

    pub(crate) fn push(&self, value: Value<'v>) {
        assert!(self.remaining_capacity() >= 1);
        unsafe {
            *self.mut_ptr_at(self.len()) = value;
            *self.len.get() += 1;
        }
    }

    /// `self.extend_from_within(..)`.
    pub(crate) fn double(&self) {
        assert!(self.remaining_capacity() >= self.len());
        unsafe {
            ptr::copy_nonoverlapping(self.ptr_at(0), self.mut_ptr_at(self.len()), self.len());
            *self.len.get() *= 2;
        }
    }

    /// Extend with given elements.
    ///
    /// Return `Err` if any of the elements is an error.
    /// Panic if there's not enough capacity.
    pub(crate) fn try_extend<E>(
        &self,
        iter: impl IntoIterator<Item = Result<Value<'v>, E>>,
    ) -> Result<(), E> {
        for item in iter {
            self.push(item?);
        }
        Ok(())
    }

    pub(crate) fn extend_from_slice(&self, slice: &[Value<'v>]) {
        assert!(self.remaining_capacity() >= slice.len());
        unsafe {
            ptr::copy_nonoverlapping(slice.as_ptr(), self.mut_ptr_at(self.len()), slice.len());
            *self.len.get() += slice.len() as u32;
        }
    }

    pub(crate) fn clear(&self) {
        debug_assert!(!self.iter_count_is_non_zero());
        unsafe {
            *self.len.get() = 0;
        }
    }

    pub(crate) fn remove(&self, index: usize) -> Value<'v> {
        debug_assert!(!self.iter_count_is_non_zero());
        unsafe {
            assert!(index < self.len());
            let r = self.get_unchecked(index);
            ptr::copy(
                self.ptr_at(index + 1),
                self.mut_ptr_at(index),
                self.len() - 1 - index,
            );
            *self.len.get() -= 1;
            r
        }
    }
}

/// This type is not visible to user, but still add meaningful `Display` for consistency.
impl<'v> Display for Array<'v> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "array(")?;
        display_list(self.content(), f)?;
        write!(f, ", cap={})", self.capacity)?;
        Ok(())
    }
}

#[starlark_value(type = "array")]
impl<'v> StarlarkValue<'v> for Array<'v> {
    fn is_special(_: Private) -> bool
    where
        Self: Sized,
    {
        true
    }

    fn length(&self) -> crate::Result<i32> {
        Ok(self.len() as i32)
    }

    unsafe fn iter_next(&self, index: usize, _heap: &'v Heap) -> Option<Value<'v>> {
        debug_assert!(self.len() == 0 || self.iter_count_is_non_zero());
        self.content().get(index).copied()
    }

    unsafe fn iter_stop(&self) {
        self.dec_iter_count();
    }

    unsafe fn iter_size_hint(&self, index: usize) -> (usize, Option<usize>) {
        debug_assert!(index <= self.len());
        let rem = self.len() - index;
        (rem, Some(rem))
    }
}

impl<'v> Serialize for Array<'v> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(self.content().iter())
    }
}

#[cfg(test)]
mod tests {
    use crate::values::Heap;
    use crate::values::Value;

    #[test]
    fn debug() {
        let heap = Heap::new();
        let array = heap.alloc_array(10);
        array.push(heap.alloc(23));
        // Just check it does not crash.
        drop(array.to_string());
    }

    #[test]
    fn display() {
        let heap = Heap::new();
        let array = heap.alloc_array(10);
        array.push(heap.alloc(29));
        array.push(Value::new_none());
        assert_eq!("array([29, None], cap=10)", array.to_string());
    }

    #[test]
    fn push() {
        let heap = Heap::new();
        let array = heap.alloc_array(10);
        array.push(heap.alloc(17));
        array.push(heap.alloc(19));
        assert_eq!(heap.alloc(19), array.content()[1]);
    }
}
