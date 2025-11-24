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

use crate::typing::Ty;
use crate::values::list::UnpackList;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::UnpackAndDiscard;
use crate::values::UnpackValue;
use crate::values::Value;

/// A list type marker.
///
/// [`StarlarkTypeRepr`] provides `list[T]`.
/// [`UnpackValue`] implementation verifies the types of items.
pub struct ListType<T> {
    _item: std::marker::PhantomData<T>,
}

impl<T: StarlarkTypeRepr> StarlarkTypeRepr for ListType<T> {
    type Canonical = ListType<T::Canonical>;

    fn starlark_type_repr() -> Ty {
        Ty::list(T::starlark_type_repr())
    }
}

impl<'v, T: UnpackValue<'v>> UnpackValue<'v> for ListType<T> {
    type Error = <T as UnpackValue<'v>>::Error;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        match UnpackList::<UnpackAndDiscard<T>>::unpack_value_impl(value) {
            Ok(Some(_)) => Ok(Some(ListType {
                _item: std::marker::PhantomData,
            })),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
