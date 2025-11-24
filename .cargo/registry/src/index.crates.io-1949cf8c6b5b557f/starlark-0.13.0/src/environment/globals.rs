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

use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use itertools::Itertools;
use once_cell::sync::Lazy;
use once_cell::sync::OnceCell;

use crate::__derive_refs::components::NativeCallableComponents;
use crate::collections::symbol::map::SymbolMap;
use crate::collections::SmallMap;
use crate::docs::DocItem;
use crate::docs::DocModule;
use crate::docs::DocString;
use crate::docs::DocStringKind;
use crate::docs::DocType;
use crate::stdlib;
pub use crate::stdlib::LibraryExtension;
use crate::typing::Ty;
use crate::values::function::NativeFunc;
use crate::values::function::SpecialBuiltinFunction;
use crate::values::namespace::value::MaybeDocHiddenValue;
use crate::values::namespace::FrozenNamespace;
use crate::values::types::function::NativeFunction;
use crate::values::AllocFrozenValue;
use crate::values::FrozenHeap;
use crate::values::FrozenHeapRef;
use crate::values::FrozenStringValue;
use crate::values::FrozenValue;
use crate::values::Value;

/// The global values available during execution.
#[derive(Clone, Dupe, Debug, Allocative)]
pub struct Globals(Arc<GlobalsData>);

type GlobalValue = MaybeDocHiddenValue<'static, FrozenValue>;

#[derive(Debug, Allocative)]
struct GlobalsData {
    heap: FrozenHeapRef,
    variables: SymbolMap<GlobalValue>,
    variable_names: Vec<FrozenStringValue>,
    docstring: Option<String>,
}

/// Used to build a [`Globals`] value.
#[derive(Debug)]
pub struct GlobalsBuilder {
    // The heap everything is allocated in
    heap: FrozenHeap,
    // Normal top-level variables, e.g. True/hash
    variables: SymbolMap<GlobalValue>,
    // The list of struct fields, pushed to the end
    namespace_fields: Vec<SmallMap<FrozenStringValue, GlobalValue>>,
    /// The raw docstring for this module
    ///
    /// FIXME(JakobDegen): This should probably be removed. Having a docstring on a `GlobalsBuilder`
    /// doesn't really make sense, because there's no way good way to combine multiple docstrings.
    docstring: Option<String>,
}

impl Globals {
    /// Create an empty [`Globals`], with no functions in scope.
    pub fn new() -> Self {
        GlobalsBuilder::new().build()
    }

    /// Create a [`Globals`] following the
    /// [Starlark standard](https://github.com/bazelbuild/starlark/blob/master/spec.md#built-in-constants-and-functions).
    pub fn standard() -> Self {
        GlobalsBuilder::standard().build()
    }

    /// Create a [`Globals`] combining those functions in the Starlark standard plus
    /// all those defined in [`LibraryExtension`].
    ///
    /// This function is public to use in the `starlark` binary,
    /// but users of starlark should list the extensions they want explicitly.
    #[doc(hidden)]
    pub fn extended_internal() -> Self {
        GlobalsBuilder::extended().build()
    }

    /// Empty globals.
    pub(crate) fn empty() -> &'static Globals {
        static EMPTY: Lazy<Globals> = Lazy::new(|| GlobalsBuilder::new().build());
        &EMPTY
    }

    /// Create a [`Globals`] combining those functions in the Starlark standard plus
    /// all those given in the [`LibraryExtension`] arguments.
    pub fn extended_by(extensions: &[LibraryExtension]) -> Self {
        GlobalsBuilder::extended_by(extensions).build()
    }

    /// This function is only safe if you first call `heap` and keep a reference to it.
    /// Therefore, don't expose it on the public API.
    pub(crate) fn get<'v>(&'v self, name: &str) -> Option<Value<'v>> {
        self.get_frozen(name).map(FrozenValue::to_value)
    }

    /// This function is only safe if you first call `heap` and keep a reference to it.
    /// Therefore, don't expose it on the public API.
    pub(crate) fn get_frozen(&self, name: &str) -> Option<FrozenValue> {
        self.0.variables.get_str(name).map(|x| x.value)
    }

    /// Get all the names defined in this environment.
    pub fn names(&self) -> impl Iterator<Item = FrozenStringValue> + '_ {
        self.0.variable_names.iter().copied()
    }

    /// Iterate over all the items in this environment.
    /// Note returned values are owned by this globals.
    pub fn iter(&self) -> impl Iterator<Item = (&str, FrozenValue)> {
        self.0.variables.iter().map(|(n, v)| (n.as_str(), v.value))
    }

    pub(crate) fn heap(&self) -> &FrozenHeapRef {
        &self.0.heap
    }

    /// Print information about the values in this object.
    pub fn describe(&self) -> String {
        self.0
            .variables
            .iter()
            .map(|(name, val)| val.value.to_value().describe(name.as_str()))
            .join("\n")
    }

    /// Get the documentation for the object itself
    pub fn docstring(&self) -> Option<&str> {
        self.0.docstring.as_deref()
    }

    /// Get the documentation for both the object itself, and its members.
    pub fn documentation(&self) -> DocModule {
        let (docs, members) = common_documentation(
            &self.0.docstring,
            self.0
                .variables
                .iter()
                .filter(|(_, v)| !v.doc_hidden)
                .map(|(n, v)| (n.as_str(), v.value)),
        );
        DocModule {
            docs,
            members: members.collect(),
        }
    }
}

impl GlobalsBuilder {
    /// Create an empty [`GlobalsBuilder`], with no functions in scope.
    pub fn new() -> Self {
        Self {
            heap: FrozenHeap::new(),
            variables: SymbolMap::new(),
            namespace_fields: Vec::new(),
            docstring: None,
        }
    }

    /// Create a [`GlobalsBuilder`] following the
    /// [Starlark standard](https://github.com/bazelbuild/starlark/blob/master/spec.md#built-in-constants-and-functions).
    pub fn standard() -> Self {
        stdlib::standard_environment()
    }

    /// Create a [`GlobalsBuilder`] combining those functions in the Starlark standard plus
    /// all those defined in [`LibraryExtension`].
    pub(crate) fn extended() -> Self {
        Self::extended_by(LibraryExtension::all())
    }

    /// Create a [`GlobalsBuilder`] combining those functions in the Starlark standard plus
    /// all those defined in [`LibraryExtension`].
    pub fn extended_by(extensions: &[LibraryExtension]) -> Self {
        let mut res = Self::standard();
        for x in extensions {
            x.add(&mut res);
        }
        res
    }

    /// Add a nested namespace to the builder. If `f` adds the definition `foo`,
    /// it will end up on a namespace `name`, accessible as `name.foo`.
    pub fn namespace(&mut self, name: &str, f: impl FnOnce(&mut GlobalsBuilder)) {
        self.namespace_inner(name, false, f)
    }

    /// Same as `namespace`, but this value will not show up in generated documentation.
    pub fn namespace_no_docs(&mut self, name: &str, f: impl FnOnce(&mut GlobalsBuilder)) {
        self.namespace_inner(name, true, f)
    }

    fn namespace_inner(
        &mut self,
        name: &str,
        doc_hidden: bool,
        f: impl FnOnce(&mut GlobalsBuilder),
    ) {
        self.namespace_fields.push(SmallMap::new());
        f(self);
        let fields = self.namespace_fields.pop().unwrap();
        self.set_inner(
            name,
            self.heap.alloc(FrozenNamespace::new(fields)),
            doc_hidden,
        );
    }

    /// A fluent API for modifying [`GlobalsBuilder`] and returning the result.
    pub fn with(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }

    /// A fluent API for modifying [`GlobalsBuilder`] using [`namespace`](GlobalsBuilder::namespace).
    pub fn with_namespace(mut self, name: &str, f: impl Fn(&mut GlobalsBuilder)) -> Self {
        self.namespace(name, f);
        self
    }

    /// Called at the end to build a [`Globals`].
    pub fn build(self) -> Globals {
        let mut variable_names: Vec<_> = self
            .variables
            .keys()
            .map(|x| self.heap.alloc_str_intern(x.as_str()))
            .collect();
        variable_names.sort();
        Globals(Arc::new(GlobalsData {
            heap: self.heap.into_ref(),
            variables: self.variables,
            variable_names,
            docstring: self.docstring,
        }))
    }

    /// Set a value in the [`GlobalsBuilder`].
    pub fn set<'v, V: AllocFrozenValue>(&'v mut self, name: &str, value: V) {
        let value = value.alloc_frozen_value(&self.heap);
        self.set_inner(name, value, false)
    }

    fn set_inner<'v>(&'v mut self, name: &str, value: FrozenValue, doc_hidden: bool) {
        let value = MaybeDocHiddenValue {
            value,
            doc_hidden,
            phantom: Default::default(),
        };
        match self.namespace_fields.last_mut() {
            None => {
                // TODO(nga): do not quietly ignore redefinitions.
                self.variables.insert(name, value)
            }
            Some(fields) => {
                let name = self.heap.alloc_str(name);
                fields.insert(name, value)
            }
        };
    }

    /// Set a method. This function is usually called from code
    /// generated by `starlark_derive` and rarely needs to be called manually.
    pub fn set_function<F>(
        &mut self,
        name: &str,
        components: NativeCallableComponents,
        as_type: Option<(Ty, DocType)>,
        ty: Option<Ty>,
        special_builtin_function: Option<SpecialBuiltinFunction>,
        f: F,
    ) where
        F: NativeFunc,
    {
        self.set(
            name,
            NativeFunction {
                function: Box::new(f),
                name: name.to_owned(),
                speculative_exec_safe: components.speculative_exec_safe,
                as_type: as_type.as_ref().map(|x| x.0.dupe()),
                ty: ty.unwrap_or_else(|| {
                    Ty::from_native_callable_components(
                        &components,
                        as_type.as_ref().map(|x| x.0.dupe()),
                    )
                    .unwrap() // TODO(nga): do not unwrap.
                }),
                docs: components.into_docs(as_type),
                special_builtin_function,
            },
        )
    }

    /// Heap where globals are allocated. Can be used to allocate additional values.
    pub fn frozen_heap(&self) -> &FrozenHeap {
        &self.heap
    }

    /// Allocate a value using the same underlying heap as the [`GlobalsBuilder`],
    /// only intended for values that are referred to by those which are passed
    /// to [`set`](GlobalsBuilder::set).
    pub fn alloc<'v, V: AllocFrozenValue>(&'v self, value: V) -> FrozenValue {
        value.alloc_frozen_value(&self.heap)
    }

    /// Set per module docstring.
    ///
    /// This function is called by the `starlark_derive` generated code
    /// and rarely needs to be called manually.
    pub fn set_docstring(&mut self, docstring: &str) {
        self.docstring = Some(docstring.to_owned());
    }
}

/// Used to create globals.
pub struct GlobalsStatic(OnceCell<Globals>);

impl GlobalsStatic {
    /// Create a new [`GlobalsStatic`].
    pub const fn new() -> Self {
        Self(OnceCell::new())
    }

    fn globals(&'static self, x: impl FnOnce(&mut GlobalsBuilder)) -> &'static Globals {
        self.0.get_or_init(|| GlobalsBuilder::new().with(x).build())
    }

    /// Get a function out of the object. Requires that the function passed only set a single
    /// value. If populated via a `#[starlark_module]`, that means a single function in it.
    pub fn function(&'static self, x: impl FnOnce(&mut GlobalsBuilder)) -> FrozenValue {
        let globals = self.globals(x);
        assert!(
            globals.0.variables.len() == 1,
            "GlobalsBuilder.function must have exactly 1 member, you had {}",
            globals
                .names()
                .map(|s| format!("`{}`", s.as_str()))
                .join(", ")
        );

        globals.0.variables.values().next().unwrap().value
    }

    /// Move all the globals in this [`GlobalsBuilder`] into a new one. All variables will
    /// only be allocated once (ensuring things like function comparison works properly).
    pub fn populate(&'static self, x: impl FnOnce(&mut GlobalsBuilder), out: &mut GlobalsBuilder) {
        let globals = self.globals(x);
        for (name, value) in globals.0.variables.iter() {
            out.set_inner(name.as_str(), value.value, value.doc_hidden)
        }
        out.docstring = globals.0.docstring.clone();
    }
}

pub(crate) fn common_documentation<'a>(
    docstring: &Option<String>,
    members: impl IntoIterator<Item = (&'a str, FrozenValue)>,
) -> (Option<DocString>, impl Iterator<Item = (String, DocItem)>) {
    let main_docs = docstring
        .as_ref()
        .and_then(|ds| DocString::from_docstring(DocStringKind::Rust, ds));
    let member_docs = members
        .into_iter()
        .map(|(name, val)| (name.to_owned(), val.to_value().documentation()))
        .sorted_by(|(l, _), (r, _)| Ord::cmp(l, r));

    (main_docs, member_docs)
}

#[cfg(test)]
mod tests {
    use starlark_derive::starlark_module;

    use super::*;
    use crate as starlark;

    #[test]
    fn test_send_sync()
    where
        Globals: Send + Sync,
    {
    }

    #[starlark_module]
    fn register_foo(builder: &mut GlobalsBuilder) {
        fn foo() -> anyhow::Result<i32> {
            Ok(1)
        }
    }

    #[test]
    fn test_doc_hidden() {
        let mut globals = GlobalsBuilder::new();
        globals.namespace_no_docs("ns_hidden", |_| {});
        globals.namespace("ns", |globals| {
            globals.namespace_no_docs("nested_ns_hidden", |_| {});
            globals.set("x", FrozenValue::new_none());
        });
        let docs = globals.build().documentation();

        let (k, v) = docs.members.into_iter().exactly_one().ok().unwrap();
        assert_eq!(&k, "ns");
        let DocItem::Module(docs) = v else {
            unreachable!()
        };
        assert_eq!(&docs.members.into_keys().exactly_one().ok().unwrap(), "x");
    }
}
