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

use dupe::Dupe;
use once_cell::sync::OnceCell;
use starlark_map::Hashed;

use crate::__derive_refs::components::NativeCallableComponents;
use crate::collections::symbol::map::SymbolMap;
use crate::collections::symbol::symbol::Symbol;
use crate::docs::DocType;
use crate::environment::common_documentation;
use crate::typing::Ty;
use crate::values::function::NativeAttr;
use crate::values::function::NativeAttribute;
use crate::values::function::NativeMeth;
use crate::values::function::NativeMethod;
use crate::values::types::unbound::UnboundValue;
use crate::values::AllocFrozenValue;
use crate::values::FrozenHeap;
use crate::values::FrozenHeapRef;
use crate::values::FrozenRef;
use crate::values::FrozenValue;
use crate::values::FrozenValueTyped;
use crate::values::Heap;
use crate::values::Value;

/// Methods of an object.
#[derive(Clone, Debug)]
pub struct Methods {
    /// This field holds the objects referenced in `members`.
    #[allow(dead_code)]
    heap: FrozenHeapRef,
    members: SymbolMap<UnboundValue>,
    docstring: Option<String>,
}

/// Used to build a [`Methods`] value.
#[derive(Debug)]
pub struct MethodsBuilder {
    /// The heap everything is allocated in.
    heap: FrozenHeap,
    /// Members, either `NativeMethod` or `NativeAttribute`.
    members: SymbolMap<UnboundValue>,
    /// The raw docstring for the main object.
    ///
    /// FIXME(JakobDegen): This should probably be removed. Not only can these docstrings not be
    /// combined with each other, but having the main documentation for the object on the methods
    /// instead of on the object type directly is extraordinarily confusing.
    docstring: Option<String>,
}

impl Methods {
    pub(crate) fn get<'v>(&'v self, name: &str) -> Option<Value<'v>> {
        Some(self.members.get_str(name)?.to_frozen_value().to_value())
    }

    /// Gets the type of the member
    ///
    /// In the case of an attribute, this is the type the attribute evaluates to, while in the case
    /// of a method, this is the `TyCallable`
    pub(crate) fn get_ty(&self, name: &str) -> Option<Ty> {
        match self.members.get_str(name)? {
            UnboundValue::Attr(attr, _) => Some(attr.typ.dupe()),
            UnboundValue::Method(method, _) => Some(method.ty.dupe()),
        }
    }

    #[inline]
    pub(crate) fn get_hashed(&self, name: Hashed<&str>) -> Option<&UnboundValue> {
        self.members.get_hashed_str(name)
    }

    #[inline]
    pub(crate) fn get_frozen_symbol(&self, name: &Symbol) -> Option<&UnboundValue> {
        self.members.get(name)
    }

    pub(crate) fn names(&self) -> Vec<String> {
        self.members.keys().map(|x| x.as_str().to_owned()).collect()
    }

    pub(crate) fn members(&self) -> impl Iterator<Item = (&str, FrozenValue)> {
        self.members
            .iter()
            .map(|(k, v)| (k.as_str(), v.to_frozen_value()))
    }

    /// Fetch the documentation.
    pub fn documentation(&self, ty: Ty) -> DocType {
        let (docs, members) = common_documentation(
            &self.docstring,
            self.members
                .iter()
                .map(|(n, v)| (n.as_str(), v.to_frozen_value())),
        );

        DocType {
            docs,
            members: members
                .filter_map(|(n, item)| {
                    // This is only `None` if the item is a module, but types shouldn't really have
                    // modules in them anyway, so that seems ok
                    Some((n, item.try_as_member_with_collapsed_object().ok()?))
                })
                .collect(),
            ty,
            constructor: None,
        }
    }
}

impl Methods {
    /// Create an empty [`Methods`], with no functions in scope.
    pub fn new() -> Self {
        MethodsBuilder::new().build()
    }
}

impl MethodsBuilder {
    /// Create an empty [`MethodsBuilder`], with no functions in scope.
    pub fn new() -> Self {
        MethodsBuilder {
            heap: FrozenHeap::new(),
            members: SymbolMap::new(),
            docstring: None,
        }
    }

    /// Called at the end to build a [`Methods`].
    pub fn build(self) -> Methods {
        Methods {
            heap: self.heap.into_ref(),
            members: self.members,
            docstring: self.docstring,
        }
    }

    /// A fluent API for modifying [`MethodsBuilder`] and returning the result.
    pub fn with(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }

    /// Set the raw docstring for this object.
    pub fn set_docstring(&mut self, docstring: &str) {
        self.docstring = Some(docstring.to_owned());
    }

    /// Set a constant value in the [`MethodsBuilder`] that will be suitable for use with
    /// [`StarlarkValue::get_methods`](crate::values::StarlarkValue::get_methods).
    pub fn set_attribute<'v, V: AllocFrozenValue>(
        &'v mut self,
        name: &str,
        value: V,
        docstring: Option<String>,
    ) {
        // We want to build an attribute, that ignores its self argument, and does no subsequent allocation.
        let value = self.heap.alloc(value);
        self.set_attribute_fn(
            name,
            true,
            docstring,
            V::starlark_type_repr(),
            move |_, _| Ok(value.to_value()),
        );
    }

    /// Set an attribute. This function is usually called from code
    /// generated by `starlark_derive` and rarely needs to be called manually.
    pub fn set_attribute_fn<F>(
        &mut self,
        name: &str,
        speculative_exec_safe: bool,
        docstring: Option<String>,
        typ: Ty,
        f: F,
    ) where
        F: for<'v> Fn(Value<'v>, &'v Heap) -> crate::Result<Value<'v>> + Send + Sync + 'static,
    {
        self.members.insert(
            name,
            UnboundValue::Attr(
                FrozenValueTyped::new(self.heap.alloc(NativeAttribute {
                    speculative_exec_safe,
                    docstring,
                    typ,
                }))
                .unwrap(),
                FrozenRef::<dyn NativeAttr + 'static>::new(
                    self.heap.alloc_any_debug_type_name(f).as_ref(),
                ),
            ),
        );
    }

    /// Set a method. This function is usually called from code
    /// generated by `starlark_derive` and rarely needs to be called manually.
    pub fn set_method<F>(&mut self, name: &str, components: NativeCallableComponents, f: F)
    where
        F: NativeMeth,
    {
        // TODO(nga): do not unwrap.
        let ty = Ty::from_native_callable_components(&components, None).unwrap();

        let function = FrozenRef::<dyn NativeMeth + 'static>::new(
            self.heap.alloc_any_debug_type_name(f).as_ref(),
        );
        self.members.insert(
            name,
            UnboundValue::Method(
                FrozenValueTyped::new(self.heap.alloc(NativeMethod {
                    function,
                    name: name.to_owned(),
                    speculative_exec_safe: components.speculative_exec_safe,
                    docs: components.into_docs(None),
                    ty,
                }))
                .unwrap(),
                function,
            ),
        );
    }

    /// Allocate a value using the same underlying heap as the [`MethodsBuilder`]
    pub fn alloc<'v, V: AllocFrozenValue>(&'v self, value: V) -> FrozenValue {
        value.alloc_frozen_value(&self.heap)
    }
}

/// Used to create methods for a [`StarlarkValue`](crate::values::StarlarkValue).
///
/// To define a method `foo()` on your type, define
///  usually written as:
///
/// ```ignore
/// fn my_methods(builder: &mut GlobalsBuilder) {
///     fn foo(me: ARef<Foo>) -> anyhow::Result<NoneType> {
///         ...
///     }
/// }
///
/// impl StarlarkValue<'_> for Foo {
///     ...
///     fn get_methods(&self) -> Option<&'static Globals> {
///         static RES: GlobalsStatic = GlobalsStatic::new();
///         RES.methods(module_creator)
///     }
///     ...
/// }
/// ```
pub struct MethodsStatic(OnceCell<Methods>);

impl MethodsStatic {
    /// Create a new [`MethodsStatic`].
    pub const fn new() -> Self {
        Self(OnceCell::new())
    }

    /// Populate the globals with a builder function. Always returns `Some`, but using this API
    /// to be a better fit for [`StarlarkValue.get_methods`](crate::values::StarlarkValue::get_methods).
    pub fn methods(&'static self, x: impl FnOnce(&mut MethodsBuilder)) -> Option<&'static Methods> {
        Some(self.0.get_or_init(|| MethodsBuilder::new().with(x).build()))
    }

    /// Copy all the methods in this [`MethodsBuilder`] into a new one. All variables will
    /// only be allocated once (ensuring things like function comparison works properly).
    pub fn populate(&'static self, x: impl FnOnce(&mut MethodsBuilder), out: &mut MethodsBuilder) {
        let methods = self.methods(x).unwrap();
        for (name, value) in methods.members.iter() {
            out.members.insert(name.as_str(), value.clone());
        }
        out.docstring = methods.docstring.clone();
    }
}

#[cfg(test)]
mod tests {
    use allocative::Allocative;
    use derive_more::Display;
    use starlark_derive::starlark_value;
    use starlark_derive::NoSerialize;
    use starlark_derive::ProvidesStaticType;

    use crate as starlark;
    use crate::assert::Assert;
    use crate::environment::Methods;
    use crate::environment::MethodsStatic;
    use crate::starlark_simple_value;
    use crate::values::StarlarkValue;

    #[test]
    fn test_set_attribute() {
        #[derive(Debug, Display, ProvidesStaticType, NoSerialize, Allocative)]
        #[display("Magic")]
        struct Magic;
        starlark_simple_value!(Magic);

        #[starlark_value(type = "magic")]
        impl<'v> StarlarkValue<'v> for Magic {
            fn get_methods() -> Option<&'static Methods> {
                static RES: MethodsStatic = MethodsStatic::new();
                RES.methods(|x| {
                    x.set_attribute("my_type", "magic", None);
                    x.set_attribute("my_value", 42, None);
                })
            }
        }

        let mut a = Assert::new();
        a.globals_add(|x| x.set("magic", Magic));
        a.pass(
            r#"
assert_eq(magic.my_type, "magic")
assert_eq(magic.my_value, 42)"#,
        );
    }
}
