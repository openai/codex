/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is licensed under both the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree and the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree.
 */

//! A cheap version of [`Clone`].

pub mod __macro_refs;
pub(crate) mod iter;
pub(crate) mod option;

use std::cell::Cell;
use std::mem::ManuallyDrop;
use std::num::*;
use std::rc::Rc;
use std::sync::Arc;

pub use dupe_derive::Clone_;
pub use dupe_derive::Copy_;
pub use dupe_derive::Dupe;
pub use dupe_derive::Dupe_;

pub use crate::iter::IterDupedExt;
pub use crate::option::OptionDupedExt;

/// Like [`Clone`], but should only be available if [`Clone`] is
/// constant time and zero allocation (e.g. a few [`Arc`] bumps).
/// The implementation of `dupe` should _always_ call `clone`.
pub trait Dupe: Clone {
    #[inline]
    fn dupe(&self) -> Self {
        self.clone()
    }
}

// Smart pointer/wrapper types
impl<A: ?Sized> Dupe for &A {}
impl<A: ?Sized> Dupe for *const A {}
impl<A: ?Sized> Dupe for *mut A {}
impl<A: ?Sized> Dupe for Arc<A> {}
impl<A: ?Sized> Dupe for std::sync::Weak<A> {}
impl<A: ?Sized> Dupe for Rc<A> {}
impl<A: ?Sized> Dupe for std::rc::Weak<A> {}
impl<A: Copy> Dupe for Cell<A> {}
impl<A: Dupe> Dupe for ManuallyDrop<A> {}

// Small containers
impl<A: Dupe> Dupe for Option<A> {}
impl<T: Dupe, E: Dupe> Dupe for Result<T, E> {}
impl<A: Dupe> Dupe for std::ops::Bound<A> {}
impl<A: Dupe> Dupe for std::pin::Pin<A> {}
impl<A: Dupe> Dupe for std::ptr::NonNull<A> {}
impl<A: Dupe> Dupe for std::task::Poll<A> {}
impl Dupe for () {}
impl<A: Dupe> Dupe for (A,) {}
impl<A: Dupe, B: Dupe> Dupe for (A, B) {}
impl<A: Dupe, B: Dupe, C: Dupe> Dupe for (A, B, C) {}
impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe> Dupe for (A, B, C, D) {}
impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe> Dupe for (A, B, C, D, E) {}
impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe, F: Dupe> Dupe for (A, B, C, D, E, F) {}
impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe, F: Dupe, G: Dupe> Dupe for (A, B, C, D, E, F, G) {}
impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe, F: Dupe, G: Dupe, H: Dupe> Dupe
    for (A, B, C, D, E, F, G, H)
{
}
impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe, F: Dupe, G: Dupe, H: Dupe, I: Dupe> Dupe
    for (A, B, C, D, E, F, G, H, I)
{
}
impl<A: Dupe, B: Dupe, C: Dupe, D: Dupe, E: Dupe, F: Dupe, G: Dupe, H: Dupe, I: Dupe, J: Dupe> Dupe
    for (A, B, C, D, E, F, G, H, I, J)
{
}

impl<A: Dupe, const N: usize> Dupe for [A; N] {}

// Atomic types
impl Dupe for bool {}
impl Dupe for char {}
impl Dupe for u8 {}
impl Dupe for u16 {}
impl Dupe for u32 {}
impl Dupe for u64 {}
impl Dupe for u128 {}
impl Dupe for usize {}
impl Dupe for i8 {}
impl Dupe for i16 {}
impl Dupe for i32 {}
impl Dupe for i64 {}
impl Dupe for i128 {}
impl Dupe for isize {}
impl Dupe for f32 {}
impl Dupe for f64 {}
impl Dupe for NonZeroU8 {}
impl Dupe for NonZeroU16 {}
impl Dupe for NonZeroU32 {}
impl Dupe for NonZeroU64 {}
impl Dupe for NonZeroU128 {}
impl Dupe for NonZeroUsize {}
impl Dupe for NonZeroI8 {}
impl Dupe for NonZeroI16 {}
impl Dupe for NonZeroI32 {}
impl Dupe for NonZeroI64 {}
impl Dupe for NonZeroI128 {}
impl Dupe for NonZeroIsize {}

// Other std types that are Copyable
impl Dupe for std::any::TypeId {}
impl Dupe for std::marker::PhantomPinned {}
impl Dupe for std::net::Ipv4Addr {}
impl Dupe for std::net::Ipv6Addr {}
impl Dupe for std::net::SocketAddrV4 {}
impl Dupe for std::net::SocketAddrV6 {}
impl Dupe for std::thread::ThreadId {}
impl Dupe for std::time::Instant {}
impl Dupe for std::time::SystemTime {}
impl Dupe for std::time::Duration {}
impl<T: ?Sized> Dupe for std::marker::PhantomData<T> {}

impl<R> Dupe for fn() -> R {}
impl<A1, R> Dupe for fn(A1) -> R {}
impl<A1, A2, R> Dupe for fn(A1, A2) -> R {}
impl<A1, A2, A3, R> Dupe for fn(A1, A2, A3) -> R {}
impl<A1, A2, A3, A4, R> Dupe for fn(A1, A2, A3, A4) -> R {}
impl<A1, A2, A3, A4, A5, R> Dupe for fn(A1, A2, A3, A4, A5) -> R {}
impl<A1, A2, A3, A4, A5, A6, R> Dupe for fn(A1, A2, A3, A4, A5, A6) -> R {}
impl<A1, A2, A3, A4, A5, A6, A7, R> Dupe for fn(A1, A2, A3, A4, A5, A6, A7) -> R {}
impl<A1, A2, A3, A4, A5, A6, A7, A8, R> Dupe for fn(A1, A2, A3, A4, A5, A6, A7, A8) -> R {}
impl<A1, A2, A3, A4, A5, A6, A7, A8, A9, R> Dupe for fn(A1, A2, A3, A4, A5, A6, A7, A8, A9) -> R {}
impl<A1, A2, A3, A4, A5, A6, A7, A8, A9, A10, R> Dupe
    for fn(A1, A2, A3, A4, A5, A6, A7, A8, A9, A10) -> R
{
}
impl<A1, A2, A3, A4, A5, A6, A7, A8, A9, A10, A11, R> Dupe
    for fn(A1, A2, A3, A4, A5, A6, A7, A8, A9, A10, A11) -> R
{
}
impl<A1, A2, A3, A4, A5, A6, A7, A8, A9, A10, A11, A12, R> Dupe
    for fn(A1, A2, A3, A4, A5, A6, A7, A8, A9, A10, A11, A12) -> R
{
}
// Rust goes up to 12 arguments for traits, so we follow

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)] // Not actually unused, this makes testing the derive macro work
    use crate as dupe;

    #[test]
    fn test_dupe_generic() {
        #[derive(Clone, Dupe, Debug, PartialEq, Eq)]
        struct Foo {}
        #[derive(Clone, Dupe, Debug, PartialEq, Eq)]
        struct FooT<T> {
            foo: T,
        }
        #[derive(Clone, Dupe, Debug, PartialEq, Eq)]
        struct Faz;

        let x = Foo {};
        assert_eq!(x, Dupe::dupe(&x));

        let x = FooT { foo: 1 };
        assert_eq!(x, Dupe::dupe(&x));

        let x = Faz;
        assert_eq!(x, Dupe::dupe(&x));
    }

    #[test]
    fn test_dupe_() {
        #[derive(Debug, PartialEq, Eq)]
        struct NoClone();
        #[derive(Dupe_, Debug, PartialEq, Eq)]
        struct FooT<T> {
            foo: Arc<T>,
        }

        impl<T> Clone for FooT<T> {
            fn clone(&self) -> Self {
                FooT {
                    foo: self.foo.clone(),
                }
            }
        }

        let x = FooT {
            foo: Arc::new(NoClone()),
        };
        assert_eq!(x, Dupe::dupe(&x));
    }

    #[test]
    fn test_dupe_enum() {
        #[derive(Clone, Dupe, Debug, PartialEq, Eq)]
        struct Foo();
        #[derive(Clone, Dupe, Debug, PartialEq, Eq)]
        struct Foo2;
        #[derive(Clone, Dupe, Debug, PartialEq, Eq)]
        struct Bar(i64, bool, Foo2);
        #[derive(Clone, Dupe, Debug, PartialEq, Eq)]
        struct Baz {
            foo: usize,
        }
        #[derive(Clone, Dupe, Debug, PartialEq, Eq)]
        enum Qux {
            Foo(),
            Foo2,
            Bar(Foo, Bar),
            Baz { foo: Foo, bar: Bar, baz: Baz },
        }

        let x = Qux::Bar(Foo(), Bar(8, true, Foo2));
        assert_eq!(x, Dupe::dupe(&x));
        let x = Qux::Baz {
            foo: Foo(),
            bar: Bar(7, false, Foo2),
            baz: Baz { foo: 9 },
        };
        assert_eq!(x, Dupe::dupe(&x));
        let x = Qux::Foo();
        assert_eq!(x, Dupe::dupe(&x));
        let x = Qux::Foo2;
        assert_eq!(x, Dupe::dupe(&x));
    }

    #[test]
    fn test_dupe_fn()
    where
        fn(usize): Dupe,
        fn(String, Vec<usize>) -> bool: Dupe,
    {
        // Tests are in the where
    }
}
