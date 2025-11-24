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

//! Types supporting documentation for code written in or for Starlark.

// TODO(nga): document it
#![allow(missing_docs)]

pub mod code;
pub mod markdown;
pub mod multipage;
mod parse;
#[cfg(test)]
mod tests;

use std::iter;

use allocative::Allocative;
pub use parse::DocStringKind;
use starlark_map::small_map::SmallMap;

use crate as starlark;
use crate::eval::runtime::params::display::iter_fmt_param_spec;
pub use crate::eval::runtime::params::display::FmtParam;
use crate::typing::Ty;
use crate::values::type_repr::StarlarkTypeRepr;
use crate::values::StarlarkValue;
use crate::values::Trace;
use crate::values::Value;

/// The documentation provided by a user for a specific module, object, function, etc.
#[derive(Debug, Clone, PartialEq, Trace, Default, Allocative)]
pub struct DocString {
    /// The first line of a doc string. This has whitespace trimmed from it.
    pub summary: String,
    /// The contents of a doc string that follow the summary, and a single blank line.
    /// This also has whitespace trimmed from it, and it is dedented.
    pub details: Option<String>,
}

/// The documentation for a module/namespace.
///
/// See the docs on [`DocType`] for the distinction between that type and this one.
#[derive(Debug, Clone, PartialEq, Default, Allocative)]
pub struct DocModule {
    pub docs: Option<DocString>,
    pub members: SmallMap<String, DocItem>,
}

/// Documents a single function.
#[derive(Debug, Clone, PartialEq, Default, Allocative)]
pub struct DocFunction {
    /// Documentation for the function. If parsed, this should generally be the first statement
    /// of a function's body if that statement is a string literal. Any sections like "Args:",
    /// "Returns", etc are kept intact. It is up to the consumer to remove these sections if
    /// they are present.
    pub docs: Option<DocString>,
    /// The parameters that this function takes. Docs for these parameters should generally be
    /// extracted from the main docstring's details.
    pub params: DocParams,
    /// Details about what this function returns.
    pub ret: DocReturn,
}

impl DocFunction {
    /// Used by LSP. Return starred name and the doc.
    pub fn find_param_with_name(&self, param_name: &str) -> Option<(String, &DocParam)> {
        self.params
            .doc_params_with_starred_names()
            .find(|(_, p)| p.name == param_name)
    }
}

/// Function parameters.
#[derive(Debug, Clone, PartialEq, Default, Allocative)]
pub struct DocParams {
    pub pos_only: Vec<DocParam>,
    pub pos_or_named: Vec<DocParam>,
    pub args: Option<DocParam>,
    pub named_only: Vec<DocParam>,
    pub kwargs: Option<DocParam>,
}

impl DocParams {
    /// Iterate parameters ignoring information about positional-only, named-only.
    pub(crate) fn doc_params(&self) -> impl Iterator<Item = &DocParam> {
        iter::empty()
            .chain(&self.pos_only)
            .chain(&self.pos_or_named)
            .chain(&self.args)
            .chain(&self.named_only)
            .chain(&self.kwargs)
    }

    pub(crate) fn doc_params_with_starred_names(
        &self,
    ) -> impl Iterator<Item = (String, &DocParam)> {
        iter::empty()
            .chain(self.pos_only.iter().map(|p| (p.name.clone(), p)))
            .chain(self.pos_or_named.iter().map(|p| (p.name.clone(), p)))
            .chain(self.args.iter().map(|p| (format!("*{}", p.name), p)))
            .chain(self.named_only.iter().map(|p| (p.name.clone(), p)))
            .chain(self.kwargs.iter().map(|p| (format!("**{}", p.name), p)))
    }

    pub(crate) fn doc_params_mut(&mut self) -> impl Iterator<Item = &mut DocParam> {
        iter::empty()
            .chain(&mut self.pos_only)
            .chain(&mut self.pos_or_named)
            .chain(&mut self.args)
            .chain(&mut self.named_only)
            .chain(&mut self.kwargs)
    }

    /// Non-star parameters.
    pub fn regular_params(&self) -> impl Iterator<Item = &DocParam> {
        iter::empty()
            .chain(&self.pos_only)
            .chain(&self.pos_or_named)
            .chain(&self.named_only)
    }

    /// Iterate params with `/` and `*` markers to output function signature.
    pub fn fmt_params(&self) -> impl Iterator<Item = FmtParam<&'_ DocParam>> {
        iter_fmt_param_spec(
            &self.pos_only,
            &self.pos_or_named,
            self.args.as_ref(),
            &self.named_only,
            self.kwargs.as_ref(),
        )
    }
}

/// A single parameter of a function.
#[derive(Debug, Clone, PartialEq, Allocative)]
pub struct DocParam {
    /// Does not include `*` or `**`.
    pub name: String,
    pub docs: Option<DocString>,
    /// Element type for `*args` and value type for `**kwargs`.
    pub typ: Ty,
    pub default_value: Option<String>,
}

/// Details about the return value of a function.
#[derive(Debug, Clone, PartialEq, Allocative)]
pub struct DocReturn {
    /// Extra semantic details around the returned value's meaning.
    pub docs: Option<DocString>,
    pub typ: Ty,
}

impl Default for DocReturn {
    fn default() -> Self {
        DocReturn {
            docs: None,
            typ: Ty::any(),
        }
    }
}

/// A single property of an object. These are explicitly not functions (see [`DocMember`]).
#[derive(Debug, Clone, PartialEq, Allocative)]
pub struct DocProperty {
    pub docs: Option<DocString>,
    pub typ: Ty,
}

/// A named member of an object.
#[derive(Debug, Clone, PartialEq, Allocative)]
pub enum DocMember {
    Property(DocProperty),
    Function(DocFunction),
}

impl DocMember {
    pub(crate) fn from_value(value: Value) -> Self {
        // If we have a value which is a complex type, the right type to put in the docs is not the type
        // it represents, but it's just a property we should point at
        match value.documentation() {
            DocItem::Member(x) => x,
            _ => DocMember::Property(DocProperty {
                docs: None,
                typ: value.get_type_starlark_repr(),
            }),
        }
    }
}

/// The documentation for a type
///
/// This is distinct from a module since, well, types and modules are different things, but more
/// importantly because the members here are expected to be attributes on *values* of the type, not
/// on the type itself. In other words, if I have a global `FooRecord`, and its documentation says
/// that it's a type with member `x`, then the expectation is not that `FooRecord.x` works, but
/// rather that `foo.x` works, where `foo` is of type `FooRecord`. On the other hand, if there's a
/// global `m`, and `m`'s documentation says it's a module with member `x`, then `m.x` should work.
#[derive(Debug, Clone, PartialEq, Allocative)]
pub struct DocType {
    pub docs: Option<DocString>,
    /// Name and details of each attr/function that can be accessed on this type.
    pub members: SmallMap<String, DocMember>,
    pub ty: Ty,
    pub constructor: Option<DocFunction>,
}

impl DocType {
    pub fn from_starlark_value<T: StarlarkValue<'static>>() -> DocType {
        let ty = T::starlark_type_repr();
        match T::get_methods() {
            Some(methods) => methods.documentation(ty),
            None => DocType {
                docs: None,
                members: SmallMap::new(),
                ty,
                constructor: None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Allocative)]
pub enum DocItem {
    Module(DocModule),
    Type(DocType),
    Member(DocMember),
}

impl DocItem {
    /// Get the underlying [`DocString`] for this item, if it exists.
    pub fn get_doc_string(&self) -> Option<&DocString> {
        match self {
            DocItem::Module(m) => m.docs.as_ref(),
            DocItem::Type(o) => o.docs.as_ref(),
            DocItem::Member(DocMember::Function(f)) => f.docs.as_ref(),
            DocItem::Member(DocMember::Property(p)) => p.docs.as_ref(),
        }
    }

    /// Get the summary of the underlying [`DocString`] for this item, if it exists.
    pub fn get_doc_summary(&self) -> Option<&str> {
        self.get_doc_string().map(|ds| ds.summary.as_str())
    }

    /// Converts to a doc member, if possible.
    ///
    /// This conversion is trivial, except in the case of objects - those are flattened into a
    /// single property that just indicates their type
    pub fn try_as_member_with_collapsed_object(&self) -> Result<DocMember, &DocModule> {
        match self {
            DocItem::Module(m) => Err(m),
            DocItem::Member(m) => Ok(m.clone()),
            DocItem::Type(o) => Ok(DocMember::Property(DocProperty {
                docs: o.docs.clone(),
                typ: o.ty.clone(),
            })),
        }
    }

    pub fn try_as_member(&self) -> Option<DocMember> {
        match self {
            DocItem::Member(m) => Some(m.clone()),
            _ => None,
        }
    }
}

impl DocMember {
    /// Get the underlying [`DocString`] for this item, if it exists.
    pub fn get_doc_string(&self) -> Option<&DocString> {
        match self {
            DocMember::Function(f) => f.docs.as_ref(),
            DocMember::Property(p) => p.docs.as_ref(),
        }
    }

    /// Get the summary of the underlying [`DocString`] for this item, if it exists.
    pub fn get_doc_summary(&self) -> Option<&str> {
        self.get_doc_string().map(|ds| ds.summary.as_str())
    }
}

impl DocParam {
    /// Get the underlying [`DocString`] for this item, if it exists.
    pub fn get_doc_string(&self) -> Option<&DocString> {
        self.docs.as_ref()
    }

    /// Get the summary of the underlying [`DocString`] for this item, if it exists.
    pub fn get_doc_summary(&self) -> Option<&str> {
        self.get_doc_string().map(|ds| ds.summary.as_str())
    }
}
