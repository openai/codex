use crate::de::{deserialize_seq, has_flatten, Parameters, TupleForm};
#[cfg(feature = "deserialize_in_place")]
use crate::de::{deserialize_seq_in_place, place_lifetime};
use crate::fragment::{Fragment, Stmts};
use crate::internals::ast::Field;
use crate::internals::attr;
use crate::private;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

/// Generates `Deserialize::deserialize` body for a `struct Tuple(...);` including `struct Newtype(T);`
pub(super) fn deserialize(
    params: &Parameters,
    fields: &[Field],
    cattrs: &attr::Container,
    form: TupleForm,
) -> Fragment {
    assert!(
        !has_flatten(fields),
        "tuples and tuple variants cannot have flatten fields"
    );

    let field_count = fields
        .iter()
        .filter(|field| !field.attrs.skip_deserializing())
        .count();

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
        TupleForm::Tuple => construct,
        TupleForm::ExternallyTagged(variant_ident) | TupleForm::Untagged(variant_ident) => {
            quote!(#construct::#variant_ident)
        }
    };
    let expecting = match form {
        TupleForm::Tuple => format!("tuple struct {}", params.type_name()),
        TupleForm::ExternallyTagged(variant_ident) | TupleForm::Untagged(variant_ident) => {
            format!("tuple variant {}::{}", params.type_name(), variant_ident)
        }
    };
    let expecting = cattrs.expecting().unwrap_or(&expecting);

    let nfields = fields.len();

    let visit_newtype_struct = match form {
        TupleForm::Tuple if nfields == 1 => {
            Some(deserialize_newtype_struct(&type_path, params, &fields[0]))
        }
        _ => None,
    };

    let visit_seq = Stmts(deserialize_seq(
        &type_path, params, fields, false, cattrs, expecting,
    ));

    let visitor_expr = quote! {
        __Visitor {
            marker: _serde::#private::PhantomData::<#this_type #ty_generics>,
            lifetime: _serde::#private::PhantomData,
        }
    };
    let dispatch = match form {
        TupleForm::Tuple if nfields == 1 => {
            let type_name = cattrs.name().deserialize_name();
            quote! {
                _serde::Deserializer::deserialize_newtype_struct(__deserializer, #type_name, #visitor_expr)
            }
        }
        TupleForm::Tuple => {
            let type_name = cattrs.name().deserialize_name();
            quote! {
                _serde::Deserializer::deserialize_tuple_struct(__deserializer, #type_name, #field_count, #visitor_expr)
            }
        }
        TupleForm::ExternallyTagged(_) => quote! {
            _serde::de::VariantAccess::tuple_variant(__variant, #field_count, #visitor_expr)
        },
        TupleForm::Untagged(_) => quote! {
            _serde::Deserializer::deserialize_tuple(__deserializer, #field_count, #visitor_expr)
        },
    };

    let visitor_var = if field_count == 0 {
        quote!(_)
    } else {
        quote!(mut __seq)
    };

    quote_block! {
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

            #visit_newtype_struct

            #[inline]
            fn visit_seq<__A>(self, #visitor_var: __A) -> _serde::#private::Result<Self::Value, __A::Error>
            where
                __A: _serde::de::SeqAccess<#delife>,
            {
                #visit_seq
            }
        }

        #dispatch
    }
}

fn deserialize_newtype_struct(
    type_path: &TokenStream,
    params: &Parameters,
    field: &Field,
) -> TokenStream {
    let delife = params.borrowed.de_lifetime();
    let field_ty = field.ty;
    let deserializer_var = quote!(__e);

    let value = match field.attrs.deserialize_with() {
        None => {
            let span = field.original.span();
            let func = quote_spanned!(span=> <#field_ty as _serde::Deserialize>::deserialize);
            quote! {
                #func(#deserializer_var)?
            }
        }
        Some(path) => {
            // If #path returns wrong type, error will be reported here (^^^^^).
            // We attach span of the path to the function so it will be reported
            // on the #[serde(with = "...")]
            //                       ^^^^^
            quote_spanned! {path.span()=>
                #path(#deserializer_var)?
            }
        }
    };

    let mut result = quote!(#type_path(__field0));
    if params.has_getter {
        let this_type = &params.this_type;
        let (_, ty_generics, _) = params.generics.split_for_impl();
        result = quote! {
            _serde::#private::Into::<#this_type #ty_generics>::into(#result)
        };
    }

    quote! {
        #[inline]
        fn visit_newtype_struct<__E>(self, #deserializer_var: __E) -> _serde::#private::Result<Self::Value, __E::Error>
        where
            __E: _serde::Deserializer<#delife>,
        {
            let __field0: #field_ty = #value;
            _serde::#private::Ok(#result)
        }
    }
}

/// Generates `Deserialize::deserialize_in_place` body for a `struct Tuple(...);` including `struct Newtype(T);`
#[cfg(feature = "deserialize_in_place")]
pub(super) fn deserialize_in_place(
    params: &Parameters,
    fields: &[Field],
    cattrs: &attr::Container,
) -> Fragment {
    assert!(
        !has_flatten(fields),
        "tuples and tuple variants cannot have flatten fields"
    );

    let field_count = fields
        .iter()
        .filter(|field| !field.attrs.skip_deserializing())
        .count();

    let this_type = &params.this_type;
    let (de_impl_generics, de_ty_generics, ty_generics, where_clause) =
        params.generics_with_de_lifetime();
    let delife = params.borrowed.de_lifetime();

    let expecting = format!("tuple struct {}", params.type_name());
    let expecting = cattrs.expecting().unwrap_or(&expecting);

    let nfields = fields.len();

    let visit_newtype_struct = if nfields == 1 {
        // We do not generate deserialize_in_place if every field has a
        // deserialize_with.
        assert!(fields[0].attrs.deserialize_with().is_none());

        Some(quote! {
            #[inline]
            fn visit_newtype_struct<__E>(self, __e: __E) -> _serde::#private::Result<Self::Value, __E::Error>
            where
                __E: _serde::Deserializer<#delife>,
            {
                _serde::Deserialize::deserialize_in_place(__e, &mut self.place.0)
            }
        })
    } else {
        None
    };

    let visit_seq = Stmts(deserialize_seq_in_place(params, fields, cattrs, expecting));

    let visitor_expr = quote! {
        __Visitor {
            place: __place,
            lifetime: _serde::#private::PhantomData,
        }
    };

    let type_name = cattrs.name().deserialize_name();
    let dispatch = if nfields == 1 {
        quote!(_serde::Deserializer::deserialize_newtype_struct(__deserializer, #type_name, #visitor_expr))
    } else {
        quote!(_serde::Deserializer::deserialize_tuple_struct(__deserializer, #type_name, #field_count, #visitor_expr))
    };

    let visitor_var = if field_count == 0 {
        quote!(_)
    } else {
        quote!(mut __seq)
    };

    let in_place_impl_generics = de_impl_generics.in_place();
    let in_place_ty_generics = de_ty_generics.in_place();
    let place_life = place_lifetime();

    quote_block! {
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

            #visit_newtype_struct

            #[inline]
            fn visit_seq<__A>(self, #visitor_var: __A) -> _serde::#private::Result<Self::Value, __A::Error>
            where
                __A: _serde::de::SeqAccess<#delife>,
            {
                #visit_seq
            }
        }

        #dispatch
    }
}
