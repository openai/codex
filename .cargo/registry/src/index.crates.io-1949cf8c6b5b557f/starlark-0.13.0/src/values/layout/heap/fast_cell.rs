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

use std::cell::Cell;
use std::cell::UnsafeCell;
use std::mem;
use std::mem::MaybeUninit;

/// Faster but less safe alternative to `RefCell<T>`:
/// all operations except `borrow` are `unsafe` and may lead to undefined behavior.
#[derive(Debug)]
pub(crate) struct FastCell<T> {
    /// The value.
    ///
    /// The idea is this: when we take the value out of cell,
    /// we write zeros to `value` field. So accessing this field will result in definite crash.
    /// This is worse than proper panic, but no runtime overhead like in `RefCell`.
    value: UnsafeCell<MaybeUninit<T>>,
    /// Whether the cell contains a value or zeros.
    init: Cell<bool>,
}

impl<T> Drop for FastCell<T> {
    fn drop(&mut self) {
        if self.init.get() {
            unsafe {
                (*self.value.get()).assume_init_drop();
            }
        }
    }
}

impl<T: Default> Default for FastCell<T> {
    fn default() -> Self {
        Self {
            value: UnsafeCell::new(MaybeUninit::new(T::default())),
            init: Cell::new(true),
        }
    }
}

impl<T> FastCell<T> {
    /// Get a reference to the value.
    ///
    /// This operation is safe under assumption that other `unsafe` operations
    /// do not leave self in invalid state.
    pub(crate) fn borrow(&self) -> &T {
        debug_assert!(self.init.get());
        unsafe { (*self.value.get()).assume_init_ref() }
    }

    pub(crate) fn try_borrow(&self) -> Option<&T> {
        if self.init.get() {
            Some(self.borrow())
        } else {
            None
        }
    }

    /// Get a mutable reference to the value.
    ///
    /// This function is unsafe because it's caller responsibility to guarantee
    /// there are no other references to the value, and nobody is going
    /// to obtain references to value while mutable reference exists.
    pub(crate) unsafe fn get_mut(&self) -> *mut T {
        debug_assert!(self.init.get());
        (*self.value.get()).as_mut_ptr()
    }

    /// Take the value out of the cell.
    pub(crate) unsafe fn take(&self) -> T {
        assert!(self.init.get());
        self.init.set(false);
        // Replace the `value` field with zeros so that accessing it will crash.
        mem::replace(&mut *self.value.get(), MaybeUninit::zeroed()).assume_init()
    }

    /// Put the value into the cell.
    pub(crate) unsafe fn set(&self, value: T) {
        assert!(!self.init.get());
        self.init.set(true);
        *self.value.get() = MaybeUninit::new(value);
    }
}
