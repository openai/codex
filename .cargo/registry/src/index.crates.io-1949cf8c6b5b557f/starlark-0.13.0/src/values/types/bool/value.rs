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

use std::cmp::Ordering;
use std::fmt;
use std::fmt::Display;
use std::hash::Hasher;

use allocative::Allocative;
use starlark_derive::starlark_value;
use starlark_derive::ProvidesStaticType;
use starlark_map::StarlarkHashValue;
use starlark_map::StarlarkHasher;

use crate as starlark;
use crate::__derive_refs::serde::Serialize;
use crate::private::Private;
use crate::typing::Ty;
use crate::values::layout::avalue::alloc_static;
use crate::values::layout::avalue::AValueBasic;
use crate::values::layout::avalue::AValueImpl;
use crate::values::layout::heap::repr::AValueRepr;
use crate::values::StarlarkValue;
use crate::values::Value;
use crate::values::ValueError;

/// The result of calling `type()` on booleans.
pub const BOOL_TYPE: &str = "bool";

/// `bool` value.
#[derive(ProvidesStaticType, Debug, Serialize, Allocative)]
#[serde(transparent)]
pub struct StarlarkBool(pub(crate) bool);

impl Display for StarlarkBool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 {
            write!(f, "True")
        } else {
            write!(f, "False")
        }
    }
}

pub(crate) static VALUE_FALSE_TRUE: [AValueRepr<AValueImpl<AValueBasic<StarlarkBool>>>; 2] = [
    alloc_static(StarlarkBool(false)),
    alloc_static(StarlarkBool(true)),
];

/// Define the bool type
#[starlark_value(type = BOOL_TYPE)]
impl<'v> StarlarkValue<'v> for StarlarkBool {
    fn is_special(_: Private) -> bool
    where
        Self: Sized,
    {
        true
    }

    fn collect_repr(&self, s: &mut String) {
        // repr() for bool is quite hot, so optimise it
        if self.0 {
            s.push_str("True")
        } else {
            s.push_str("False")
        }
    }

    fn to_bool(&self) -> bool {
        self.0
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        hasher.write_u8(if self.0 { 1 } else { 0 });
        Ok(())
    }

    fn get_hash(&self, _private: Private) -> crate::Result<StarlarkHashValue> {
        // These constants are just two random numbers.
        Ok(StarlarkHashValue::new_unchecked(if self.0 {
            0xa4acba08
        } else {
            0x71e8ba71
        }))
    }

    fn compare(&self, other: Value) -> crate::Result<Ordering> {
        if let Some(other) = other.unpack_bool() {
            Ok(self.0.cmp(&other))
        } else {
            ValueError::unsupported_with(self, "<>", other)
        }
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(Ty::bool())
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::bool()
    }
}
