// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

// In this case consistency between impls is more important
// than using pointer casts
#![allow(clippy::transmute_ptr_to_ptr)]

use crate::Yokeable;
use core::{
    mem::{self, ManuallyDrop},
    ptr,
};

macro_rules! copy_yoke_impl {
    () => {
        #[inline]
        fn transform(&self) -> &Self::Output {
            self
        }
        #[inline]
        fn transform_owned(self) -> Self::Output {
            self
        }
        #[inline]
        unsafe fn make(this: Self::Output) -> Self {
            this
        }
        #[inline]
        fn transform_mut<F>(&'a mut self, f: F)
        where
            F: 'static + for<'b> FnOnce(&'b mut Self::Output),
        {
            f(self)
        }
    };
}
macro_rules! impl_copy_type {
    ($ty:ty) => {
        // Safety: all the types that this macro is used to generate impls of Yokeable for do not
        // borrow any memory.
        unsafe impl<'a> Yokeable<'a> for $ty {
            type Output = Self;
            copy_yoke_impl!();
        }
    };
}

impl_copy_type!(());
impl_copy_type!(u8);
impl_copy_type!(u16);
impl_copy_type!(u32);
impl_copy_type!(u64);
impl_copy_type!(u128);
impl_copy_type!(usize);
impl_copy_type!(i8);
impl_copy_type!(i16);
impl_copy_type!(i32);
impl_copy_type!(i64);
impl_copy_type!(i128);
impl_copy_type!(isize);
impl_copy_type!(char);
impl_copy_type!(bool);

// This is for when we're implementing Yoke on a complex type such that it's not
// obvious to the compiler that the lifetime is covariant
//
// Safety: the caller of this macro must ensure that `Self` is indeed covariant in 'a.
macro_rules! unsafe_complex_yoke_impl {
    () => {
        fn transform(&'a self) -> &'a Self::Output {
            // Safety: equivalent to casting the lifetime. Macro caller ensures covariance.
            unsafe { mem::transmute(self) }
        }

        fn transform_owned(self) -> Self::Output {
            debug_assert!(mem::size_of::<Self::Output>() == mem::size_of::<Self>());
            // Safety: equivalent to casting the lifetime. Macro caller ensures covariance.
            unsafe {
                let ptr: *const Self::Output = (&self as *const Self).cast();
                let _ = ManuallyDrop::new(self);
                ptr::read(ptr)
            }
        }

        unsafe fn make(from: Self::Output) -> Self {
            debug_assert!(mem::size_of::<Self::Output>() == mem::size_of::<Self>());
            let ptr: *const Self = (&from as *const Self::Output).cast();
            let _ = ManuallyDrop::new(from);
            // Safety: `ptr` is certainly valid, aligned and points to a properly initialized value, as
            // it comes from a value that was moved into a ManuallyDrop.
            unsafe { ptr::read(ptr) }
        }

        fn transform_mut<F>(&'a mut self, f: F)
        where
            F: 'static + for<'b> FnOnce(&'b mut Self::Output),
        {
            // Cast away the lifetime of Self
            // Safety: this is equivalent to f(transmute(self)), and the documentation of the trait
            // method explains why doing so is sound.
            unsafe { f(mem::transmute::<&'a mut Self, &'a mut Self::Output>(self)) }
        }
    };
}

// Safety: since T implements Yokeable<'a>, Option<T<'b>> must be covariant on 'b or the Yokeable
// implementation on T would be unsound.
unsafe impl<'a, T: 'static + for<'b> Yokeable<'b>> Yokeable<'a> for Option<T> {
    type Output = Option<<T as Yokeable<'a>>::Output>;
    unsafe_complex_yoke_impl!();
}

// Safety: since T1, T2 implement Yokeable<'a>, (T1<'b>, T2<'b>) must be covariant on 'b or the Yokeable
// implementation on T would be unsound.
unsafe impl<'a, T1: 'static + for<'b> Yokeable<'b>, T2: 'static + for<'b> Yokeable<'b>> Yokeable<'a>
    for (T1, T2)
{
    type Output = (<T1 as Yokeable<'a>>::Output, <T2 as Yokeable<'a>>::Output);
    unsafe_complex_yoke_impl!();
}

// Safety: since T implements Yokeable<'a>, [T<'b>; N] must be covariant on 'b or the Yokeable
// implementation on T would be unsound.
unsafe impl<'a, T: 'static + for<'b> Yokeable<'b>, const N: usize> Yokeable<'a> for [T; N] {
    type Output = [<T as Yokeable<'a>>::Output; N];
    unsafe_complex_yoke_impl!();
}
