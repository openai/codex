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

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::format_ident;
use quote::quote;
use quote::ToTokens;

use crate::module::render::fun::render_fun;
use crate::module::render::fun::render_none;
use crate::module::render::fun::render_some;
use crate::module::simple_param::SimpleParam;
use crate::module::typ::SpecialParam;
use crate::module::typ::StarAttr;
use crate::module::typ::StarConst;
use crate::module::typ::StarFun;
use crate::module::typ::StarModule;
use crate::module::typ::StarStmt;
use crate::module::util::ident_string;

pub(crate) fn render(x: StarModule) -> syn::Result<TokenStream> {
    Ok(render_impl(x)?.to_token_stream())
}

fn render_impl(x: StarModule) -> syn::Result<syn::ItemFn> {
    let StarModule {
        name,
        globals_builder,
        visibility,
        attrs,
        docstring,
        stmts,
        module_kind,
    } = x;
    let statics = format_ident!("{}", module_kind.statics_type_name());
    let stmts: Vec<_> = stmts
        .into_iter()
        .map(render_stmt)
        .collect::<syn::Result<_>>()?;
    let set_docstring = docstring.map(|ds| quote!(globals_builder.set_docstring(#ds);));
    Ok(syn::parse_quote! {
        #( #attrs )*
        #visibility fn #name(globals_builder: #globals_builder) {
            fn build(globals_builder: #globals_builder) {
                #set_docstring
                #( #stmts )*
                // Mute warning if stmts is empty.
                let _ = globals_builder;
            }
            static RES: starlark::environment::#statics = starlark::environment::#statics::new();
            RES.populate(build, globals_builder);
        }
    })
}

fn render_stmt(x: StarStmt) -> syn::Result<syn::Stmt> {
    match x {
        StarStmt::Const(x) => Ok(render_const(x)),
        StarStmt::Attr(x) => Ok(render_attr(x)),
        StarStmt::Fun(x) => render_fun(x),
    }
}

fn render_const(x: StarConst) -> syn::Stmt {
    let StarConst { name, ty, value } = x;
    let name = ident_string(&name);
    syn::parse_quote! {
        globals_builder.set::<#ty>(#name, #value);
    }
}

fn render_attr(x: StarAttr) -> syn::Stmt {
    let StarAttr {
        name,
        this,
        heap,
        attrs,
        return_type,
        speculative_exec_safe,
        body,
        docstring,
    } = x;
    let name_str = ident_string(&name);
    let name_inner = syn::Ident::new(&format!("{}__inner", name_str), name.span());
    let docstring: syn::Expr = match docstring {
        Some(d) => render_some(syn::parse_quote! { #d.to_owned() }),
        None => render_none(),
    };

    let let_heap = if let Some(SpecialParam {
        param: SimpleParam { ident, ty, .. },
    }) = heap
    {
        Some(quote! { let #ident: #ty = __heap; })
    } else {
        None
    };

    let this_value: syn::Ident = syn::parse_quote! { s_this_value };

    let unpack = this.render_prepare(&this.param.ident, &this_value);

    let inner: syn::ItemFn = syn::parse_quote! {
        #( #attrs )*
        #[allow(non_snake_case)] // Starlark doesn't have this convention
        fn #name_inner<'v>(
            #this_value: starlark::values::Value<'v>,
            #[allow(unused_variables)]
            __heap: &'v starlark::values::Heap,
        ) -> #return_type {
            #[allow(unused_variables)]
            #unpack
            #let_heap
            #body
        }
    };

    let outer: syn::ItemFn = syn::parse_quote! {
        #[allow(non_snake_case)]
        fn #name<'v>(
            #[allow(unused_variables)]
            this: starlark::values::Value<'v>,
            heap: &'v starlark::values::Heap,
        ) -> starlark::Result<starlark::values::Value<'v>> {
            Ok(heap.alloc(#name_inner(this, heap)?))
        }
    };

    syn::parse_quote! {
        {
            #inner
            #outer

            globals_builder.set_attribute_fn(
                #name_str,
                #speculative_exec_safe,
                #docstring,
                starlark::values::type_repr::type_repr_from_attr_impl(#name_inner),
                #name
            );
        }
    }
}

/// Get the lifetimes that are mentioned in a given type and its nested generics.
fn get_lifetimes_inner<'a>(ret: &mut HashSet<&'a syn::Lifetime>, typ: &'a syn::Type) {
    match typ {
        syn::Type::Path(path) => {
            if let Some(segment) = path.path.segments.last() {
                match &segment.arguments {
                    syn::PathArguments::None => {}
                    syn::PathArguments::AngleBracketed(args) => {
                        for arg in &args.args {
                            match arg {
                                syn::GenericArgument::Lifetime(l) => {
                                    ret.insert(l);
                                }
                                syn::GenericArgument::Type(t) => get_lifetimes_inner(ret, t),
                                _ => {}
                            };
                        }
                    }
                    syn::PathArguments::Parenthesized(args) => {
                        for t in &args.inputs {
                            get_lifetimes_inner(ret, t);
                        }
                        match &args.output {
                            syn::ReturnType::Default => {}
                            syn::ReturnType::Type(_, t) => get_lifetimes_inner(ret, t),
                        };
                    }
                };
            }
        }
        syn::Type::Group(g) => get_lifetimes_inner(ret, &g.elem),
        syn::Type::Paren(p) => get_lifetimes_inner(ret, &p.elem),
        syn::Type::Ptr(p) => get_lifetimes_inner(ret, &p.elem),
        syn::Type::Reference(r) => {
            if let Some(l) = &r.lifetime {
                ret.insert(l);
            };
            get_lifetimes_inner(ret, &r.elem);
        }
        syn::Type::Tuple(t) => {
            for t in &t.elems {
                get_lifetimes_inner(ret, t);
            }
        }
        _ => {}
    };
}

/// Get the lifetime specifications to use with a function based on the lifetimes mentioned in `typ`.
///
/// e.g. `i32` would return ``, `Vec<(&'a str, &'b str)>` would return `<'a, 'b>`
fn get_lifetimes(typ: &syn::Type) -> TokenStream {
    let mut ret = HashSet::new();
    get_lifetimes_inner(&mut ret, typ);
    if ret.is_empty() {
        TokenStream::new()
    } else {
        let mut ret: Vec<_> = ret.into_iter().filter(|l| l.ident != "_").collect();
        ret.sort_by(|l, r| l.ident.cmp(&r.ident));
        quote!(<#(#ret),*>)
    }
}

pub(crate) fn render_starlark_type(typ: &syn::Type) -> syn::Expr {
    let lifetimes = get_lifetimes(typ);
    syn::parse_quote! {
        {
            #[allow(clippy::extra_unused_lifetimes)]
            fn get_type_string #lifetimes() -> starlark::typing::Ty {
                <#typ as starlark::values::type_repr::StarlarkTypeRepr>::starlark_type_repr()
            }
            get_type_string()
        }
    }
}

pub(crate) fn render_starlark_return_type(fun: &StarFun) -> syn::Expr {
    let struct_name = fun.struct_name();
    syn::parse_quote! {
        #struct_name::return_type_starlark_type_repr()
    }
}
