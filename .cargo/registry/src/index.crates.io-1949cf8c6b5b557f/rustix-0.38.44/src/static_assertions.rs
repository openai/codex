//! Workarounds for Rust 1.63 where some things in the `static_assertions`
//! crate do things that don't work in const contexts. We want to call them in
//! const contexts in Rust versions where that's supported so that problems are
//! caught at compile time, and fall back to dynamic asserts in Rust 1.63.

#![allow(unused_macros)]

macro_rules! assert_eq_size {
    ($x:ty, $y:ty) => {
        assert_eq!(core::mem::size_of::<$x>(), core::mem::size_of::<$y>());
    };
}

macro_rules! assert_eq_align {
    ($x:ty, $y:ty) => {
        assert_eq!(core::mem::align_of::<$x>(), core::mem::align_of::<$y>());
    };
}

macro_rules! const_assert_eq {
    ($x:expr, $y:expr) => {
        assert_eq!($x, $y);
    };
}

macro_rules! const_assert_ne {
    ($x:expr, $y:expr) => {
        assert_ne!($x, $y);
    };
}

macro_rules! const_assert {
    ($x:expr) => {
        assert!($x);
    };
}
