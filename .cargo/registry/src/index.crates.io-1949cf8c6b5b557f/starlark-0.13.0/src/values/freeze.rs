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

use std::cell::OnceCell;
use std::cell::RefCell;
use std::cell::UnsafeCell;
use std::marker;
use std::marker::PhantomData;

use starlark_map::small_map::SmallMap;
use starlark_map::small_set::SmallSet;
use starlark_map::Hashed;
use starlark_syntax::slice_vec_ext::VecExt;

use crate::values::FreezeResult;
use crate::values::Freezer;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::StringValue;
use crate::values::Value;

/// Need to be implemented for non-simple `StarlarkValue`.
///
/// This is called on freeze of the heap. Must produce a replacement object to place
/// in the frozen heap.
///
/// For relatively simple cases it can be implemented with `#[derive(Freeze)]`:
///
/// ```
/// # struct AdditionalData;
///
/// use starlark::values::Freeze;
/// use starlark::values::FreezeResult;
///
/// #[derive(Freeze)]
/// struct MyType<V> {
///     value: V,
///     // This field does not implement `Freeze`, but we can use it as is for freeze.
///     #[freeze(identity)]
///     data: AdditionalData,
/// }
/// ```
pub trait Freeze {
    /// When type is frozen, it is frozen into this type.
    type Frozen;

    /// Freeze a value. The frozen value _must_ be equal to the original,
    /// and produce the same hash.
    ///
    /// Note during freeze, `Value` objects in `Self` might be already special forward-objects,
    /// trying to unpack these objects will crash the process.
    /// So the function is only allowed to access `Value` objects after it froze them.
    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen>;
}

impl Freeze for String {
    type Frozen = String;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<String> {
        Ok(self)
    }
}

impl Freeze for i32 {
    type Frozen = i32;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<i32> {
        Ok(self)
    }
}

impl Freeze for u32 {
    type Frozen = u32;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<u32> {
        Ok(self)
    }
}

impl Freeze for i64 {
    type Frozen = i64;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<i64> {
        Ok(self)
    }
}

impl Freeze for u64 {
    type Frozen = u64;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<u64> {
        Ok(self)
    }
}

impl Freeze for usize {
    type Frozen = usize;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<usize> {
        Ok(self)
    }
}

impl Freeze for bool {
    type Frozen = bool;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<bool> {
        Ok(self)
    }
}

impl<'v, T: 'static> Freeze for marker::PhantomData<&'v T> {
    type Frozen = PhantomData<&'static T>;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<PhantomData<&'static T>> {
        Ok(marker::PhantomData)
    }
}

impl<T> Freeze for Vec<T>
where
    T: Freeze,
{
    type Frozen = Vec<T::Frozen>;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Vec<T::Frozen>> {
        self.into_try_map(|v| v.freeze(freezer))
    }
}

impl<T> Freeze for RefCell<T>
where
    T: Freeze,
{
    type Frozen = T::Frozen;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<T::Frozen> {
        self.into_inner().freeze(freezer)
    }
}

impl<T> Freeze for UnsafeCell<T>
where
    T: Freeze,
{
    type Frozen = UnsafeCell<T::Frozen>;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(UnsafeCell::new(self.into_inner().freeze(freezer)?))
    }
}

impl<T> Freeze for OnceCell<T>
where
    T: Freeze,
{
    type Frozen = Option<T::Frozen>;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        self.into_inner().freeze(freezer)
    }
}

impl<T> Freeze for Box<T>
where
    T: Freeze,
{
    type Frozen = Box<T::Frozen>;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        Ok(Box::new((*self).freeze(freezer)?))
    }
}

impl<T> Freeze for Box<[T]>
where
    T: Freeze,
{
    type Frozen = Box<[T::Frozen]>;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        self.into_vec()
            .into_try_map(|v| v.freeze(freezer))
            .map(|v| v.into_boxed_slice())
    }
}

impl<T> Freeze for Option<T>
where
    T: Freeze,
{
    type Frozen = Option<T::Frozen>;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Option<T::Frozen>> {
        self.map(|v| v.freeze(freezer)).transpose()
    }
}

impl<K: Freeze> Freeze for Hashed<K> {
    type Frozen = Hashed<K::Frozen>;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        // `freeze` must not change hash.
        Ok(Hashed::new_unchecked(
            self.hash(),
            self.into_key().freeze(freezer)?,
        ))
    }
}

impl<K, V> Freeze for SmallMap<K, V>
where
    K: Freeze,
    V: Freeze,
{
    type Frozen = SmallMap<K::Frozen, V::Frozen>;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<SmallMap<K::Frozen, V::Frozen>> {
        let mut new = SmallMap::with_capacity(self.len());
        for (key, value) in self.into_iter_hashed() {
            let hash = key.hash();
            let key = key.into_key().freeze(freezer)?;
            // TODO(nga): verify hash unchanged after freeze.
            let key = Hashed::new_unchecked(hash, key);
            let value = value.freeze(freezer)?;
            new.insert_hashed_unique_unchecked(key, value);
        }
        Ok(new)
    }
}

impl<T> Freeze for SmallSet<T>
where
    T: Freeze,
{
    type Frozen = SmallSet<T::Frozen>;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<Self::Frozen> {
        let mut new = SmallSet::with_capacity(self.len());
        for value in self.into_iter_hashed() {
            let value = value.freeze(freezer)?;
            // TODO(nga): verify hash unchanged after freeze.
            new.insert_hashed_unique_unchecked(value);
        }
        Ok(new)
    }
}

impl<'v> Freeze for Value<'v> {
    type Frozen = FrozenValue;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<FrozenValue> {
        freezer.freeze(self)
    }
}

impl Freeze for FrozenValue {
    type Frozen = FrozenValue;

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<FrozenValue> {
        Ok(self)
    }
}

impl<'v> Freeze for StringValue<'v> {
    type Frozen = FrozenStringValue;

    fn freeze(self, freezer: &Freezer) -> FreezeResult<FrozenStringValue> {
        self.freeze(freezer)
    }
}

impl Freeze for () {
    type Frozen = ();

    fn freeze(self, _freezer: &Freezer) -> FreezeResult<()> {
        Ok(())
    }
}

impl<A: Freeze> Freeze for (A,) {
    type Frozen = (A::Frozen,);

    fn freeze(self, freezer: &Freezer) -> FreezeResult<(A::Frozen,)> {
        Ok((self.0.freeze(freezer)?,))
    }
}

impl<A: Freeze, B: Freeze> Freeze for (A, B) {
    type Frozen = (A::Frozen, B::Frozen);

    fn freeze(self, freezer: &Freezer) -> FreezeResult<(A::Frozen, B::Frozen)> {
        Ok((self.0.freeze(freezer)?, self.1.freeze(freezer)?))
    }
}

impl<A: Freeze, B: Freeze, C: Freeze> Freeze for (A, B, C) {
    type Frozen = (A::Frozen, B::Frozen, C::Frozen);

    fn freeze(self, freezer: &Freezer) -> FreezeResult<(A::Frozen, B::Frozen, C::Frozen)> {
        Ok((
            self.0.freeze(freezer)?,
            self.1.freeze(freezer)?,
            self.2.freeze(freezer)?,
        ))
    }
}

impl<A: Freeze, B: Freeze, C: Freeze, D: Freeze> Freeze for (A, B, C, D) {
    type Frozen = (A::Frozen, B::Frozen, C::Frozen, D::Frozen);

    fn freeze(
        self,
        freezer: &Freezer,
    ) -> FreezeResult<(A::Frozen, B::Frozen, C::Frozen, D::Frozen)> {
        Ok((
            self.0.freeze(freezer)?,
            self.1.freeze(freezer)?,
            self.2.freeze(freezer)?,
            self.3.freeze(freezer)?,
        ))
    }
}
