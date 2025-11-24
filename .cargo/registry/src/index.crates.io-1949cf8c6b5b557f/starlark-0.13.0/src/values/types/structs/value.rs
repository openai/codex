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

use std::cmp::Ordering;
use std::fmt;
use std::fmt::Display;
use std::hash::Hash;
use std::hash::Hasher;

use allocative::Allocative;
use display_container::fmt_keyed_container;
use serde::Serialize;
use starlark_derive::starlark_value;
use starlark_derive::Freeze;
use starlark_derive::Trace;
use starlark_map::small_map::SmallMap;
use starlark_map::Hashed;
use starlark_map::StarlarkHasher;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::coerce::coerce;
use crate::coerce::Coerce;
use crate::docs::DocItem;
use crate::docs::DocMember;
use crate::docs::DocProperty;
use crate::starlark_complex_value;
use crate::typing::Ty;
use crate::typing::TyStruct;
use crate::util::arc_str::ArcStr;
use crate::values::comparison::compare_small_map;
use crate::values::comparison::equals_small_map;
use crate::values::structs::unordered_hasher::UnorderedHasher;
use crate::values::FreezeResult;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::StringValue;
use crate::values::StringValueLike;
use crate::values::Value;
use crate::values::ValueError;
use crate::values::ValueLike;

impl<'v, V: ValueLike<'v>> StructGen<'v, V> {
    /// The result of calling `type()` on a struct.
    pub(crate) const TYPE: &'static str = "struct";

    /// Create a new [`Struct`].
    pub(crate) fn new(fields: SmallMap<V::String, V>) -> Self {
        Self { fields }
    }

    /// Iterate over the elements in the struct.
    pub(crate) fn iter<'a>(&'a self) -> impl ExactSizeIterator<Item = (StringValue<'v>, V)> + 'a
    where
        'v: 'a,
    {
        self.fields
            .iter()
            .map(|(name, value)| (name.to_string_value(), *value))
    }

    fn self_ty(&self) -> Ty {
        Ty::custom(TyStruct {
            fields: self
                .fields
                .iter()
                .map(|(name, value)| (ArcStr::from(name.as_str()), Ty::of_value(value.to_value())))
                .collect(),
            extra: false,
        })
    }
}

impl StructGen<'static, FrozenValue> {
    pub(crate) fn iter_frozen(
        &self,
    ) -> impl ExactSizeIterator<Item = (FrozenStringValue, FrozenValue)> + '_ {
        self.fields.iter().map(|(name, value)| (*name, *value))
    }
}

starlark_complex_value!(pub(crate) Struct<'v>);

/// The result of calling `struct()`.
#[derive(Clone, Default, Debug, Trace, Freeze, ProvidesStaticType, Allocative)]
#[repr(C)]
pub(crate) struct StructGen<'v, V: ValueLike<'v>> {
    /// The fields in a struct.
    pub(crate) fields: SmallMap<V::String, V>,
}

unsafe impl<'v> Coerce<StructGen<'v, Value<'v>>> for StructGen<'static, FrozenValue> {}

impl<'v, V: ValueLike<'v>> Display for StructGen<'v, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_keyed_container(
            f,
            "struct(",
            ")",
            "=",
            self.iter().map(|(k, v)| (k.as_str(), v)),
        )
    }
}

#[starlark_value(type = Struct::TYPE)]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for StructGen<'v, V>
where
    Self: ProvidesStaticType<'v>,
{
    fn collect_repr_cycle(&self, collector: &mut String) {
        collector.push_str("struct(...)");
    }

    fn equals(&self, other: Value<'v>) -> crate::Result<bool> {
        match Struct::from_value(other) {
            None => Ok(false),
            Some(other) => {
                equals_small_map(coerce(&self.fields), &other.fields, |x, y| x.equals(*y))
            }
        }
    }

    fn compare(&self, other: Value<'v>) -> crate::Result<Ordering> {
        match Struct::from_value(other) {
            None => ValueError::unsupported_with(self, "cmp()", other),
            Some(other) => compare_small_map(
                coerce(&self.fields),
                &other.fields,
                |k| k.as_str(),
                |x, y| x.compare(*y),
            ),
        }
    }

    fn get_attr(&self, attribute: &str, heap: &'v Heap) -> Option<Value<'v>> {
        self.get_attr_hashed(Hashed::new(attribute), heap)
    }

    fn get_attr_hashed(&self, attribute: Hashed<&str>, _heap: &'v Heap) -> Option<Value<'v>> {
        coerce(&self.fields).get_hashed(attribute).copied()
    }

    fn write_hash(&self, hasher: &mut StarlarkHasher) -> crate::Result<()> {
        // Must use unordered hash because equality is unordered,
        // and `a = b  =>  hash(a) = hash(b)`.
        let mut unordered_hasher = UnorderedHasher::new();

        for (k, v) in self.fields.iter_hashed() {
            // Should hash key and value together, so two structs
            // `a=1 b=2` and `a=2 b=1` would produce different hashes.
            let mut entry_hasher = StarlarkHasher::new();
            k.hash().hash(&mut entry_hasher);
            v.write_hash(&mut entry_hasher)?;
            unordered_hasher.write_hash(entry_hasher.finish());
        }

        hasher.write_u64(unordered_hasher.finish());

        Ok(())
    }

    fn dir_attr(&self) -> Vec<String> {
        self.fields.keys().map(|x| x.as_str().to_owned()).collect()
    }

    fn documentation(&self) -> DocItem {
        // This treats structs as being value-like, and intentionally generates bad docs in the case
        // of namespace-like usage. See
        // <https://fb.workplace.com/groups/starlark/permalink/1463680027654154/> for some
        // additional discussion
        let typ = self.self_ty();
        DocItem::Member(DocMember::Property(DocProperty { docs: None, typ }))
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::any_struct()
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(self.self_ty())
    }
}

impl<'v, V: ValueLike<'v>> Serialize for StructGen<'v, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_map(self.iter())
    }
}

#[cfg(test)]
mod tests {

    use crate::assert;

    #[test]
    fn test_repr() {
        assert::eq("repr(struct(a=1, b=[]))", "'struct(a=1, b=[])'");
        assert::eq("str(struct(a=1, b=[]))", "'struct(a=1, b=[])'");
    }

    #[test]
    fn test_repr_cycle() {
        assert::eq(
            "l = []; s = struct(f=l); l.append(s); repr(s)",
            "'struct(f=[struct(...)])'",
        );
        assert::eq(
            "l = []; s = struct(f=l); l.append(s); str(s)",
            "'struct(f=[struct(...)])'",
        );
    }

    #[test]
    fn test_to_json_cycle() {
        assert::fail(
            "l = []; s = struct(f=l); l.append(s); json.encode(s)",
            "Cycle detected when serializing value of type `struct` to JSON",
        );
    }

    #[test]
    fn test_to_json() {
        assert::all_true(
            r#"
json.encode(struct(key = None)) == '{"key":null}'
json.encode(struct(key = True)) == '{"key":true}'
json.encode(struct(key = False)) == '{"key":false}'
json.encode(struct(key = 42)) == '{"key":42}'
json.encode(struct(key = 'value')) == '{"key":"value"}'
json.encode(struct(key = 'value"')) == '{"key":"value\\\""}'
json.encode(struct(key = 'value\\')) == '{"key":"value\\\\"}'
json.encode(struct(key = 'value/')) == '{"key":"value/"}'
json.encode(struct(key = 'value\u0008')) == '{"key":"value\\b"}'
json.encode(struct(key = 'value\u000C')) == '{"key":"value\\f"}'
json.encode(struct(key = 'value\n')) == '{"key":"value\\n"}'
json.encode(struct(key = 'value\r')) == '{"key":"value\\r"}'
json.encode(struct(key = 'value\t')) == '{"key":"value\\t"}'
json.encode(struct(foo = 42, bar = "some")) == '{"foo":42,"bar":"some"}'
json.encode(struct(foo = struct(bar = "some"))) == '{"foo":{"bar":"some"}}'
json.encode(struct(foo = ["bar/", "some"])) == '{"foo":["bar/","some"]}'
json.encode(struct(foo = [struct(bar = "some")])) == '{"foo":[{"bar":"some"}]}'
"#,
        );
    }

    #[test]
    fn test_comparison_bug() {
        // TODO(nga): this should be false, because `a < b`,
        //   and comparisons are usually lexicographic.
        // TODO(nga): Also, since structs are ordered, but not sorted,
        //   comparisons on structs should be forbidden
        //   (because it is too expensive to sort keys on each comparison).
        assert::is_true("struct(b=1) < struct(a=1, x=1)")
    }
}
