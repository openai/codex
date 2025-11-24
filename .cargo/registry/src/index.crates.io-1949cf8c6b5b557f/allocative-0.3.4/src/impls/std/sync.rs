/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

use std::alloc::Layout;
use std::mem;
use std::rc;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI16;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::AtomicI64;
use std::sync::atomic::AtomicI8;
use std::sync::atomic::AtomicIsize;
use std::sync::atomic::AtomicU16;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicU8;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::sync::Weak;

use crate::allocative_trait::Allocative;
use crate::impls::common::PTR_NAME;
use crate::key::Key;
use crate::visitor::Visitor;

impl<T: Allocative> Allocative for RwLock<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        if let Ok(data) = self.try_read() {
            visitor.visit_field(Key::new("data"), &*data);
        }
        visitor.exit();
    }
}

#[allow(dead_code)] // Only used for its size
#[repr(C)] // as does std
struct RcBox<T: ?Sized> {
    a: usize,
    b: usize,
    t: T,
}

impl<T: ?Sized> RcBox<T> {
    fn layout(val: &T) -> Layout {
        let val_layout = Layout::for_value(val);
        // See rcbox_layout_for_value_layout in std
        Layout::new::<RcBox<()>>()
            .extend(val_layout)
            .unwrap()
            .0
            .pad_to_align()
    }
}

impl<T: Allocative + ?Sized> Allocative for Arc<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            let visitor = visitor.enter_shared(
                PTR_NAME,
                // Possibly fat pointer for size
                mem::size_of::<*const T>(),
                // Force thin pointer for identity, as fat pointers can have
                // different VTable addresses attached to the same memory
                Arc::as_ptr(self) as *const (),
            );
            if let Some(mut visitor) = visitor {
                {
                    let val: &T = self;
                    let mut visitor =
                        visitor.enter(Key::new("ArcInner"), RcBox::layout(val).size());
                    val.visit(&mut visitor);
                    visitor.exit();
                }
                visitor.exit();
            }
        }
        visitor.exit();
    }
}

impl<T: Allocative + ?Sized> Allocative for Weak<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            if let Some(arc) = self.upgrade() {
                arc.visit(&mut visitor);
            }
        }
        visitor.exit();
    }
}

impl<T: Allocative> Allocative for Rc<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            let visitor = visitor.enter_shared(
                PTR_NAME,
                // Possibly fat pointer for size
                mem::size_of::<*const T>(),
                // Force thin pointer for identity, as fat pointers can have
                // different VTable addresses attached to the same memory
                Rc::as_ptr(self) as *const (),
            );
            if let Some(mut visitor) = visitor {
                {
                    let val: &T = self;
                    let mut visitor = visitor.enter(Key::new("RcInner"), RcBox::layout(val).size());
                    val.visit(&mut visitor);
                    visitor.exit();
                }
                visitor.exit();
            }
        }
        visitor.exit();
    }
}

impl<T: Allocative> Allocative for rc::Weak<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        {
            if let Some(rc) = self.upgrade() {
                rc.visit(&mut visitor);
            }
        }
        visitor.exit();
    }
}

impl Allocative for AtomicU8 {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}

impl Allocative for AtomicU16 {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}

impl Allocative for AtomicU32 {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}

impl Allocative for AtomicU64 {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}

impl Allocative for AtomicUsize {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}

impl Allocative for AtomicI8 {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}

impl Allocative for AtomicI16 {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}

impl Allocative for AtomicI32 {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}

impl Allocative for AtomicI64 {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}

impl Allocative for AtomicBool {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}

impl Allocative for AtomicIsize {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        visitor.enter_self_sized::<Self>().exit();
    }
}

impl<T: Allocative> Allocative for Mutex<T> {
    fn visit<'a, 'b: 'a>(&self, visitor: &'a mut Visitor<'b>) {
        let mut visitor = visitor.enter_self_sized::<Self>();
        if let Ok(data) = self.try_lock() {
            visitor.visit_field(Key::new("data"), &*data);
        }
        visitor.exit();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate as allocative;
    use crate::golden::golden_test;
    use crate::Allocative;

    #[derive(Allocative)]
    #[repr(align(64))]
    struct CacheLine(u8);

    #[test]
    fn test_arc_align() {
        assert_eq!(std::mem::size_of::<CacheLine>(), 64);

        // ArcInner has two usizes, and then a 64-byte-aligned CacheLine
        // in repr(C) order. So it should have a self size of 64 including
        // padding.
        golden_test!(&Arc::new(CacheLine(0)));
    }
}
