/*
 * Copyright 2018 The Starlark in Rust Authors.
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

use std::cell::Ref;
use std::cell::RefCell;
use std::ptr;

/// "Unleak" previously leaked `RefCell` borrow (which is `Ref`).
#[inline]
pub(crate) unsafe fn unleak_borrow<T: ?Sized>(ref_cell: &RefCell<T>) {
    // Note this call contains a runtime assertion that the `RefCell` is borrowed.
    let r = ref_cell.borrow();
    unsafe {
        // Drop `b` twice to decrement the borrow counter.
        drop::<Ref<T>>(ptr::read(&r as *const Ref<T>));
        drop::<Ref<T>>(r);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::mem;

    #[test]
    fn test_unleak_borrow() {
        let ref_cell = RefCell::new(1);
        assert!(ref_cell.try_borrow_mut().is_ok());
        mem::forget(ref_cell.borrow());
        assert!(
            ref_cell.try_borrow_mut().is_err(),
            "RefCell is borrowed, so we cannot borrow it mutably"
        );
        unsafe { super::unleak_borrow(&ref_cell) };
        assert!(
            ref_cell.try_borrow_mut().is_ok(),
            "Borrow is unleaked, so we can borrow it mutably"
        );

        // Now do the same twice.

        mem::forget(ref_cell.borrow());
        mem::forget(ref_cell.borrow());

        assert!(
            ref_cell.try_borrow_mut().is_err(),
            "RefCell is borrowed, so we cannot borrow it mutably"
        );
        unsafe { super::unleak_borrow(&ref_cell) };
        assert!(
            ref_cell.try_borrow_mut().is_err(),
            "RefCell is still borrowed"
        );
        unsafe { super::unleak_borrow(&ref_cell) };
        assert!(ref_cell.try_borrow_mut().is_ok());
    }
}
