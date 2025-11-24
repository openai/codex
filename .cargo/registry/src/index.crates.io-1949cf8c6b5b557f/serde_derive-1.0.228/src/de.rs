use crate::deprecated::allow_deprecated;
use crate::fragment::{Expr, Fragment, Stmts};
use crate::internals::ast::{Container, Data, Field, Style, Variant};
use crate::internals::name::Name;
use crate::internals::{attr, replace_receiver, ungroup, Ctxt, Derive};
use crate::{bound, dummy, pretend, private, this};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use std::collections::BTreeSet;
use std::ptr;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{parse_quote, Ident, Index, Member};

mod enum_;
mod enum_adjacently;
mod enum_externally;
mod enum_internally;
mod enum_untagged;
mod identifier;
mod struct_;
mod tuple;
mod unit;

pub fn expand_derive_deserialize(input: &mut syn::DeriveInput) -> syn::Result<TokenStream> {
    replace_receiver(input);

    let ctxt = Ctxt::new();
    let cont = match Container::from_ast(&ctxt, input, Derive::Deserialize, &private.ident()) {
        Some(cont) => cont,
        None => return Err(ctxt.check().unwrap_err()),
    };
    precondition(&ctxt, &cont);
    ctxt.check()?;

    let ident = &cont.ident;
    let params = Parameters::new(&cont);
    let (de_impl_generics, _, ty_generics, where_clause) = params.generics_with_de_lifetime();
    let body = Stmts(deserialize_body(&cont, &params));
    let delife = params.borrowed.de_lifetime();
    let allow_deprecated = allow_deprecated(input);

    let impl_block = if let Some(remote) = cont.attrs.remote() {
        let vis = &input.vis;
        let used = pretend::pretend_used(&cont, params.is_packed);
        quote! {
            #[automatically_derived]
            #allow_deprecated
            impl #de_impl_generics #ident #ty_generics #where_clause {
                #vis fn deserialize<__D>(__deserializer: __D) -> _serde::#private::Result<#remote #ty_generics, __D::Error>
                where
                    __D: _serde::Deserializer<#delife>,
                {
                    #used
                    #body
                }
            }
        }
    } else {
        let fn_deserialize_in_place = deserialize_in_place_body(&cont, &params);

        quote! {
            #[automatically_derived]
            #allow_deprecated
            impl #de_impl_generics _serde::Deserialize<#delife> for #ident #ty_generics #where_clause {
                fn deserialize<__D>(__deserializer: __D) -> _serde::#private::Result<Self, __D::Error>
                where
                    __D: _serde::Deserializer<#delife>,
                {
                    #body
                }

                #fn_deserialize_in_place
            }
        }
    };

    Ok(dummy::wrap_in_const(
        cont.attrs.custom_serde_path(),
        impl_block,
    ))
}

fn precondition(cx: &Ctxt, cont: &Container) {
    precondition_sized(cx, cont);
    precondition_no_de_lifetime(cx, cont);
}

fn precondition_sized(cx: &Ctxt, cont: &Container) {
    if let Data::Struct(_, fields) = &cont.data {
        if let Some(last) = fields.last() {
            if let syn::Type::Slice(_) = ungroup(last.ty) {
                cx.error_spanned_by(
                    cont.original,
                    "cannot deserialize a dynamically sized struct",
                );
            }
        }
    }
}

fn precondition_no_de_lifetime(cx: &Ctxt, cont: &Container) {
    if let BorrowedLifetimes::Borrowed(_) = borrowed_lifetimes(cont) {
        for param in cont.generics.lifetimes() {
            if param.lifetime.to_string() == "'de" {
                cx.error_spanned_by(
                    &param.lifetime,
                    "cannot deserialize when there is a lifetime parameter called 'de",
                );
                return;
            }
        }
    }
}

struct Parameters {
    /// Name of the type the `derive` is on.
    local: syn::Ident,

    /// Path to the type the impl is for. Either a single `Ident` for local
    /// types (does not include generic parameters) or `some::remote::Path` for
    /// remote types.
    this_type: syn::Path,

    /// Same as `this_type` but using `::<T>` for generic parameters for use in
    /// expression position.
    this_value: syn::Path,

    /// Generics including any explicit and inferred bounds for the impl.
    generics: syn::Generics,

    /// Lifetimes borrowed from the deserializer. These will become bounds on
    /// the `'de` lifetime of the deserializer.
    borrowed: BorrowedLifetimes,

    /// At least one field has a serde(getter) attribute, implying that the
    /// remote type has a private field.
    has_getter: bool,

    /// Type has a repr(packed) attribute.
    is_packed: bool,
}

impl Parameters {
    fn new(cont: &Container) -> Self {
        let local = cont.ident.clone();
        let this_type = this::this_type(cont);
        let this_value = this::this_value(cont);
        let borrowed = borrowed_lifetimes(cont);
        let generics = build_generics(cont, &borrowed);
        let has_getter = cont.data.has_getter();
        let is_packed = cont.attrs.is_packed();

        Parameters {
            local,
            this_type,
            this_value,
            generics,
            borrowed,
            has_getter,
            is_packed,
        }
    }

    /// Type name to use in error messages and `&'static str` arguments to
    /// various Deserializer methods.
    fn type_name(&self) -> String {
        self.this_type.segments.last().unwrap().ident.to_string()
    }

    /// Split the data structure's generics into the pieces to use for its
    /// `Deserialize` impl, augmented with an additional `'de` lifetime for use
    /// as the `Deserialize` trait's lifetime.
    fn generics_with_de_lifetime(
        &self,
    ) -> (
        DeImplGenerics,
        DeTypeGenerics,
        syn::TypeGenerics,
        Option<&syn::WhereClause>,
    ) {
        let de_impl_generics = DeImplGenerics(self);
        let de_ty_generics = DeTypeGenerics(self);
        let (_, ty_generics, where_clause) = self.generics.split_for_impl();
        (de_impl_generics, de_ty_generics, ty_generics, where_clause)
    }
}

// All the generics in the input, plus a bound `T: Deserialize` for each generic
// field type that will be deserialized by us, plus a bound `T: Default` for
// each generic field type that will be set to a default value.
fn build_generics(cont: &Container, borrowed: &BorrowedLifetimes) -> syn::Generics {
    let generics = bound::without_defaults(cont.generics);

    let generics = bound::with_where_predicates_from_fields(cont, &generics, attr::Field::de_bound);

    let generics =
        bound::with_where_predicates_from_variants(cont, &generics, attr::Variant::de_bound);

    match cont.attrs.de_bound() {
        Some(predicates) => bound::with_where_predicates(&generics, predicates),
        None => {
            let generics = match *cont.attrs.default() {
                attr::Default::Default => bound::with_self_bound(
                    cont,
                    &generics,
                    &parse_quote!(_serde::#private::Default),
                ),
                attr::Default::None | attr::Default::Path(_) => generics,
            };

            let delife = borrowed.de_lifetime();
            let generics = bound::with_bound(
                cont,
                &generics,
                needs_deserialize_bound,
                &parse_quote!(_serde::Deserialize<#delife>),
            );

            bound::with_bound(
                cont,
                &generics,
                requires_default,
                &parse_quote!(_serde::#private::Default),
            )
        }
    }
}

// Fields with a `skip_deserializing` or `deserialize_with` attribute, or which
// belong to a variant with a `skip_deserializing` or `deserialize_with`
// attribute, are not deserialized by us so we do not generate a bound. Fields
// with a `bound` attribute specify their own bound so we do not generate one.
// All other fields may need a `T: Deserialize` bound where T is the type of the
// field.
fn needs_deserialize_bound(field: &attr::Field, variant: Option<&attr::Variant>) -> bool {
    !field.skip_deserializing()
        && field.deserialize_with().is_none()
        && field.de_bound().is_none()
        && variant.map_or(true, |variant| {
            !variant.skip_deserializing()
                && variant.deserialize_with().is_none()
                && variant.de_bound().is_none()
        })
}

// Fields with a `default` attribute (not `default=...`), and fields with a
// `skip_deserializing` attribute that do not also have `default=...`.
fn requires_default(field: &attr::Field, _variant: Option<&attr::Variant>) -> bool {
    if let attr::Default::Default = *field.default() {
        true
    } else {
        false
    }
}

enum BorrowedLifetimes {
    Borrowed(BTreeSet<syn::Lifetime>),
    Static,
}

impl BorrowedLifetimes {
    fn de_lifetime(&self) -> syn::Lifetime {
        match *self {
            BorrowedLifetimes::Borrowed(_) => syn::Lifetime::new("'de", Span::call_site()),
            BorrowedLifetimes::Static => syn::Lifetime::new("'static", Span::call_site()),
        }
    }

    fn de_lifetime_param(&self) -> Option<syn::LifetimeParam> {
        match self {
            BorrowedLifetimes::Borrowed(bounds) => Some(syn::LifetimeParam {
                attrs: Vec::new(),
                lifetime: syn::Lifetime::new("'de", Span::call_site()),
                colon_token: None,
                bounds: bounds.iter().cloned().collect(),
            }),
            BorrowedLifetimes::Static => None,
        }
    }
}

// The union of lifetimes borrowed by each field of the container.
//
// These turn into bounds on the `'de` lifetime of the Deserialize impl. If
// lifetimes `'a` and `'b` are borrowed but `'c` is not, the impl is:
//
//     impl<'de: 'a + 'b, 'a, 'b, 'c> Deserialize<'de> for S<'a, 'b, 'c>
//
// If any borrowed lifetime is `'static`, then `'de: 'static` would be redundant
// and we use plain `'static` instead of `'de`.
fn borrowed_lifetimes(cont: &Container) -> BorrowedLifetimes {
    let mut lifetimes = BTreeSet::new();
    for field in cont.data.all_fields() {
        if !field.attrs.skip_deserializing() {
            lifetimes.extend(field.attrs.borrowed_lifetimes().iter().cloned());
        }
    }
    if lifetimes.iter().any(|b| b.to_string() == "'static") {
        BorrowedLifetimes::Static
    } else {
        BorrowedLifetimes::Borrowed(lifetimes)
    }
}

fn deserialize_body(cont: &Container, params: &Parameters) -> Fragment {
    if cont.attrs.transparent() {
        deserialize_transparent(cont, params)
    } else if let Some(type_from) = cont.attrs.type_from() {
        deserialize_from(type_from)
    } else if let Some(type_try_from) = cont.attrs.type_try_from() {
        deserialize_try_from(type_try_from)
    } else if let attr::Identifier::No = cont.attrs.identifier() {
        match &cont.data {
            Data::Enum(variants) => enum_::deserialize(params, variants, &cont.attrs),
            Data::Struct(Style::Struct, fields) => {
                struct_::deserialize(params, fields, &cont.attrs, StructForm::Struct)
            }
            Data::Struct(Style::Tuple, fields) | Data::Struct(Style::Newtype, fields) => {
                tuple::deserialize(params, fields, &cont.attrs, TupleForm::Tuple)
            }
            Data::Struct(Style::Unit, _) => unit::deserialize(params, &cont.attrs),
        }
    } else {
        match &cont.data {
            Data::Enum(variants) => identifier::deserialize_custom(params, variants, &cont.attrs),
            Data::Struct(_, _) => unreachable!("checked in serde_derive_internals"),
        }
    }
}

#[cfg(feature = "deserialize_in_place")]
fn deserialize_in_place_body(cont: &Container, params: &Parameters) -> Option<Stmts> {
    // Only remote derives have getters, and we do not generate
    // deserialize_in_place for remote derives.
    assert!(!params.has_getter);

    if cont.attrs.transparent()
        || cont.attrs.type_from().is_some()
        || cont.attrs.type_try_from().is_some()
        || cont.attrs.identifier().is_some()
        || cont
            .data
            .all_fields()
            .all(|f| f.attrs.deserialize_with().is_some())
    {
        return None;
    }

    let code = match &cont.data {
        Data::Struct(Style::Struct, fields) => {
            struct_::deserialize_in_place(params, fields, &cont.attrs)?
        }
        Data::Struct(Style::Tuple, fields) | Data::Struct(Style::Newtype, fields) => {
            tuple::deserialize_in_place(params, fields, &cont.attrs)
        }
        Data::Enum(_) | Data::Struct(Style::Unit, _) => {
            return None;
        }
    };

    let delife = params.borrowed.de_lifetime();
    let stmts = Stmts(code);

    let fn_deserialize_in_place = quote_block! {
        fn deserialize_in_place<__D>(__deserializer: __D, __place: &mut Self) -> _serde::#private::Result<(), __D::Error>
        where
            __D: _serde::Deserializer<#delife>,
        {
            #stmts
        }
    };

    Some(Stmts(fn_deserialize_in_place))
}

#[cfg(not(feature = "deserialize_in_place"))]
fn deserialize_in_place_body(_cont: &Container, _params: &Parameters) -> Option<Stmts> {
    None
}

/// Generates `Deserialize::deserialize` body for a type with `#[serde(transparent)]` attribute
fn deserialize_transparent(cont: &Container, params: &Parameters) -> Fragment {
    let fields = match &cont.data {
        Data::Struct(_, fields) => fields,
        Data::Enum(_) => unreachable!(),
    };

    let this_value = &params.this_value;
    let transparent_field = fields.iter().find(|f| f.attrs.transparent()).unwrap();

    let path = match transparent_field.attrs.deserialize_with() {
        Some(path) => quote!(#path),
        None => {
            let span = transparent_field.original.span();
            quote_spanned!(span=> _serde::Deserialize::deserialize)
        }
    };

    let assign = fields.iter().map(|field| {
        let member = &field.member;
        if ptr::eq(field, transparent_field) {
            quote!(#member: __transparent)
        } else {
            let value = match field.attrs.default() {
                attr::Default::Default => quote!(_serde::#private::Default::default()),
                // If #path returns wrong type, error will be reported here (^^^^^).
                // We attach span of the path to the function so it will be reported
                // on the #[serde(default = "...")]
                //                          ^^^^^
                attr::Default::Path(path) => quote_spanned!(path.span()=> #path()),
                attr::Default::None => quote!(_serde::#private::PhantomData),
            };
            quote!(#member: #value)
        }
    });

    quote_block! {
        _serde::#private::Result::map(
            #path(__deserializer),
            |__transparent| #this_value { #(#assign),* })
    }
}

/// Generates `Deserialize::deserialize` body for a type with `#[serde(from)]` attribute
fn deserialize_from(type_from: &syn::Type) -> Fragment {
    quote_block! {
        _serde::#private::Result::map(
            <#type_from as _serde::Deserialize>::deserialize(__deserializer),
            _serde::#private::From::from)
    }
}

/// Generates `Deserialize::deserialize` body for a type with `#[serde(try_from)]` attribute
fn deserialize_try_from(type_try_from: &syn::Type) -> Fragment {
    quote_block! {
        _serde::#private::Result::and_then(
            <#type_try_from as _serde::Deserialize>::deserialize(__deserializer),
            |v| _serde::#private::TryFrom::try_from(v).map_err(_serde::de::Error::custom))
    }
}

enum TupleForm<'a> {
    Tuple,
    /// Contains a variant name
    ExternallyTagged(&'a syn::Ident),
    /// Contains a variant name
    Untagged(&'a syn::Ident),
}

fn deserialize_seq(
    type_path: &TokenStream,
    params: &Parameters,
    fields: &[Field],
    is_struct: bool,
    cattrs: &attr::Container,
    expecting: &str,
) -> Fragment {
    let vars = (0..fields.len()).map(field_i as fn(_) -> _);

    let deserialized_count = fields
        .iter()
        .filter(|field| !field.attrs.skip_deserializing())
        .count();
    let expecting = if deserialized_count == 1 {
        format!("{} with 1 element", expecting)
    } else {
        format!("{} with {} elements", expecting, deserialized_count)
    };
    let expecting = cattrs.expecting().unwrap_or(&expecting);

    let mut index_in_seq = 0_usize;
    let let_values = vars.clone().zip(fields).map(|(var, field)| {
        if field.attrs.skip_deserializing() {
            let default = Expr(expr_is_missing(field, cattrs));
            quote! {
                let #var = #default;
            }
        } else {
            let visit = match field.attrs.deserialize_with() {
                None => {
                    let field_ty = field.ty;
                    let span = field.original.span();
                    let func =
                        quote_spanned!(span=> _serde::de::SeqAccess::next_element::<#field_ty>);
                    quote!(#func(&mut __seq)?)
                }
                Some(path) => {
                    let (wrapper, wrapper_ty) = wrap_deserialize_field_with(params, field.ty, path);
                    quote!({
                        #wrapper
                        _serde::#private::Option::map(
                            _serde::de::SeqAccess::next_element::<#wrapper_ty>(&mut __seq)?,
                            |__wrap| __wrap.value)
                    })
                }
            };
            let value_if_none = expr_is_missing_seq(None, index_in_seq, field, cattrs, expecting);
            let assign = quote! {
                let #var = match #visit {
                    _serde::#private::Some(__value) => __value,
                    _serde::#private::None => #value_if_none,
                };
            };
            index_in_seq += 1;
            assign
        }
    });

    let mut result = if is_struct {
        let names = fields.iter().map(|f| &f.member);
        quote! {
            #type_path { #( #names: #vars ),* }
        }
    } else {
        quote! {
            #type_path ( #(#vars),* )
        }
    };

    if params.has_getter {
        let this_type = &params.this_type;
        let (_, ty_generics, _) = params.generics.split_for_impl();
        result = quote! {
            _serde::#private::Into::<#this_type #ty_generics>::into(#result)
        };
    }

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

    quote_block! {
        #let_default
        #(#let_values)*
        _serde::#private::Ok(#result)
    }
}

#[cfg(feature = "deserialize_in_place")]
fn deserialize_seq_in_place(
    params: &Parameters,
    fields: &[Field],
    cattrs: &attr::Container,
    expecting: &str,
) -> Fragment {
    let deserialized_count = fields
        .iter()
        .filter(|field| !field.attrs.skip_deserializing())
        .count();
    let expecting = if deserialized_count == 1 {
        format!("{} with 1 element", expecting)
    } else {
        format!("{} with {} elements", expecting, deserialized_count)
    };
    let expecting = cattrs.expecting().unwrap_or(&expecting);

    let mut index_in_seq = 0usize;
    let write_values = fields.iter().map(|field| {
        let member = &field.member;

        if field.attrs.skip_deserializing() {
            let default = Expr(expr_is_missing(field, cattrs));
            quote! {
                self.place.#member = #default;
            }
        } else {
            let value_if_none = expr_is_missing_seq(Some(quote!(self.place.#member = )), index_in_seq, field, cattrs, expecting);
            let write = match field.attrs.deserialize_with() {
                None => {
                    quote! {
                        if let _serde::#private::None = _serde::de::SeqAccess::next_element_seed(&mut __seq,
                            _serde::#private::de::InPlaceSeed(&mut self.place.#member))?
                        {
                            #value_if_none;
                        }
                    }
                }
                Some(path) => {
                    let (wrapper, wrapper_ty) = wrap_deserialize_field_with(params, field.ty, path);
                    quote!({
                        #wrapper
                        match _serde::de::SeqAccess::next_element::<#wrapper_ty>(&mut __seq)? {
                            _serde::#private::Some(__wrap) => {
                                self.place.#member = __wrap.value;
                            }
                            _serde::#private::None => {
                                #value_if_none;
                            }
                        }
                    })
                }
            };
            index_in_seq += 1;
            write
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
        #let_default
        #(#write_values)*
        _serde::#private::Ok(())
    }
}

enum StructForm<'a> {
    Struct,
    /// Contains a variant name
    ExternallyTagged(&'a syn::Ident),
    /// Contains a variant name
    InternallyTagged(&'a syn::Ident),
    /// Contains a variant name
    Untagged(&'a syn::Ident),
}

struct FieldWithAliases<'a> {
    ident: Ident,
    aliases: &'a BTreeSet<Name>,
}

fn field_i(i: usize) -> Ident {
    Ident::new(&format!("__field{}", i), Span::call_site())
}

/// This function wraps the expression in `#[serde(deserialize_with = "...")]`
/// in a trait to prevent it from accessing the internal `Deserialize` state.
fn wrap_deserialize_with(
    params: &Parameters,
    value_ty: &TokenStream,
    deserialize_with: &syn::ExprPath,
) -> (TokenStream, TokenStream) {
    let this_type = &params.this_type;
    let (de_impl_generics, de_ty_generics, ty_generics, where_clause) =
        params.generics_with_de_lifetime();
    let delife = params.borrowed.de_lifetime();
    let deserializer_var = quote!(__deserializer);

    // If #deserialize_with returns wrong type, error will be reported here (^^^^^).
    // We attach span of the path to the function so it will be reported
    // on the #[serde(with = "...")]
    //                       ^^^^^
    let value = quote_spanned! {deserialize_with.span()=>
        #deserialize_with(#deserializer_var)?
    };
    let wrapper = quote! {
        #[doc(hidden)]
        struct __DeserializeWith #de_impl_generics #where_clause {
            value: #value_ty,
            phantom: _serde::#private::PhantomData<#this_type #ty_generics>,
            lifetime: _serde::#private::PhantomData<&#delife ()>,
        }

        #[automatically_derived]
        impl #de_impl_generics _serde::Deserialize<#delife> for __DeserializeWith #de_ty_generics #where_clause {
            fn deserialize<__D>(#deserializer_var: __D) -> _serde::#private::Result<Self, __D::Error>
            where
                __D: _serde::Deserializer<#delife>,
            {
                _serde::#private::Ok(__DeserializeWith {
                    value: #value,
                    phantom: _serde::#private::PhantomData,
                    lifetime: _serde::#private::PhantomData,
                })
            }
        }
    };

    let wrapper_ty = quote!(__DeserializeWith #de_ty_generics);

    (wrapper, wrapper_ty)
}

fn wrap_deserialize_field_with(
    params: &Parameters,
    field_ty: &syn::Type,
    deserialize_with: &syn::ExprPath,
) -> (TokenStream, TokenStream) {
    wrap_deserialize_with(params, &quote!(#field_ty), deserialize_with)
}

// Generates closure that converts single input parameter to the final value.
fn unwrap_to_variant_closure(
    params: &Parameters,
    variant: &Variant,
    with_wrapper: bool,
) -> TokenStream {
    let this_value = &params.this_value;
    let variant_ident = &variant.ident;

    let (arg, wrapper) = if with_wrapper {
        (quote! { __wrap }, quote! { __wrap.value })
    } else {
        let field_tys = variant.fields.iter().map(|field| field.ty);
        (quote! { __wrap: (#(#field_tys),*) }, quote! { __wrap })
    };

    let field_access = (0..variant.fields.len()).map(|n| {
        Member::Unnamed(Index {
            index: n as u32,
            span: Span::call_site(),
        })
    });

    match variant.style {
        Style::Struct if variant.fields.len() == 1 => {
            let member = &variant.fields[0].member;
            quote! {
                |#arg| #this_value::#variant_ident { #member: #wrapper }
            }
        }
        Style::Struct => {
            let members = variant.fields.iter().map(|field| &field.member);
            quote! {
                |#arg| #this_value::#variant_ident { #(#members: #wrapper.#field_access),* }
            }
        }
        Style::Tuple => quote! {
            |#arg| #this_value::#variant_ident(#(#wrapper.#field_access),*)
        },
        Style::Newtype => quote! {
            |#arg| #this_value::#variant_ident(#wrapper)
        },
        Style::Unit => quote! {
            |#arg| #this_value::#variant_ident
        },
    }
}

fn expr_is_missing(field: &Field, cattrs: &attr::Container) -> Fragment {
    match field.attrs.default() {
        attr::Default::Default => {
            let span = field.original.span();
            let func = quote_spanned!(span=> _serde::#private::Default::default);
            return quote_expr!(#func());
        }
        attr::Default::Path(path) => {
            // If #path returns wrong type, error will be reported here (^^^^^).
            // We attach span of the path to the function so it will be reported
            // on the #[serde(default = "...")]
            //                          ^^^^^
            return Fragment::Expr(quote_spanned!(path.span()=> #path()));
        }
        attr::Default::None => { /* below */ }
    }

    match *cattrs.default() {
        attr::Default::Default | attr::Default::Path(_) => {
            let member = &field.member;
            return quote_expr!(__default.#member);
        }
        attr::Default::None => { /* below */ }
    }

    let name = field.attrs.name().deserialize_name();
    match field.attrs.deserialize_with() {
        None => {
            let span = field.original.span();
            let func = quote_spanned!(span=> _serde::#private::de::missing_field);
            quote_expr! {
                #func(#name)?
            }
        }
        Some(_) => {
            quote_expr! {
                return _serde::#private::Err(<__A::Error as _serde::de::Error>::missing_field(#name))
            }
        }
    }
}

fn expr_is_missing_seq(
    assign_to: Option<TokenStream>,
    index: usize,
    field: &Field,
    cattrs: &attr::Container,
    expecting: &str,
) -> TokenStream {
    match field.attrs.default() {
        attr::Default::Default => {
            let span = field.original.span();
            return quote_spanned!(span=> #assign_to _serde::#private::Default::default());
        }
        attr::Default::Path(path) => {
            // If #path returns wrong type, error will be reported here (^^^^^).
            // We attach span of the path to the function so it will be reported
            // on the #[serde(default = "...")]
            //                          ^^^^^
            return quote_spanned!(path.span()=> #assign_to #path());
        }
        attr::Default::None => { /* below */ }
    }

    match *cattrs.default() {
        attr::Default::Default | attr::Default::Path(_) => {
            let member = &field.member;
            quote!(#assign_to __default.#member)
        }
        attr::Default::None => quote!(
            return _serde::#private::Err(_serde::de::Error::invalid_length(#index, &#expecting))
        ),
    }
}

fn effective_style(variant: &Variant) -> Style {
    match variant.style {
        Style::Newtype if variant.fields[0].attrs.skip_deserializing() => Style::Unit,
        other => other,
    }
}

/// True if there is any field with a `#[serde(flatten)]` attribute, other than
/// fields which are skipped.
fn has_flatten(fields: &[Field]) -> bool {
    fields
        .iter()
        .any(|field| field.attrs.flatten() && !field.attrs.skip_deserializing())
}

struct DeImplGenerics<'a>(&'a Parameters);
#[cfg(feature = "deserialize_in_place")]
struct InPlaceImplGenerics<'a>(&'a Parameters);

impl<'a> ToTokens for DeImplGenerics<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut generics = self.0.generics.clone();
        if let Some(de_lifetime) = self.0.borrowed.de_lifetime_param() {
            generics.params = Some(syn::GenericParam::Lifetime(de_lifetime))
                .into_iter()
                .chain(generics.params)
                .collect();
        }
        let (impl_generics, _, _) = generics.split_for_impl();
        impl_generics.to_tokens(tokens);
    }
}

#[cfg(feature = "deserialize_in_place")]
impl<'a> ToTokens for InPlaceImplGenerics<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let place_lifetime = place_lifetime();
        let mut generics = self.0.generics.clone();

        // Add lifetime for `&'place mut Self, and `'a: 'place`
        for param in &mut generics.params {
            match param {
                syn::GenericParam::Lifetime(param) => {
                    param.bounds.push(place_lifetime.lifetime.clone());
                }
                syn::GenericParam::Type(param) => {
                    param.bounds.push(syn::TypeParamBound::Lifetime(
                        place_lifetime.lifetime.clone(),
                    ));
                }
                syn::GenericParam::Const(_) => {}
            }
        }
        generics.params = Some(syn::GenericParam::Lifetime(place_lifetime))
            .into_iter()
            .chain(generics.params)
            .collect();
        if let Some(de_lifetime) = self.0.borrowed.de_lifetime_param() {
            generics.params = Some(syn::GenericParam::Lifetime(de_lifetime))
                .into_iter()
                .chain(generics.params)
                .collect();
        }
        let (impl_generics, _, _) = generics.split_for_impl();
        impl_generics.to_tokens(tokens);
    }
}

#[cfg(feature = "deserialize_in_place")]
impl<'a> DeImplGenerics<'a> {
    fn in_place(self) -> InPlaceImplGenerics<'a> {
        InPlaceImplGenerics(self.0)
    }
}

struct DeTypeGenerics<'a>(&'a Parameters);
#[cfg(feature = "deserialize_in_place")]
struct InPlaceTypeGenerics<'a>(&'a Parameters);

fn de_type_generics_to_tokens(
    mut generics: syn::Generics,
    borrowed: &BorrowedLifetimes,
    tokens: &mut TokenStream,
) {
    if borrowed.de_lifetime_param().is_some() {
        let def = syn::LifetimeParam {
            attrs: Vec::new(),
            lifetime: syn::Lifetime::new("'de", Span::call_site()),
            colon_token: None,
            bounds: Punctuated::new(),
        };
        // Prepend 'de lifetime to list of generics
        generics.params = Some(syn::GenericParam::Lifetime(def))
            .into_iter()
            .chain(generics.params)
            .collect();
    }
    let (_, ty_generics, _) = generics.split_for_impl();
    ty_generics.to_tokens(tokens);
}

impl<'a> ToTokens for DeTypeGenerics<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        de_type_generics_to_tokens(self.0.generics.clone(), &self.0.borrowed, tokens);
    }
}

#[cfg(feature = "deserialize_in_place")]
impl<'a> ToTokens for InPlaceTypeGenerics<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut generics = self.0.generics.clone();
        generics.params = Some(syn::GenericParam::Lifetime(place_lifetime()))
            .into_iter()
            .chain(generics.params)
            .collect();

        de_type_generics_to_tokens(generics, &self.0.borrowed, tokens);
    }
}

#[cfg(feature = "deserialize_in_place")]
impl<'a> DeTypeGenerics<'a> {
    fn in_place(self) -> InPlaceTypeGenerics<'a> {
        InPlaceTypeGenerics(self.0)
    }
}

#[cfg(feature = "deserialize_in_place")]
fn place_lifetime() -> syn::LifetimeParam {
    syn::LifetimeParam {
        attrs: Vec::new(),
        lifetime: syn::Lifetime::new("'place", Span::call_site()),
        colon_token: None,
        bounds: Punctuated::new(),
    }
}
