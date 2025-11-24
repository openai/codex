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

use std::fmt;
use std::fmt::Display;
use std::marker::PhantomData;

use allocative::Allocative;
use display_container::fmt_keyed_container;
use serde::Serialize;
use starlark_derive::starlark_value;
use starlark_derive::Freeze;
use starlark_derive::Trace;
use starlark_map::small_map::SmallMap;
use starlark_map::Hashed;

use crate as starlark;
use crate::any::ProvidesStaticType;
use crate::coerce::Coerce;
use crate::docs::DocItem;
use crate::docs::DocModule;
use crate::starlark_complex_value;
use crate::typing::Ty;
use crate::util::arc_str::ArcStr;
use crate::values::types::namespace::typing::TyNamespace;
use crate::values::FreezeResult;
use crate::values::FrozenValue;
use crate::values::Heap;
use crate::values::StarlarkValue;
use crate::values::StringValueLike;
use crate::values::Value;
use crate::values::ValueLike;

#[derive(Clone, Coerce, Debug, Trace, Freeze, Allocative)]
#[repr(C)]
pub(crate) struct MaybeDocHiddenValue<'v, V: ValueLike<'v>> {
    pub(crate) value: V,
    pub(crate) doc_hidden: bool,
    pub(crate) phantom: PhantomData<&'v ()>,
}

/// The return value of `namespace()`
#[derive(Clone, Debug, Trace, Freeze, ProvidesStaticType, Allocative)]
#[repr(C)]
pub struct NamespaceGen<'v, V: ValueLike<'v>> {
    fields: SmallMap<V::String, MaybeDocHiddenValue<'v, V>>,
}

impl<'v, V: ValueLike<'v>> NamespaceGen<'v, V> {
    pub(crate) fn new(fields: SmallMap<V::String, MaybeDocHiddenValue<'v, V>>) -> Self {
        Self { fields }
    }

    pub fn get(&self, key: &str) -> Option<V> {
        self.fields.get_hashed(Hashed::new(key)).map(|v| v.value)
    }
}

unsafe impl<'v> Coerce<NamespaceGen<'v, Value<'v>>> for NamespaceGen<'static, FrozenValue> {}

starlark_complex_value!(pub Namespace<'v>);

impl<'v, V: ValueLike<'v>> Display for NamespaceGen<'v, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_keyed_container(
            f,
            "namespace(",
            ")",
            "=",
            self.fields.iter().map(|(k, v)| (k.as_str(), v.value)),
        )
    }
}

#[starlark_value(type = "namespace")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for NamespaceGen<'v, V>
where
    Self: ProvidesStaticType<'v>,
{
    fn collect_repr_cycle(&self, collector: &mut String) {
        collector.push_str("namespace(...)");
    }

    fn get_attr(&self, attribute: &str, heap: &'v Heap) -> Option<Value<'v>> {
        self.get_attr_hashed(Hashed::new(attribute), heap)
    }

    fn get_attr_hashed(&self, attribute: Hashed<&str>, _heap: &'v Heap) -> Option<Value<'v>> {
        self.fields
            .get_hashed(attribute)
            .map(|v| v.value.to_value())
    }

    fn dir_attr(&self) -> Vec<String> {
        self.fields.keys().map(|x| x.as_str().to_owned()).collect()
    }

    fn documentation(&self) -> DocItem {
        DocItem::Module(DocModule {
            docs: None,
            members: self
                .fields
                .iter()
                .filter(|(_, v)| !v.doc_hidden)
                .map(|(k, v)| (k.as_str().to_owned(), v.value.to_value().documentation()))
                .collect(),
        })
    }

    fn get_type_starlark_repr() -> Ty {
        Ty::custom(TyNamespace {
            fields: Default::default(),
            extra: true,
        })
    }

    fn typechecker_ty(&self) -> Option<Ty> {
        Some(Ty::custom(TyNamespace {
            fields: self
                .fields
                .iter()
                .map(|(name, value)| {
                    (
                        ArcStr::from(name.as_str()),
                        Ty::of_value(value.value.to_value()),
                    )
                })
                .collect(),
            extra: false,
        }))
    }
}

impl<'v, V: ValueLike<'v>> Serialize for NamespaceGen<'v, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_map(self.fields.iter().map(|(k, v)| (k, v.value)))
    }
}

#[cfg(test)]
mod tests {

    use crate::assert;

    #[test]
    fn test_repr() {
        assert::eq("repr(namespace(a=1, b=[]))", "'namespace(a=1, b=[])'");
        assert::eq("str(namespace(a=1, b=[]))", "'namespace(a=1, b=[])'");
    }

    #[test]
    fn test_repr_cycle() {
        assert::eq(
            "l = []; s = namespace(f=l); l.append(s); repr(s)",
            "'namespace(f=[namespace(...)])'",
        );
        assert::eq(
            "l = []; s = namespace(f=l); l.append(s); str(s)",
            "'namespace(f=[namespace(...)])'",
        );
    }

    #[test]
    fn test_to_json_cycle() {
        assert::fail(
            "l = []; s = namespace(f=l); l.append(s); json.encode(s)",
            "Cycle detected when serializing value of type `namespace` to JSON",
        );
    }

    #[test]
    fn test_kwargs() {
        assert::eq(
            "d = {'b': 2}; s = namespace(a=1, **d); str(s)",
            "'namespace(a=1, b=2)'",
        );
    }
}
