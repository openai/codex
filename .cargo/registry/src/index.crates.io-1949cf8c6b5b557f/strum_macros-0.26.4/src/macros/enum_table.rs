use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{spanned::Spanned, Data, DeriveInput, Fields};

use crate::helpers::{non_enum_error, snakify, HasStrumVariantProperties};

pub fn enum_table_inner(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let gen = &ast.generics;
    let vis = &ast.vis;
    let mut doc_comment = format!("A map over the variants of `{}`", name);

    if gen.lifetimes().count() > 0 {
        return Err(syn::Error::new(
            Span::call_site(),
            "`EnumTable` doesn't support enums with lifetimes.",
        ));
    }

    let variants = match &ast.data {
        Data::Enum(v) => &v.variants,
        _ => return Err(non_enum_error()),
    };

    let table_name = format_ident!("{}Table", name);

    // the identifiers of each variant, in PascalCase
    let mut pascal_idents = Vec::new();
    // the identifiers of each struct field, in snake_case
    let mut snake_idents = Vec::new();
    // match arms in the form `MyEnumTable::Variant => &self.variant,`
    let mut get_matches = Vec::new();
    // match arms in the form `MyEnumTable::Variant => &mut self.variant,`
    let mut get_matches_mut = Vec::new();
    // match arms in the form `MyEnumTable::Variant => self.variant = new_value`
    let mut set_matches = Vec::new();
    // struct fields of the form `variant: func(MyEnum::Variant),*
    let mut closure_fields = Vec::new();
    // struct fields of the form `variant: func(MyEnum::Variant, self.variant),`
    let mut transform_fields = Vec::new();

    // identifiers for disabled variants
    let mut disabled_variants = Vec::new();
    // match arms for disabled variants
    let mut disabled_matches = Vec::new();

    for variant in variants {
        // skip disabled variants
        if variant.get_variant_properties()?.disabled.is_some() {
            let disabled_ident = &variant.ident;
            let panic_message = format!(
                "Can't use `{}` with `{}` - variant is disabled for Strum features",
                disabled_ident, table_name
            );
            disabled_variants.push(disabled_ident);
            disabled_matches.push(quote!(#name::#disabled_ident => panic!(#panic_message),));
            continue;
        }

        // Error on variants with data
        if variant.fields != Fields::Unit {
            return Err(syn::Error::new(
                variant.fields.span(),
                "`EnumTable` doesn't support enums with non-unit variants",
            ));
        };

        let pascal_case = &variant.ident;
        let snake_case = format_ident!("_{}", snakify(&pascal_case.to_string()));

        get_matches.push(quote! {#name::#pascal_case => &self.#snake_case,});
        get_matches_mut.push(quote! {#name::#pascal_case => &mut self.#snake_case,});
        set_matches.push(quote! {#name::#pascal_case => self.#snake_case = new_value,});
        closure_fields.push(quote! {#snake_case: func(#name::#pascal_case),});
        transform_fields.push(quote! {#snake_case: func(#name::#pascal_case, &self.#snake_case),});
        pascal_idents.push(pascal_case);
        snake_idents.push(snake_case);
    }

    // Error on empty enums
    if pascal_idents.is_empty() {
        return Err(syn::Error::new(
            variants.span(),
            "`EnumTable` requires at least one non-disabled variant",
        ));
    }

    // if the index operation can panic, add that to the documentation
    if !disabled_variants.is_empty() {
        doc_comment.push_str(&format!(
            "\n# Panics\nIndexing `{}` with any of the following variants will cause a panic:",
            table_name
        ));
        for variant in disabled_variants {
            doc_comment.push_str(&format!("\n\n- `{}::{}`", name, variant));
        }
    }

    let doc_new = format!(
        "Create a new {} with a value for each variant of {}",
        table_name, name
    );
    let doc_closure = format!(
        "Create a new {} by running a function on each variant of `{}`",
        table_name, name
    );
    let doc_transform = format!("Create a new `{}` by running a function on each variant of `{}` and the corresponding value in the current `{0}`", table_name, name);
    let doc_filled = format!(
        "Create a new `{}` with the same value in each field.",
        table_name
    );
    let doc_option_all = format!("Converts `{}<Option<T>>` into `Option<{0}<T>>`. Returns `Some` if all fields are `Some`, otherwise returns `None`.", table_name);
    let doc_result_all_ok = format!("Converts `{}<Result<T, E>>` into `Result<{0}<T>, E>`. Returns `Ok` if all fields are `Ok`, otherwise returns `Err`.", table_name);

    Ok(quote! {
        #[doc = #doc_comment]
        #[allow(
            missing_copy_implementations,
        )]
        #[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
        #vis struct #table_name<T> {
            #(#snake_idents: T,)*
        }

        impl<T: Clone> #table_name<T> {
            #[doc = #doc_filled]
            #vis fn filled(value: T) -> #table_name<T> {
                #table_name {
                    #(#snake_idents: value.clone(),)*
                }
            }
        }

        impl<T> #table_name<T> {
            #[doc = #doc_new]
            #vis fn new(
                #(#snake_idents: T,)*
            ) -> #table_name<T> {
                #table_name {
                    #(#snake_idents,)*
                }
            }

            #[doc = #doc_closure]
            #vis fn from_closure<F: Fn(#name)->T>(func: F) -> #table_name<T> {
              #table_name {
                #(#closure_fields)*
              }
            }

            #[doc = #doc_transform]
            #vis fn transform<U, F: Fn(#name, &T)->U>(&self, func: F) -> #table_name<U> {
              #table_name {
                #(#transform_fields)*
              }
            }

        }

        impl<T> ::core::ops::Index<#name> for #table_name<T> {
            type Output = T;

            fn index(&self, idx: #name) -> &T {
                match idx {
                    #(#get_matches)*
                    #(#disabled_matches)*
                }
            }
        }

        impl<T> ::core::ops::IndexMut<#name> for #table_name<T> {
            fn index_mut(&mut self, idx: #name) -> &mut T {
                match idx {
                    #(#get_matches_mut)*
                    #(#disabled_matches)*
                }
            }
        }

        impl<T> #table_name<::core::option::Option<T>> {
            #[doc = #doc_option_all]
            #vis fn all(self) -> ::core::option::Option<#table_name<T>> {
                if let #table_name {
                    #(#snake_idents: ::core::option::Option::Some(#snake_idents),)*
                } = self {
                    ::core::option::Option::Some(#table_name {
                        #(#snake_idents,)*
                    })
                } else {
                    ::core::option::Option::None
                }
            }
        }

        impl<T, E> #table_name<::core::result::Result<T, E>> {
            #[doc = #doc_result_all_ok]
            #vis fn all_ok(self) -> ::core::result::Result<#table_name<T>, E> {
                ::core::result::Result::Ok(#table_name {
                    #(#snake_idents: self.#snake_idents?,)*
                })
            }
        }
    })
}
