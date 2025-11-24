//! This crate implements a structure that can be used as a generic array type.
//! Core Rust array types `[T; N]` can't be used generically with
//! respect to `N`, so for example this:
//!
//! ```rust{compile_fail}
//! struct Foo<T, N> {
//!     data: [T; N]
//! }
//! ```
//!
//! won't work.
//!
//! **generic-array** exports a `GenericArray<T,N>` type, which lets
//! the above be implemented as:
//!
//! ```rust
//! use generic_array::{ArrayLength, GenericArray};
//!
//! struct Foo<T, N: ArrayLength<T>> {
//!     data: GenericArray<T,N>
//! }
//! ```
//!
//! The `ArrayLength<T>` trait is implemented by default for
//! [unsigned integer types](../typenum/uint/index.html) from
//! [typenum](../typenum/index.html):
//!
//! ```rust
//! # use generic_array::{ArrayLength, GenericArray};
//! use generic_array::typenum::U5;
//!
//! struct Foo<N: ArrayLength<i32>> {
//!     data: GenericArray<i32, N>
//! }
//!
//! # fn main() {
//! let foo = Foo::<U5>{data: GenericArray::default()};
//! # }
//! ```
//!
//! For example, `GenericArray<T, U5>` would work almost like `[T; 5]`:
//!
//! ```rust
//! # use generic_array::{ArrayLength, GenericArray};
//! use generic_array::typenum::U5;
//!
//! struct Foo<T, N: ArrayLength<T>> {
//!     data: GenericArray<T, N>
//! }
//!
//! # fn main() {
//! let foo = Foo::<i32, U5>{data: GenericArray::default()};
//! # }
//! ```
//!
//! For ease of use, an `arr!` macro is provided - example below:
//!
//! ```
//! # #[macro_use]
//! # extern crate generic_array;
//! # extern crate typenum;
//! # fn main() {
//! let array = arr![u32; 1, 2, 3];
//! assert_eq!(array[2], 3);
//! # }
//! ```

#![deny(missing_docs)]
#![deny(meta_variable_misuse)]
#![no_std]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

#[cfg(feature = "serde")]
extern crate serde;

#[cfg(feature = "zeroize")]
extern crate zeroize;

#[cfg(test)]
extern crate bincode;

pub extern crate typenum;

mod hex;
mod impls;

#[cfg(feature = "serde")]
mod impl_serde;

#[cfg(feature = "zeroize")]
mod impl_zeroize;

use core::iter::FromIterator;
use core::marker::PhantomData;
use core::mem::{MaybeUninit, ManuallyDrop};
use core::ops::{Deref, DerefMut};
use core::{mem, ptr, slice};
use typenum::bit::{B0, B1};
use typenum::uint::{UInt, UTerm, Unsigned};

#[cfg_attr(test, macro_use)]
pub mod arr;
pub mod functional;
pub mod iter;
pub mod sequence;

use self::functional::*;
pub use self::iter::GenericArrayIter;
use self::sequence::*;

/// Trait making `GenericArray` work, marking types to be used as length of an array
pub unsafe trait ArrayLength<T>: Unsigned {
    /// Associated type representing the array type for the number
    type ArrayType;
}

unsafe impl<T> ArrayLength<T> for UTerm {
    #[doc(hidden)]
    type ArrayType = [T; 0];
}

/// Internal type used to generate a struct of appropriate size
#[allow(dead_code)]
#[repr(C)]
#[doc(hidden)]
pub struct GenericArrayImplEven<T, U> {
    parent1: U,
    parent2: U,
    _marker: PhantomData<T>,
}

impl<T: Clone, U: Clone> Clone for GenericArrayImplEven<T, U> {
    fn clone(&self) -> GenericArrayImplEven<T, U> {
        GenericArrayImplEven {
            parent1: self.parent1.clone(),
            parent2: self.parent2.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T: Copy, U: Copy> Copy for GenericArrayImplEven<T, U> {}

/// Internal type used to generate a struct of appropriate size
#[allow(dead_code)]
#[repr(C)]
#[doc(hidden)]
pub struct GenericArrayImplOdd<T, U> {
    parent1: U,
    parent2: U,
    data: T,
}

impl<T: Clone, U: Clone> Clone for GenericArrayImplOdd<T, U> {
    fn clone(&self) -> GenericArrayImplOdd<T, U> {
        GenericArrayImplOdd {
            parent1: self.parent1.clone(),
            parent2: self.parent2.clone(),
            data: self.data.clone(),
        }
    }
}

impl<T: Copy, U: Copy> Copy for GenericArrayImplOdd<T, U> {}

unsafe impl<T, N: ArrayLength<T>> ArrayLength<T> for UInt<N, B0> {
    #[doc(hidden)]
    type ArrayType = GenericArrayImplEven<T, N::ArrayType>;
}

unsafe impl<T, N: ArrayLength<T>> ArrayLength<T> for UInt<N, B1> {
    #[doc(hidden)]
    type ArrayType = GenericArrayImplOdd<T, N::ArrayType>;
}

/// Struct representing a generic array - `GenericArray<T, N>` works like [T; N]
#[allow(dead_code)]
#[repr(transparent)]
pub struct GenericArray<T, U: ArrayLength<T>> {
    data: U::ArrayType,
}

unsafe impl<T: Send, N: ArrayLength<T>> Send for GenericArray<T, N> {}
unsafe impl<T: Sync, N: ArrayLength<T>> Sync for GenericArray<T, N> {}

impl<T, N> Deref for GenericArray<T, N>
where
    N: ArrayLength<T>,
{
    type Target = [T];

    #[inline(always)]
    fn deref(&self) -> &[T] {
        unsafe { slice::from_raw_parts(self as *const Self as *const T, N::USIZE) }
    }
}

impl<T, N> DerefMut for GenericArray<T, N>
where
    N: ArrayLength<T>,
{
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { slice::from_raw_parts_mut(self as *mut Self as *mut T, N::USIZE) }
    }
}

/// Creates an array one element at a time using a mutable iterator
/// you can write to with `ptr::write`.
///
/// Increment the position while iterating to mark off created elements,
/// which will be dropped if `into_inner` is not called.
#[doc(hidden)]
pub struct ArrayBuilder<T, N: ArrayLength<T>> {
    array: MaybeUninit<GenericArray<T, N>>,
    position: usize,
}

impl<T, N: ArrayLength<T>> ArrayBuilder<T, N> {
    #[doc(hidden)]
    #[inline]
    pub unsafe fn new() -> ArrayBuilder<T, N> {
        ArrayBuilder {
            array: MaybeUninit::uninit(),
            position: 0,
        }
    }

    /// Creates a mutable iterator for writing to the array using `ptr::write`.
    ///
    /// Increment the position value given as a mutable reference as you iterate
    /// to mark how many elements have been created.
    #[doc(hidden)]
    #[inline]
    pub unsafe fn iter_position(&mut self) -> (slice::IterMut<T>, &mut usize) {
        ((&mut *self.array.as_mut_ptr()).iter_mut(), &mut self.position)
    }

    /// When done writing (assuming all elements have been written to),
    /// get the inner array.
    #[doc(hidden)]
    #[inline]
    pub unsafe fn into_inner(self) -> GenericArray<T, N> {
        let array = ptr::read(&self.array);

        mem::forget(self);

        array.assume_init()
    }
}

impl<T, N: ArrayLength<T>> Drop for ArrayBuilder<T, N> {
    fn drop(&mut self) {
        if mem::needs_drop::<T>() {
            unsafe {
                for value in &mut (&mut *self.array.as_mut_ptr())[..self.position] {
                    ptr::drop_in_place(value);
                }
            }
        }
    }
}

/// Consumes an array.
///
/// Increment the position while iterating and any leftover elements
/// will be dropped if position does not go to N
#[doc(hidden)]
pub struct ArrayConsumer<T, N: ArrayLength<T>> {
    array: ManuallyDrop<GenericArray<T, N>>,
    position: usize,
}

impl<T, N: ArrayLength<T>> ArrayConsumer<T, N> {
    #[doc(hidden)]
    #[inline]
    pub unsafe fn new(array: GenericArray<T, N>) -> ArrayConsumer<T, N> {
        ArrayConsumer {
            array: ManuallyDrop::new(array),
            position: 0,
        }
    }

    /// Creates an iterator and mutable reference to the internal position
    /// to keep track of consumed elements.
    ///
    /// Increment the position as you iterate to mark off consumed elements
    #[doc(hidden)]
    #[inline]
    pub unsafe fn iter_position(&mut self) -> (slice::Iter<T>, &mut usize) {
        (self.array.iter(), &mut self.position)
    }
}

impl<T, N: ArrayLength<T>> Drop for ArrayConsumer<T, N> {
    fn drop(&mut self) {
        if mem::needs_drop::<T>() {
            for value in &mut self.array[self.position..N::USIZE] {
                unsafe {
                    ptr::drop_in_place(value);
                }
            }
        }
    }
}

impl<'a, T: 'a, N> IntoIterator for &'a GenericArray<T, N>
where
    N: ArrayLength<T>,
{
    type IntoIter = slice::Iter<'a, T>;
    type Item = &'a T;

    fn into_iter(self: &'a GenericArray<T, N>) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl<'a, T: 'a, N> IntoIterator for &'a mut GenericArray<T, N>
where
    N: ArrayLength<T>,
{
    type IntoIter = slice::IterMut<'a, T>;
    type Item = &'a mut T;

    fn into_iter(self: &'a mut GenericArray<T, N>) -> Self::IntoIter {
        self.as_mut_slice().iter_mut()
    }
}

impl<T, N> FromIterator<T> for GenericArray<T, N>
where
    N: ArrayLength<T>,
{
    fn from_iter<I>(iter: I) -> GenericArray<T, N>
    where
        I: IntoIterator<Item = T>,
    {
        unsafe {
            let mut destination = ArrayBuilder::new();

            {
                let (destination_iter, position) = destination.iter_position();

                iter.into_iter()
                    .zip(destination_iter)
                    .for_each(|(src, dst)| {
                        ptr::write(dst, src);

                        *position += 1;
                    });
            }

            if destination.position < N::USIZE {
                from_iter_length_fail(destination.position, N::USIZE);
            }

            destination.into_inner()
        }
    }
}

#[inline(never)]
#[cold]
fn from_iter_length_fail(length: usize, expected: usize) -> ! {
    panic!(
        "GenericArray::from_iter received {} elements but expected {}",
        length, expected
    );
}

unsafe impl<T, N> GenericSequence<T> for GenericArray<T, N>
where
    N: ArrayLength<T>,
    Self: IntoIterator<Item = T>,
{
    type Length = N;
    type Sequence = Self;

    fn generate<F>(mut f: F) -> GenericArray<T, N>
    where
        F: FnMut(usize) -> T,
    {
        unsafe {
            let mut destination = ArrayBuilder::new();

            {
                let (destination_iter, position) = destination.iter_position();

                destination_iter.enumerate().for_each(|(i, dst)| {
                    ptr::write(dst, f(i));

                    *position += 1;
                });
            }

            destination.into_inner()
        }
    }

    #[doc(hidden)]
    fn inverted_zip<B, U, F>(
        self,
        lhs: GenericArray<B, Self::Length>,
        mut f: F,
    ) -> MappedSequence<GenericArray<B, Self::Length>, B, U>
    where
        GenericArray<B, Self::Length>:
            GenericSequence<B, Length = Self::Length> + MappedGenericSequence<B, U>,
        Self: MappedGenericSequence<T, U>,
        Self::Length: ArrayLength<B> + ArrayLength<U>,
        F: FnMut(B, Self::Item) -> U,
    {
        unsafe {
            let mut left = ArrayConsumer::new(lhs);
            let mut right = ArrayConsumer::new(self);

            let (left_array_iter, left_position) = left.iter_position();
            let (right_array_iter, right_position) = right.iter_position();

            FromIterator::from_iter(left_array_iter.zip(right_array_iter).map(|(l, r)| {
                let left_value = ptr::read(l);
                let right_value = ptr::read(r);

                *left_position += 1;
                *right_position += 1;

                f(left_value, right_value)
            }))
        }
    }

    #[doc(hidden)]
    fn inverted_zip2<B, Lhs, U, F>(self, lhs: Lhs, mut f: F) -> MappedSequence<Lhs, B, U>
    where
        Lhs: GenericSequence<B, Length = Self::Length> + MappedGenericSequence<B, U>,
        Self: MappedGenericSequence<T, U>,
        Self::Length: ArrayLength<B> + ArrayLength<U>,
        F: FnMut(Lhs::Item, Self::Item) -> U,
    {
        unsafe {
            let mut right = ArrayConsumer::new(self);

            let (right_array_iter, right_position) = right.iter_position();

            FromIterator::from_iter(
                lhs.into_iter()
                    .zip(right_array_iter)
                    .map(|(left_value, r)| {
                        let right_value = ptr::read(r);

                        *right_position += 1;

                        f(left_value, right_value)
                    }),
            )
        }
    }
}

unsafe impl<T, U, N> MappedGenericSequence<T, U> for GenericArray<T, N>
where
    N: ArrayLength<T> + ArrayLength<U>,
    GenericArray<U, N>: GenericSequence<U, Length = N>,
{
    type Mapped = GenericArray<U, N>;
}

unsafe impl<T, N> FunctionalSequence<T> for GenericArray<T, N>
where
    N: ArrayLength<T>,
    Self: GenericSequence<T, Item = T, Length = N>,
{
    fn map<U, F>(self, mut f: F) -> MappedSequence<Self, T, U>
    where
        Self::Length: ArrayLength<U>,
        Self: MappedGenericSequence<T, U>,
        F: FnMut(T) -> U,
    {
        unsafe {
            let mut source = ArrayConsumer::new(self);

            let (array_iter, position) = source.iter_position();

            FromIterator::from_iter(array_iter.map(|src| {
                let value = ptr::read(src);

                *position += 1;

                f(value)
            }))
        }
    }

    #[inline]
    fn zip<B, Rhs, U, F>(self, rhs: Rhs, f: F) -> MappedSequence<Self, T, U>
    where
        Self: MappedGenericSequence<T, U>,
        Rhs: MappedGenericSequence<B, U, Mapped = MappedSequence<Self, T, U>>,
        Self::Length: ArrayLength<B> + ArrayLength<U>,
        Rhs: GenericSequence<B, Length = Self::Length>,
        F: FnMut(T, Rhs::Item) -> U,
    {
        rhs.inverted_zip(self, f)
    }

    fn fold<U, F>(self, init: U, mut f: F) -> U
    where
        F: FnMut(U, T) -> U,
    {
        unsafe {
            let mut source = ArrayConsumer::new(self);

            let (array_iter, position) = source.iter_position();

            array_iter.fold(init, |acc, src| {
                let value = ptr::read(src);

                *position += 1;

                f(acc, value)
            })
        }
    }
}

impl<T, N> GenericArray<T, N>
where
    N: ArrayLength<T>,
{
    /// Extracts a slice containing the entire array.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self.deref()
    }

    /// Extracts a mutable slice containing the entire array.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.deref_mut()
    }

    /// Converts slice to a generic array reference with inferred length;
    ///
    /// # Panics
    ///
    /// Panics if the slice is not equal to the length of the array.
    #[inline]
    pub fn from_slice(slice: &[T]) -> &GenericArray<T, N> {
        slice.into()
    }

    /// Converts mutable slice to a mutable generic array reference
    ///
    /// # Panics
    ///
    /// Panics if the slice is not equal to the length of the array.
    #[inline]
    pub fn from_mut_slice(slice: &mut [T]) -> &mut GenericArray<T, N> {
        slice.into()
    }
}

impl<'a, T, N: ArrayLength<T>> From<&'a [T]> for &'a GenericArray<T, N> {
    /// Converts slice to a generic array reference with inferred length;
    ///
    /// # Panics
    ///
    /// Panics if the slice is not equal to the length of the array.
    #[inline]
    fn from(slice: &[T]) -> &GenericArray<T, N> {
        assert_eq!(slice.len(), N::USIZE);

        unsafe { &*(slice.as_ptr() as *const GenericArray<T, N>) }
    }
}

impl<'a, T, N: ArrayLength<T>> From<&'a mut [T]> for &'a mut GenericArray<T, N> {
    /// Converts mutable slice to a mutable generic array reference
    ///
    /// # Panics
    ///
    /// Panics if the slice is not equal to the length of the array.
    #[inline]
    fn from(slice: &mut [T]) -> &mut GenericArray<T, N> {
        assert_eq!(slice.len(), N::USIZE);

        unsafe { &mut *(slice.as_mut_ptr() as *mut GenericArray<T, N>) }
    }
}

impl<T: Clone, N> GenericArray<T, N>
where
    N: ArrayLength<T>,
{
    /// Construct a `GenericArray` from a slice by cloning its content
    ///
    /// # Panics
    ///
    /// Panics if the slice is not equal to the length of the array.
    #[inline]
    pub fn clone_from_slice(list: &[T]) -> GenericArray<T, N> {
        Self::from_exact_iter(list.iter().cloned())
            .expect("Slice must be the same length as the array")
    }
}

impl<T, N> GenericArray<T, N>
where
    N: ArrayLength<T>,
{
    /// Creates a new `GenericArray` instance from an iterator with a specific size.
    ///
    /// Returns `None` if the size is not equal to the number of elements in the `GenericArray`.
    pub fn from_exact_iter<I>(iter: I) -> Option<Self>
    where
        I: IntoIterator<Item = T>,
    {
        let mut iter = iter.into_iter();

        unsafe {
            let mut destination = ArrayBuilder::new();

            {
                let (destination_iter, position) = destination.iter_position();

                destination_iter.zip(&mut iter).for_each(|(dst, src)| {
                    ptr::write(dst, src);

                    *position += 1;
                });

                // The iterator produced fewer than `N` elements.
                if *position != N::USIZE {
                    return None;
                }

                // The iterator produced more than `N` elements.
                if iter.next().is_some() {
                    return None;
                }
            }

            Some(destination.into_inner())
        }
    }
}

/// A reimplementation of the `transmute` function, avoiding problems
/// when the compiler can't prove equal sizes.
#[inline]
#[doc(hidden)]
pub unsafe fn transmute<A, B>(a: A) -> B {
    let a = ManuallyDrop::new(a);
    ::core::ptr::read(&*a as *const A as *const B)
}

#[cfg(test)]
mod test {
    // Compile with:
    // cargo rustc --lib --profile test --release --
    //      -C target-cpu=native -C opt-level=3 --emit asm
    // and view the assembly to make sure test_assembly generates
    // SIMD instructions instead of a naive loop.

    #[inline(never)]
    pub fn black_box<T>(val: T) -> T {
        use core::{mem, ptr};

        let ret = unsafe { ptr::read_volatile(&val) };
        mem::forget(val);
        ret
    }

    #[test]
    fn test_assembly() {
        use crate::functional::*;

        let a = black_box(arr![i32; 1, 3, 5, 7]);
        let b = black_box(arr![i32; 2, 4, 6, 8]);

        let c = (&a).zip(b, |l, r| l + r);

        let d = a.fold(0, |a, x| a + x);

        assert_eq!(c, arr![i32; 3, 7, 11, 15]);

        assert_eq!(d, 16);
    }
}
