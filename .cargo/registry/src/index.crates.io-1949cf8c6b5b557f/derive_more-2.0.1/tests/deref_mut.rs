#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(nightly, feature(never_type))]
#![allow(dead_code)] // some code is tested for type checking only

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, vec, vec::Vec};

use derive_more::DerefMut;

#[derive(DerefMut)]
#[deref_mut(forward)]
struct MyBoxedInt(Box<i32>);

// `Deref` implementation is required for `DerefMut`.
impl ::core::ops::Deref for MyBoxedInt {
    type Target = <Box<i32> as ::core::ops::Deref>::Target;
    #[inline]
    fn deref(&self) -> &Self::Target {
        <Box<i32> as ::core::ops::Deref>::deref(&self.0)
    }
}

#[derive(DerefMut)]
struct NumRef<'a> {
    #[deref_mut(forward)]
    num: &'a mut i32,
}

// `Deref` implementation is required for `DerefMut`.
impl<'a> ::core::ops::Deref for NumRef<'a> {
    type Target = <&'a mut i32 as ::core::ops::Deref>::Target;
    #[inline]
    fn deref(&self) -> &Self::Target {
        <&'a mut i32 as ::core::ops::Deref>::deref(&self.num)
    }
}

#[derive(DerefMut)]
#[deref_mut(forward)]
struct NumRef2<'a> {
    num: &'a mut i32,
    #[deref_mut(ignore)]
    useless: bool,
}

// `Deref` implementation is required for `DerefMut`.
impl<'a> ::core::ops::Deref for NumRef2<'a> {
    type Target = <&'a mut i32 as ::core::ops::Deref>::Target;
    #[inline]
    fn deref(&self) -> &Self::Target {
        <&'a mut i32 as ::core::ops::Deref>::deref(&self.num)
    }
}

#[derive(DerefMut)]
struct MyInt(i32);

// `Deref` implementation is required for `DerefMut`.
impl ::core::ops::Deref for MyInt {
    type Target = i32;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(DerefMut)]
struct Point1D {
    x: i32,
}

// `Deref` implementation is required for `DerefMut`.
impl ::core::ops::Deref for Point1D {
    type Target = i32;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.x
    }
}

#[derive(DerefMut)]
struct CoolVec {
    cool: bool,
    #[deref_mut]
    vec: Vec<i32>,
}

// `Deref` implementation is required for `DerefMut`.
impl ::core::ops::Deref for CoolVec {
    type Target = Vec<i32>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.vec
    }
}

#[derive(DerefMut)]
struct GenericVec<T>(Vec<T>);

// `Deref` implementation is required for `DerefMut`.
impl<T> ::core::ops::Deref for GenericVec<T> {
    type Target = Vec<T>;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[test]
fn deref_mut_generic() {
    let mut gv = GenericVec::<i32>(vec![42]);
    assert!(gv.get_mut(0).is_some());
}

#[derive(DerefMut)]
struct GenericBox<T>(#[deref_mut(forward)] Box<T>);

// `Deref` implementation is required for `DerefMut`.
impl<T> ::core::ops::Deref for GenericBox<T>
where
    Box<T>: ::core::ops::Deref,
{
    type Target = <Box<T> as ::core::ops::Deref>::Target;
    #[inline]
    fn deref(&self) -> &Self::Target {
        <Box<T> as ::core::ops::Deref>::deref(&self.0)
    }
}

#[test]
fn deref_mut_generic_forward() {
    let mut boxed = GenericBox(Box::new(1i32));
    *boxed = 3;
    assert_eq!(*boxed, 3i32);
}

#[cfg(nightly)]
mod never {
    use super::*;

    #[derive(DerefMut)]
    struct Tuple(!);

    // `Deref` implementation is required for `DerefMut`.
    impl ::core::ops::Deref for Tuple {
        type Target = !;
        #[inline]
        fn deref(&self) -> &Self::Target {
            self.0
        }
    }

    #[derive(DerefMut)]
    struct Struct {
        field: !,
    }

    // `Deref` implementation is required for `DerefMut`.
    impl ::core::ops::Deref for Struct {
        type Target = !;
        #[inline]
        fn deref(&self) -> &Self::Target {
            self.field
        }
    }
}
