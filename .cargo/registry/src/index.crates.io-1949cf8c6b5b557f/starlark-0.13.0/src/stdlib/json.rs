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

// https://github.com/rust-lang/rust-clippy/issues/11142
#![allow(clippy::needless_borrow)]

use std::str::FromStr;

use either::Either;
use num_bigint::BigInt;
use starlark_derive::starlark_module;

use crate as starlark;
use crate::collections::SmallMap;
use crate::environment::GlobalsBuilder;
use crate::typing::Ty;
use crate::values::dict::AllocDict;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::types::int::int_or_big::StarlarkInt;
use crate::values::AllocFrozenValue;
use crate::values::AllocValue;
use crate::values::FrozenHeap;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::Value;

impl StarlarkTypeRepr for serde_json::Number {
    type Canonical = Either<i32, f64>;

    fn starlark_type_repr() -> Ty {
        Either::<i32, f64>::starlark_type_repr()
    }
}

impl<'a> StarlarkTypeRepr for &'a serde_json::Number {
    type Canonical = serde_json::Number;

    fn starlark_type_repr() -> Ty {
        serde_json::Number::starlark_type_repr()
    }
}

impl<'v, 'a> AllocValue<'v> for &'a serde_json::Number {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        if let Some(x) = self.as_u64() {
            heap.alloc(x)
        } else if let Some(x) = self.as_i64() {
            heap.alloc(x)
        } else if let Some(x) = self.as_f64() {
            heap.alloc(x)
        } else if let Ok(x) = BigInt::from_str(&self.to_string()) {
            heap.alloc(StarlarkInt::from(x))
        } else {
            panic!("Unrepresentable number: {:?}", self)
        }
    }
}

impl<'v> AllocValue<'v> for serde_json::Number {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        // If you follow this hint, it becomes infinite recursion
        #[allow(clippy::needless_borrows_for_generic_args)]
        heap.alloc(&self)
    }
}

impl<'a> AllocFrozenValue for &'a serde_json::Number {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        if let Some(x) = self.as_u64() {
            heap.alloc(x)
        } else if let Some(x) = self.as_i64() {
            heap.alloc(x)
        } else if let Some(x) = self.as_f64() {
            heap.alloc(x)
        } else if let Ok(x) = BigInt::from_str(&self.to_string()) {
            heap.alloc(StarlarkInt::from(x))
        } else {
            panic!("Unrepresentable number: {:?}", self)
        }
    }
}

impl AllocFrozenValue for serde_json::Number {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        // If you follow this hint, it becomes infinite recursion
        #[allow(clippy::needless_borrows_for_generic_args)]
        heap.alloc(&self)
    }
}

impl<'a, K: StarlarkTypeRepr, V: StarlarkTypeRepr> StarlarkTypeRepr for &'a serde_json::Map<K, V> {
    type Canonical = <serde_json::Map<K, V> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        AllocDict::<SmallMap<K, V>>::starlark_type_repr()
    }
}

impl<K: StarlarkTypeRepr, V: StarlarkTypeRepr> StarlarkTypeRepr for serde_json::Map<K, V> {
    type Canonical = <SmallMap<K, V> as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        AllocDict::<SmallMap<K, V>>::starlark_type_repr()
    }
}

impl<'a, 'v> AllocValue<'v> for &'a serde_json::Map<String, serde_json::Value> {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        // For some reason `IntoIter` is implemented only for `String, Value` parameters,
        // so this implementation is limited.
        heap.alloc(AllocDict(self))
    }
}

impl<'v> AllocValue<'v> for serde_json::Map<String, serde_json::Value> {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        // If you follow this hint, it becomes infinite recursion
        #[allow(clippy::needless_borrows_for_generic_args)]
        heap.alloc(&self)
    }
}

impl<'a> AllocFrozenValue for &'a serde_json::Map<String, serde_json::Value> {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        heap.alloc(AllocDict(self.iter().map(|(k, v)| (k.as_str(), v))))
    }
}

impl AllocFrozenValue for serde_json::Map<String, serde_json::Value> {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        // If you follow this hint, it becomes infinite recursion
        #[allow(clippy::needless_borrows_for_generic_args)]
        heap.alloc(&self)
    }
}

impl<'a> StarlarkTypeRepr for &'a serde_json::Value {
    type Canonical = <serde_json::Value as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        // Any.
        Value::starlark_type_repr()
    }
}

impl StarlarkTypeRepr for serde_json::Value {
    type Canonical = <FrozenValue as StarlarkTypeRepr>::Canonical;

    fn starlark_type_repr() -> Ty {
        // Any.
        Value::starlark_type_repr()
    }
}

impl<'v, 'a> AllocValue<'v> for &'a serde_json::Value {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        match self {
            serde_json::Value::Null => Value::new_none(),
            serde_json::Value::Bool(x) => Value::new_bool(*x),
            serde_json::Value::Number(x) => heap.alloc(x),
            serde_json::Value::String(x) => heap.alloc(x.as_str()),
            serde_json::Value::Array(x) => heap.alloc(x.as_slice()),
            serde_json::Value::Object(x) => heap.alloc(x),
        }
    }
}

impl<'v> AllocValue<'v> for serde_json::Value {
    fn alloc_value(self, heap: &'v Heap) -> Value<'v> {
        // If you follow this hint, it becomes infinite recursion
        #[allow(clippy::needless_borrows_for_generic_args)]
        heap.alloc(&self)
    }
}

impl<'a> AllocFrozenValue for &'a serde_json::Value {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        match self {
            serde_json::Value::Null => FrozenValue::new_none(),
            serde_json::Value::Bool(x) => FrozenValue::new_bool(*x),
            serde_json::Value::Number(x) => heap.alloc(x),
            serde_json::Value::String(x) => heap.alloc(x.as_str()),
            serde_json::Value::Array(x) => heap.alloc(x.as_slice()),
            serde_json::Value::Object(x) => heap.alloc(x),
        }
    }
}

impl AllocFrozenValue for serde_json::Value {
    fn alloc_frozen_value(self, heap: &FrozenHeap) -> FrozenValue {
        // If you follow this hint, it becomes infinite recursion
        #[allow(clippy::needless_borrows_for_generic_args)]
        heap.alloc(&self)
    }
}

pub(crate) fn json(globals: &mut GlobalsBuilder) {
    #[starlark_module]
    fn json_members(globals: &mut GlobalsBuilder) {
        fn encode(#[starlark(require = pos)] x: Value) -> anyhow::Result<String> {
            x.to_json()
        }

        fn decode<'v>(
            #[starlark(require = pos)] x: &str,
            heap: &'v Heap,
        ) -> anyhow::Result<Value<'v>> {
            Ok(heap.alloc(serde_json::from_str::<serde_json::Value>(x)?))
        }
    }

    // Copying Bazel's json module: https://bazel.build/rules/lib/json
    // or starlark-go json module:
    // https://github.com/google/starlark-go/blob/d1966c6b9fcd6631f48f5155f47afcd7adcc78c2/lib/json/json.go#L28
    globals.namespace("json", json_members);
}

#[cfg(test)]
mod tests {
    use crate::assert::Assert;

    #[test]
    fn test_json_encode() {
        let a = Assert::new();
        a.eq("'[10]'", "json.encode([10])");
    }

    #[test]
    fn test_json_decode() {
        let a = Assert::new();
        a.eq(
            "[10, None, False, {'k': 'v'}]",
            "json.decode('[10, null, false, {\"k\": \"v\"}]')",
        );

        a.eq("3.142", "json.decode('3.142')");
        a.eq(
            "123456789123456789123456789",
            "json.decode('123456789123456789123456789')",
        );
    }
}
