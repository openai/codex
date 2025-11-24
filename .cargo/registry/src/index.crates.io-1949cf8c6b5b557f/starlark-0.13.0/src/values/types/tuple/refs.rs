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

use std::convert::Infallible;
use std::iter;
use std::slice;

use ref_cast::ref_cast_custom;
use ref_cast::RefCastCustom;

use crate::typing::Ty;
use crate::values::tuple::UnpackTuple;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::tuple::value::FrozenTuple;
use crate::values::types::tuple::value::Tuple;
use crate::values::FrozenValue;
use crate::values::UnpackValue;
use crate::values::Value;
use crate::values::ValueLike;

/// Reference to tuple data in Starlark heap.
#[derive(RefCastCustom, Debug)]
#[repr(transparent)]
pub struct TupleRef<'v> {
    contents: [Value<'v>],
}

/// Reference to tuple data in frozen Starlark heap.
#[repr(transparent)]
#[derive(RefCastCustom, Debug)]
pub struct FrozenTupleRef {
    contents: [FrozenValue],
}

impl<'v> TupleRef<'v> {
    /// `type(())`, which is `"tuple"`.
    pub const TYPE: &'static str = FrozenTupleRef::TYPE;

    #[ref_cast_custom]
    fn new(slice: &'v [Value<'v>]) -> &'v TupleRef<'v>;

    /// Downcast a value to a tuple.
    pub fn from_value(value: Value<'v>) -> Option<&'v TupleRef<'v>> {
        Some(Self::new(Tuple::from_value(value)?.content()))
    }

    /// Downcast a value to a tuple.
    pub fn from_frozen_value(value: FrozenValue) -> Option<&'v TupleRef<'v>> {
        Self::from_value(value.to_value())
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.contents.len()
    }

    /// Tuple elements.
    pub fn content(&self) -> &[Value<'v>] {
        &self.contents
    }

    /// Iterate over the contents.
    pub fn iter<'a>(&'a self) -> iter::Copied<slice::Iter<'a, Value<'v>>> {
        self.content().iter().copied()
    }
}

impl FrozenTupleRef {
    /// `type(())`, which is `"tuple"`.
    pub const TYPE: &'static str = FrozenTuple::TYPE;

    #[ref_cast_custom]
    fn new(slice: &'static [FrozenValue]) -> &'static FrozenTupleRef;

    /// Downcast a value to a tuple.
    pub fn from_frozen_value(value: FrozenValue) -> Option<&'static FrozenTupleRef> {
        Some(Self::new(value.downcast_ref::<FrozenTuple>()?.content()))
    }

    /// Number of elements.
    pub fn len(&self) -> usize {
        self.contents.len()
    }

    /// Tuple elements.
    pub fn content(&self) -> &[FrozenValue] {
        &self.contents
    }

    /// Iterate over contents.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = FrozenValue> + '_ {
        self.content().iter().copied()
    }
}

impl<'v> StarlarkTypeRepr for &'v TupleRef<'v> {
    type Canonical = <UnpackTuple<FrozenValue> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        Ty::any_tuple()
    }
}

impl<'a> StarlarkTypeRepr for &'a FrozenTupleRef {
    type Canonical = <UnpackTuple<FrozenValue> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        Ty::any_tuple()
    }
}

impl<'v> UnpackValue<'v> for &'v TupleRef<'v> {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        Ok(TupleRef::from_value(value))
    }
}

impl<'v> UnpackValue<'v> for &'v FrozenTupleRef {
    type Error = crate::Error;

    fn unpack_value_impl(value: Value<'v>) -> crate::Result<Option<Self>> {
        let Some(value) = value.unpack_frozen() else {
            // TODO(nga): return error.
            return Ok(None);
        };
        Ok(FrozenTupleRef::from_frozen_value(value))
    }
}
