/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

#![cfg(feature = "triomphe")]

use std::alloc::Layout;
use std::mem;
use std::sync::atomic::AtomicUsize;

use triomphe::Arc;
use triomphe::HeaderSlice;
use triomphe::HeaderWithLength;
use triomphe::ThinArc;

use crate::impls::common::PTR_NAME;
use crate::Allocative;
use crate::Key;
use crate::Visitor;

impl<H: Allocative, T: Allocative + ?Sized> Allocative for HeaderSlice<H, T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self(self);
        visitor.visit_field(Key::new("header"), &self.header);
        visitor.visit_field(Key::new("slice"), &self.slice);
        visitor.exit();
    }
}

impl<H: Allocative> Allocative for HeaderWithLength<H> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self(self);
        visitor.visit_field(Key::new("header"), &self.header);
        visitor.visit_field(Key::new("len"), &self.length);
        visitor.exit();
    }
}

#[repr(C)]
struct ThinArcInnerRepr<H> {
    _counter: AtomicUsize,
    _header: H,
    _len: usize,
}

impl<H: Allocative, T: Allocative> Allocative for ThinArc<H, T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            if let Some(mut visitor) = visitor.enter_shared(
                PTR_NAME,
                mem::size_of::<*mut u8>(),
                ThinArc::ptr(self) as *const (),
            ) {
                let size = Layout::new::<ThinArcInnerRepr<H>>()
                    .extend(Layout::array::<T>(self.slice.len()).unwrap())
                    .unwrap()
                    .0
                    .pad_to_align()
                    .size();
                {
                    let mut visitor =
                        visitor.enter(Key::for_type_name::<ThinArcInnerRepr<H>>(), size);
                    visitor.visit_field::<HeaderSlice<HeaderWithLength<H>, [T]>>(
                        Key::new("data"),
                        self,
                    );
                }
                visitor.exit();
            }
        }
        visitor.exit();
    }
}

struct ArcInnerRepr {
    _counter: AtomicUsize,
}

impl<T: Allocative + ?Sized> Allocative for Arc<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            if let Some(mut visitor) = visitor.enter_shared(
                PTR_NAME,
                mem::size_of::<*mut T>(),
                Arc::heap_ptr(self) as *const (),
            ) {
                let size = Layout::new::<ArcInnerRepr>()
                    .extend(Layout::for_value::<T>(self))
                    .unwrap()
                    .0
                    .pad_to_align()
                    .size();
                {
                    let mut visitor = visitor.enter(Key::for_type_name::<ArcInnerRepr>(), size);
                    visitor.visit_field::<T>(Key::new("data"), self);
                    visitor.exit();
                }
                visitor.exit();
            }
        }
        visitor.exit();
    }
}

#[cfg(test)]
mod tests {
    use triomphe::Arc;

    use crate::golden::golden_test;

    #[test]
    fn test_simple() {
        let arc = Arc::new("abc".to_owned());
        golden_test!(&arc);
    }

    #[test]
    fn test_shared() {
        let arc = Arc::new("a".repeat(100));
        let vec = vec![arc.clone(), arc.clone(), arc];
        golden_test!(&vec);
    }

    #[test]
    fn test_align() {
        let arc = Arc::new(0u8);
        golden_test!(&arc);
    }
}
