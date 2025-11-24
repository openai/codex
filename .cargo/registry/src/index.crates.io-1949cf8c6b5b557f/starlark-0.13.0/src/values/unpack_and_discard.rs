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

use allocative::Allocative;
use dupe::Clone_;
use dupe::Copy_;
use dupe::Dupe_;

use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::UnpackValue;
use crate::values::Value;

/// Unpack the value of type `T`, but do not store result.
///
/// This can be used when type needs to be checked, but the unpacked value is not needed.
#[derive(Clone_, Copy_, Dupe_, Allocative)]
#[allocative(bound = "")]
pub struct UnpackAndDiscard<T>(PhantomData<fn() -> T>);

impl<T: StarlarkTypeRepr> StarlarkTypeRepr for UnpackAndDiscard<T> {
    type Canonical = T::Canonical;

    fn starlark_type_repr() -> crate::typing::Ty {
        <Self::Canonical as StarlarkTypeRepr>::starlark_type_repr()
    }
}

impl<'v, T: UnpackValue<'v>> UnpackValue<'v> for UnpackAndDiscard<T> {
    type Error = T::Error;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        match T::unpack_value_impl(value) {
            Ok(None) => Ok(None),
            Ok(Some(_)) => Ok(Some(UnpackAndDiscard(PhantomData))),
            Err(e) => Err(e),
        }
    }
}
