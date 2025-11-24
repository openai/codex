/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::any::Any;
use std::any::TypeId;
use std::cmp::Ordering;
use std::marker::PhantomData;

/// Ordering between arbitrary types.
pub struct OrdAny<'a> {
    type_id: TypeId,
    cmp: unsafe fn(*const (), *const ()) -> Ordering,
    val: *const (),
    _marker: PhantomData<&'a dyn Any>,
}

impl<'a> OrdAny<'a> {
    #[inline]
    pub fn new<A: Ord + 'static>(a: &'a A) -> Self {
        OrdAny {
            type_id: TypeId::of::<A>(),
            cmp: |this, other| {
                // SAFETY: We only call `cmp` with
                //   `this.type_id == other.type_id == TypeId::of::<A>()`.
                let this = unsafe { &*(this as *const A) };
                let other = unsafe { &*(other as *const A) };
                this.cmp(other)
            },
            val: a as *const A as *const (),
            _marker: PhantomData,
        }
    }

    /// Get `TypeId` of the referenced type.
    #[inline]
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }
}

impl<'a> PartialEq for OrdAny<'a> {
    #[inline]
    fn eq(&self, other: &OrdAny<'a>) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<'a> Eq for OrdAny<'a> {}

impl<'a> PartialOrd for OrdAny<'a> {
    #[inline]
    fn partial_cmp(&self, other: &OrdAny<'a>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Compare by type id first, then by value.
impl<'a> Ord for OrdAny<'a> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        let type_cmp = self.type_id.cmp(&other.type_id);
        if type_cmp != Ordering::Equal {
            return type_cmp;
        }
        unsafe { (self.cmp)(self.val, other.val) }
    }
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;
    use std::cmp::Ordering;

    use crate::OrdAny;

    #[test]
    fn test_ord_any() {
        assert!(OrdAny::new(&1) < OrdAny::new(&2));
        assert!(OrdAny::new(&true) > OrdAny::new(&false));
        assert_eq!(
            Ordering::Equal,
            OrdAny::new(&String::new()).cmp(&OrdAny::new(&String::new()))
        );
        assert_eq!(
            OrdAny::new(&1i32) < OrdAny::new(&true),
            TypeId::of::<i32>() < TypeId::of::<bool>()
        );
    }
}
