use crate::deprecated::allow_deprecated;
use crate::fragment::{Fragment, Match, Stmts};
use crate::internals::ast::{Container, Data, Field, Style, Variant};
use crate::internals::name::Name;
use crate::internals::{attr, replace_receiver, Ctxt, Derive};
use crate::{bound, dummy, pretend, private, this};
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{parse_quote, Ident, Index, Member};

pub fn expand_derive_serialize(input: &mut syn::DeriveInput) -> syn::Result<TokenStream> {
    replace_receiver(input);

    let ctxt = Ctxt::new();
    let cont = match Container::from_ast(&ctxt, input, Derive::Serialize, &private.ident()) {
        Some(cont) => cont,
        None => return Err(ctxt.check().unwrap_err()),
    };
    precondition(&ctxt, &cont);
    ctxt.check()?;

    let ident = &cont.ident;
    let params = Parameters::new(&cont);
    let (impl_generics, ty_generics, where_clause) = params.generics.split_for_impl();
    let body = Stmts(serialize_body(&cont, &params));
    let allow_deprecated = allow_deprecated(input);

    let impl_block = if let Some(remote) = cont.attrs.remote() {
        let vis = &input.vis;
        let used = pretend::pretend_used(&cont, params.is_packed);
        quote! {
            #[automatically_derived]
            #allow_deprecated
            impl #impl_generics #ident #ty_generics #where_clause {
                #vis fn serialize<__S>(__self: &#remote #ty_generics, __serializer: __S) -> _serde::#private::Result<__S::Ok, __S::Error>
                where
                    __S: _serde::Serializer,
                {
                    #used
                    #body
                }
            }
        }
    } else {
        quote! {
            #[automatically_derived]
            #allow_deprecated
            impl #impl_generics _serde::Serialize for #ident #ty_generics #where_clause {
                fn serialize<__S>(&self, __serializer: __S) -> _serde::#private::Result<__S::Ok, __S::Error>
                where
                    __S: _serde::Serializer,
                {
                    #body
                }
            }
        }
    };

    Ok(dummy::wrap_in_const(
        cont.attrs.custom_serde_path(),
        impl_block,
    ))
}

fn precondition(cx: &Ctxt, cont: &Container) {
    match cont.attrs.identifier() {
        attr::Identifier::No => {}
        attr::Identifier::Field => {
            cx.error_spanned_by(cont.original, "field identifiers cannot be serialized");
        }
        attr::Identifier::Variant => {
            cx.error_spanned_by(cont.original, "variant identifiers cannot be serialized");
        }
    }
}

struct Parameters {
    /// Variable holding the value being serialized. Either `self` for local
    /// types or `__self` for remote types.
    self_var: Ident,

    /// Path to the type the impl is for. Either a single `Ident` for local
    /// types (does not include generic parameters) or `some::remote::Path` for
    /// remote types.
    this_type: syn::Path,

    /// Same as `this_type` but using `::<T>` for generic parameters for use in
    /// expression position.
    this_value: syn::Path,

    /// Generics including any explicit and inferred bounds for the impl.
    generics: syn::Generics,

    /// Type has a `serde(remote = "...")` attribute.
    is_remote: bool,

    /// Type has a repr(packed) attribute.
    is_packed: bool,
}

impl Parameters {
    fn new(cont: &Container) -> Self {
        let is_remote = cont.attrs.remote().is_some();
        let self_var = if is_remote {
            Ident::new("__self", Span::call_site())
        } else {
            Ident::new("self", Span::call_site())
        };

        let this_type = this::this_type(cont);
        let this_value = this::this_value(cont);
        let is_packed = cont.attrs.is_packed();
        let generics = build_generics(cont);

        Parameters {
            self_var,
            this_type,
            this_value,
            generics,
            is_remote,
            is_packed,
        }
    }

    /// Type name to use in error messages and `&'static str` arguments to
    /// various Serializer methods.
    fn type_name(&self) -> String {
        self.this_type.segments.last().unwrap().ident.to_string()
    }
}

// All the generics in the input, plus a bound `T: Serialize` for each generic
// field type that will be serialized by us.
fn build_generics(cont: &Container) -> syn::Generics {
    let generics = bound::without_defaults(cont.generics);

    let generics =
        bound::with_where_predicates_from_fields(cont, &generics, attr::Field::ser_bound);

    let generics =
        bound::with_where_predicates_from_variants(cont, &generics, attr::Variant::ser_bound);

    match cont.attrs.ser_bound() {
        Some(predicates) => bound::with_where_predicates(&generics, predicates),
        None => bound::with_bound(
            cont,
            &generics,
            needs_serialize_bound,
            &parse_quote!(_serde::Serialize),
        ),
    }
}

// Fields with a `skip_serializing` or `serialize_with` attribute, or which
// belong to a variant with a `skip_serializing` or `serialize_with` attribute,
// are not serialized by us so we do not generate a bound. Fields with a `bound`
// attribute specify their own bound so we do not generate one. All other fields
// may need a `T: Serialize` bound where T is the type of the field.
fn needs_serialize_bound(field: &attr::Field, variant: Option<&attr::Variant>) -> bool {
    !field.skip_serializing()
        && field.serialize_with().is_none()
        && field.ser_bound().is_none()
        && variant.map_or(true, |variant| {
            !variant.skip_serializing()
                && variant.serialize_with().is_none()
                && variant.ser_bound().is_none()
        })
}

fn serialize_body(cont: &Container, params: &Parameters) -> Fragment {
    if cont.attrs.transparent() {
        serialize_transparent(cont, params)
    } else if let Some(type_into) = cont.attrs.type_into() {
        serialize_into(params, type_into)
    } else {
        match &cont.data {
            Data::Enum(variants) => serialize_enum(params, variants, &cont.attrs),
            Data::Struct(Style::Struct, fields) => serialize_struct(params, fields, &cont.attrs),
            Data::Struct(Style::Tuple, fields) => {
                serialize_tuple_struct(params, fields, &cont.attrs)
            }
            Data::Struct(Style::Newtype, fields) => {
                serialize_newtype_struct(params, &fields[0], &cont.attrs)
            }
            Data::Struct(Style::Unit, _) => serialize_unit_struct(&cont.attrs),
        }
    }
}

fn serialize_transparent(cont: &Container, params: &Parameters) -> Fragment {
    let fields = match &cont.data {
        Data::Struct(_, fields) => fields,
        Data::Enum(_) => unreachable!(),
    };

    let self_var = &params.self_var;
    let transparent_field = fields.iter().find(|f| f.attrs.transparent()).unwrap();
    let member = &transparent_field.member;

    let path = match transparent_field.attrs.serialize_with() {
        Some(path) => quote!(#path),
        None => {
            let span = transparent_field.original.span();
            quote_spanned!(span=> _serde::Serialize::serialize)
        }
    };

    quote_block! {
        #path(&#self_var.#member, __serializer)
    }
}

fn serialize_into(params: &Parameters, type_into: &syn::Type) -> Fragment {
    let self_var = &params.self_var;
    quote_block! {
        _serde::Serialize::serialize(
            &_serde::#private::Into::<#type_into>::into(_serde::#private::Clone::clone(#self_var)),
            __serializer)
    }
}

fn serialize_unit_struct(cattrs: &attr::Container) -> Fragment {
    let type_name = cattrs.name().serialize_name();

    quote_expr! {
        _serde::Serializer::serialize_unit_struct(__serializer, #type_name)
    }
}

fn serialize_newtype_struct(
    params: &Parameters,
    field: &Field,
    cattrs: &attr::Container,
) -> Fragment {
    let type_name = cattrs.name().serialize_name();

    let mut field_expr = get_member(
        params,
        field,
        &Member::Unnamed(Index {
            index: 0,
            span: Span::call_site(),
        }),
    );
    if let Some(path) = field.attrs.serialize_with() {
        field_expr = wrap_serialize_field_with(params, field.ty, path, &field_expr);
    }

    let span = field.original.span();
    let func = quote_spanned!(span=> _serde::Serializer::serialize_newtype_struct);
    quote_expr! {
        #func(__serializer, #type_name, #field_expr)
    }
}

fn serialize_tuple_struct(
    params: &Parameters,
    fields: &[Field],
    cattrs: &attr::Container,
) -> Fragment {
    let serialize_stmts =
        serialize_tuple_struct_visitor(fields, params, false, &TupleTrait::SerializeTupleStruct);

    let type_name = cattrs.name().serialize_name();

    let mut serialized_fields = fields
        .iter()
        .enumerate()
        .filter(|(_, field)| !field.attrs.skip_serializing())
        .peekable();

    let let_mut = mut_if(serialized_fields.peek().is_some());

    let len = serialized_fields
        .map(|(i, field)| match field.attrs.skip_serializing_if() {
            None => quote!(1),
            Some(path) => {
                let index = syn::Index {
                    index: i as u32,
                    span: Span::call_site(),
                };
                let field_expr = get_member(params, field, &Member::Unnamed(index));
                quote!(if #path(#field_expr) { 0 } else { 1 })
            }
        })
        .fold(quote!(0), |sum, expr| quote!(#sum + #expr));

    quote_block! {
        let #let_mut __serde_state = _serde::Serializer::serialize_tuple_struct(__serializer, #type_name, #len)?;
        #(#serialize_stmts)*
        _serde::ser::SerializeTupleStruct::end(__serde_state)
    }
}

fn serialize_struct(params: &Parameters, fields: &[Field], cattrs: &attr::Container) -> Fragment {
    assert!(
        fields.len() as u64 <= u64::from(u32::MAX),
        "too many fields in {}: {}, maximum supported count is {}",
        cattrs.name().serialize_name(),
        fields.len(),
        u32::MAX,
    );

    let has_non_skipped_flatten = fields
        .iter()
        .any(|field| field.attrs.flatten() && !field.attrs.skip_serializing());
    if has_non_skipped_flatten {
        serialize_struct_as_map(params, fields, cattrs)
    } else {
        serialize_struct_as_struct(params, fields, cattrs)
    }
}

fn serialize_struct_tag_field(cattrs: &attr::Container, struct_trait: &StructTrait) -> TokenStream {
    match cattrs.tag() {
        attr::TagType::Internal { tag } => {
            let type_name = cattrs.name().serialize_name();
            let func = struct_trait.serialize_field(Span::call_site());
            quote! {
                #func(&mut __serde_state, #tag, #type_name)?;
            }
        }
        _ => quote! {},
    }
}

fn serialize_struct_as_struct(
    params: &Parameters,
    fields: &[Field],
    cattrs: &attr::Container,
) -> Fragment {
    let serialize_fields =
        serialize_struct_visitor(fields, params, false, &StructTrait::SerializeStruct);

    let type_name = cattrs.name().serialize_name();

    let tag_field = serialize_struct_tag_field(cattrs, &StructTrait::SerializeStruct);
    let tag_field_exists = !tag_field.is_empty();

    let mut serialized_fields = fields
        .iter()
        .filter(|&field| !field.attrs.skip_serializing())
        .peekable();

    let let_mut = mut_if(serialized_fields.peek().is_some() || tag_field_exists);

    let len = serialized_fields
        .map(|field| match field.attrs.skip_serializing_if() {
            None => quote!(1),
            Some(path) => {
                let field_expr = get_member(params, field, &field.member);
                quote!(if #path(#field_expr) { 0 } else { 1 })
            }
        })
        .fold(
            quote!(#tag_field_exists as usize),
            |sum, expr| quote!(#sum + #expr),
        );

    quote_block! {
        let #let_mut __serde_state = _serde::Serializer::serialize_struct(__serializer, #type_name, #len)?;
        #tag_field
        #(#serialize_fields)*
        _serde::ser::SerializeStruct::end(__serde_state)
    }
}

fn serialize_struct_as_map(
    params: &Parameters,
    fields: &[Field],
    cattrs: &attr::Container,
) -> Fragment {
    let serialize_fields =
        serialize_struct_visitor(fields, params, false, &StructTrait::SerializeMap);

    let tag_field = serialize_struct_tag_field(cattrs, &StructTrait::SerializeMap);
    let tag_field_exists = !tag_field.is_empty();

    let mut serialized_fields = fields
        .iter()
        .filter(|&field| !field.attrs.skip_serializing())
        .peekable();

    let let_mut = mut_if(serialized_fields.peek().is_some() || tag_field_exists);

    quote_block! {
        let #let_mut __serde_state = _serde::Serializer::serialize_map(__serializer, _serde::#private::None)?;
        #tag_field
        #(#serialize_fields)*
        _serde::ser::SerializeMap::end(__serde_state)
    }
}

fn serialize_enum(params: &Parameters, variants: &[Variant], cattrs: &attr::Container) -> Fragment {
    assert!(variants.len() as u64 <= u64::from(u32::MAX));

    let self_var = &params.self_var;

    let mut arms: Vec<_> = variants
        .iter()
        .enumerate()
        .map(|(variant_index, variant)| {
            serialize_variant(params, variant, variant_index as u32, cattrs)
        })
        .collect();

    if cattrs.remote().is_some() && cattrs.non_exhaustive() {
        arms.push(quote! {
            ref unrecognized => _serde::#private::Err(_serde::ser::Error::custom(_serde::#private::ser::CannotSerializeVariant(unrecognized))),
        });
    }

    quote_expr! {
        match *#self_var {
            #(#arms)*
        }
    }
}

fn serialize_variant(
    params: &Parameters,
    variant: &Variant,
    variant_index: u32,
    cattrs: &attr::Container,
) -> TokenStream {
    let this_value = &params.this_value;
    let variant_ident = &variant.ident;

    if variant.attrs.skip_serializing() {
        let skipped_msg = format!(
            "the enum variant {}::{} cannot be serialized",
            params.type_name(),
            variant_ident
        );
        let skipped_err = quote! {
            _serde::#private::Err(_serde::ser::Error::custom(#skipped_msg))
        };
        let fields_pat = match variant.style {
            Style::Unit => quote!(),
            Style::Newtype | Style::Tuple => quote!((..)),
            Style::Struct => quote!({ .. }),
        };
        quote! {
            #this_value::#variant_ident #fields_pat => #skipped_err,
        }
    } else {
        // variant wasn't skipped
        let case = match variant.style {
            Style::Unit => {
                quote! {
                    #this_value::#variant_ident
                }
            }
            Style::Newtype => {
                quote! {
                    #this_value::#variant_ident(ref __field0)
                }
            }
            Style::Tuple => {
                let field_names = (0..variant.fields.len())
                    .map(|i| Ident::new(&format!("__field{}", i), Span::call_site()));
                quote! {
                    #this_value::#variant_ident(#(ref #field_names),*)
                }
            }
            Style::Struct => {
                let members = variant.fields.iter().map(|f| &f.member);
                quote! {
                    #this_value::#variant_ident { #(ref #members),* }
                }
            }
        };

        let body = Match(match (cattrs.tag(), variant.attrs.untagged()) {
            (attr::TagType::External, false) => {
                serialize_externally_tagged_variant(params, variant, variant_index, cattrs)
            }
            (attr::TagType::Internal { tag }, false) => {
                serialize_internally_tagged_variant(params, variant, cattrs, tag)
            }
            (attr::TagType::Adjacent { tag, content }, false) => {
                serialize_adjacently_tagged_variant(
                    params,
                    variant,
                    cattrs,
                    variant_index,
                    tag,
                    content,
                )
            }
            (attr::TagType::None, _) | (_, true) => {
                serialize_untagged_variant(params, variant, cattrs)
            }
        });

        quote! {
            #case => #body
        }
    }
}

fn serialize_externally_tagged_variant(
    params: &Parameters,
    variant: &Variant,
    variant_index: u32,
    cattrs: &attr::Container,
) -> Fragment {
    let type_name = cattrs.name().serialize_name();
    let variant_name = variant.attrs.name().serialize_name();

    if let Some(path) = variant.attrs.serialize_with() {
        let ser = wrap_serialize_variant_with(params, path, variant);
        return quote_expr! {
            _serde::Serializer::serialize_newtype_variant(
                __serializer,
                #type_name,
                #variant_index,
                #variant_name,
                #ser,
            )
        };
    }

    match effective_style(variant) {
        Style::Unit => {
            quote_expr! {
                _serde::Serializer::serialize_unit_variant(
                    __serializer,
                    #type_name,
                    #variant_index,
                    #variant_name,
                )
            }
        }
        Style::Newtype => {
            let field = &variant.fields[0];
            let mut field_expr = quote!(__field0);
            if let Some(path) = field.attrs.serialize_with() {
                field_expr = wrap_serialize_field_with(params, field.ty, path, &field_expr);
            }

            let span = field.original.span();
            let func = quote_spanned!(span=> _serde::Serializer::serialize_newtype_variant);
            quote_expr! {
                #func(
                    __serializer,
                    #type_name,
                    #variant_index,
                    #variant_name,
                    #field_expr,
                )
            }
        }
        Style::Tuple => serialize_tuple_variant(
            TupleVariant::ExternallyTagged {
                type_name,
                variant_index,
                variant_name,
            },
            params,
            &variant.fields,
        ),
        Style::Struct => serialize_struct_variant(
            StructVariant::ExternallyTagged {
                variant_index,
                variant_name,
            },
            params,
            &variant.fields,
            type_name,
        ),
    }
}

fn serialize_internally_tagged_variant(
    params: &Parameters,
    variant: &Variant,
    cattrs: &attr::Container,
    tag: &str,
) -> Fragment {
    let type_name = cattrs.name().serialize_name();
    let variant_name = variant.attrs.name().serialize_name();

    let enum_ident_str = params.type_name();
    let variant_ident_str = variant.ident.to_string();

    if let Some(path) = variant.attrs.serialize_with() {
        let ser = wrap_serialize_variant_with(params, path, variant);
        return quote_expr! {
            _serde::#private::ser::serialize_tagged_newtype(
                __serializer,
                #enum_ident_str,
                #variant_ident_str,
                #tag,
                #variant_name,
                #ser,
            )
        };
    }

    match effective_style(variant) {
        Style::Unit => {
            quote_block! {
                let mut __struct = _serde::Serializer::serialize_struct(
                    __serializer, #type_name, 1)?;
                _serde::ser::SerializeStruct::serialize_field(
                    &mut __struct, #tag, #variant_name)?;
                _serde::ser::SerializeStruct::end(__struct)
            }
        }
        Style::Newtype => {
            let field = &variant.fields[0];
            let mut field_expr = quote!(__field0);
            if let Some(path) = field.attrs.serialize_with() {
                field_expr = wrap_serialize_field_with(params, field.ty, path, &field_expr);
            }

            let span = field.original.span();
            let func = quote_spanned!(span=> _serde::#private::ser::serialize_tagged_newtype);
            quote_expr! {
                #func(
                    __serializer,
                    #enum_ident_str,
                    #variant_ident_str,
                    #tag,
                    #variant_name,
                    #field_expr,
                )
            }
        }
        Style::Struct => serialize_struct_variant(
            StructVariant::InternallyTagged { tag, variant_name },
            params,
            &variant.fields,
            type_name,
        ),
        Style::Tuple => unreachable!("checked in serde_derive_internals"),
    }
}

fn serialize_adjacently_tagged_variant(
    params: &Parameters,
    variant: &Variant,
    cattrs: &attr::Container,
    variant_index: u32,
    tag: &str,
    content: &str,
) -> Fragment {
    let this_type = &params.this_type;
    let type_name = cattrs.name().serialize_name();
    let variant_name = variant.attrs.name().serialize_name();
    let serialize_variant = quote! {
        &_serde::#private::ser::AdjacentlyTaggedEnumVariant {
            enum_name: #type_name,
            variant_index: #variant_index,
            variant_name: #variant_name,
        }
    };

    let inner = Stmts(if let Some(path) = variant.attrs.serialize_with() {
        let ser = wrap_serialize_variant_with(params, path, variant);
        quote_expr! {
            _serde::Serialize::serialize(#ser, __serializer)
        }
    } else {
        match effective_style(variant) {
            Style::Unit => {
                return quote_block! {
                    let mut __struct = _serde::Serializer::serialize_struct(
                        __serializer, #type_name, 1)?;
                    _serde::ser::SerializeStruct::serialize_field(
                        &mut __struct, #tag, #serialize_variant)?;
                    _serde::ser::SerializeStruct::end(__struct)
                };
            }
            Style::Newtype => {
                let field = &variant.fields[0];
                let mut field_expr = quote!(__field0);
                if let Some(path) = field.attrs.serialize_with() {
                    field_expr = wrap_serialize_field_with(params, field.ty, path, &field_expr);
                }

                let span = field.original.span();
                let func = quote_spanned!(span=> _serde::ser::SerializeStruct::serialize_field);
                return quote_block! {
                    let mut __struct = _serde::Serializer::serialize_struct(
                        __serializer, #type_name, 2)?;
                    _serde::ser::SerializeStruct::serialize_field(
                        &mut __struct, #tag, #serialize_variant)?;
                    #func(
                        &mut __struct, #content, #field_expr)?;
                    _serde::ser::SerializeStruct::end(__struct)
                };
            }
            Style::Tuple => {
                serialize_tuple_variant(TupleVariant::Untagged, params, &variant.fields)
            }
            Style::Struct => serialize_struct_variant(
                StructVariant::Untagged,
                params,
                &variant.fields,
                variant_name,
            ),
        }
    });

    let fields_ty = variant.fields.iter().map(|f| &f.ty);
    let fields_ident: &[_] = &match variant.style {
        Style::Unit => {
            if variant.attrs.serialize_with().is_some() {
                vec![]
            } else {
                unreachable!()
            }
        }
        Style::Newtype => vec![Member::Named(Ident::new("__field0", Span::call_site()))],
        Style::Tuple => (0..variant.fields.len())
            .map(|i| Member::Named(Ident::new(&format!("__field{}", i), Span::call_site())))
            .collect(),
        Style::Struct => variant.fields.iter().map(|f| f.member.clone()).collect(),
    };

    let (_, ty_generics, where_clause) = params.generics.split_for_impl();

    let wrapper_generics = if fields_ident.is_empty() {
        params.generics.clone()
    } else {
        bound::with_lifetime_bound(&params.generics, "'__a")
    };
    let (wrapper_impl_generics, wrapper_ty_generics, _) = wrapper_generics.split_for_impl();

    quote_block! {
        #[doc(hidden)]
        struct __AdjacentlyTagged #wrapper_generics #where_clause {
            data: (#(&'__a #fields_ty,)*),
            phantom: _serde::#private::PhantomData<#this_type #ty_generics>,
        }

        #[automatically_derived]
        impl #wrapper_impl_generics _serde::Serialize for __AdjacentlyTagged #wrapper_ty_generics #where_clause {
            fn serialize<__S>(&self, __serializer: __S) -> _serde::#private::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                // Elements that have skip_serializing will be unused.
                #[allow(unused_variables)]
                let (#(#fields_ident,)*) = self.data;
                #inner
            }
        }

        let mut __struct = _serde::Serializer::serialize_struct(
            __serializer, #type_name, 2)?;
        _serde::ser::SerializeStruct::serialize_field(
            &mut __struct, #tag, #serialize_variant)?;
        _serde::ser::SerializeStruct::serialize_field(
            &mut __struct, #content, &__AdjacentlyTagged {
                data: (#(#fields_ident,)*),
                phantom: _serde::#private::PhantomData::<#this_type #ty_generics>,
            })?;
        _serde::ser::SerializeStruct::end(__struct)
    }
}

fn serialize_untagged_variant(
    params: &Parameters,
    variant: &Variant,
    cattrs: &attr::Container,
) -> Fragment {
    if let Some(path) = variant.attrs.serialize_with() {
        let ser = wrap_serialize_variant_with(params, path, variant);
        return quote_expr! {
            _serde::Serialize::serialize(#ser, __serializer)
        };
    }

    match effective_style(variant) {
        Style::Unit => {
            quote_expr! {
                _serde::Serializer::serialize_unit(__serializer)
            }
        }
        Style::Newtype => {
            let field = &variant.fields[0];
            let mut field_expr = quote!(__field0);
            if let Some(path) = field.attrs.serialize_with() {
                field_expr = wrap_serialize_field_with(params, field.ty, path, &field_expr);
            }

            let span = field.original.span();
            let func = quote_spanned!(span=> _serde::Serialize::serialize);
            quote_expr! {
                #func(#field_expr, __serializer)
            }
        }
        Style::Tuple => serialize_tuple_variant(TupleVariant::Untagged, params, &variant.fields),
        Style::Struct => {
            let type_name = cattrs.name().serialize_name();
            serialize_struct_variant(StructVariant::Untagged, params, &variant.fields, type_name)
        }
    }
}

enum TupleVariant<'a> {
    ExternallyTagged {
        type_name: &'a Name,
        variant_index: u32,
        variant_name: &'a Name,
    },
    Untagged,
}

fn serialize_tuple_variant(
    context: TupleVariant,
    params: &Parameters,
    fields: &[Field],
) -> Fragment {
    let tuple_trait = match context {
        TupleVariant::ExternallyTagged { .. } => TupleTrait::SerializeTupleVariant,
        TupleVariant::Untagged => TupleTrait::SerializeTuple,
    };

    let serialize_stmts = serialize_tuple_struct_visitor(fields, params, true, &tuple_trait);

    let mut serialized_fields = fields
        .iter()
        .enumerate()
        .filter(|(_, field)| !field.attrs.skip_serializing())
        .peekable();

    let let_mut = mut_if(serialized_fields.peek().is_some());

    let len = serialized_fields
        .map(|(i, field)| match field.attrs.skip_serializing_if() {
            None => quote!(1),
            Some(path) => {
                let field_expr = Ident::new(&format!("__field{}", i), Span::call_site());
                quote!(if #path(#field_expr) { 0 } else { 1 })
            }
        })
        .fold(quote!(0), |sum, expr| quote!(#sum + #expr));

    match context {
        TupleVariant::ExternallyTagged {
            type_name,
            variant_index,
            variant_name,
        } => {
            quote_block! {
                let #let_mut __serde_state = _serde::Serializer::serialize_tuple_variant(
                    __serializer,
                    #type_name,
                    #variant_index,
                    #variant_name,
                    #len)?;
                #(#serialize_stmts)*
                _serde::ser::SerializeTupleVariant::end(__serde_state)
            }
        }
        TupleVariant::Untagged => {
            quote_block! {
                let #let_mut __serde_state = _serde::Serializer::serialize_tuple(
                    __serializer,
                    #len)?;
                #(#serialize_stmts)*
                _serde::ser::SerializeTuple::end(__serde_state)
            }
        }
    }
}

enum StructVariant<'a> {
    ExternallyTagged {
        variant_index: u32,
        variant_name: &'a Name,
    },
    InternallyTagged {
        tag: &'a str,
        variant_name: &'a Name,
    },
    Untagged,
}

fn serialize_struct_variant(
    context: StructVariant,
    params: &Parameters,
    fields: &[Field],
    name: &Name,
) -> Fragment {
    if fields.iter().any(|field| field.attrs.flatten()) {
        return serialize_struct_variant_with_flatten(context, params, fields, name);
    }

    let struct_trait = match context {
        StructVariant::ExternallyTagged { .. } => StructTrait::SerializeStructVariant,
        StructVariant::InternallyTagged { .. } | StructVariant::Untagged => {
            StructTrait::SerializeStruct
        }
    };

    let serialize_fields = serialize_struct_visitor(fields, params, true, &struct_trait);

    let mut serialized_fields = fields
        .iter()
        .filter(|&field| !field.attrs.skip_serializing())
        .peekable();

    let let_mut = mut_if(serialized_fields.peek().is_some());

    let len = serialized_fields
        .map(|field| {
            let member = &field.member;

            match field.attrs.skip_serializing_if() {
                Some(path) => quote!(if #path(#member) { 0 } else { 1 }),
                None => quote!(1),
            }
        })
        .fold(quote!(0), |sum, expr| quote!(#sum + #expr));

    match context {
        StructVariant::ExternallyTagged {
            variant_index,
            variant_name,
        } => {
            quote_block! {
                let #let_mut __serde_state = _serde::Serializer::serialize_struct_variant(
                    __serializer,
                    #name,
                    #variant_index,
                    #variant_name,
                    #len,
                )?;
                #(#serialize_fields)*
                _serde::ser::SerializeStructVariant::end(__serde_state)
            }
        }
        StructVariant::InternallyTagged { tag, variant_name } => {
            quote_block! {
                let mut __serde_state = _serde::Serializer::serialize_struct(
                    __serializer,
                    #name,
                    #len + 1,
                )?;
                _serde::ser::SerializeStruct::serialize_field(
                    &mut __serde_state,
                    #tag,
                    #variant_name,
                )?;
                #(#serialize_fields)*
                _serde::ser::SerializeStruct::end(__serde_state)
            }
        }
        StructVariant::Untagged => {
            quote_block! {
                let #let_mut __serde_state = _serde::Serializer::serialize_struct(
                    __serializer,
                    #name,
                    #len,
                )?;
                #(#serialize_fields)*
                _serde::ser::SerializeStruct::end(__serde_state)
            }
        }
    }
}

fn serialize_struct_variant_with_flatten(
    context: StructVariant,
    params: &Parameters,
    fields: &[Field],
    name: &Name,
) -> Fragment {
    let struct_trait = StructTrait::SerializeMap;
    let serialize_fields = serialize_struct_visitor(fields, params, true, &struct_trait);

    let mut serialized_fields = fields
        .iter()
        .filter(|&field| !field.attrs.skip_serializing())
        .peekable();

    let let_mut = mut_if(serialized_fields.peek().is_some());

    match context {
        StructVariant::ExternallyTagged {
            variant_index,
            variant_name,
        } => {
            let this_type = &params.this_type;
            let fields_ty = fields.iter().map(|f| &f.ty);
            let members = &fields.iter().map(|f| &f.member).collect::<Vec<_>>();

            let (_, ty_generics, where_clause) = params.generics.split_for_impl();
            let wrapper_generics = bound::with_lifetime_bound(&params.generics, "'__a");
            let (wrapper_impl_generics, wrapper_ty_generics, _) = wrapper_generics.split_for_impl();

            quote_block! {
                #[doc(hidden)]
                struct __EnumFlatten #wrapper_generics #where_clause {
                    data: (#(&'__a #fields_ty,)*),
                    phantom: _serde::#private::PhantomData<#this_type #ty_generics>,
                }

                #[automatically_derived]
                impl #wrapper_impl_generics _serde::Serialize for __EnumFlatten #wrapper_ty_generics #where_clause {
                    fn serialize<__S>(&self, __serializer: __S) -> _serde::#private::Result<__S::Ok, __S::Error>
                    where
                        __S: _serde::Serializer,
                    {
                        let (#(#members,)*) = self.data;
                        let #let_mut __serde_state = _serde::Serializer::serialize_map(
                            __serializer,
                            _serde::#private::None)?;
                        #(#serialize_fields)*
                        _serde::ser::SerializeMap::end(__serde_state)
                    }
                }

                _serde::Serializer::serialize_newtype_variant(
                    __serializer,
                    #name,
                    #variant_index,
                    #variant_name,
                    &__EnumFlatten {
                        data: (#(#members,)*),
                        phantom: _serde::#private::PhantomData::<#this_type #ty_generics>,
                    })
            }
        }
        StructVariant::InternallyTagged { tag, variant_name } => {
            quote_block! {
                let #let_mut __serde_state = _serde::Serializer::serialize_map(
                    __serializer,
                    _serde::#private::None)?;
                _serde::ser::SerializeMap::serialize_entry(
                    &mut __serde_state,
                    #tag,
                    #variant_name,
                )?;
                #(#serialize_fields)*
                _serde::ser::SerializeMap::end(__serde_state)
            }
        }
        StructVariant::Untagged => {
            quote_block! {
                let #let_mut __serde_state = _serde::Serializer::serialize_map(
                    __serializer,
                    _serde::#private::None)?;
                #(#serialize_fields)*
                _serde::ser::SerializeMap::end(__serde_state)
            }
        }
    }
}

fn serialize_tuple_struct_visitor(
    fields: &[Field],
    params: &Parameters,
    is_enum: bool,
    tuple_trait: &TupleTrait,
) -> Vec<TokenStream> {
    fields
        .iter()
        .enumerate()
        .filter(|(_, field)| !field.attrs.skip_serializing())
        .map(|(i, field)| {
            let mut field_expr = if is_enum {
                let id = Ident::new(&format!("__field{}", i), Span::call_site());
                quote!(#id)
            } else {
                get_member(
                    params,
                    field,
                    &Member::Unnamed(Index {
                        index: i as u32,
                        span: Span::call_site(),
                    }),
                )
            };

            let skip = field
                .attrs
                .skip_serializing_if()
                .map(|path| quote!(#path(#field_expr)));

            if let Some(path) = field.attrs.serialize_with() {
                field_expr = wrap_serialize_field_with(params, field.ty, path, &field_expr);
            }

            let span = field.original.span();
            let func = tuple_trait.serialize_element(span);
            let ser = quote! {
                #func(&mut __serde_state, #field_expr)?;
            };

            match skip {
                None => ser,
                Some(skip) => quote!(if !#skip { #ser }),
            }
        })
        .collect()
}

fn serialize_struct_visitor(
    fields: &[Field],
    params: &Parameters,
    is_enum: bool,
    struct_trait: &StructTrait,
) -> Vec<TokenStream> {
    fields
        .iter()
        .filter(|&field| !field.attrs.skip_serializing())
        .map(|field| {
            let member = &field.member;

            let mut field_expr = if is_enum {
                quote!(#member)
            } else {
                get_member(params, field, member)
            };

            let key_expr = field.attrs.name().serialize_name();

            let skip = field
                .attrs
                .skip_serializing_if()
                .map(|path| quote!(#path(#field_expr)));

            if let Some(path) = field.attrs.serialize_with() {
                field_expr = wrap_serialize_field_with(params, field.ty, path, &field_expr);
            }

            let span = field.original.span();
            let ser = if field.attrs.flatten() {
                let func = quote_spanned!(span=> _serde::Serialize::serialize);
                quote! {
                    #func(&#field_expr, _serde::#private::ser::FlatMapSerializer(&mut __serde_state))?;
                }
            } else {
                let func = struct_trait.serialize_field(span);
                quote! {
                    #func(&mut __serde_state, #key_expr, #field_expr)?;
                }
            };

            match skip {
                None => ser,
                Some(skip) => {
                    if let Some(skip_func) = struct_trait.skip_field(span) {
                        quote! {
                            if !#skip {
                                #ser
                            } else {
                                #skip_func(&mut __serde_state, #key_expr)?;
                            }
                        }
                    } else {
                        quote! {
                            if !#skip {
                                #ser
                            }
                        }
                    }
                }
            }
        })
        .collect()
}

fn wrap_serialize_field_with(
    params: &Parameters,
    field_ty: &syn::Type,
    serialize_with: &syn::ExprPath,
    field_expr: &TokenStream,
) -> TokenStream {
    wrap_serialize_with(params, serialize_with, &[field_ty], &[quote!(#field_expr)])
}

fn wrap_serialize_variant_with(
    params: &Parameters,
    serialize_with: &syn::ExprPath,
    variant: &Variant,
) -> TokenStream {
    let field_tys: Vec<_> = variant.fields.iter().map(|field| field.ty).collect();
    let field_exprs: Vec<_> = variant
        .fields
        .iter()
        .map(|field| {
            let id = match &field.member {
                Member::Named(ident) => ident.clone(),
                Member::Unnamed(member) => {
                    Ident::new(&format!("__field{}", member.index), Span::call_site())
                }
            };
            quote!(#id)
        })
        .collect();
    wrap_serialize_with(
        params,
        serialize_with,
        field_tys.as_slice(),
        field_exprs.as_slice(),
    )
}

fn wrap_serialize_with(
    params: &Parameters,
    serialize_with: &syn::ExprPath,
    field_tys: &[&syn::Type],
    field_exprs: &[TokenStream],
) -> TokenStream {
    let this_type = &params.this_type;
    let (_, ty_generics, where_clause) = params.generics.split_for_impl();

    let wrapper_generics = if field_exprs.is_empty() {
        params.generics.clone()
    } else {
        bound::with_lifetime_bound(&params.generics, "'__a")
    };
    let (wrapper_impl_generics, wrapper_ty_generics, _) = wrapper_generics.split_for_impl();

    let field_access = (0..field_exprs.len()).map(|n| {
        Member::Unnamed(Index {
            index: n as u32,
            span: Span::call_site(),
        })
    });

    let self_var = quote!(self);
    let serializer_var = quote!(__s);

    // If #serialize_with returns wrong type, error will be reported on here.
    // We attach span of the path to this piece so error will be reported
    // on the #[serde(with = "...")]
    //                       ^^^^^
    let wrapper_serialize = quote_spanned! {serialize_with.span()=>
        #serialize_with(#(#self_var.values.#field_access, )* #serializer_var)
    };

    quote!(&{
        #[doc(hidden)]
        struct __SerializeWith #wrapper_impl_generics #where_clause {
            values: (#(&'__a #field_tys, )*),
            phantom: _serde::#private::PhantomData<#this_type #ty_generics>,
        }

        #[automatically_derived]
        impl #wrapper_impl_generics _serde::Serialize for __SerializeWith #wrapper_ty_generics #where_clause {
            fn serialize<__S>(&#self_var, #serializer_var: __S) -> _serde::#private::Result<__S::Ok, __S::Error>
            where
                __S: _serde::Serializer,
            {
                #wrapper_serialize
            }
        }

        __SerializeWith {
            values: (#(#field_exprs, )*),
            phantom: _serde::#private::PhantomData::<#this_type #ty_generics>,
        }
    })
}

// Serialization of an empty struct results in code like:
//
//     let mut __serde_state = serializer.serialize_struct("S", 0)?;
//     _serde::ser::SerializeStruct::end(__serde_state)
//
// where we want to omit the `mut` to avoid a warning.
fn mut_if(is_mut: bool) -> Option<TokenStream> {
    if is_mut {
        Some(quote!(mut))
    } else {
        None
    }
}

fn get_member(params: &Parameters, field: &Field, member: &Member) -> TokenStream {
    let self_var = &params.self_var;
    match (params.is_remote, field.attrs.getter()) {
        (false, None) => {
            if params.is_packed {
                quote!(&{#self_var.#member})
            } else {
                quote!(&#self_var.#member)
            }
        }
        (true, None) => {
            let inner = if params.is_packed {
                quote!(&{#self_var.#member})
            } else {
                quote!(&#self_var.#member)
            };
            let ty = field.ty;
            quote!(_serde::#private::ser::constrain::<#ty>(#inner))
        }
        (true, Some(getter)) => {
            let ty = field.ty;
            quote!(_serde::#private::ser::constrain::<#ty>(&#getter(#self_var)))
        }
        (false, Some(_)) => {
            unreachable!("getter is only allowed for remote impls");
        }
    }
}

fn effective_style(variant: &Variant) -> Style {
    match variant.style {
        Style::Newtype if variant.fields[0].attrs.skip_serializing() => Style::Unit,
        other => other,
    }
}

enum StructTrait {
    SerializeMap,
    SerializeStruct,
    SerializeStructVariant,
}

impl StructTrait {
    fn serialize_field(&self, span: Span) -> TokenStream {
        match *self {
            StructTrait::SerializeMap => {
                quote_spanned!(span=> _serde::ser::SerializeMap::serialize_entry)
            }
            StructTrait::SerializeStruct => {
                quote_spanned!(span=> _serde::ser::SerializeStruct::serialize_field)
            }
            StructTrait::SerializeStructVariant => {
                quote_spanned!(span=> _serde::ser::SerializeStructVariant::serialize_field)
            }
        }
    }

    fn skip_field(&self, span: Span) -> Option<TokenStream> {
        match *self {
            StructTrait::SerializeMap => None,
            StructTrait::SerializeStruct => {
                Some(quote_spanned!(span=> _serde::ser::SerializeStruct::skip_field))
            }
            StructTrait::SerializeStructVariant => {
                Some(quote_spanned!(span=> _serde::ser::SerializeStructVariant::skip_field))
            }
        }
    }
}

enum TupleTrait {
    SerializeTuple,
    SerializeTupleStruct,
    SerializeTupleVariant,
}

impl TupleTrait {
    fn serialize_element(&self, span: Span) -> TokenStream {
        match *self {
            TupleTrait::SerializeTuple => {
                quote_spanned!(span=> _serde::ser::SerializeTuple::serialize_element)
            }
            TupleTrait::SerializeTupleStruct => {
                quote_spanned!(span=> _serde::ser::SerializeTupleStruct::serialize_field)
            }
            TupleTrait::SerializeTupleVariant => {
                quote_spanned!(span=> _serde::ser::SerializeTupleVariant::serialize_field)
            }
        }
    }
}
