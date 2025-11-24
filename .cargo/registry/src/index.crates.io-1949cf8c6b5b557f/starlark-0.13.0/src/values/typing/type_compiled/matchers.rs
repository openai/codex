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

use allocative::Allocative;
use dupe::Dupe;
use starlark_derive::Freeze;
use starlark_derive::Trace;

use crate as starlark;
use crate::typing::starlark_value::TyStarlarkValue;
use crate::values::dict::value::FrozenDict;
use crate::values::dict::DictRef;
use crate::values::list::value::FrozenList;
use crate::values::list::ListRef;
use crate::values::set::refs::SetRef;
use crate::values::set::value::FrozenSet;
use crate::values::starlark_type_id::StarlarkTypeId;
use crate::values::starlark_type_id::StarlarkTypeIdAligned;
use crate::values::tuple::value::Tuple;
use crate::values::types::int::int_or_big::StarlarkIntRef;
use crate::values::typing::type_compiled::matcher::TypeMatcher;
use crate::values::typing::type_compiled::matcher::TypeMatcherBox;
use crate::values::FreezeResult;
use crate::values::UnpackValue;
use crate::values::Value;

#[derive(Clone, Copy, Dupe, Allocative, Debug)]
pub(crate) struct IsAny;

impl TypeMatcher for IsAny {
    fn matches(&self, _value: Value) -> bool {
        true
    }

    fn is_wildcard(&self) -> bool {
        true
    }
}

#[derive(Clone, Copy, Dupe, Allocative, Debug)]
pub(crate) struct IsNever;

impl TypeMatcher for IsNever {
    fn matches(&self, _value: Value) -> bool {
        false
    }
}

#[derive(Clone, Copy, Dupe, Allocative, Debug)]
pub(crate) struct IsStr;

impl TypeMatcher for IsStr {
    fn matches(&self, value: Value) -> bool {
        value.unpack_str().is_some()
    }
}

#[derive(Clone, Copy, Dupe, Allocative, Debug)]
pub(crate) struct IsList;

impl TypeMatcher for IsList {
    fn matches(&self, value: Value) -> bool {
        value.starlark_type_id() == StarlarkTypeId::of::<FrozenList>()
    }
}

#[derive(Clone, Allocative, Debug)]
pub(crate) struct IsListOf<I: TypeMatcher>(pub(crate) I);

impl<I: TypeMatcher> TypeMatcher for IsListOf<I> {
    fn matches(&self, value: Value) -> bool {
        match ListRef::from_value(value) {
            None => false,
            Some(list) => list.iter().all(|v| self.0.matches(v)),
        }
    }
}

#[derive(Clone, Allocative, Debug)]
pub(crate) struct IsTupleOf<A: TypeMatcher>(pub(crate) A);

impl<A: TypeMatcher> TypeMatcher for IsTupleOf<A> {
    fn matches(&self, value: Value) -> bool {
        match Tuple::from_value(value) {
            None => false,
            Some(v) => v.iter().all(|v| self.0.matches(v)),
        }
    }
}

#[derive(Clone, Allocative, Debug)]
pub(crate) struct IsTupleElems(pub(crate) Vec<TypeMatcherBox>);

impl TypeMatcher for IsTupleElems {
    fn matches(&self, value: Value) -> bool {
        match Tuple::from_value(value) {
            Some(v) if v.len() == self.0.len() => {
                v.iter().zip(self.0.iter()).all(|(v, t)| t.0.matches_dyn(v))
            }
            _ => false,
        }
    }
}

#[derive(Clone, Allocative, Debug)]
pub(crate) struct IsTupleElems0;

impl TypeMatcher for IsTupleElems0 {
    fn matches(&self, value: Value) -> bool {
        match Tuple::from_value(value).map(|t| t.content()) {
            Some([]) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Allocative, Debug)]
pub(crate) struct IsTupleElems1<A: TypeMatcher>(pub(crate) A);

impl<A: TypeMatcher> TypeMatcher for IsTupleElems1<A> {
    fn matches(&self, value: Value) -> bool {
        match Tuple::from_value(value).map(|t| t.content()) {
            Some([v0]) => self.0.matches(*v0),
            _ => false,
        }
    }
}

#[derive(Clone, Allocative, Debug)]
pub(crate) struct IsTupleElems2<A: TypeMatcher, B: TypeMatcher>(pub(crate) A, pub(crate) B);

impl<A: TypeMatcher, B: TypeMatcher> TypeMatcher for IsTupleElems2<A, B> {
    fn matches(&self, value: Value) -> bool {
        match Tuple::from_value(value).map(|t| t.content()) {
            Some([v0, v1]) => self.0.matches(*v0) && self.1.matches(*v1),
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Dupe, Allocative, Debug)]
pub(crate) struct IsDict;

impl TypeMatcher for IsDict {
    fn matches(&self, value: Value) -> bool {
        value.starlark_type_id() == StarlarkTypeId::of::<FrozenDict>()
    }
}

#[derive(Clone, Allocative, Debug)]
pub(crate) struct IsDictOf<K: TypeMatcher, V: TypeMatcher>(pub(crate) K, pub(crate) V);

impl<K: TypeMatcher, V: TypeMatcher> TypeMatcher for IsDictOf<K, V> {
    fn matches(&self, value: Value) -> bool {
        match DictRef::from_value(value) {
            None => false,
            Some(dict) => dict
                .iter()
                .all(|(k, v)| self.0.matches(k) && self.1.matches(v)),
        }
    }
}

#[derive(Clone, Copy, Dupe, Allocative, Debug)]
pub(crate) struct IsSet;

impl TypeMatcher for IsSet {
    fn matches(&self, value: Value) -> bool {
        value.starlark_type_id() == StarlarkTypeId::of::<FrozenSet>()
    }
}

#[derive(Clone, Allocative, Debug)]
pub(crate) struct IsSetOf<I: TypeMatcher>(pub(crate) I);

impl<I: TypeMatcher> TypeMatcher for IsSetOf<I> {
    fn matches(&self, value: Value) -> bool {
        match SetRef::unpack_value_opt(value) {
            Some(set) => set.aref.iter().all(|v| self.0.matches(v)),
            _ => false,
        }
    }
}

#[derive(Clone, Allocative, Debug)]
pub(crate) struct IsAnyOfTwo<A: TypeMatcher, B: TypeMatcher>(pub(crate) A, pub(crate) B);

impl<A: TypeMatcher, B: TypeMatcher> TypeMatcher for IsAnyOfTwo<A, B> {
    fn matches(&self, value: Value) -> bool {
        self.0.matches(value) || self.1.matches(value)
    }
}

#[derive(Clone, Allocative, Debug)]
pub(crate) struct IsAnyOf(pub(crate) Vec<TypeMatcherBox>);

impl TypeMatcher for IsAnyOf {
    fn matches(&self, value: Value) -> bool {
        self.0.iter().any(|t| t.matches(value))
    }
}

#[derive(Allocative, Clone, Copy, Dupe, Debug)]
pub(crate) struct IsCallable;

impl TypeMatcher for IsCallable {
    fn matches(&self, value: Value) -> bool {
        value.vtable().starlark_value.HAS_invoke
    }
}

#[derive(Allocative, Clone, Copy, Dupe, Debug)]
pub(crate) struct IsType;

impl TypeMatcher for IsType {
    fn matches(&self, value: Value) -> bool {
        TyStarlarkValue::is_type_from_vtable(&value.vtable().starlark_value)
    }
}

#[derive(Copy, Clone, Dupe, Debug, Allocative)]
pub(crate) struct IsIterable;

impl TypeMatcher for IsIterable {
    fn matches(&self, value: Value) -> bool {
        TyStarlarkValue::is_iterable(&value.vtable().starlark_value)
    }
}

#[derive(Clone, Copy, Dupe, Allocative, Debug)]
pub(crate) struct IsInt;

impl TypeMatcher for IsInt {
    fn matches(&self, value: Value) -> bool {
        StarlarkIntRef::unpack(value).is_some()
    }
}

#[derive(Clone, Copy, Dupe, Allocative, Debug)]
pub(crate) struct IsBool;

impl TypeMatcher for IsBool {
    fn matches(&self, value: Value) -> bool {
        value.unpack_bool().is_some()
    }
}

#[derive(Clone, Copy, Dupe, Allocative, Debug)]
pub(crate) struct IsNone;

impl TypeMatcher for IsNone {
    fn matches(&self, value: Value) -> bool {
        value.is_none()
    }
}

#[derive(Allocative, Debug, Clone)]
pub(crate) struct StarlarkTypeIdMatcher {
    starlark_type_id: StarlarkTypeIdAligned,
}

impl StarlarkTypeIdMatcher {
    pub(crate) fn new(ty: TyStarlarkValue) -> StarlarkTypeIdMatcher {
        StarlarkTypeIdMatcher {
            starlark_type_id: StarlarkTypeIdAligned::new(ty.starlark_type_id()),
        }
    }
}

impl TypeMatcher for StarlarkTypeIdMatcher {
    fn matches(&self, value: Value) -> bool {
        value.starlark_type_id() == self.starlark_type_id.get()
    }
}

#[derive(Allocative, Debug, Trace, Freeze, Clone)]
pub(crate) struct IsName(pub(crate) String);

impl TypeMatcher for IsName {
    fn matches(&self, value: Value) -> bool {
        self.0 == value.get_type()
    }
}
