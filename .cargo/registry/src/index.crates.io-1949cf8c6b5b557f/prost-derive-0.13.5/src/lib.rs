#![doc(html_root_url = "https://docs.rs/prost-derive/0.13.5")]
// The `quote!` macro requires deep recursion.
#![recursion_limit = "4096"]

extern crate alloc;
extern crate proc_macro;

use anyhow::{bail, Error};
use itertools::Itertools;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    punctuated::Punctuated, Data, DataEnum, DataStruct, DeriveInput, Expr, Fields, FieldsNamed,
    FieldsUnnamed, Ident, Index, Variant,
};

mod field;
use crate::field::Field;

fn try_message(input: TokenStream) -> Result<TokenStream, Error> {
    let input: DeriveInput = syn::parse2(input)?;

    let ident = input.ident;

    syn::custom_keyword!(skip_debug);
    let skip_debug = input
        .attrs
        .into_iter()
        .any(|a| a.path().is_ident("prost") && a.parse_args::<skip_debug>().is_ok());

    let variant_data = match input.data {
        Data::Struct(variant_data) => variant_data,
        Data::Enum(..) => bail!("Message can not be derived for an enum"),
        Data::Union(..) => bail!("Message can not be derived for a union"),
    };

    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let (is_struct, fields) = match variant_data {
        DataStruct {
            fields: Fields::Named(FieldsNamed { named: fields, .. }),
            ..
        } => (true, fields.into_iter().collect()),
        DataStruct {
            fields:
                Fields::Unnamed(FieldsUnnamed {
                    unnamed: fields, ..
                }),
            ..
        } => (false, fields.into_iter().collect()),
        DataStruct {
            fields: Fields::Unit,
            ..
        } => (false, Vec::new()),
    };

    let mut next_tag: u32 = 1;
    let mut fields = fields
        .into_iter()
        .enumerate()
        .flat_map(|(i, field)| {
            let field_ident = field.ident.map(|x| quote!(#x)).unwrap_or_else(|| {
                let index = Index {
                    index: i as u32,
                    span: Span::call_site(),
                };
                quote!(#index)
            });
            match Field::new(field.attrs, Some(next_tag)) {
                Ok(Some(field)) => {
                    next_tag = field.tags().iter().max().map(|t| t + 1).unwrap_or(next_tag);
                    Some(Ok((field_ident, field)))
                }
                Ok(None) => None,
                Err(err) => Some(Err(
                    err.context(format!("invalid message field {}.{}", ident, field_ident))
                )),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    // We want Debug to be in declaration order
    let unsorted_fields = fields.clone();

    // Sort the fields by tag number so that fields will be encoded in tag order.
    // TODO: This encodes oneof fields in the position of their lowest tag,
    // regardless of the currently occupied variant, is that consequential?
    // See: https://developers.google.com/protocol-buffers/docs/encoding#order
    fields.sort_by_key(|(_, field)| field.tags().into_iter().min().unwrap());
    let fields = fields;

    if let Some(duplicate_tag) = fields
        .iter()
        .flat_map(|(_, field)| field.tags())
        .duplicates()
        .next()
    {
        bail!(
            "message {} has multiple fields with tag {}",
            ident,
            duplicate_tag
        )
    };

    let encoded_len = fields
        .iter()
        .map(|(field_ident, field)| field.encoded_len(quote!(self.#field_ident)));

    let encode = fields
        .iter()
        .map(|(field_ident, field)| field.encode(quote!(self.#field_ident)));

    let merge = fields.iter().map(|(field_ident, field)| {
        let merge = field.merge(quote!(value));
        let tags = field.tags().into_iter().map(|tag| quote!(#tag));
        let tags = Itertools::intersperse(tags, quote!(|));

        quote! {
            #(#tags)* => {
                let mut value = &mut self.#field_ident;
                #merge.map_err(|mut error| {
                    error.push(STRUCT_NAME, stringify!(#field_ident));
                    error
                })
            },
        }
    });

    let struct_name = if fields.is_empty() {
        quote!()
    } else {
        quote!(
            const STRUCT_NAME: &'static str = stringify!(#ident);
        )
    };

    let clear = fields
        .iter()
        .map(|(field_ident, field)| field.clear(quote!(self.#field_ident)));

    let default = if is_struct {
        let default = fields.iter().map(|(field_ident, field)| {
            let value = field.default();
            quote!(#field_ident: #value,)
        });
        quote! {#ident {
            #(#default)*
        }}
    } else {
        let default = fields.iter().map(|(_, field)| {
            let value = field.default();
            quote!(#value,)
        });
        quote! {#ident (
            #(#default)*
        )}
    };

    let methods = fields
        .iter()
        .flat_map(|(field_ident, field)| field.methods(field_ident))
        .collect::<Vec<_>>();
    let methods = if methods.is_empty() {
        quote!()
    } else {
        quote! {
            #[allow(dead_code)]
            impl #impl_generics #ident #ty_generics #where_clause {
                #(#methods)*
            }
        }
    };

    let expanded = quote! {
        impl #impl_generics ::prost::Message for #ident #ty_generics #where_clause {
            #[allow(unused_variables)]
            fn encode_raw(&self, buf: &mut impl ::prost::bytes::BufMut) {
                #(#encode)*
            }

            #[allow(unused_variables)]
            fn merge_field(
                &mut self,
                tag: u32,
                wire_type: ::prost::encoding::wire_type::WireType,
                buf: &mut impl ::prost::bytes::Buf,
                ctx: ::prost::encoding::DecodeContext,
            ) -> ::core::result::Result<(), ::prost::DecodeError>
            {
                #struct_name
                match tag {
                    #(#merge)*
                    _ => ::prost::encoding::skip_field(wire_type, tag, buf, ctx),
                }
            }

            #[inline]
            fn encoded_len(&self) -> usize {
                0 #(+ #encoded_len)*
            }

            fn clear(&mut self) {
                #(#clear;)*
            }
        }

        impl #impl_generics ::core::default::Default for #ident #ty_generics #where_clause {
            fn default() -> Self {
                #default
            }
        }
    };
    let expanded = if skip_debug {
        expanded
    } else {
        let debugs = unsorted_fields.iter().map(|(field_ident, field)| {
            let wrapper = field.debug(quote!(self.#field_ident));
            let call = if is_struct {
                quote!(builder.field(stringify!(#field_ident), &wrapper))
            } else {
                quote!(builder.field(&wrapper))
            };
            quote! {
                 let builder = {
                     let wrapper = #wrapper;
                     #call
                 };
            }
        });
        let debug_builder = if is_struct {
            quote!(f.debug_struct(stringify!(#ident)))
        } else {
            quote!(f.debug_tuple(stringify!(#ident)))
        };
        quote! {
            #expanded

            impl #impl_generics ::core::fmt::Debug for #ident #ty_generics #where_clause {
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    let mut builder = #debug_builder;
                    #(#debugs;)*
                    builder.finish()
                }
            }
        }
    };

    let expanded = quote! {
        #expanded

        #methods
    };

    Ok(expanded)
}

#[proc_macro_derive(Message, attributes(prost))]
pub fn message(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    try_message(input.into()).unwrap().into()
}

fn try_enumeration(input: TokenStream) -> Result<TokenStream, Error> {
    let input: DeriveInput = syn::parse2(input)?;
    let ident = input.ident;

    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let punctuated_variants = match input.data {
        Data::Enum(DataEnum { variants, .. }) => variants,
        Data::Struct(_) => bail!("Enumeration can not be derived for a struct"),
        Data::Union(..) => bail!("Enumeration can not be derived for a union"),
    };

    // Map the variants into 'fields'.
    let mut variants: Vec<(Ident, Expr)> = Vec::new();
    for Variant {
        ident,
        fields,
        discriminant,
        ..
    } in punctuated_variants
    {
        match fields {
            Fields::Unit => (),
            Fields::Named(_) | Fields::Unnamed(_) => {
                bail!("Enumeration variants may not have fields")
            }
        }

        match discriminant {
            Some((_, expr)) => variants.push((ident, expr)),
            None => bail!("Enumeration variants must have a discriminant"),
        }
    }

    if variants.is_empty() {
        panic!("Enumeration must have at least one variant");
    }

    let default = variants[0].0.clone();

    let is_valid = variants.iter().map(|(_, value)| quote!(#value => true));
    let from = variants
        .iter()
        .map(|(variant, value)| quote!(#value => ::core::option::Option::Some(#ident::#variant)));

    let try_from = variants
        .iter()
        .map(|(variant, value)| quote!(#value => ::core::result::Result::Ok(#ident::#variant)));

    let is_valid_doc = format!("Returns `true` if `value` is a variant of `{}`.", ident);
    let from_i32_doc = format!(
        "Converts an `i32` to a `{}`, or `None` if `value` is not a valid variant.",
        ident
    );

    let expanded = quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            #[doc=#is_valid_doc]
            pub fn is_valid(value: i32) -> bool {
                match value {
                    #(#is_valid,)*
                    _ => false,
                }
            }

            #[deprecated = "Use the TryFrom<i32> implementation instead"]
            #[doc=#from_i32_doc]
            pub fn from_i32(value: i32) -> ::core::option::Option<#ident> {
                match value {
                    #(#from,)*
                    _ => ::core::option::Option::None,
                }
            }
        }

        impl #impl_generics ::core::default::Default for #ident #ty_generics #where_clause {
            fn default() -> #ident {
                #ident::#default
            }
        }

        impl #impl_generics ::core::convert::From::<#ident> for i32 #ty_generics #where_clause {
            fn from(value: #ident) -> i32 {
                value as i32
            }
        }

        impl #impl_generics ::core::convert::TryFrom::<i32> for #ident #ty_generics #where_clause {
            type Error = ::prost::UnknownEnumValue;

            fn try_from(value: i32) -> ::core::result::Result<#ident, ::prost::UnknownEnumValue> {
                match value {
                    #(#try_from,)*
                    _ => ::core::result::Result::Err(::prost::UnknownEnumValue(value)),
                }
            }
        }
    };

    Ok(expanded)
}

#[proc_macro_derive(Enumeration, attributes(prost))]
pub fn enumeration(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    try_enumeration(input.into()).unwrap().into()
}

fn try_oneof(input: TokenStream) -> Result<TokenStream, Error> {
    let input: DeriveInput = syn::parse2(input)?;

    let ident = input.ident;

    syn::custom_keyword!(skip_debug);
    let skip_debug = input
        .attrs
        .into_iter()
        .any(|a| a.path().is_ident("prost") && a.parse_args::<skip_debug>().is_ok());

    let variants = match input.data {
        Data::Enum(DataEnum { variants, .. }) => variants,
        Data::Struct(..) => bail!("Oneof can not be derived for a struct"),
        Data::Union(..) => bail!("Oneof can not be derived for a union"),
    };

    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Map the variants into 'fields'.
    let mut fields: Vec<(Ident, Field)> = Vec::new();
    for Variant {
        attrs,
        ident: variant_ident,
        fields: variant_fields,
        ..
    } in variants
    {
        let variant_fields = match variant_fields {
            Fields::Unit => Punctuated::new(),
            Fields::Named(FieldsNamed { named: fields, .. })
            | Fields::Unnamed(FieldsUnnamed {
                unnamed: fields, ..
            }) => fields,
        };
        if variant_fields.len() != 1 {
            bail!("Oneof enum variants must have a single field");
        }
        match Field::new_oneof(attrs)? {
            Some(field) => fields.push((variant_ident, field)),
            None => bail!("invalid oneof variant: oneof variants may not be ignored"),
        }
    }

    // Oneof variants cannot be oneofs themselves, so it's impossible to have a field with multiple
    // tags.
    assert!(fields.iter().all(|(_, field)| field.tags().len() == 1));

    if let Some(duplicate_tag) = fields
        .iter()
        .flat_map(|(_, field)| field.tags())
        .duplicates()
        .next()
    {
        bail!(
            "invalid oneof {}: multiple variants have tag {}",
            ident,
            duplicate_tag
        );
    }

    let encode = fields.iter().map(|(variant_ident, field)| {
        let encode = field.encode(quote!(*value));
        quote!(#ident::#variant_ident(ref value) => { #encode })
    });

    let merge = fields.iter().map(|(variant_ident, field)| {
        let tag = field.tags()[0];
        let merge = field.merge(quote!(value));
        quote! {
            #tag => {
                match field {
                    ::core::option::Option::Some(#ident::#variant_ident(ref mut value)) => {
                        #merge
                    },
                    _ => {
                        let mut owned_value = ::core::default::Default::default();
                        let value = &mut owned_value;
                        #merge.map(|_| *field = ::core::option::Option::Some(#ident::#variant_ident(owned_value)))
                    },
                }
            }
        }
    });

    let encoded_len = fields.iter().map(|(variant_ident, field)| {
        let encoded_len = field.encoded_len(quote!(*value));
        quote!(#ident::#variant_ident(ref value) => #encoded_len)
    });

    let expanded = quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            /// Encodes the message to a buffer.
            pub fn encode(&self, buf: &mut impl ::prost::bytes::BufMut) {
                match *self {
                    #(#encode,)*
                }
            }

            /// Decodes an instance of the message from a buffer, and merges it into self.
            pub fn merge(
                field: &mut ::core::option::Option<#ident #ty_generics>,
                tag: u32,
                wire_type: ::prost::encoding::wire_type::WireType,
                buf: &mut impl ::prost::bytes::Buf,
                ctx: ::prost::encoding::DecodeContext,
            ) -> ::core::result::Result<(), ::prost::DecodeError>
            {
                match tag {
                    #(#merge,)*
                    _ => unreachable!(concat!("invalid ", stringify!(#ident), " tag: {}"), tag),
                }
            }

            /// Returns the encoded length of the message without a length delimiter.
            #[inline]
            pub fn encoded_len(&self) -> usize {
                match *self {
                    #(#encoded_len,)*
                }
            }
        }

    };
    let expanded = if skip_debug {
        expanded
    } else {
        let debug = fields.iter().map(|(variant_ident, field)| {
            let wrapper = field.debug(quote!(*value));
            quote!(#ident::#variant_ident(ref value) => {
                let wrapper = #wrapper;
                f.debug_tuple(stringify!(#variant_ident))
                    .field(&wrapper)
                    .finish()
            })
        });
        quote! {
            #expanded

            impl #impl_generics ::core::fmt::Debug for #ident #ty_generics #where_clause {
                fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                    match *self {
                        #(#debug,)*
                    }
                }
            }
        }
    };

    Ok(expanded)
}

#[proc_macro_derive(Oneof, attributes(prost))]
pub fn oneof(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    try_oneof(input.into()).unwrap().into()
}

#[cfg(test)]
mod test {
    use crate::{try_message, try_oneof};
    use quote::quote;

    #[test]
    fn test_rejects_colliding_message_fields() {
        let output = try_message(quote!(
            struct Invalid {
                #[prost(bool, tag = "1")]
                a: bool,
                #[prost(oneof = "super::Whatever", tags = "4, 5, 1")]
                b: Option<super::Whatever>,
            }
        ));
        assert_eq!(
            output
                .expect_err("did not reject colliding message fields")
                .to_string(),
            "message Invalid has multiple fields with tag 1"
        );
    }

    #[test]
    fn test_rejects_colliding_oneof_variants() {
        let output = try_oneof(quote!(
            pub enum Invalid {
                #[prost(bool, tag = "1")]
                A(bool),
                #[prost(bool, tag = "3")]
                B(bool),
                #[prost(bool, tag = "1")]
                C(bool),
            }
        ));
        assert_eq!(
            output
                .expect_err("did not reject colliding oneof variants")
                .to_string(),
            "invalid oneof Invalid: multiple variants have tag 1"
        );
    }

    #[test]
    fn test_rejects_multiple_tags_oneof_variant() {
        let output = try_oneof(quote!(
            enum What {
                #[prost(bool, tag = "1", tag = "2")]
                A(bool),
            }
        ));
        assert_eq!(
            output
                .expect_err("did not reject multiple tags on oneof variant")
                .to_string(),
            "duplicate tag attributes: 1 and 2"
        );

        let output = try_oneof(quote!(
            enum What {
                #[prost(bool, tag = "3")]
                #[prost(tag = "4")]
                A(bool),
            }
        ));
        assert!(output.is_err());
        assert_eq!(
            output
                .expect_err("did not reject multiple tags on oneof variant")
                .to_string(),
            "duplicate tag attributes: 3 and 4"
        );

        let output = try_oneof(quote!(
            enum What {
                #[prost(bool, tags = "5,6")]
                A(bool),
            }
        ));
        assert!(output.is_err());
        assert_eq!(
            output
                .expect_err("did not reject multiple tags on oneof variant")
                .to_string(),
            "unknown attribute(s): #[prost(tags = \"5,6\")]"
        );
    }
}
