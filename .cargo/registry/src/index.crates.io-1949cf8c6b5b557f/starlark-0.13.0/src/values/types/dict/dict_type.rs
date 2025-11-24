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

use std::marker::PhantomData;

use either::Either;

use crate::typing::Ty;
use crate::values::dict::UnpackDictEntries;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::UnpackAndDiscard;
use crate::values::UnpackValue;
use crate::values::Value;

/// A dict type marker.
///
/// [`StarlarkTypeRepr`] provides `dict[K, V]`.
/// [`UnpackValue`] implementation verifies the types of entries and discards them.
pub struct DictType<K: StarlarkTypeRepr, V: StarlarkTypeRepr> {
    k: PhantomData<K>,
    v: PhantomData<V>,
}

impl<K: StarlarkTypeRepr, V: StarlarkTypeRepr> StarlarkTypeRepr for DictType<K, V> {
    type Canonical = DictType<K::Canonical, V::Canonical>;

    fn starlark_type_repr() -> Ty {
        Ty::dict(K::starlark_type_repr(), V::starlark_type_repr())
    }
}

impl<'v, K: UnpackValue<'v>, V: UnpackValue<'v>> UnpackValue<'v> for DictType<K, V> {
    type Error = Either<K::Error, V::Error>;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        match UnpackDictEntries::<UnpackAndDiscard<K>, UnpackAndDiscard<V>>::unpack_value_impl(
            value,
        ) {
            Ok(Some(_)) => Ok(Some(DictType {
                k: PhantomData,
                v: PhantomData,
            })),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
