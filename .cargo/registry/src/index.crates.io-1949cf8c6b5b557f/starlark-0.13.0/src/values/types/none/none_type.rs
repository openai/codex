/*
 * Copyright 2019 The Starlark in Rust Authors.
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
use std::hash::Hasher;

use allocative::Allocative;
use derive_more::Display;
use dupe::Dupe;
use serde::Serialize;
use serde::Serializer;
use starlark_derive::starlark_value;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::collections::StarlarkHashValue;
use crate::collections::StarlarkHasher;
use crate::private::Private;
use crate::typing::Ty;
use crate::values::layout::avalue::alloc_static;
use crate::values::layout::avalue::AValueBasic;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::UnpackValue;
use crate::values::Value;

/// Define the None type, use [`NoneType`] in Rust.
#[derive(Debug, Clone, Dupe, ProvidesStaticType, Display, Allocative)]
#[display("None")]
pub struct NoneType;

impl NoneType {
    /// The result of `type(None)`.
    pub const TYPE: &'static str = "NoneType";
}

/// Define the NoneType type
#[starlark_value(type = NoneType::TYPE)]
impl<'v> StarlarkValue<'v> for NoneType {
    fn is_special(_: Private) -> bool
    where
        Self: Sized,
    {
        true
    }

    fn to_bool(&self) -> bool {
        false
    }
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        // just took the result of hash(None) in macos python 2.7.10 interpreter.
        hasher.write_u64(9_223_380_832_852_120_682);
        Ok(())
    }

    fn get_hash(&self, _private: Private) -> crate::Result<StarlarkHashValue> {
        // Just a random number.
        Ok(StarlarkHashValue::new_unchecked(0xf9c2263d))
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::none()
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(Ty::none())
    }

    fn eval_type(&self) -> Option<Ty> {
        Some(Ty::none())
    }
}

impl<'v> AllocValue<'v> for NoneType {
    fn alloc_value(self, _heap: &'v Heap) -> Value<'v> {
        Value::new_none()
    }
}

impl Serialize for NoneType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_none()
    }
}

pub(crate) static VALUE_NONE: AValueRepr<AValueImpl<'static, AValueBasic<NoneType>>> =
    alloc_static(NoneType);

impl AllocFrozenValue for NoneType {
    fn alloc_frozen_value(self, _heap: &FrozenHeap) -> FrozenValue {
        FrozenValue::new_none()
    }
}

impl<'v> UnpackValue<'v> for NoneType {
    type Error = Infallible;

    fn unpack_value_impl(value: Value<'v>) -> Result<Option<Self>, Self::Error> {
        if value.is_none() {
            Ok(Some(NoneType))
        } else {
            Ok(None)
        }
    }
}
