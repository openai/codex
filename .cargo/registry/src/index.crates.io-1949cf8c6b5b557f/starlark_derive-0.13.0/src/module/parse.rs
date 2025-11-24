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

mod fun;

use dupe::Dupe;
use syn::spanned::Spanned;
use syn::Attribute;
use syn::Expr;
use syn::ExprLit;
use syn::FnArg;
use syn::Item;
use syn::ItemConst;
use syn::ItemFn;
use syn::Meta;
use syn::MetaNameValue;
use syn::PatType;
use syn::Stmt;
use syn::Type;
use syn::TypeReference;
use syn::Visibility;

use crate::module::parse::fun::parse_fun;
use crate::module::typ::StarConst;
use crate::module::typ::StarModule;
use crate::module::typ::StarStmt;
use crate::module::util::is_type_name;

#[derive(Debug, Copy, Clone, Dupe, PartialEq, Eq)]
pub(crate) enum ModuleKind {
    Globals,
    Methods,
}

impl ModuleKind {
    pub(crate) fn statics_type_name(self) -> &'static str {
        match self {
            ModuleKind::Globals => "GlobalsStatic",
            ModuleKind::Methods => "MethodsStatic",
        }
    }
}

pub(crate) fn parse(mut input: ItemFn) -> syn::Result<StarModule> {
    let (module_docstring, attrs) = parse_module_attributes(&input)?;
    let visibility = input.vis;
    let sig_span = input.sig.span();
    let name = input.sig.ident;

    if input.sig.inputs.len() != 1 {
        return Err(syn::Error::new(
            sig_span,
            "function must have exactly one argument",
        ));
    }
    let arg = input.sig.inputs.pop().unwrap();
    let arg_span = arg.span();

    let (ty, module_kind) = match arg.into_value() {
        FnArg::Typed(PatType { ty, .. }) if is_mut_globals_builder(&ty) => {
            (ty, ModuleKind::Globals)
        }
        FnArg::Typed(PatType { ty, .. }) if is_mut_methods_builder(&ty) => {
            (ty, ModuleKind::Methods)
        }
        _ => {
            return Err(syn::Error::new(
                arg_span,
                "Expected a mutable globals or methods builder",
            ));
        }
    };
    Ok(StarModule {
        module_kind,
        visibility,
        globals_builder: *ty,
        name,
        attrs,
        docstring: module_docstring,
        stmts: input
            .block
            .stmts
            .into_iter()
            .map(|stmt| parse_stmt(stmt, module_kind))
            .collect::<syn::Result<_>>()?,
    })
}

fn is_attribute_docstring(x: &Attribute) -> syn::Result<Option<String>> {
    if x.path().is_ident("doc") {
        if let Meta::NameValue(MetaNameValue {
            value:
                Expr::Lit(ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }),
            ..
        }) = &x.meta
        {
            let ds = s.value();
            if ds.starts_with("# ") || ds.starts_with("## ") {
                return Err(syn::Error::new(
                    x.span(),
                    "Docstrings may not contain H1 or H2 headers (`#` or `##`) as this leads to \
                    poor generated documentation. Use at least H3 (`###`) instead.",
                ));
            }

            return Ok(Some(ds));
        }
    }
    Ok(None)
}

/// Return (docstring, other attributes)
fn parse_module_attributes(input: &ItemFn) -> syn::Result<(Option<String>, Vec<Attribute>)> {
    let mut doc_attrs = Vec::new();
    let mut attrs = Vec::new();
    for attr in &input.attrs {
        if let Some(ds) = is_attribute_docstring(attr)? {
            doc_attrs.push(ds);
        } else {
            attrs.push(attr.clone());
        }
    }
    let docs = if doc_attrs.is_empty() {
        None
    } else {
        Some(doc_attrs.join("\n"))
    };
    Ok((docs, attrs))
}

fn parse_stmt(stmt: Stmt, module_kind: ModuleKind) -> syn::Result<StarStmt> {
    match stmt {
        Stmt::Item(Item::Fn(x)) => parse_fun(x, module_kind),
        Stmt::Item(Item::Const(x)) => Ok(StarStmt::Const(parse_const(x)?)),
        s => Err(syn::Error::new(
            s.span(),
            "Can only put constants and functions inside a #[starlark_module]",
        )),
    }
}

fn parse_visibility(vis: &Visibility) -> syn::Result<()> {
    match vis {
        Visibility::Inherited => Ok(()),
        _ => Err(syn::Error::new(
            vis.span(),
            "Visibility modifiers are not allowed inside a `#[starlark_module]`",
        )),
    }
}

fn parse_const(x: ItemConst) -> syn::Result<StarConst> {
    parse_visibility(&x.vis)?;

    Ok(StarConst {
        name: x.ident,
        ty: *x.ty,
        value: *x.expr,
    })
}

fn is_mut_something(x: &Type, smth: &str) -> bool {
    match x {
        Type::Reference(TypeReference {
            mutability: Some(_),
            elem: x,
            ..
        }) => is_type_name(x, smth),
        _ => false,
    }
}

pub(crate) fn is_ref_something(x: &Type, smth: &str) -> bool {
    match x {
        Type::Reference(TypeReference {
            mutability: None,
            elem: x,
            ..
        }) => is_type_name(x, smth),
        _ => false,
    }
}

// Is the type `&mut GlobalsBuilder`
fn is_mut_globals_builder(x: &Type) -> bool {
    is_mut_something(x, "GlobalsBuilder")
}

// Is the type `&mut MethodsBuilder`
fn is_mut_methods_builder(x: &Type) -> bool {
    is_mut_something(x, "MethodsBuilder")
}
