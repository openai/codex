// SPDX-License-Identifier: Apache-2.0 OR MIT

// https://discord.com/channels/273534239310479360/512792629516173323/870075511009857617

#![allow(hidden_glob_reexports)]

extern crate pin_project as pin_project_orig;
extern crate self as pin_project;

pub use ::pin_project_orig::*;
mod __private {
    pub use ::pin_project_orig::__private::*;
    pub trait Drop {}
}

use std::{marker::PhantomPinned, mem};

#[pin_project] //~ ERROR conflicting implementations of trait `_::FooMustNotImplDrop`
struct S {
    #[pin]
    f: (u8, PhantomPinned),
}

impl Drop for S {
    fn drop(&mut self) {
        let prev = &self.f.0 as *const _ as usize;
        let moved = mem::take(&mut self.f); // move pinned field
        let moved = &moved.0 as *const _ as usize;
        assert_eq!(prev, moved); // panic
    }
}

fn main() {
    let mut x = Box::pin(S { f: (1, PhantomPinned) });
    let _f = x.as_mut().project().f; // first mutable access
}
