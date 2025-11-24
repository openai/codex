/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

//! A trait to represent zero-cost conversions.

use std::alloc::Layout;
use std::collections::HashMap;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ptr;

pub use starlark_derive::Coerce;
use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;

/// A marker trait such that the existence of `From: Coerce<To>` implies
/// that `From` can be treat as `To` without any data manipulation.
/// Particularly useful for containers, e.g. `Vec<From>` can be treated as
/// `Vec<To>` in _O(1)_. If such an instance is available,
/// you can use [`coerce`] to perform the conversion.
///
/// Importantly, you must make sure Rust does not change the type representation
/// between the different types (typically using a `repr` directive),
/// and it must be safe for the `From` to be treated as `To`, namely same (or less restrictive) alignment,
/// no additional invariants, value can be dropped as `To`.
///
/// If you only need [`coerce`] on newtype references,
/// then the [`ref-cast` crate](https://crates.io/crates/ref-cast)
/// provides that, along with automatic derivations (no `unsafe` required).
pub unsafe trait Coerce<To: ?Sized> {}

/// A marker trait such that the existence of `From: CoerceKey<To>` implies
/// that `From` can be treat as `To` without any data manipulation.
/// Furthermore, above and beyond [`Coerce`], any provided [`Hash`](std::hash::Hash),
/// [`Eq`], [`PartialEq`], [`Ord`] and [`PartialOrd`] traits must give identical results
/// on the `From` and `To` values.
///
/// This trait is mostly expected to be a requirement for the keys of associative-map
/// containers, hence the `Key` in the name.
pub unsafe trait CoerceKey<To: ?Sized>: Coerce<To> {}

unsafe impl<'a, From: ?Sized, To: ?Sized> Coerce<&'a To> for &'a From where From: Coerce<To> {}
unsafe impl<'a, From: ?Sized, To: ?Sized> CoerceKey<&'a To> for &'a From where From: CoerceKey<To> {}

unsafe impl<From, To> Coerce<[To]> for [From] where From: Coerce<To> {}
unsafe impl<From, To> CoerceKey<[To]> for [From] where From: CoerceKey<To> {}

unsafe impl<From, To> Coerce<Vec<To>> for Vec<From> where From: Coerce<To> {}
unsafe impl<From, To> CoerceKey<Vec<To>> for Vec<From> where From: CoerceKey<To> {}

unsafe impl<From: ?Sized, To: ?Sized> CoerceKey<Box<To>> for Box<From> where From: CoerceKey<To> {}
unsafe impl<From: ?Sized, To: ?Sized> Coerce<Box<To>> for Box<From> where From: Coerce<To> {}

unsafe impl<From, To> Coerce<HashSet<To>> for HashSet<From> where From: CoerceKey<To> {}

unsafe impl<FromK, FromV, ToK, ToV> Coerce<HashMap<ToK, ToV>> for HashMap<FromK, FromV>
where
    FromK: CoerceKey<ToK>,
    FromV: Coerce<ToV>,
{
}

unsafe impl<From1: Coerce<To1>, To1> Coerce<(To1,)> for (From1,) {}
unsafe impl<From1: CoerceKey<To1>, To1> CoerceKey<(To1,)> for (From1,) {}

unsafe impl<From1: Coerce<To1>, From2: Coerce<To2>, To1, To2> Coerce<(To1, To2)>
    for (From1, From2)
{
}
unsafe impl<From1: CoerceKey<To1>, From2: CoerceKey<To2>, To1, To2> CoerceKey<(To1, To2)>
    for (From1, From2)
{
}
unsafe impl<From: Coerce<To>, To, const N: usize> Coerce<[To; N]> for [From; N] {}
unsafe impl<From: CoerceKey<To>, To, const N: usize> CoerceKey<[To; N]> for [From; N] {}

unsafe impl<From, To> Coerce<PhantomData<To>> for PhantomData<From> {}

// We can't define a blanket `Coerce<T> for T` because that conflicts with the specific traits above.
// Therefore, we define instances where we think they might be useful, rather than trying to do every concrete type.
unsafe impl Coerce<String> for String {}
unsafe impl CoerceKey<String> for String {}

unsafe impl Coerce<str> for str {}
unsafe impl CoerceKey<str> for str {}

unsafe impl Coerce<()> for () {}
unsafe impl CoerceKey<()> for () {}

unsafe impl<FromK, FromV, ToK, ToV> Coerce<SmallMap<ToK, ToV>> for SmallMap<FromK, FromV>
where
    FromK: CoerceKey<ToK>,
    FromV: Coerce<ToV>,
{
}

unsafe impl<From, To> Coerce<SmallSet<To>> for SmallSet<From> where From: Coerce<To> {}

/// Safely convert between types which have a `Coerce` relationship.
/// Often the second type argument will need to be given explicitly,
/// e.g. `coerce::<_, ToType>(x)`.
#[inline]
pub fn coerce<From, To>(x: From) -> To
where
    From: Coerce<To>,
{
    assert_eq!(Layout::new::<From>(), Layout::new::<To>());
    let x = ManuallyDrop::new(x);
    unsafe { ptr::read(x.deref() as *const From as *const To) }
}

#[cfg(test)]
mod tests {
    use std::marker;

    use super::*;
    use crate as starlark;

    #[test]
    fn test_ptr_coerce() {
        fn f<'v>(x: (&'static str,)) -> (&'v str,) {
            coerce(x)
        }

        let x = "test".to_owned();
        assert_eq!(f(("test",)), (x.as_str(),))
    }

    #[test]
    fn test_coerce_type_and_lifetime_params() {
        #[repr(C)]
        struct Aaa<'a>(&'a u32);
        #[repr(C)]
        struct Bbb<'a>(&'a u32);

        unsafe impl<'a> Coerce<Bbb<'a>> for Aaa<'a> {}

        #[derive(Coerce)]
        #[repr(C)]
        struct StructWithLifetimeAndTypeParams<'a, X> {
            x: X,
            _marker: marker::PhantomData<&'a u32>,
        }

        let ten = 10;
        let old = StructWithLifetimeAndTypeParams::<Aaa> {
            x: Aaa(&ten),
            _marker: marker::PhantomData,
        };

        let new: StructWithLifetimeAndTypeParams<Bbb> = coerce(old);
        assert_eq!(10, *new.x.0);
    }

    #[test]
    fn test_coerce_is_unsound() {
        // TODO(nga): fix it.

        #[repr(transparent)]
        struct Newtype(u8);

        unsafe impl Coerce<u8> for Newtype {}

        #[derive(Coerce)]
        #[repr(transparent)]
        struct Struct<T: Trait>(T::Assoc);

        trait Trait {
            type Assoc;
        }
        impl Trait for u8 {
            type Assoc = ();
        }
        impl Trait for Newtype {
            type Assoc = [u8; 50];
        }

        let s: &Struct<u8> = &Struct(());
        // This should fail to compile because `[u8; 50]` is not coercible to `()`.
        let _c: &Struct<Newtype> = coerce(s);
    }
}
