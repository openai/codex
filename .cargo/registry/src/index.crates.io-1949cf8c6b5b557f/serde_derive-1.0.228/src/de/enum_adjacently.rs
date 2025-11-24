//! Deserialization for adjacently tagged enums:
//!
//! ```ignore
//! #[serde(tag = "...", content = "...")]
//! enum Enum {}
//! ```

use crate::de::enum_;
use crate::de::enum_untagged;
use crate::de::{field_i, Parameters};
use crate::fragment::{Fragment, Match};
use crate::internals::ast::{Style, Variant};
use crate::internals::attr;
use crate::private;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

/// Generates `Deserialize::deserialize` body for an `enum Enum {...}` with `#[serde(tag, content)]` attributes
pub(super) fn deserialize(
    params: &Parameters,
    variants: &[Variant],
    cattrs: &attr::Container,
    tag: &str,
    content: &str,
) -> Fragment {
    let this_type = &params.this_type;
    let this_value = &params.this_value;
    let (de_impl_generics, de_ty_generics, ty_generics, where_clause) =
        params.generics_with_de_lifetime();
    let delife = params.borrowed.de_lifetime();

    let (variants_stmt, variant_visitor) = enum_::prepare_enum_variant_enum(variants);

    let variant_arms: &Vec<_> = &variants
        .iter()
        .enumerate()
        .filter(|&(_, variant)| !variant.attrs.skip_deserializing())
        .map(|(i, variant)| {
            let variant_index = field_i(i);

            let block = Match(enum_untagged::deserialize_variant(params, variant, cattrs));

            quote! {
                __Field::#variant_index => #block
            }
        })
        .collect();

    let rust_name = params.type_name();
    let expecting = format!("adjacently tagged enum {}", rust_name);
    let expecting = cattrs.expecting().unwrap_or(&expecting);
    let type_name = cattrs.name().deserialize_name();
    let deny_unknown_fields = cattrs.deny_unknown_fields();

    // If unknown fields are allowed, we pick the visitor that can step over
    // those. Otherwise we pick the visitor that fails on unknown keys.
    let field_visitor_ty = if deny_unknown_fields {
        quote! { _serde::#private::de::TagOrContentFieldVisitor }
    } else {
        quote! { _serde::#private::de::TagContentOtherFieldVisitor }
    };

    let mut missing_content = quote! {
        _serde::#private::Err(<__A::Error as _serde::de::Error>::missing_field(#content))
    };
    let mut missing_content_fallthrough = quote!();
    let missing_content_arms = variants
        .iter()
        .enumerate()
        .filter(|&(_, variant)| !variant.attrs.skip_deserializing())
        .filter_map(|(i, variant)| {
            let variant_index = field_i(i);
            let variant_ident = &variant.ident;

            let arm = match variant.style {
                Style::Unit => quote! {
                    _serde::#private::Ok(#this_value::#variant_ident)
                },
                Style::Newtype if variant.attrs.deserialize_with().is_none() => {
                    let span = variant.original.span();
                    let func = quote_spanned!(span=> _serde::#private::de::missing_field);
                    quote! {
                        #func(#content).map(#this_value::#variant_ident)
                    }
                }
                _ => {
                    missing_content_fallthrough = quote!(_ => #missing_content);
                    return None;
                }
            };
            Some(quote! {
                __Field::#variant_index => #arm,
            })
        })
        .collect::<Vec<_>>();
    if !missing_content_arms.is_empty() {
        missing_content = quote! {
            match __field {
                #(#missing_content_arms)*
                #missing_content_fallthrough
            }
        };
    }

    // Advance the map by one key, returning early in case of error.
    let next_key = quote! {
        _serde::de::MapAccess::next_key_seed(&mut __map, #field_visitor_ty {
            tag: #tag,
            content: #content,
        })?
    };

    let variant_from_map = quote! {
        _serde::de::MapAccess::next_value_seed(&mut __map, _serde::#private::de::AdjacentlyTaggedEnumVariantSeed::<__Field> {
            enum_name: #rust_name,
            variants: VARIANTS,
            fields_enum: _serde::#private::PhantomData
        })?
    };

    // When allowing unknown fields, we want to transparently step through keys
    // we don't care about until we find `tag`, `content`, or run out of keys.
    let next_relevant_key = if deny_unknown_fields {
        next_key
    } else {
        quote!({
            let mut __rk : _serde::#private::Option<_serde::#private::de::TagOrContentField> = _serde::#private::None;
            while let _serde::#private::Some(__k) = #next_key {
                match __k {
                    _serde::#private::de::TagContentOtherField::Other => {
                        let _ = _serde::de::MapAccess::next_value::<_serde::de::IgnoredAny>(&mut __map)?;
                        continue;
                    },
                    _serde::#private::de::TagContentOtherField::Tag => {
                        __rk = _serde::#private::Some(_serde::#private::de::TagOrContentField::Tag);
                        break;
                    }
                    _serde::#private::de::TagContentOtherField::Content => {
                        __rk = _serde::#private::Some(_serde::#private::de::TagOrContentField::Content);
                        break;
                    }
                }
            }

            __rk
        })
    };

    // Step through remaining keys, looking for duplicates of previously-seen
    // keys. When unknown fields are denied, any key that isn't a duplicate will
    // at this point immediately produce an error.
    let visit_remaining_keys = quote! {
        match #next_relevant_key {
            _serde::#private::Some(_serde::#private::de::TagOrContentField::Tag) => {
                _serde::#private::Err(<__A::Error as _serde::de::Error>::duplicate_field(#tag))
            }
            _serde::#private::Some(_serde::#private::de::TagOrContentField::Content) => {
                _serde::#private::Err(<__A::Error as _serde::de::Error>::duplicate_field(#content))
            }
            _serde::#private::None => _serde::#private::Ok(__ret),
        }
    };

    let finish_content_then_tag = if variant_arms.is_empty() {
        quote! {
            match #variant_from_map {}
        }
    } else {
        quote! {
            let __seed = __Seed {
                variant: #variant_from_map,
                marker: _serde::#private::PhantomData,
                lifetime: _serde::#private::PhantomData,
            };
            let __deserializer = _serde::#private::de::ContentDeserializer::<__A::Error>::new(__content);
            let __ret = _serde::de::DeserializeSeed::deserialize(__seed, __deserializer)?;
            // Visit remaining keys, looking for duplicates.
            #visit_remaining_keys
        }
    };

    quote_block! {
        #variant_visitor

        #variants_stmt

        #[doc(hidden)]
        struct __Seed #de_impl_generics #where_clause {
            variant: __Field,
            marker: _serde::#private::PhantomData<#this_type #ty_generics>,
            lifetime: _serde::#private::PhantomData<&#delife ()>,
        }

        #[automatically_derived]
        impl #de_impl_generics _serde::de::DeserializeSeed<#delife> for __Seed #de_ty_generics #where_clause {
            type Value = #this_type #ty_generics;

            fn deserialize<__D>(self, __deserializer: __D) -> _serde::#private::Result<Self::Value, __D::Error>
            where
                __D: _serde::Deserializer<#delife>,
            {
                match self.variant {
                    #(#variant_arms)*
                }
            }
        }

        #[doc(hidden)]
        struct __Visitor #de_impl_generics #where_clause {
            marker: _serde::#private::PhantomData<#this_type #ty_generics>,
            lifetime: _serde::#private::PhantomData<&#delife ()>,
        }

        #[automatically_derived]
        impl #de_impl_generics _serde::de::Visitor<#delife> for __Visitor #de_ty_generics #where_clause {
            type Value = #this_type #ty_generics;

            fn expecting(&self, __formatter: &mut _serde::#private::Formatter) -> _serde::#private::fmt::Result {
                _serde::#private::Formatter::write_str(__formatter, #expecting)
            }

            fn visit_map<__A>(self, mut __map: __A) -> _serde::#private::Result<Self::Value, __A::Error>
            where
                __A: _serde::de::MapAccess<#delife>,
            {
                // Visit the first relevant key.
                match #next_relevant_key {
                    // First key is the tag.
                    _serde::#private::Some(_serde::#private::de::TagOrContentField::Tag) => {
                        // Parse the tag.
                        let __field = #variant_from_map;
                        // Visit the second key.
                        match #next_relevant_key {
                            // Second key is a duplicate of the tag.
                            _serde::#private::Some(_serde::#private::de::TagOrContentField::Tag) => {
                                _serde::#private::Err(<__A::Error as _serde::de::Error>::duplicate_field(#tag))
                            }
                            // Second key is the content.
                            _serde::#private::Some(_serde::#private::de::TagOrContentField::Content) => {
                                let __ret = _serde::de::MapAccess::next_value_seed(&mut __map,
                                    __Seed {
                                        variant: __field,
                                        marker: _serde::#private::PhantomData,
                                        lifetime: _serde::#private::PhantomData,
                                    })?;
                                // Visit remaining keys, looking for duplicates.
                                #visit_remaining_keys
                            }
                            // There is no second key; might be okay if the we have a unit variant.
                            _serde::#private::None => #missing_content
                        }
                    }
                    // First key is the content.
                    _serde::#private::Some(_serde::#private::de::TagOrContentField::Content) => {
                        // Buffer up the content.
                        let __content = _serde::de::MapAccess::next_value_seed(&mut __map, _serde::#private::de::ContentVisitor::new())?;
                        // Visit the second key.
                        match #next_relevant_key {
                            // Second key is the tag.
                            _serde::#private::Some(_serde::#private::de::TagOrContentField::Tag) => {
                                #finish_content_then_tag
                            }
                            // Second key is a duplicate of the content.
                            _serde::#private::Some(_serde::#private::de::TagOrContentField::Content) => {
                                _serde::#private::Err(<__A::Error as _serde::de::Error>::duplicate_field(#content))
                            }
                            // There is no second key.
                            _serde::#private::None => {
                                _serde::#private::Err(<__A::Error as _serde::de::Error>::missing_field(#tag))
                            }
                        }
                    }
                    // There is no first key.
                    _serde::#private::None => {
                        _serde::#private::Err(<__A::Error as _serde::de::Error>::missing_field(#tag))
                    }
                }
            }

            fn visit_seq<__A>(self, mut __seq: __A) -> _serde::#private::Result<Self::Value, __A::Error>
            where
                __A: _serde::de::SeqAccess<#delife>,
            {
                // Visit the first element - the tag.
                match _serde::de::SeqAccess::next_element(&mut __seq)? {
                    _serde::#private::Some(__variant) => {
                        // Visit the second element - the content.
                        match _serde::de::SeqAccess::next_element_seed(
                            &mut __seq,
                            __Seed {
                                variant: __variant,
                                marker: _serde::#private::PhantomData,
                                lifetime: _serde::#private::PhantomData,
                            },
                        )? {
                            _serde::#private::Some(__ret) => _serde::#private::Ok(__ret),
                            // There is no second element.
                            _serde::#private::None => {
                                _serde::#private::Err(_serde::de::Error::invalid_length(1, &self))
                            }
                        }
                    }
                    // There is no first element.
                    _serde::#private::None => {
                        _serde::#private::Err(_serde::de::Error::invalid_length(0, &self))
                    }
                }
            }
        }

        #[doc(hidden)]
        const FIELDS: &'static [&'static str] = &[#tag, #content];
        _serde::Deserializer::deserialize_struct(
            __deserializer,
            #type_name,
            FIELDS,
            __Visitor {
                marker: _serde::#private::PhantomData::<#this_type #ty_generics>,
                lifetime: _serde::#private::PhantomData,
            },
        )
    }
}
