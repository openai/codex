// Copyright 2016 lazy-static.rs Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// https://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// https://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

extern crate core;
extern crate std;

use self::std::cell::Cell;
use self::std::mem::MaybeUninit;
use self::std::prelude::v1::*;
use self::std::sync::Once;
#[allow(deprecated)]
pub use self::std::sync::ONCE_INIT;

#[allow(dead_code)] // Used in macros
pub struct Lazy<T: Sync>(Cell<MaybeUninit<T>>, Once);

impl<T: Sync> Lazy<T> {
    #[allow(deprecated)]
    pub const INIT: Self = Lazy(Cell::new(MaybeUninit::uninit()), ONCE_INIT);

    #[inline(always)]
    pub fn get<F>(&'static self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        self.1.call_once(|| {
            self.0.set(MaybeUninit::new(f()));
        });

        // `self.0` is guaranteed to be initialized by this point
        // The `Once` will catch and propagate panics
        unsafe { &*(*self.0.as_ptr()).as_ptr() }
    }
}

unsafe impl<T: Sync> Sync for Lazy<T> {}

#[macro_export]
#[doc(hidden)]
macro_rules! __lazy_static_create {
    ($NAME:ident, $T:ty) => {
        static $NAME: $crate::lazy::Lazy<$T> = $crate::lazy::Lazy::INIT;
    };
}
