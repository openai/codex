use crate::de::identifier;
use crate::de::{
    deserialize_seq, expr_is_missing, field_i, has_flatten, wrap_deserialize_field_with,
    FieldWithAliases, Parameters, StructForm,
};
#[cfg(feature = "deserialize_in_place")]
use crate::de::{deserialize_seq_in_place, place_lifetime};
use crate::fragment::{Expr, Fragment, Match, Stmts};
use crate::internals::ast::Field;
use crate::internals::attr;
use crate::private;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

/// Generates `Deserialize::deserialize` body for a `struct Struct {...}`
pub(super) fn deserialize(
    params: &Parameters,
    fields: &[Field],
    cattrs: &attr::Container,
    form: StructForm,
) -> Fragment {
    let this_type = &params.this_type;
    let this_value = &params.this_value;
    let (de_impl_generics, de_ty_generics, ty_generics, where_clause) =
        params.generics_with_de_lifetime();
    let delife = params.borrowed.de_lifetime();

    // If there are getters (implying private fields), construct the local type
    // and use an `Into` conversion to get the remote type. If there are no
    // getters then construct the target type directly.
    let construct = if params.has_getter {
        let local = &params.local;
        quote!(#local)
    } else {
        quote!(#this_value)
    };

    let type_path = match form {
        StructForm::Struct => construct,
        StructForm::ExternallyTagged(variant_ident)
        | StructForm::InternallyTagged(variant_ident)
        | StructForm::Untagged(variant_ident) => quote!(#construct::#variant_ident),
    };
    let expecting = match form {
        StructForm::Struct => format!("struct {}", params.type_name()),
        StructForm::ExternallyTagged(variant_ident)
        | StructForm::InternallyTagged(variant_ident)
        | StructForm::Untagged(variant_ident) => {
            format!("struct variant {}::{}", params.type_name(), variant_ident)
        }
    };
    let expecting = cattrs.expecting().unwrap_or(&expecting);

    let deserialized_fields: Vec<_> = fields
        .iter()
        .enumerate()
        // Skip fields that shouldn't be deserialized or that were flattened,
        // so they don't appear in the storage in their literal form
        .filter(|&(_, field)| !field.attrs.skip_deserializing() && !field.attrs.flatten())
        .map(|(i, field)| FieldWithAliases {
            ident: field_i(i),
            aliases: field.attrs.aliases(),
        })
        .collect();

    let has_flatten = has_flatten(fields);
    let field_visitor = deserialize_field_identifier(&deserialized_fields, cattrs, has_flatten);

    // untagged struct variants do not get a visit_seq method. The same applies to
    // structs that only have a map representation.
    let visit_seq = match form {
        StructForm::Untagged(_) => None,
        _ if has_flatten => None,
        _ => {
            let mut_seq = if deserialized_fields.is_empty() {
                quote!(_)
            } else {
                quote!(mut __seq)
            };

            let visit_seq = Stmts(deserialize_seq(
                &type_path, params, fields, true, cattrs, expecting,
            ));

            Some(quote! {
                #[inline]
                fn visit_seq<__A>(self, #mut_seq: __A) -> _serde::#private::Result<Self::Value, __A::Error>
                where
                    __A: _serde::de::SeqAccess<#delife>,
                {
                    #visit_seq
                }
            })
        }
    };
    let visit_map = Stmts(deserialize_map(
        &type_path,
        params,
        fields,
        cattrs,
        has_flatten,
    ));

    let visitor_seed = match form {
        StructForm::ExternallyTagged(..) if has_flatten => Some(quote! {
            #[automatically_derived]
            impl #de_impl_generics _serde::de::DeserializeSeed<#delife> for __Visitor #de_ty_generics #where_clause {
                type Value = #this_type #ty_generics;

                fn deserialize<__D>(self, __deserializer: __D) -> _serde::#private::Result<Self::Value, __D::Error>
                where
                    __D: _serde::Deserializer<#delife>,
                {
                    _serde::Deserializer::deserialize_map(__deserializer, self)
                }
            }
        }),
        _ => None,
    };

    let fields_stmt = if has_flatten {
        None
    } else {
        let field_names = deserialized_fields.iter().flat_map(|field| field.aliases);

        Some(quote! {
            #[doc(hidden)]
            const FIELDS: &'static [&'static str] = &[ #(#field_names),* ];
        })
    };

    let visitor_expr = quote! {
        __Visitor {
            marker: _serde::#private::PhantomData::<#this_type #ty_generics>,
            lifetime: _serde::#private::PhantomData,
        }
    };
    let dispatch = match form {
        StructForm::Struct if has_flatten => quote! {
            _serde::Deserializer::deserialize_map(__deserializer, #visitor_expr)
        },
        StructForm::Struct => {
            let type_name = cattrs.name().deserialize_name();
            quote! {
                _serde::Deserializer::deserialize_struct(__deserializer, #type_name, FIELDS, #visitor_expr)
            }
        }
        StructForm::ExternallyTagged(_) if has_flatten => quote! {
            _serde::de::VariantAccess::newtype_variant_seed(__variant, #visitor_expr)
        },
        StructForm::ExternallyTagged(_) => quote! {
            _serde::de::VariantAccess::struct_variant(__variant, FIELDS, #visitor_expr)
        },
        StructForm::InternallyTagged(_) => quote! {
            _serde::Deserializer::deserialize_any(__deserializer, #visitor_expr)
        },
        StructForm::Untagged(_) => quote! {
            _serde::Deserializer::deserialize_any(__deserializer, #visitor_expr)
        },
    };

    quote_block! {
        #field_visitor

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

            #visit_seq

            #[inline]
            fn visit_map<__A>(self, mut __map: __A) -> _serde::#private::Result<Self::Value, __A::Error>
            where
                __A: _serde::de::MapAccess<#delife>,
            {
                #visit_map
            }
        }

        #visitor_seed

        #fields_stmt

        #dispatch
    }
}

fn deserialize_map(
    struct_path: &TokenStream,
    params: &Parameters,
    fields: &[Field],
    cattrs: &attr::Container,
    has_flatten: bool,
) -> Fragment {
    // Create the field names for the fields.
    let fields_names: Vec<_> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| (field, field_i(i)))
        .collect();

    // Declare each field that will be deserialized.
    let let_values = fields_names
        .iter()
        .filter(|&&(field, _)| !field.attrs.skip_deserializing() && !field.attrs.flatten())
        .map(|(field, name)| {
            let field_ty = field.ty;
            quote! {
                let mut #name: _serde::#private::Option<#field_ty> = _serde::#private::None;
            }
        });

    // Collect contents for flatten fields into a buffer
    let let_collect = if has_flatten {
        Some(quote! {
            let mut __collect = _serde::#private::Vec::<_serde::#private::Option<(
                _serde::#private::de::Content,
                _serde::#private::de::Content
            )>>::new();
        })
    } else {
        None
    };

    // Match arms to extract a value for a field.
    let value_arms = fields_names
        .iter()
        .filter(|&&(field, _)| !field.attrs.skip_deserializing() && !field.attrs.flatten())
        .map(|(field, name)| {
            let deser_name = field.attrs.name().deserialize_name();

            let visit = match field.attrs.deserialize_with() {
                None => {
                    let field_ty = field.ty;
                    let span = field.original.span();
                    let func =
                        quote_spanned!(span=> _serde::de::MapAccess::next_value::<#field_ty>);
                    quote! {
                        #func(&mut __map)?
                    }
                }
                Some(path) => {
                    let (wrapper, wrapper_ty) = wrap_deserialize_field_with(params, field.ty, path);
                    quote!({
                        #wrapper
                        match _serde::de::MapAccess::next_value::<#wrapper_ty>(&mut __map) {
                            _serde::#private::Ok(__wrapper) => __wrapper.value,
                            _serde::#private::Err(__err) => {
                                return _serde::#private::Err(__err);
                            }
                        }
                    })
                }
            };
            quote! {
                __Field::#name => {
                    if _serde::#private::Option::is_some(&#name) {
                        return _serde::#private::Err(<__A::Error as _serde::de::Error>::duplicate_field(#deser_name));
                    }
                    #name = _serde::#private::Some(#visit);
                }
            }
        });

    // Visit ignored values to consume them
    let ignored_arm = if has_flatten {
        Some(quote! {
            __Field::__other(__name) => {
                __collect.push(_serde::#private::Some((
                    __name,
                    _serde::de::MapAccess::next_value_seed(&mut __map, _serde::#private::de::ContentVisitor::new())?)));
            }
        })
    } else if cattrs.deny_unknown_fields() {
        None
    } else {
        Some(quote! {
            _ => { let _ = _serde::de::MapAccess::next_value::<_serde::de::IgnoredAny>(&mut __map)?; }
        })
    };

    let all_skipped = fields.iter().all(|field| field.attrs.skip_deserializing());
    let match_keys = if cattrs.deny_unknown_fields() && all_skipped {
        quote! {
            // FIXME: Once feature(exhaustive_patterns) is stable:
            // let _serde::#private::None::<__Field> = _serde::de::MapAccess::next_key(&mut __map)?;
            _serde::#private::Option::map(
                _serde::de::MapAccess::next_key::<__Field>(&mut __map)?,
                |__impossible| match __impossible {});
        }
    } else {
        quote! {
            while let _serde::#private::Some(__key) = _serde::de::MapAccess::next_key::<__Field>(&mut __map)? {
                match __key {
                    #(#value_arms)*
                    #ignored_arm
                }
            }
        }
    };

    let extract_values = fields_names
        .iter()
        .filter(|&&(field, _)| !field.attrs.skip_deserializing() && !field.attrs.flatten())
        .map(|(field, name)| {
            let missing_expr = Match(expr_is_missing(field, cattrs));

            quote! {
                let #name = match #name {
                    _serde::#private::Some(#name) => #name,
                    _serde::#private::None => #missing_expr
                };
            }
        });

    let extract_collected = fields_names
        .iter()
        .filter(|&&(field, _)| field.attrs.flatten() && !field.attrs.skip_deserializing())
        .map(|(field, name)| {
            let field_ty = field.ty;
            let func = match field.attrs.deserialize_with() {
                None => {
                    let span = field.original.span();
                    quote_spanned!(span=> _serde::de::Deserialize::deserialize)
                }
                Some(path) => quote!(#path),
            };
            quote! {
                let #name: #field_ty = #func(
                    _serde::#private::de::FlatMapDeserializer(
                        &mut __collect,
                        _serde::#private::PhantomData))?;
            }
        });

    let collected_deny_unknown_fields = if has_flatten && cattrs.deny_unknown_fields() {
        Some(quote! {
            if let _serde::#private::Some(_serde::#private::Some((__key, _))) =
                __collect.into_iter().filter(_serde::#private::Option::is_some).next()
            {
                if let _serde::#private::Some(__key) = _serde::#private::de::content_as_str(&__key) {
                    return _serde::#private::Err(
                        _serde::de::Error::custom(format_args!("unknown field `{}`", &__key)));
                } else {
                    return _serde::#private::Err(
                        _serde::de::Error::custom(format_args!("unexpected map key")));
                }
            }
        })
    } else {
        None
    };

    let result = fields_names.iter().map(|(field, name)| {
        let member = &field.member;
        if field.attrs.skip_deserializing() {
            let value = Expr(expr_is_missing(field, cattrs));
            quote!(#member: #value)
        } else {
            quote!(#member: #name)
        }
    });

    let let_default = match cattrs.default() {
        attr::Default::Default => Some(quote!(
            let __default: Self::Value = _serde::#private::Default::default();
        )),
        // If #path returns wrong type, error will be reported here (^^^^^).
        // We attach span of the path to the function so it will be reported
        // on the #[serde(default = "...")]
        //                          ^^^^^
        attr::Default::Path(path) => Some(quote_spanned!(path.span()=>
            let __default: Self::Value = #path();
        )),
        attr::Default::None => {
            // We don't need the default value, to prevent an unused variable warning
            // we'll leave the line empty.
            None
        }
    };

    let mut result = quote!(#struct_path { #(#result),* });
    if params.has_getter {
        let this_type = &params.this_type;
        let (_, ty_generics, _) = params.generics.split_for_impl();
        result = quote! {
            _serde::#private::Into::<#this_type #ty_generics>::into(#result)
        };
    }

    quote_block! {
        #(#let_values)*

        #let_collect

        #match_keys

        #let_default

        #(#extract_values)*

        #(#extract_collected)*

        #collected_deny_unknown_fields

        _serde::#private::Ok(#result)
    }
}

/// Generates `Deserialize::deserialize_in_place` body for a `struct Struct {...}`
#[cfg(feature = "deserialize_in_place")]
pub(super) fn deserialize_in_place(
    params: &Parameters,
    fields: &[Field],
    cattrs: &attr::Container,
) -> Option<Fragment> {
    // for now we do not support in_place deserialization for structs that
    // are represented as map.
    if has_flatten(fields) {
        return None;
    }

    let this_type = &params.this_type;
    let (de_impl_generics, de_ty_generics, ty_generics, where_clause) =
        params.generics_with_de_lifetime();
    let delife = params.borrowed.de_lifetime();

    let expecting = format!("struct {}", params.type_name());
    let expecting = cattrs.expecting().unwrap_or(&expecting);

    let deserialized_fields: Vec<_> = fields
        .iter()
        .enumerate()
        .filter(|&(_, field)| !field.attrs.skip_deserializing())
        .map(|(i, field)| FieldWithAliases {
            ident: field_i(i),
            aliases: field.attrs.aliases(),
        })
        .collect();

    let field_visitor = deserialize_field_identifier(&deserialized_fields, cattrs, false);

    let mut_seq = if deserialized_fields.is_empty() {
        quote!(_)
    } else {
        quote!(mut __seq)
    };
    let visit_seq = Stmts(deserialize_seq_in_place(params, fields, cattrs, expecting));
    let visit_map = Stmts(deserialize_map_in_place(params, fields, cattrs));
    let field_names = deserialized_fields.iter().flat_map(|field| field.aliases);
    let type_name = cattrs.name().deserialize_name();

    let in_place_impl_generics = de_impl_generics.in_place();
    let in_place_ty_generics = de_ty_generics.in_place();
    let place_life = place_lifetime();

    Some(quote_block! {
        #field_visitor

        #[doc(hidden)]
        struct __Visitor #in_place_impl_generics #where_clause {
            place: &#place_life mut #this_type #ty_generics,
            lifetime: _serde::#private::PhantomData<&#delife ()>,
        }

        #[automatically_derived]
        impl #in_place_impl_generics _serde::de::Visitor<#delife> for __Visitor #in_place_ty_generics #where_clause {
            type Value = ();

            fn expecting(&self, __formatter: &mut _serde::#private::Formatter) -> _serde::#private::fmt::Result {
                _serde::#private::Formatter::write_str(__formatter, #expecting)
            }

            #[inline]
            fn visit_seq<__A>(self, #mut_seq: __A) -> _serde::#private::Result<Self::Value, __A::Error>
            where
                __A: _serde::de::SeqAccess<#delife>,
            {
                #visit_seq
            }

            #[inline]
            fn visit_map<__A>(self, mut __map: __A) -> _serde::#private::Result<Self::Value, __A::Error>
            where
                __A: _serde::de::MapAccess<#delife>,
            {
                #visit_map
            }
        }

        #[doc(hidden)]
        const FIELDS: &'static [&'static str] = &[ #(#field_names),* ];

        _serde::Deserializer::deserialize_struct(__deserializer, #type_name, FIELDS, __Visitor {
            place: __place,
            lifetime: _serde::#private::PhantomData,
        })
    })
}

#[cfg(feature = "deserialize_in_place")]
fn deserialize_map_in_place(
    params: &Parameters,
    fields: &[Field],
    cattrs: &attr::Container,
) -> Fragment {
    assert!(
        !has_flatten(fields),
        "inplace deserialization of maps does not support flatten fields"
    );

    // Create the field names for the fields.
    let fields_names: Vec<_> = fields
        .iter()
        .enumerate()
        .map(|(i, field)| (field, field_i(i)))
        .collect();

    // For deserialize_in_place, declare booleans for each field that will be
    // deserialized.
    let let_flags = fields_names
        .iter()
        .filter(|&&(field, _)| !field.attrs.skip_deserializing())
        .map(|(_, name)| {
            quote! {
                let mut #name: bool = false;
            }
        });

    // Match arms to extract a value for a field.
    let value_arms_from = fields_names
        .iter()
        .filter(|&&(field, _)| !field.attrs.skip_deserializing())
        .map(|(field, name)| {
            let deser_name = field.attrs.name().deserialize_name();
            let member = &field.member;

            let visit = match field.attrs.deserialize_with() {
                None => {
                    quote! {
                        _serde::de::MapAccess::next_value_seed(&mut __map, _serde::#private::de::InPlaceSeed(&mut self.place.#member))?
                    }
                }
                Some(path) => {
                    let (wrapper, wrapper_ty) = wrap_deserialize_field_with(params, field.ty, path);
                    quote!({
                        #wrapper
                        self.place.#member = match _serde::de::MapAccess::next_value::<#wrapper_ty>(&mut __map) {
                            _serde::#private::Ok(__wrapper) => __wrapper.value,
                            _serde::#private::Err(__err) => {
                                return _serde::#private::Err(__err);
                            }
                        };
                    })
                }
            };
            quote! {
                __Field::#name => {
                    if #name {
                        return _serde::#private::Err(<__A::Error as _serde::de::Error>::duplicate_field(#deser_name));
                    }
                    #visit;
                    #name = true;
                }
            }
        });

    // Visit ignored values to consume them
    let ignored_arm = if cattrs.deny_unknown_fields() {
        None
    } else {
        Some(quote! {
            _ => { let _ = _serde::de::MapAccess::next_value::<_serde::de::IgnoredAny>(&mut __map)?; }
        })
    };

    let all_skipped = fields.iter().all(|field| field.attrs.skip_deserializing());

    let match_keys = if cattrs.deny_unknown_fields() && all_skipped {
        quote! {
            // FIXME: Once feature(exhaustive_patterns) is stable:
            // let _serde::#private::None::<__Field> = _serde::de::MapAccess::next_key(&mut __map)?;
            _serde::#private::Option::map(
                _serde::de::MapAccess::next_key::<__Field>(&mut __map)?,
                |__impossible| match __impossible {});
        }
    } else {
        quote! {
            while let _serde::#private::Some(__key) = _serde::de::MapAccess::next_key::<__Field>(&mut __map)? {
                match __key {
                    #(#value_arms_from)*
                    #ignored_arm
                }
            }
        }
    };

    let check_flags = fields_names
        .iter()
        .filter(|&&(field, _)| !field.attrs.skip_deserializing())
        .map(|(field, name)| {
            let missing_expr = expr_is_missing(field, cattrs);
            // If missing_expr unconditionally returns an error, don't try
            // to assign its value to self.place.
            if field.attrs.default().is_none()
                && cattrs.default().is_none()
                && field.attrs.deserialize_with().is_some()
            {
                let missing_expr = Stmts(missing_expr);
                quote! {
                    if !#name {
                        #missing_expr;
                    }
                }
            } else {
                let member = &field.member;
                let missing_expr = Expr(missing_expr);
                quote! {
                    if !#name {
                        self.place.#member = #missing_expr;
                    };
                }
            }
        });

    let this_type = &params.this_type;
    let (_, ty_generics, _) = params.generics.split_for_impl();

    let let_default = match cattrs.default() {
        attr::Default::Default => Some(quote!(
            let __default: #this_type #ty_generics = _serde::#private::Default::default();
        )),
        // If #path returns wrong type, error will be reported here (^^^^^).
        // We attach span of the path to the function so it will be reported
        // on the #[serde(default = "...")]
        //                          ^^^^^
        attr::Default::Path(path) => Some(quote_spanned!(path.span()=>
            let __default: #this_type #ty_generics = #path();
        )),
        attr::Default::None => {
            // We don't need the default value, to prevent an unused variable warning
            // we'll leave the line empty.
            None
        }
    };

    quote_block! {
        #(#let_flags)*

        #match_keys

        #let_default

        #(#check_flags)*

        _serde::#private::Ok(())
    }
}

/// Generates enum and its `Deserialize` implementation that represents each
/// non-skipped field of the struct
fn deserialize_field_identifier(
    deserialized_fields: &[FieldWithAliases],
    cattrs: &attr::Container,
    has_flatten: bool,
) -> Stmts {
    let (ignore_variant, fallthrough) = if has_flatten {
        let ignore_variant = quote!(__other(_serde::#private::de::Content<'de>),);
        let fallthrough = quote!(_serde::#private::Ok(__Field::__other(__value)));
        (Some(ignore_variant), Some(fallthrough))
    } else if cattrs.deny_unknown_fields() {
        (None, None)
    } else {
        let ignore_variant = quote!(__ignore,);
        let fallthrough = quote!(_serde::#private::Ok(__Field::__ignore));
        (Some(ignore_variant), Some(fallthrough))
    };

    Stmts(identifier::deserialize_generated(
        deserialized_fields,
        has_flatten,
        false,
        ignore_variant,
        fallthrough,
    ))
}
