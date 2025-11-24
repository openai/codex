#![forbid(unsafe_code)]

#[macro_use]
extern crate quote;
#[macro_use]
extern crate syn;
extern crate proc_macro;

mod ast;
mod attr;
mod metadata;
mod regex_syntax;
mod schema_exprs;

use ast::*;
use proc_macro2::TokenStream;
use syn::spanned::Spanned;

#[proc_macro_derive(JsonSchema, attributes(schemars, serde, validate))]
pub fn derive_json_schema_wrapper(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    derive_json_schema(input, false)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(JsonSchema_repr, attributes(schemars, serde))]
pub fn derive_json_schema_repr_wrapper(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    derive_json_schema(input, true)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn derive_json_schema(mut input: syn::DeriveInput, repr: bool) -> syn::Result<TokenStream> {
    attr::process_serde_attrs(&mut input)?;

    let mut cont = Container::from_ast(&input)?;
    add_trait_bounds(&mut cont);

    let crate_alias = cont.attrs.crate_name.as_ref().map(|path| {
        quote_spanned! {path.span()=>
            use #path as schemars;
        }
    });

    let type_name = &cont.ident;
    let (impl_generics, ty_generics, where_clause) = cont.generics.split_for_impl();

    if let Some(transparent_field) = cont.transparent_field() {
        let (ty, type_def) = schema_exprs::type_for_field_schema(transparent_field);
        return Ok(quote! {
            const _: () = {
                #crate_alias
                #type_def

                #[automatically_derived]
                impl #impl_generics schemars::JsonSchema for #type_name #ty_generics #where_clause {
                    fn is_referenceable() -> bool {
                        <#ty as schemars::JsonSchema>::is_referenceable()
                    }

                    fn schema_name() -> std::string::String {
                        <#ty as schemars::JsonSchema>::schema_name()
                    }

                    fn schema_id() -> std::borrow::Cow<'static, str> {
                        <#ty as schemars::JsonSchema>::schema_id()
                    }

                    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
                        <#ty as schemars::JsonSchema>::json_schema(generator)
                    }

                    fn _schemars_private_non_optional_json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
                        <#ty as schemars::JsonSchema>::_schemars_private_non_optional_json_schema(generator)
                    }

                    fn _schemars_private_is_option() -> bool {
                        <#ty as schemars::JsonSchema>::_schemars_private_is_option()
                    }
                };
            };
        });
    }

    let mut schema_base_name = cont.name().to_string();

    if !cont.attrs.is_renamed {
        if let Some(path) = cont.serde_attrs.remote() {
            if let Some(segment) = path.segments.last() {
                schema_base_name = segment.ident.to_string();
            }
        }
    }

    // FIXME improve handling of generic type params which may not implement JsonSchema
    let type_params: Vec<_> = cont.generics.type_params().map(|ty| &ty.ident).collect();
    let const_params: Vec<_> = cont.generics.const_params().map(|c| &c.ident).collect();
    let params: Vec<_> = type_params.iter().chain(const_params.iter()).collect();

    let (schema_name, schema_id) = if params.is_empty()
        || (cont.attrs.is_renamed && !schema_base_name.contains('{'))
    {
        (
            quote! {
                #schema_base_name.to_owned()
            },
            quote! {
                std::borrow::Cow::Borrowed(std::concat!(
                    std::module_path!(),
                    "::",
                    #schema_base_name
                ))
            },
        )
    } else if cont.attrs.is_renamed {
        let mut schema_name_fmt = schema_base_name;
        for tp in &params {
            schema_name_fmt.push_str(&format!("{{{}:.0}}", tp));
        }
        (
            quote! {
                format!(#schema_name_fmt #(,#type_params=#type_params::schema_name())* #(,#const_params=#const_params)*)
            },
            quote! {
                std::borrow::Cow::Owned(
                    format!(
                        std::concat!(
                            std::module_path!(),
                            "::",
                            #schema_name_fmt
                        )
                        #(,#type_params=#type_params::schema_id())*
                        #(,#const_params=#const_params)*
                    )
                )
            },
        )
    } else {
        let mut schema_name_fmt = schema_base_name;
        schema_name_fmt.push_str("_for_{}");
        schema_name_fmt.push_str(&"_and_{}".repeat(params.len() - 1));
        (
            quote! {
                format!(#schema_name_fmt #(,#type_params::schema_name())* #(,#const_params)*)
            },
            quote! {
                std::borrow::Cow::Owned(
                    format!(
                        std::concat!(
                            std::module_path!(),
                            "::",
                            #schema_name_fmt
                        )
                        #(,#type_params::schema_id())*
                        #(,#const_params)*
                    )
                )
            },
        )
    };

    let schema_expr = if repr {
        schema_exprs::expr_for_repr(&cont)?
    } else {
        schema_exprs::expr_for_container(&cont)
    };

    Ok(quote! {
        const _: () = {
            #crate_alias

            #[automatically_derived]
            #[allow(unused_braces)]
            impl #impl_generics schemars::JsonSchema for #type_name #ty_generics #where_clause {
                fn schema_name() -> std::string::String {
                    #schema_name
                }

                fn schema_id() -> std::borrow::Cow<'static, str> {
                    #schema_id
                }

                fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
                    #schema_expr
                }
            };
        };
    })
}

fn add_trait_bounds(cont: &mut Container) {
    if let Some(bounds) = cont.serde_attrs.ser_bound() {
        let where_clause = cont.generics.make_where_clause();
        where_clause.predicates.extend(bounds.iter().cloned());
    } else {
        // No explicit trait bounds specified, assume the Rust convention of adding the trait to each type parameter
        // TODO consider also adding trait bound to associated types when used as fields - I think Serde does this?
        for param in &mut cont.generics.params {
            if let syn::GenericParam::Type(ref mut type_param) = *param {
                type_param.bounds.push(parse_quote!(schemars::JsonSchema));
            }
        }
    }
}
