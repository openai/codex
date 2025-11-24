//! Re-exports either the Rust `std` library or `core` and `alloc` when `std` is
//! disabled.
//!
//! `crate::stdlib::...` should be used rather than `std::` when adding code that
//! will be available with the standard library disabled.
//!
//! Note that this module is called `stdlib` rather than `std`, as Rust 1.34.0
//! does not permit redefining the name `stdlib` (although this works on the
//! latest stable Rust).
#[cfg(feature = "std")]
pub(crate) use std::*;

#[cfg(not(feature = "std"))]
pub(crate) use self::no_std::*;

#[cfg(not(feature = "std"))]
mod no_std {
    // We pre-emptively export everything from libcore/liballoc, (even modules
    // we aren't using currently) to make adding new code easier. Therefore,
    // some of these imports will be unused.
    #![allow(unused_imports)]

    pub(crate) use core::{
        any, array, ascii, cell, char, clone, cmp, convert, default, f32, f64, ffi, future, hash,
        hint, i128, i16, i8, isize, iter, marker, mem, num, ops, option, pin, ptr, result, task,
        time, u128, u16, u32, u8, usize,
    };

    pub(crate) use alloc::{boxed, collections, rc, string, vec};

    pub(crate) mod borrow {
        pub(crate) use alloc::borrow::*;
        pub(crate) use core::borrow::*;
    }

    pub(crate) mod fmt {
        pub(crate) use alloc::fmt::*;
        pub(crate) use core::fmt::*;
    }

    pub(crate) mod slice {
        pub(crate) use alloc::slice::*;
        pub(crate) use core::slice::*;
    }

    pub(crate) mod str {
        pub(crate) use alloc::str::*;
        pub(crate) use core::str::*;
    }

    pub(crate) mod sync {
        pub(crate) use crate::spin::MutexGuard;
        pub(crate) use alloc::sync::*;
        pub(crate) use core::sync::*;

        /// This wraps `spin::Mutex` to return a `Result`, so that it can be
        /// used with code written against `std::sync::Mutex`.
        ///
        /// Since `spin::Mutex` doesn't support poisoning, the `Result` returned
        /// by `lock` will always be `Ok`.
        #[derive(Debug, Default)]
        pub(crate) struct Mutex<T> {
            inner: crate::spin::Mutex<T>,
        }

        impl<T> Mutex<T> {
            // pub(crate) fn new(data: T) -> Self {
            //     Self {
            //         inner: crate::spin::Mutex::new(data),
            //     }
            // }

            pub(crate) fn lock(&self) -> Result<MutexGuard<'_, T>, ()> {
                Ok(self.inner.lock())
            }
        }
    }
}
