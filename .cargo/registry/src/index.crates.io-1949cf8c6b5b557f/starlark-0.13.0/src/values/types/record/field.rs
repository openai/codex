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

use std::fmt;
use std::fmt::Display;
use std::hash::Hash;

use allocative::Allocative;
use dupe::Dupe;
use starlark_derive::starlark_value;
use starlark_derive::Freeze;
use starlark_derive::NoSerialize;
use starlark_derive::Trace;
use starlark_map::StarlarkHasher;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::coerce::Coerce;
use crate::starlark_complex_value;
use crate::typing::Ty;
use crate::values::typing::type_compiled::compiled::TypeCompiled;
use crate::values::FreezeResult;
use crate::values::StarlarkValue;
use crate::values::ValueLifetimeless;
use crate::values::ValueLike;

/// The result of `field()`.
#[derive(
    Clone,
    Debug,
    Dupe,
    Trace,
    Freeze,
    NoSerialize,
    ProvidesStaticType,
    Allocative
)]
pub struct FieldGen<V: ValueLifetimeless> {
    pub(crate) typ: TypeCompiled<V>,
    pub(crate) default: Option<V>,
}

impl<'v, V: ValueLike<'v>> Display for FieldGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "field(")?;
        Display::fmt(&self.typ, f)?;
        if let Some(d) = &self.default {
            write!(f, ", ")?;
            Display::fmt(d, f)?;
        }
        write!(f, ")")
    }
}

// Manual because no instance for Option<V>
unsafe impl<From: Coerce<To> + ValueLifetimeless, To: ValueLifetimeless> Coerce<FieldGen<To>>
    for FieldGen<From>
{
}

starlark_complex_value!(pub(crate) Field);

impl<V: ValueLifetimeless> FieldGen<V> {
    pub(crate) fn new(typ: TypeCompiled<V>, default: Option<V>) -> Self {
        Self { typ, default }
    }
}

impl<'v, V: ValueLike<'v>> FieldGen<V> {
    pub(crate) fn ty(&self) -> Ty {
        self.typ.as_ty().clone()
    }
}

#[starlark_value(type = "field")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for FieldGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn write_hash(&self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        self.typ.write_hash(hasher)?;
        self.default.is_some().hash(hasher);
        if let Some(d) = self.default {
            d.write_hash(hasher)?;
        }
        Ok(())
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(Ty::starlark_value::<Self>())
    }
}
