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

use dupe::Dupe;
use proc_macro2::Ident;
use proc_macro2::Span;
use syn::spanned::Spanned;
use syn::Attribute;
use syn::Block;
use syn::Expr;
use syn::Type;
use syn::Visibility;

use crate::module::parse::ModuleKind;
use crate::module::simple_param::SimpleParam;
use crate::module::util::is_type_name;
use crate::module::util::unpack_option;

#[derive(Debug)]
pub(crate) struct StarModule {
    pub module_kind: ModuleKind,
    pub visibility: Visibility,
    // We reuse the users globals_builder to make sure `use` statements etc
    // make sense
    pub globals_builder: Type,
    pub name: Ident,
    pub attrs: Vec<Attribute>,
    pub docstring: Option<String>,
    pub stmts: Vec<StarStmt>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum StarStmt {
    Const(StarConst),
    Fun(StarFun),
    Attr(StarAttr),
}

#[derive(Debug)]
pub(crate) struct StarConst {
    pub name: Ident,
    pub ty: Type,
    pub value: Expr,
}

#[derive(Debug)]
pub(crate) struct SpecialParam {
    pub(crate) param: SimpleParam,
}

#[derive(Debug)]
pub(crate) struct StarFun {
    pub name: Ident,
    pub as_type: Option<syn::Path>,
    pub attrs: Vec<Attribute>,
    pub this: Option<ThisParam>,
    pub args: RegularParams,
    /// Has `&Heap` parameter.
    pub heap: Option<SpecialParam>,
    /// Has `&mut Evaluator` parameter.
    pub eval: Option<SpecialParam>,
    /// `anyhow::Result<T>`.
    pub return_type: Type,
    pub starlark_ty_custom_function: Option<Expr>,
    pub special_builtin_function: Option<Expr>,
    pub speculative_exec_safe: bool,
    pub body: Block,
    pub source: StarFunSource,
    pub docstring: Option<String>,
}

impl StarFun {
    /// Is this function a method? (I. e. has `this` as first parameter).
    pub(crate) fn is_method(&self) -> bool {
        self.this.is_some()
    }

    pub(crate) fn span(&self) -> Span {
        self.name
            .span()
            .join(self.body.span())
            .unwrap_or_else(|| self.name.span())
    }
}

#[derive(Debug)]
pub(crate) struct StarAttr {
    pub name: Ident,
    pub this: ThisParam,
    /// Has `&Heap` parameter.
    pub heap: Option<SpecialParam>,
    pub attrs: Vec<Attribute>,
    /// `anyhow::Result<T>`.
    pub return_type: Type,
    pub speculative_exec_safe: bool,
    pub body: Block,
    pub docstring: Option<String>,
}

#[derive(Debug, PartialEq, Copy, Clone, Dupe)]
pub(crate) enum StarArgPassStyle {
    /// Parameter can be filled only positionally.
    PosOnly,
    /// Parameter can be filled positionally or by name.
    PosOrNamed,
    /// Parameter can be filled by name.
    NamedOnly,
    /// `*args`.
    Args,
    /// `**kwargs`.
    Kwargs,
}

/// Method `this` parameter, always first.
#[derive(Debug, Clone)]
pub(crate) struct ThisParam {
    pub(crate) param: SimpleParam,
}

impl ThisParam {
    pub(crate) fn render_prepare(&self, target: &syn::Ident, value: &syn::Ident) -> syn::Stmt {
        let ty = &self.param.ty;
        syn::parse_quote! {
            let #target: #ty = starlark::__derive_refs::parse_args::check_this(#value)?;
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StarArg {
    pub span: Span,
    pub(crate) param: SimpleParam,
    pub pass_style: StarArgPassStyle,
    pub default: Option<Expr>,
    pub source: StarArgSource,
}

/// `&Arguments` parameter.
#[derive(Debug)]
pub(crate) struct StarArguments {
    pub(crate) param: SimpleParam,
}

/// How we handle `&Arguments`.
#[derive(Debug)]
pub(crate) enum RegularParams {
    /// Pass `&Arguments` as is.
    Arguments(StarArguments),
    /// Unpack the `&Arguments` into a multiple typed parameters.
    Unpack(Vec<StarArg>),
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum StarArgSource {
    Unknown,
    Argument(usize),
    Required(usize),
    Optional(usize),
    Kwargs,
}

#[derive(Debug)]
pub(crate) enum StarFunSource {
    /// Function signature is single `Arguments` parameter.
    Arguments,
    /// Normal function which uses a signature and parameters parser.
    Signature { count: usize },
    /// Fast-path function of some required parameters, followed by some optional parameters.
    /// No named parameters or `*args`, but may have `**kwargs`.
    Positional {
        required: usize,
        optional: usize,
        kwargs: bool,
    },
}

impl StarArg {
    pub fn is_option(&self) -> bool {
        is_type_name(&self.param.ty, "Option")
    }

    /// Remove the `Option` if it exists, otherwise return the real type.
    pub fn without_option(&self) -> &Type {
        unpack_option(&self.param.ty).unwrap_or(&self.param.ty)
    }

    pub fn is_value(&self) -> bool {
        is_type_name(&self.param.ty, "Value")
    }

    /// Parameter type is `Option<Value>`.
    pub(crate) fn is_option_value(&self) -> bool {
        self.is_option() && is_type_name(self.without_option(), "Value")
    }

    pub fn requires_signature(&self) -> bool {
        // We need to use a signature if something has a name
        // There are *args
        // There is a default that needs promoting to a Value (since the signature stores that value)
        (self.pass_style != StarArgPassStyle::PosOnly
            && self.pass_style != StarArgPassStyle::Kwargs)
            || (self.is_value() && self.default.is_some())
    }
}
