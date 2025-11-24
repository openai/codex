// This file is part of ICU4X. For terms of use, please see the file
// called LICENSE at the top level of the ICU4X source tree
// (online at: https://github.com/unicode-org/icu4x/blob/main/LICENSE ).

use super::vecs::{MutableZeroVecLike, ZeroVecLike};
use crate::ule::vartuple::VarTupleULE;
use crate::ule::*;
use crate::vecs::{VarZeroSlice, VarZeroVec};
use crate::zerovec::{ZeroSlice, ZeroVec};
use alloc::boxed::Box;

/// Trait marking types which are allowed to be keys or values in [`ZeroMap`](super::ZeroMap).
///
/// Users should not be calling methods of this trait directly, however if you are
/// implementing your own [`AsULE`] or [`VarULE`] type you may wish to implement
/// this trait.
// this lifetime should be a GAT on Container once that is possible
pub trait ZeroMapKV<'a> {
    /// The container that can be used with this type: [`ZeroVec`] or [`VarZeroVec`].
    type Container: MutableZeroVecLike<
            'a,
            Self,
            SliceVariant = Self::Slice,
            GetType = Self::GetType,
            OwnedType = Self::OwnedType,
        > + Sized;
    type Slice: ZeroVecLike<Self, GetType = Self::GetType> + ?Sized;
    /// The type produced by `Container::get()`
    ///
    /// This type will be predetermined by the choice of `Self::Container`:
    /// For sized types this must be `T::ULE`, and for unsized types this must be `T`
    type GetType: ?Sized + 'static;
    /// The type produced by `Container::replace()` and `Container::remove()`,
    /// also used during deserialization. If `Self` is human readable serialized,
    /// deserializing to `Self::OwnedType` should produce the same value once
    /// passed through `Self::owned_as_self()`
    ///
    /// This type will be predetermined by the choice of `Self::Container`:
    /// For sized types this must be `T` and for unsized types this must be `Box<T>`
    type OwnedType: 'static;
}

macro_rules! impl_sized_kv {
    ($ty:path) => {
        impl<'a> ZeroMapKV<'a> for $ty {
            type Container = ZeroVec<'a, $ty>;
            type Slice = ZeroSlice<$ty>;
            type GetType = <$ty as AsULE>::ULE;
            type OwnedType = $ty;
        }
    };
}

impl_sized_kv!(u8);
impl_sized_kv!(u16);
impl_sized_kv!(u32);
impl_sized_kv!(u64);
impl_sized_kv!(u128);
impl_sized_kv!(i8);
impl_sized_kv!(i16);
impl_sized_kv!(i32);
impl_sized_kv!(i64);
impl_sized_kv!(i128);
impl_sized_kv!(char);
impl_sized_kv!(f32);
impl_sized_kv!(f64);

impl_sized_kv!(core::num::NonZeroU8);
impl_sized_kv!(core::num::NonZeroI8);

impl<'a, T> ZeroMapKV<'a> for Option<T>
where
    Option<T>: AsULE + 'static,
{
    type Container = ZeroVec<'a, Option<T>>;
    type Slice = ZeroSlice<Option<T>>;
    type GetType = <Option<T> as AsULE>::ULE;
    type OwnedType = Option<T>;
}

impl<'a, T> ZeroMapKV<'a> for OptionVarULE<T>
where
    T: VarULE + ?Sized,
{
    type Container = VarZeroVec<'a, OptionVarULE<T>>;
    type Slice = VarZeroSlice<OptionVarULE<T>>;
    type GetType = OptionVarULE<T>;
    type OwnedType = Box<OptionVarULE<T>>;
}

impl<'a, A, B> ZeroMapKV<'a> for VarTupleULE<A, B>
where
    A: AsULE + 'static,
    B: VarULE + ?Sized,
{
    type Container = VarZeroVec<'a, VarTupleULE<A, B>>;
    type Slice = VarZeroSlice<VarTupleULE<A, B>>;
    type GetType = VarTupleULE<A, B>;
    type OwnedType = Box<VarTupleULE<A, B>>;
}

impl<'a> ZeroMapKV<'a> for str {
    type Container = VarZeroVec<'a, str>;
    type Slice = VarZeroSlice<str>;
    type GetType = str;
    type OwnedType = Box<str>;
}

impl<'a, T> ZeroMapKV<'a> for [T]
where
    T: ULE + AsULE<ULE = T>,
{
    type Container = VarZeroVec<'a, [T]>;
    type Slice = VarZeroSlice<[T]>;
    type GetType = [T];
    type OwnedType = Box<[T]>;
}

impl<'a, T, const N: usize> ZeroMapKV<'a> for [T; N]
where
    T: AsULE + 'static,
{
    type Container = ZeroVec<'a, [T; N]>;
    type Slice = ZeroSlice<[T; N]>;
    type GetType = [T::ULE; N];
    type OwnedType = [T; N];
}

impl<'a, T> ZeroMapKV<'a> for ZeroSlice<T>
where
    T: AsULE + 'static,
{
    type Container = VarZeroVec<'a, ZeroSlice<T>>;
    type Slice = VarZeroSlice<ZeroSlice<T>>;
    type GetType = ZeroSlice<T>;
    type OwnedType = Box<ZeroSlice<T>>;
}
