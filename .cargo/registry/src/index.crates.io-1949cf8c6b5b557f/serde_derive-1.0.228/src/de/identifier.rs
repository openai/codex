//! Deserialization of struct field identifiers and enum variant identifiers by
//! way of a Rust enum.

use crate::de::{FieldWithAliases, Parameters};
use crate::fragment::{Fragment, Stmts};
use crate::internals::ast::{Style, Variant};
use crate::internals::attr;
use crate::private;
use proc_macro2::{Literal, TokenStream};
use quote::{quote, ToTokens};

// Generates `Deserialize::deserialize` body for an enum with
// `serde(field_identifier)` or `serde(variant_identifier)` attribute.
pub(super) fn deserialize_custom(
    params: &Parameters,
    variants: &[Variant],
    cattrs: &attr::Container,
) -> Fragment {
    let is_variant = match cattrs.identifier() {
        attr::Identifier::Variant => true,
        attr::Identifier::Field => false,
        attr::Identifier::No => unreachable!(),
    };

    let this_type = params.this_type.to_token_stream();
    let this_value = params.this_value.to_token_stream();

    let (ordinary, fallthrough, fallthrough_borrowed) = if let Some(last) = variants.last() {
        let last_ident = &last.ident;
        if last.attrs.other() {
            // Process `serde(other)` attribute. It would always be found on the
            // last variant (checked in `check_identifier`), so all preceding
            // are ordinary variants.
            let ordinary = &variants[..variants.len() - 1];
            let fallthrough = quote!(_serde::#private::Ok(#this_value::#last_ident));
            (ordinary, Some(fallthrough), None)
        } else if let Style::Newtype = last.style {
            let ordinary = &variants[..variants.len() - 1];
            let fallthrough = |value| {
                quote! {
                    _serde::#private::Result::map(
                        _serde::Deserialize::deserialize(
                            _serde::#private::de::IdentifierDeserializer::from(#value)
                        ),
                        #this_value::#last_ident)
                }
            };
            (
                ordinary,
                Some(fallthrough(quote!(__value))),
                Some(fallthrough(quote!(_serde::#private::de::Borrowed(
                    __value
                )))),
            )
        } else {
            (variants, None, None)
        }
    } else {
        (variants, None, None)
    };

    let idents_aliases: Vec<_> = ordinary
        .iter()
        .map(|variant| FieldWithAliases {
            ident: variant.ident.clone(),
            aliases: variant.attrs.aliases(),
        })
        .collect();

    let names = idents_aliases.iter().flat_map(|variant| variant.aliases);

    let names_const = if fallthrough.is_some() {
        None
    } else if is_variant {
        let variants = quote! {
            #[doc(hidden)]
            const VARIANTS: &'static [&'static str] = &[ #(#names),* ];
        };
        Some(variants)
    } else {
        let fields = quote! {
            #[doc(hidden)]
            const FIELDS: &'static [&'static str] = &[ #(#names),* ];
        };
        Some(fields)
    };

    let (de_impl_generics, de_ty_generics, ty_generics, where_clause) =
        params.generics_with_de_lifetime();
    let delife = params.borrowed.de_lifetime();
    let visitor_impl = Stmts(deserialize_identifier(
        &this_value,
        &idents_aliases,
        is_variant,
        fallthrough,
        fallthrough_borrowed,
        false,
        cattrs.expecting(),
    ));

    quote_block! {
        #names_const

        #[doc(hidden)]
        struct __FieldVisitor #de_impl_generics #where_clause {
            marker: _serde::#private::PhantomData<#this_type #ty_generics>,
            lifetime: _serde::#private::PhantomData<&#delife ()>,
        }

        #[automatically_derived]
        impl #de_impl_generics _serde::de::Visitor<#delife> for __FieldVisitor #de_ty_generics #where_clause {
            type Value = #this_type #ty_generics;

            #visitor_impl
        }

        let __visitor = __FieldVisitor {
            marker: _serde::#private::PhantomData::<#this_type #ty_generics>,
            lifetime: _serde::#private::PhantomData,
        };
        _serde::Deserializer::deserialize_identifier(__deserializer, __visitor)
    }
}

pub(super) fn deserialize_generated(
    deserialized_fields: &[FieldWithAliases],
    has_flatten: bool,
    is_variant: bool,
    ignore_variant: Option<TokenStream>,
    fallthrough: Option<TokenStream>,
) -> Fragment {
    let this_value = quote!(__Field);
    let field_idents: &Vec<_> = &deserialized_fields
        .iter()
        .map(|field| &field.ident)
        .collect();

    let visitor_impl = Stmts(deserialize_identifier(
        &this_value,
        deserialized_fields,
        is_variant,
        fallthrough,
        None,
        !is_variant && has_flatten,
        None,
    ));

    let lifetime = if !is_variant && has_flatten {
        Some(quote!(<'de>))
    } else {
        None
    };

    quote_block! {
        #[allow(non_camel_case_types)]
        #[doc(hidden)]
        enum __Field #lifetime {
            #(#field_idents,)*
            #ignore_variant
        }

        #[doc(hidden)]
        struct __FieldVisitor;

        #[automatically_derived]
        impl<'de> _serde::de::Visitor<'de> for __FieldVisitor {
            type Value = __Field #lifetime;

            #visitor_impl
        }

        #[automatically_derived]
        impl<'de> _serde::Deserialize<'de> for __Field #lifetime {
            #[inline]
            fn deserialize<__D>(__deserializer: __D) -> _serde::#private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<'de>,
            {
                _serde::Deserializer::deserialize_identifier(__deserializer, __FieldVisitor)
            }
        }
    }
}

fn deserialize_identifier(
    this_value: &TokenStream,
    deserialized_fields: &[FieldWithAliases],
    is_variant: bool,
    fallthrough: Option<TokenStream>,
    fallthrough_borrowed: Option<TokenStream>,
    collect_other_fields: bool,
    expecting: Option<&str>,
) -> Fragment {
    let str_mapping = deserialized_fields.iter().map(|field| {
        let ident = &field.ident;
        let aliases = field.aliases;
        let private2 = private;
        // `aliases` also contains a main name
        quote! {
            #(
                #aliases => _serde::#private2::Ok(#this_value::#ident),
            )*
        }
    });
    let bytes_mapping = deserialized_fields.iter().map(|field| {
        let ident = &field.ident;
        // `aliases` also contains a main name
        let aliases = field
            .aliases
            .iter()
            .map(|alias| Literal::byte_string(alias.value.as_bytes()));
        let private2 = private;
        quote! {
            #(
                #aliases => _serde::#private2::Ok(#this_value::#ident),
            )*
        }
    });

    let expecting = expecting.unwrap_or(if is_variant {
        "variant identifier"
    } else {
        "field identifier"
    });

    let bytes_to_str = if fallthrough.is_some() || collect_other_fields {
        None
    } else {
        Some(quote! {
            let __value = &_serde::#private::from_utf8_lossy(__value);
        })
    };

    let (
        value_as_str_content,
        value_as_borrowed_str_content,
        value_as_bytes_content,
        value_as_borrowed_bytes_content,
    ) = if collect_other_fields {
        (
            Some(quote! {
                let __value = _serde::#private::de::Content::String(_serde::#private::ToString::to_string(__value));
            }),
            Some(quote! {
                let __value = _serde::#private::de::Content::Str(__value);
            }),
            Some(quote! {
                let __value = _serde::#private::de::Content::ByteBuf(__value.to_vec());
            }),
            Some(quote! {
                let __value = _serde::#private::de::Content::Bytes(__value);
            }),
        )
    } else {
        (None, None, None, None)
    };

    let fallthrough_arm_tokens;
    let fallthrough_arm = if let Some(fallthrough) = &fallthrough {
        fallthrough
    } else if is_variant {
        fallthrough_arm_tokens = quote! {
            _serde::#private::Err(_serde::de::Error::unknown_variant(__value, VARIANTS))
        };
        &fallthrough_arm_tokens
    } else {
        fallthrough_arm_tokens = quote! {
            _serde::#private::Err(_serde::de::Error::unknown_field(__value, FIELDS))
        };
        &fallthrough_arm_tokens
    };

    let visit_other = if collect_other_fields {
        quote! {
            fn visit_bool<__E>(self, __value: bool) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                _serde::#private::Ok(__Field::__other(_serde::#private::de::Content::Bool(__value)))
            }

            fn visit_i8<__E>(self, __value: i8) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                _serde::#private::Ok(__Field::__other(_serde::#private::de::Content::I8(__value)))
            }

            fn visit_i16<__E>(self, __value: i16) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                _serde::#private::Ok(__Field::__other(_serde::#private::de::Content::I16(__value)))
            }

            fn visit_i32<__E>(self, __value: i32) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                _serde::#private::Ok(__Field::__other(_serde::#private::de::Content::I32(__value)))
            }

            fn visit_i64<__E>(self, __value: i64) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                _serde::#private::Ok(__Field::__other(_serde::#private::de::Content::I64(__value)))
            }

            fn visit_u8<__E>(self, __value: u8) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                _serde::#private::Ok(__Field::__other(_serde::#private::de::Content::U8(__value)))
            }

            fn visit_u16<__E>(self, __value: u16) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                _serde::#private::Ok(__Field::__other(_serde::#private::de::Content::U16(__value)))
            }

            fn visit_u32<__E>(self, __value: u32) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                _serde::#private::Ok(__Field::__other(_serde::#private::de::Content::U32(__value)))
            }

            fn visit_u64<__E>(self, __value: u64) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                _serde::#private::Ok(__Field::__other(_serde::#private::de::Content::U64(__value)))
            }

            fn visit_f32<__E>(self, __value: f32) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                _serde::#private::Ok(__Field::__other(_serde::#private::de::Content::F32(__value)))
            }

            fn visit_f64<__E>(self, __value: f64) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                _serde::#private::Ok(__Field::__other(_serde::#private::de::Content::F64(__value)))
            }

            fn visit_char<__E>(self, __value: char) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                _serde::#private::Ok(__Field::__other(_serde::#private::de::Content::Char(__value)))
            }

            fn visit_unit<__E>(self) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                _serde::#private::Ok(__Field::__other(_serde::#private::de::Content::Unit))
            }
        }
    } else {
        let u64_mapping = deserialized_fields.iter().enumerate().map(|(i, field)| {
            let i = i as u64;
            let ident = &field.ident;
            quote!(#i => _serde::#private::Ok(#this_value::#ident))
        });

        let u64_fallthrough_arm_tokens;
        let u64_fallthrough_arm = if let Some(fallthrough) = &fallthrough {
            fallthrough
        } else {
            let index_expecting = if is_variant { "variant" } else { "field" };
            let fallthrough_msg = format!(
                "{} index 0 <= i < {}",
                index_expecting,
                deserialized_fields.len(),
            );
            u64_fallthrough_arm_tokens = quote! {
                _serde::#private::Err(_serde::de::Error::invalid_value(
                    _serde::de::Unexpected::Unsigned(__value),
                    &#fallthrough_msg,
                ))
            };
            &u64_fallthrough_arm_tokens
        };

        quote! {
            fn visit_u64<__E>(self, __value: u64) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                match __value {
                    #(#u64_mapping,)*
                    _ => #u64_fallthrough_arm,
                }
            }
        }
    };

    let visit_borrowed = if fallthrough_borrowed.is_some() || collect_other_fields {
        let str_mapping = str_mapping.clone();
        let bytes_mapping = bytes_mapping.clone();
        let fallthrough_borrowed_arm = fallthrough_borrowed.as_ref().unwrap_or(fallthrough_arm);
        Some(quote! {
            fn visit_borrowed_str<__E>(self, __value: &'de str) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                match __value {
                    #(#str_mapping)*
                    _ => {
                        #value_as_borrowed_str_content
                        #fallthrough_borrowed_arm
                    }
                }
            }

            fn visit_borrowed_bytes<__E>(self, __value: &'de [u8]) -> _serde::#private::Result<Self::Value, __E>
            where
                __E: _serde::de::Error,
            {
                match __value {
                    #(#bytes_mapping)*
                    _ => {
                        #bytes_to_str
                        #value_as_borrowed_bytes_content
                        #fallthrough_borrowed_arm
                    }
                }
            }
        })
    } else {
        None
    };

    quote_block! {
        fn expecting(&self, __formatter: &mut _serde::#private::Formatter) -> _serde::#private::fmt::Result {
            _serde::#private::Formatter::write_str(__formatter, #expecting)
        }

        #visit_other

        fn visit_str<__E>(self, __value: &str) -> _serde::#private::Result<Self::Value, __E>
        where
            __E: _serde::de::Error,
        {
            match __value {
                #(#str_mapping)*
                _ => {
                    #value_as_str_content
                    #fallthrough_arm
                }
            }
        }

        fn visit_bytes<__E>(self, __value: &[u8]) -> _serde::#private::Result<Self::Value, __E>
        where
            __E: _serde::de::Error,
        {
            match __value {
                #(#bytes_mapping)*
                _ => {
                    #bytes_to_str
                    #value_as_bytes_content
                    #fallthrough_arm
                }
            }
        }

        #visit_borrowed
    }
}
