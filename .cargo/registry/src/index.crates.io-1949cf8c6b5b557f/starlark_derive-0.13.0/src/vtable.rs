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

use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use quote::quote_spanned;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::spanned::Spanned;
use syn::visit_mut::VisitMut;
use syn::FnArg;
use syn::ItemTrait;
use syn::Pat;
use syn::ReturnType;
use syn::TraitItem;
use syn::TraitItemFn;

/// Constant/field name for a flag whether a member is overridden.
pub(crate) fn vtable_has_field_name(name: &syn::Ident) -> syn::Ident {
    quote::format_ident!("HAS_{}", name)
}

struct Gen {
    starlark_value: ItemTrait,
}

struct VTableEntry {
    field: syn::Field,
    init: syn::FieldValue,
    init_for_black_hole: syn::FieldValue,
}

/// Description of vtable fn parameter.
struct VTableFnParam {
    /// Span for error reporting.
    span: Span,
    /// Name of the parameter.
    name: Ident,
    /// Type of the parameter.
    ty: syn::Type,
    /// Expression to convert function argument to method argument.
    unpack: syn::Expr,
}

impl VTableFnParam {
    fn fn_arg(&self) -> syn::FnArg {
        let name = &self.name;
        let ty = &self.ty;
        syn::parse_quote_spanned! {self.span=>
            #name: #ty
        }
    }
}

#[derive(Debug, Default)]
struct StarlarkInternalVTableAttrs {
    skip: bool,
}

impl syn::parse::Parse for StarlarkInternalVTableAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<syn::Ident>()?;
        if ident == "skip" {
            Ok(StarlarkInternalVTableAttrs { skip: true })
        } else {
            Err(syn::Error::new(ident.span(), "unknown attribute"))
        }
    }
}

impl Gen {
    fn parse_named_field(field: TokenStream) -> syn::Result<syn::Field> {
        struct FieldParser {
            field: syn::Field,
        }

        impl syn::parse::Parse for FieldParser {
            fn parse(input: ParseStream) -> syn::Result<Self> {
                Ok(FieldParser {
                    field: syn::Field::parse_named(input)?,
                })
            }
        }

        Ok(syn::parse2::<FieldParser>(field)?.field)
    }

    fn remove_lifetime_params(ty: &syn::Type) -> syn::Type {
        struct RemoveLifetimeParams;

        impl VisitMut for RemoveLifetimeParams {
            fn visit_lifetime_mut(&mut self, i: &mut syn::Lifetime) {
                i.ident = syn::Ident::new("_", i.ident.span());
            }
        }

        let mut ty = ty.clone();
        RemoveLifetimeParams.visit_type_mut(&mut ty);
        ty
    }

    /// Parse `StarlarkValue` method parameter into vtable `fn` parameter and unpack expr.
    fn vtable_entry_param(&self, span: Span, param: &FnArg) -> syn::Result<VTableFnParam> {
        match param {
            FnArg::Receiver(_) => Ok(VTableFnParam {
                span,
                name: syn::parse_quote_spanned! {span=>
                    this
                },
                ty: syn::parse_quote_spanned! {span=>
                    crate::values::layout::vtable::StarlarkValueRawPtr
                },
                unpack: syn::parse_quote_spanned! {span=>
                    this.value_ref::<T>()
                },
            }),
            FnArg::Typed(p) => {
                let name: &Ident = match &*p.pat {
                    Pat::Ident(p) => &p.ident,
                    p => return Err(syn::Error::new(p.span(), "parameter must be identifier")),
                };
                let ty_without_lifetime = Self::remove_lifetime_params(&p.ty);
                Ok(VTableFnParam {
                    span,
                    name: name.clone(),
                    ty: (*p.ty).clone(),
                    unpack: syn::parse_quote_spanned! {span=>
                        // We do `transmute` to get rid of lifetimes, see below.
                        // We specify type parameters explicitly
                        // to fail at compile time if there's a bug in derive.
                        std::mem::transmute::<#ty_without_lifetime, #ty_without_lifetime>(#name)
                    },
                })
            }
        }
    }

    fn vtable_entry(&self, method: &TraitItemFn) -> syn::Result<VTableEntry> {
        let fn_name = &method.sig.ident;
        let fn_ret_type = &method.sig.output;

        let params: Vec<VTableFnParam> = method
            .sig
            .inputs
            .iter()
            .map(|param| self.vtable_entry_param(method.sig.span(), param))
            .collect::<syn::Result<_>>()?;

        let field_init_param_pairs: Vec<syn::FnArg> = params.iter().map(|p| p.fn_arg()).collect();
        let ret = match &method.sig.output {
            ReturnType::Default => quote! {},
            ReturnType::Type(_, ty) => {
                quote_spanned! {method.sig.span()=>
                    -> #ty
                }
            }
        };
        let field_fn_param_types: Vec<&syn::Type> = params.iter().map(|p| &p.ty).collect();
        let field = Self::parse_named_field(quote_spanned! {method.sig.span()=>
            pub(crate) #fn_name: for<'a, 'v> fn(
                #(#field_fn_param_types),*
            ) #ret
        })?;

        let field_init_args: Vec<&syn::Expr> = params.iter().map(|p| &p.unpack).collect();
        let init = syn::parse_quote_spanned! {method.sig.span()=>
            #fn_name: {
                // It is important to put vtable entry into named function
                // instead of anonymous callback so function name is meaningful in profiler output.
                fn #fn_name<'a, 'v, 'v2, T: StarlarkValue<'v2>>(#(#field_init_param_pairs),*) #fn_ret_type {
                    unsafe {
                        // The problem is that it is concrete `'v` in
                        // ```
                        // struct StarlarkValueVTableGet<'v, T: StarlarkValue<'v>
                        // ```
                        // but we must generate vtable entry `for<'v>`.
                        //
                        // If Rust supported something like:
                        // ```
                        // struct StarlarkValueVTableGet<for<'v> T<'v>: StarlarkValue<'v>>
                        // ```
                        // it would be possible. But it doesn't. Hence all the transmutes.
                        std::mem::transmute(
                            T::#fn_name(#(#field_init_args),*)
                        )
                    }
                }
                #fn_name::<T>
            }
        };
        let field_params_names: Vec<&Ident> = params.iter().map(|p| &p.name).collect();
        let init_for_black_hole = syn::parse_quote_spanned! {method.sig.span()=>
            #fn_name: |#(#field_params_names),*| {
                panic!("BlackHole")
            }
        };
        Ok(VTableEntry {
            field,
            init,
            init_for_black_hole,
        })
    }

    fn process_item_attrs(
        &self,
        attrs: &[syn::Attribute],
    ) -> syn::Result<(StarlarkInternalVTableAttrs, Vec<syn::Attribute>)> {
        let mut new_attrs = Vec::new();
        let mut item_attrs: Option<StarlarkInternalVTableAttrs> = None;
        for attr in attrs {
            if attr.path().is_ident("starlark_internal_vtable") {
                if item_attrs.is_some() {
                    return Err(syn::Error::new(attr.span(), "duplicate attribute"));
                }
                item_attrs = Some(attr.parse_args()?);
            } else {
                new_attrs.push(attr.clone());
            }
        }
        Ok((item_attrs.unwrap_or_default(), new_attrs))
    }

    fn process_starlark_value_vtable_fn(
        &self,
        m: &mut syn::TraitItemFn,
    ) -> syn::Result<Option<VTableEntry>> {
        let (item_attrs, new_attrs) = self.process_item_attrs(&m.attrs)?;
        m.attrs = new_attrs;
        if item_attrs.skip {
            return Ok(None);
        }

        Ok(Some(self.vtable_entry(m)?))
    }

    fn process_starlark_value_vtable_type(&self, item_type: &mut syn::TraitItemType) {
        if item_type.ident == "Canonical" {
            // This won't be needed once associated type defaults are stabilized.
            // https://github.com/rust-lang/rust/issues/29661
            item_type.default = None;
        }
    }

    fn gen_starlark_value_vtable(&self) -> syn::Result<TokenStream> {
        let mut fields: Vec<syn::Field> = Vec::new();
        let mut inits: Vec<syn::FieldValue> = Vec::new();
        let mut init_black_holes: Vec<syn::FieldValue> = Vec::new();
        let mut starlark_value = self.starlark_value.clone();
        let mut extra_items: Vec<syn::TraitItem> = Vec::new();
        for item in &mut starlark_value.items {
            match item {
                TraitItem::Fn(m) => {
                    if let Some(entry) = self.process_starlark_value_vtable_fn(m)? {
                        let VTableEntry {
                            field,
                            init,
                            init_for_black_hole,
                        } = entry;
                        fields.push(field);
                        inits.push(init);
                        init_black_holes.push(init_for_black_hole);
                    }

                    // Generate `HAS_foo: bool` vtable entry for each `foo` function.
                    // It is initialized with `true` if an implementation overrides the member.
                    // This is used for example, to check if trait has `invoke` member,
                    // and if it has, type is considered implementing `typing.Callable`.
                    let has_name = vtable_has_field_name(&m.sig.ident);
                    extra_items.push(syn::parse_quote_spanned! { m.sig.span() =>
                        #[doc(hidden)]
                        const #has_name: bool = false;
                    });

                    fields.push(Self::parse_named_field(quote_spanned! { m.sig.span()=>
                        pub(crate) #has_name: bool
                    })?);
                    inits.push(syn::parse_quote_spanned! { m.sig.span() =>
                        #has_name: T::#has_name
                    });
                    init_black_holes.push(syn::parse_quote_spanned! { m.sig.span() =>
                        #has_name: false
                    });
                }
                TraitItem::Type(ty) => self.process_starlark_value_vtable_type(ty),
                TraitItem::Const(_) => {}
                item => {
                    return Err(syn::Error::new_spanned(item, "unexpected item"));
                }
            }
        }
        starlark_value.items.extend(extra_items);

        Ok(quote_spanned! {
            self.starlark_value.span() =>

            #starlark_value

            #[allow(non_upper_case_globals, non_snake_case, dead_code)]
            pub(crate) struct StarlarkValueVTable {
                #(#fields),*
            }

            #[allow(clippy::all)]
            #[allow(unused_variables)]
            impl StarlarkValueVTable {
                pub(crate) const BLACK_HOLE: StarlarkValueVTable = StarlarkValueVTable {
                    #(#init_black_holes),*
                };
            }

            pub(crate) struct StarlarkValueVTableGet<'v, T: StarlarkValue<'v>>(std::marker::PhantomData<&'v T>);

            #[allow(clippy::all)]
            #[allow(unused_variables)]
            impl<'v, T: StarlarkValue<'v>> StarlarkValueVTableGet<'v, T> {
                pub(crate) const VTABLE: StarlarkValueVTable = StarlarkValueVTable {
                    #(#inits),*
                };
            }
        })
    }
}

/// Generates the `StarlarkValueVTable` and friends.
/// At the moment of writing this macro generated: P499428269,
/// output of `cargo expand -p starlark --lib values::traits`.
pub(crate) fn starlark_internal_vtable(
    _attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let starlark_value = parse_macro_input!(input as ItemTrait);

    let gen = Gen { starlark_value };
    let generated = match gen.gen_starlark_value_vtable() {
        Ok(generated) => generated,
        Err(error) => {
            return error.to_compile_error().into();
        }
    };

    proc_macro::TokenStream::from(quote! {
        #generated
    })
}
