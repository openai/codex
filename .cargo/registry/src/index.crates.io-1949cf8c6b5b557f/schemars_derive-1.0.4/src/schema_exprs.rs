use crate::{ast::*, attr::WithAttr, idents::*};
use proc_macro2::{Span, TokenStream};
use quote::ToTokens;
use serde_derive_internals::ast::Style;
use serde_derive_internals::attr::{self as serde_attr, Default as SerdeDefault, TagType};
use syn::spanned::Spanned;

pub struct SchemaExpr {
    /// Definitions for types or functions that may be used within the creator or mutators
    definitions: Vec<TokenStream>,
    /// An expression that produces a `Schema`
    creator: TokenStream,
    /// Statements (including terminating semicolon) that mutate a var `schema` of type `Schema`
    mutators: Vec<TokenStream>,
}

impl From<TokenStream> for SchemaExpr {
    fn from(creator: TokenStream) -> Self {
        Self {
            definitions: Vec::new(),
            creator,
            mutators: Vec::new(),
        }
    }
}

impl ToTokens for SchemaExpr {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let Self {
            definitions,
            creator,
            mutators,
        } = self;

        tokens.extend(if mutators.is_empty() {
            quote!({
                #(#definitions)*
                #creator
            })
        } else {
            quote!({
                #(#definitions)*
                let mut #SCHEMA = #creator;
                #(#mutators)*
                #SCHEMA
            })
        });
    }
}

pub fn expr_for_container(cont: &Container) -> SchemaExpr {
    let type_from = cont
        .serde_attrs
        .type_from()
        .or(cont.serde_attrs.type_try_from());
    let type_into = cont.serde_attrs.type_into();

    let mut schema_expr = if let Some(with) = &cont.attrs.with {
        expr_for_container_with(cont, with)
    } else if let Some(transparent_field) = cont.transparent_field() {
        expr_for_newtype_struct(cont, transparent_field)
    } else if let (Some(from), Some(into)) = (type_from, type_into) {
        quote! {
            if #GENERATOR.contract().is_deserialize() {
                <#from as schemars::JsonSchema>::json_schema(#GENERATOR)
            } else {
                <#into as schemars::JsonSchema>::json_schema(#GENERATOR)
            }
        }
        .into()
    } else {
        let schema_expr = match &cont.data {
            Data::Struct(Style::Unit, _) => expr_for_unit_struct(),
            Data::Struct(Style::Newtype, fields) => expr_for_newtype_struct(cont, &fields[0]),
            Data::Struct(Style::Tuple, fields) => expr_for_tuple_struct(cont, fields),
            Data::Struct(Style::Struct, fields) => expr_for_struct(
                cont,
                fields,
                cont.serde_attrs.default(),
                cont.serde_attrs.deny_unknown_fields(),
            ),
            Data::Enum(variants) => expr_for_enum(cont, variants, &cont.serde_attrs),
        };

        if let Some(from) = type_from {
            quote! {
                if #GENERATOR.contract().is_deserialize() {
                    <#from as schemars::JsonSchema>::json_schema(#GENERATOR)
                } else {
                    #schema_expr
                }
            }
            .into()
        } else if let Some(into) = type_into {
            quote! {
                if #GENERATOR.contract().is_serialize() {
                    <#into as schemars::JsonSchema>::json_schema(#GENERATOR)
                } else {
                    #schema_expr
                }
            }
            .into()
        } else {
            schema_expr
        }
    };

    cont.add_mutators(&mut schema_expr.mutators);

    schema_expr
}

pub fn expr_for_repr(cont: &Container) -> Result<SchemaExpr, syn::Error> {
    let repr_type = cont.attrs.repr.as_ref().ok_or_else(|| {
        syn::Error::new(
            Span::call_site(),
            "JsonSchema_repr: missing #[repr(...)] attribute",
        )
    })?;

    let Data::Enum(variants) = &cont.data else {
        return Err(syn::Error::new(
            Span::call_site(),
            "JsonSchema_repr can only be used on enums",
        ));
    };

    if let Some(non_unit_error) = variants.iter().find_map(|v| match v.style {
        Style::Unit => None,
        _ => Some(syn::Error::new_spanned(
            v.original,
            "JsonSchema_repr: must be a unit variant",
        )),
    }) {
        return Err(non_unit_error);
    }

    let enum_ident = &cont.ident;
    let variant_idents = variants.iter().map(|v| &v.ident);

    let mut schema_expr = SchemaExpr::from(quote!({
        let mut map = schemars::_private::serde_json::Map::new();
        map.insert("type".into(), "integer".into());
        map.insert(
            "enum".into(),
            schemars::_private::serde_json::Value::Array({
                let mut enum_values = schemars::_private::alloc::vec::Vec::new();
                #(enum_values.push((#enum_ident::#variant_idents as #repr_type).into());)*
                enum_values
            }),
        );
        schemars::Schema::from(map)
    }));

    cont.add_mutators(&mut schema_expr.mutators);

    Ok(schema_expr)
}

fn expr_for_container_with(cont: &Container, with_attr: &WithAttr) -> SchemaExpr {
    let (ty, type_def) = type_for_schema(cont, with_attr);

    let mut schema_expr = SchemaExpr::from(quote! {
        <#ty as schemars::JsonSchema>::json_schema(#GENERATOR)
    });

    schema_expr.definitions.extend(type_def);

    schema_expr
}

fn expr_for_field(
    cont: &Container,
    field: &Field,
    is_internal_tagged_enum_newtype: bool,
) -> SchemaExpr {
    let (ty, type_def) = type_for_field_schema(cont, field);
    let span = field.original.span();

    let schema_expr = if field.attrs.validation.required {
        quote_spanned! {span=>
            <#ty as schemars::JsonSchema>::_schemars_private_non_optional_json_schema(#GENERATOR)
        }
    } else if is_internal_tagged_enum_newtype {
        quote_spanned! {span=>
            schemars::_private::json_schema_for_internally_tagged_enum_newtype_variant::<#ty>(#GENERATOR)
        }
    } else {
        quote_spanned! {span=>
            #GENERATOR.subschema_for::<#ty>()
        }
    };
    let mut schema_expr = SchemaExpr::from(schema_expr);

    schema_expr.definitions.extend(type_def);
    field.add_mutators(&mut schema_expr.mutators);

    schema_expr
}

fn type_for_field_schema(cont: &Container, field: &Field) -> (syn::Type, Option<TokenStream>) {
    match &field.attrs.with {
        None => (field.ty.clone(), None),
        Some(with_attr) => type_for_schema(cont, with_attr),
    }
}

fn type_for_schema(cont: &Container, with_attr: &WithAttr) -> (syn::Type, Option<TokenStream>) {
    match with_attr {
        WithAttr::Type(ty) => (ty.clone(), None),
        WithAttr::Function(fun) => {
            let cont_name = &cont.ident;
            let fn_name = fun.segments.last().unwrap().ident.to_string();
            let (impl_generics, ty_generics, where_clause) = cont.generics.split_for_impl();

            let type_def = quote_spanned! {fun.span()=>
                struct _SchemarsSchemaWithFunction<T: ?::core::marker::Sized>(::core::marker::PhantomData<T>);

                impl #impl_generics schemars::JsonSchema for _SchemarsSchemaWithFunction<#cont_name #ty_generics> #where_clause {
                    fn inline_schema() -> bool {
                        true
                    }

                    fn schema_name() -> schemars::_private::alloc::borrow::Cow<'static, str> {
                        schemars::_private::alloc::borrow::Cow::Borrowed(#fn_name)
                    }

                    fn schema_id() -> schemars::_private::alloc::borrow::Cow<'static, str> {
                        schemars::_private::alloc::borrow::Cow::Borrowed(::core::concat!(
                            "_SchemarsSchemaWithFunction/",
                            ::core::module_path!(),
                            "/",
                            ::core::stringify!(#fun)
                        ))
                    }

                    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
                        #fun(generator)
                    }
                }
            };

            (
                parse_quote!(_SchemarsSchemaWithFunction::<#cont_name #ty_generics>),
                Some(type_def),
            )
        }
    }
}

fn expr_for_enum(
    cont: &Container,
    variants: &[Variant],
    cattrs: &serde_attr::Container,
) -> SchemaExpr {
    if variants.is_empty() {
        return quote!(schemars::Schema::from(false)).into();
    }
    let deny_unknown_fields = cattrs.deny_unknown_fields();
    let variants = variants.iter();

    match cattrs.tag() {
        TagType::External => expr_for_external_tagged_enum(cont, variants, deny_unknown_fields),
        TagType::None => expr_for_untagged_enum(cont, variants, deny_unknown_fields),
        TagType::Internal { tag } => {
            expr_for_internal_tagged_enum(cont, variants, tag, deny_unknown_fields)
        }
        TagType::Adjacent { tag, content } => {
            expr_for_adjacent_tagged_enum(cont, variants, tag, content, deny_unknown_fields)
        }
    }
}

fn expr_for_external_tagged_enum<'a>(
    cont: &Container,
    variants: impl Iterator<Item = &'a Variant<'a>>,
    deny_unknown_fields: bool,
) -> SchemaExpr {
    let (unit_variants, complex_variants): (Vec<_>, Vec<_>) = variants.partition(|v| {
        v.is_unit() && v.attrs.is_default() && !v.serde_attrs.untagged() && !cont.attrs.ref_variants
    });
    let add_unit_names = unit_variants.iter().map(|v| {
        let name = v.name();
        v.with_contract_check(quote! {
            enum_values.push((#name).into());
        })
    });
    let unit_schema = SchemaExpr::from(quote!({
        let mut map = schemars::_private::serde_json::Map::new();
        map.insert("type".into(), "string".into());
        map.insert(
            "enum".into(),
            schemars::_private::serde_json::Value::Array({
                let mut enum_values = schemars::_private::alloc::vec::Vec::new();
                #(#add_unit_names)*
                enum_values
            }),
        );
        schemars::Schema::from(map)
    }));

    if complex_variants.is_empty() {
        return unit_schema;
    }

    let mut schemas = Vec::new();
    if !unit_variants.is_empty() {
        schemas.push((None, unit_schema));
    }

    schemas.extend(complex_variants.into_iter().map(|variant| {
        if variant.serde_attrs.untagged() {
            return (
                Some(variant),
                expr_for_untagged_enum_variant(cont, variant, deny_unknown_fields, true),
            );
        }

        let name = variant.name();

        let mut schema_expr =
            SchemaExpr::from(if variant.is_unit() && variant.attrs.with.is_none() {
                quote! {
                    schemars::_private::new_unit_enum_variant(#name)
                }
            } else {
                let sub_schema =
                    expr_for_untagged_enum_variant(cont, variant, deny_unknown_fields, false);
                quote! {
                    schemars::_private::new_externally_tagged_enum_variant(#name, #sub_schema)
                }
            });

        variant.add_mutators(&mut schema_expr.mutators);

        (Some(variant), schema_expr)
    }));

    variant_subschemas(cont, true, schemas)
}

fn expr_for_internal_tagged_enum<'a>(
    cont: &Container,
    variants: impl Iterator<Item = &'a Variant<'a>>,
    tag_name: &str,
    deny_unknown_fields: bool,
) -> SchemaExpr {
    let variant_schemas = variants
        .map(|variant| {
            if variant.serde_attrs.untagged() {
                return (Some(variant), expr_for_untagged_enum_variant(cont, variant, deny_unknown_fields, true))
            }

            let mut schema_expr = expr_for_internal_tagged_enum_variant(cont, variant, deny_unknown_fields);

            let name = variant.name();
            schema_expr.mutators.push(quote!(
                schemars::_private::apply_internal_enum_variant_tag(&mut #SCHEMA, #tag_name, #name, #deny_unknown_fields);
            ));

            variant.add_mutators(&mut schema_expr.mutators);

            (Some(variant), schema_expr)
        })
        .collect();

    variant_subschemas(cont, true, variant_schemas)
}

fn expr_for_untagged_enum<'a>(
    cont: &Container,
    variants: impl Iterator<Item = &'a Variant<'a>>,
    deny_unknown_fields: bool,
) -> SchemaExpr {
    let schemas = variants
        .map(|variant| {
            let schema_expr =
                expr_for_untagged_enum_variant(cont, variant, deny_unknown_fields, true);

            (Some(variant), schema_expr)
        })
        .collect();

    // Untagged enums can easily have variants whose schemas overlap; rather
    // that checking the exclusivity of each subschema we simply us `any_of`.
    variant_subschemas(cont, false, schemas)
}

fn expr_for_adjacent_tagged_enum<'a>(
    cont: &Container,
    variants: impl Iterator<Item = &'a Variant<'a>>,
    tag_name: &str,
    content_name: &str,
    deny_unknown_fields: bool,
) -> SchemaExpr {
    let schemas = variants
        .map(|variant| {
            if variant.serde_attrs.untagged() {
                return (
                    Some(variant),
                    expr_for_untagged_enum_variant(cont, variant, deny_unknown_fields, true),
                );
            }

            let content_schema = if variant.is_unit() && variant.attrs.with.is_none() {
                None
            } else {
                Some(expr_for_untagged_enum_variant(
                    cont,
                    variant,
                    deny_unknown_fields,
                    false,
                ))
            };

            let (add_content_to_props, add_content_to_required) = content_schema
                .map(|content_schema| {
                    (
                        quote!(#content_name: (#content_schema),),
                        quote!(#content_name,),
                    )
                })
                .unwrap_or_default();

            let name = variant.name();
            let tag_schema = quote! {
                schemars::json_schema!({
                    "type": "string",
                    "const": #name,
                })
            };

            let set_additional_properties = if deny_unknown_fields {
                quote! {
                    "additionalProperties": false,
                }
            } else {
                TokenStream::new()
            };

            let mut outer_schema = SchemaExpr::from(quote!(schemars::json_schema!({
                "type": "object",
                "properties": {
                    #tag_name: (#tag_schema),
                    #add_content_to_props
                },
                "required": [
                    #tag_name,
                    #add_content_to_required
                ],
                // As we're creating a "wrapper" object, we can honor the
                // disposition of deny_unknown_fields.
                #set_additional_properties
            })));

            variant.add_mutators(&mut outer_schema.mutators);

            (Some(variant), outer_schema)
        })
        .collect();

    variant_subschemas(cont, true, schemas)
}

/// Callers must determine if all subschemas are mutually exclusive. The current behaviour is to
/// assume that variants are mutually exclusive except for untagged enums.
fn variant_subschemas(
    cont: &Container,
    mut unique: bool,
    schemas: Vec<(Option<&Variant>, SchemaExpr)>,
) -> SchemaExpr {
    if schemas
        .iter()
        .any(|(v, _)| v.is_some_and(|v| v.serde_attrs.untagged()))
    {
        unique = false;
    }

    let keyword = if unique { "oneOf" } else { "anyOf" };
    let add_schemas = schemas.into_iter().map(|(variant, mut schema)| {
        if cont.attrs.ref_variants {
            schema = enum_ref_variants(cont, variant, schema);
        }

        let add = quote! {
            enum_values.push(#schema.to_value());
        };
        match variant {
            Some(v) => v.with_contract_check(add),
            None => add,
        }
    });
    quote!({
        let mut map = schemars::_private::serde_json::Map::new();
        map.insert(
            #keyword.into(),
            schemars::_private::serde_json::Value::Array({
                let mut enum_values = schemars::_private::alloc::vec::Vec::new();
                #(#add_schemas)*
                enum_values
            }),
        );
        schemars::Schema::from(map)
    })
    .into()
}

fn enum_ref_variants(cont: &Container, variant: Option<&Variant>, expr: SchemaExpr) -> SchemaExpr {
    let Some(variant) = variant else {
        return expr;
    };

    let cont_name = &cont.ident;
    // FIXME can this use serialize name where appropriate?
    let variant_name = variant.serde_attrs.name().deserialize_name();
    let (impl_generics, ty_generics, where_clause) = cont.generics.split_for_impl();

    let type_def = quote! {
        struct _SchemarsRefVariant<T: ?::core::marker::Sized>(::core::marker::PhantomData<T>);

        impl #impl_generics schemars::JsonSchema for _SchemarsRefVariant<#cont_name #ty_generics> #where_clause {
            fn inline_schema() -> bool {
                false
            }

            fn schema_name() -> schemars::_private::alloc::borrow::Cow<'static, str> {
                schemars::_private::alloc::borrow::Cow::Borrowed(#variant_name)
            }

            fn schema_id() -> schemars::_private::alloc::borrow::Cow<'static, str> {
                schemars::_private::alloc::borrow::Cow::Owned(
                    schemars::_private::alloc::format!(
                        "_SchemarsRefVariant/{}::{}",
                        <#cont_name #ty_generics as schemars::JsonSchema>::schema_id(),
                        #variant_name,
                ))
            }

            fn json_schema(#GENERATOR: &mut schemars::SchemaGenerator) -> schemars::Schema {
                #expr
            }
        }
    };

    let mut expr = SchemaExpr::from(
        quote!(#GENERATOR.subschema_for::<_SchemarsRefVariant::<#cont_name #ty_generics>>()),
    );

    expr.definitions.push(type_def);

    expr
}

// This function is also used for tagged variants, in which case the resulting SchemaExpr will be
// embedded or mutated to include the tag. `is_actually_untagged` will be true if the enum or the
// variant has the `untagged` attribute.
fn expr_for_untagged_enum_variant(
    cont: &Container,
    variant: &Variant,
    deny_unknown_fields: bool,
    is_actually_untagged: bool,
) -> SchemaExpr {
    let mut schema_expr = if let Some(with_attr) = &variant.attrs.with {
        let (ty, type_def) = type_for_schema(cont, with_attr);
        let mut schema_expr = SchemaExpr::from(quote_spanned! {variant.original.span()=>
            #GENERATOR.subschema_for::<#ty>()
        });

        schema_expr.definitions.extend(type_def);
        schema_expr
    } else {
        match variant.style {
            Style::Unit => expr_for_unit_struct(),
            Style::Newtype => expr_for_field(cont, &variant.fields[0], false),
            Style::Tuple => expr_for_tuple_struct(cont, &variant.fields),
            Style::Struct => expr_for_struct(
                cont,
                &variant.fields,
                &SerdeDefault::None,
                deny_unknown_fields,
            ),
        }
    };

    if is_actually_untagged {
        if variant.attrs.common.title.is_none() {
            let title = variant.name();
            schema_expr.mutators.push(quote! {
                if #GENERATOR.settings().untagged_enum_variant_titles {
                    #SCHEMA.insert("title".into(), #title.into());
                }
            });
        }

        variant.add_mutators(&mut schema_expr.mutators);
    }

    schema_expr
}

fn expr_for_internal_tagged_enum_variant(
    cont: &Container,
    variant: &Variant,
    deny_unknown_fields: bool,
) -> SchemaExpr {
    if let Some(with_attr) = &variant.attrs.with {
        let (ty, type_def) = type_for_schema(cont, with_attr);
        let mut schema_expr = SchemaExpr::from(quote_spanned! {variant.original.span()=>
            <#ty as schemars::JsonSchema>::json_schema(#GENERATOR)
        });

        schema_expr.definitions.extend(type_def);

        return schema_expr;
    }

    match variant.style {
        Style::Unit => expr_for_unit_struct(),
        Style::Newtype => expr_for_field(cont, &variant.fields[0], true),
        Style::Tuple => expr_for_tuple_struct(cont, &variant.fields),
        Style::Struct => expr_for_struct(
            cont,
            &variant.fields,
            &SerdeDefault::None,
            deny_unknown_fields,
        ),
    }
}

fn expr_for_unit_struct() -> SchemaExpr {
    quote! {
        #GENERATOR.subschema_for::<()>()
    }
    .into()
}

fn expr_for_newtype_struct(cont: &Container, field: &Field) -> SchemaExpr {
    expr_for_field(cont, field, false)
}

fn expr_for_tuple_struct(cont: &Container, fields: &[Field]) -> SchemaExpr {
    let fields: Vec<_> = fields
        .iter()
        .map(|f| {
            let field_expr = expr_for_field(cont, f, false);
            f.with_contract_check(quote! {
                prefix_items.push((#field_expr).to_value());
            })
        })
        .collect();

    let max_len = fields.len();

    // The length check must be done at runtime because it can vary based on the contract if any
    // items have `skip_serializing` or `skip_deserializing` attributes.
    quote!({
        let mut prefix_items = schemars::_private::alloc::vec::Vec::<schemars::_private::serde_json::Value>::with_capacity(#max_len);
        #(#fields)*
        let len = prefix_items.len();

        let mut map = schemars::_private::serde_json::Map::new();
        map.insert("type".into(), "array".into());
        if !prefix_items.is_empty() {
            map.insert("prefixItems".into(), prefix_items.into());
            map.insert("minItems".into(), len.into());
        }
        map.insert("maxItems".into(), len.into());

        schemars::Schema::from(map)
    })
    .into()
}

fn expr_for_struct(
    cont: &Container,
    fields: &[Field],
    default: &SerdeDefault,
    deny_unknown_fields: bool,
) -> SchemaExpr {
    let set_container_default = match default {
        SerdeDefault::None => None,
        SerdeDefault::Default => Some(quote!(let #STRUCT_DEFAULT = Self::default();)),
        SerdeDefault::Path(path) => Some(quote!(let #STRUCT_DEFAULT = #path();)),
    };

    // a vec of mutators
    let properties: Vec<TokenStream> = fields
        .iter()
        .filter(|f| !f.serde_attrs.skip_deserializing() || !f.serde_attrs.skip_serializing())
        .map(|field| {
            if field.serde_attrs.flatten() {
                let (ty, type_def) = type_for_field_schema(cont, field);

                let required = field.attrs.validation.required;
                let mut schema_expr = SchemaExpr::from(quote_spanned! {ty.span()=>
                    schemars::_private::json_schema_for_flatten::<#ty>(#GENERATOR, #required)
                });

                schema_expr.definitions.extend(type_def);

                field.with_contract_check(quote! {
                    schemars::_private::flatten(&mut #SCHEMA, #schema_expr);
                })
            } else {
                let mut schema_expr = expr_for_field(cont, field, false);

                if let Some(default) = field_default_expr(field, set_container_default.is_some()) {
                    schema_expr.mutators.push(quote! {
                        #default.and_then(|d| schemars::_schemars_maybe_to_value!(d))
                            .map(|d| #SCHEMA.insert("default".into(), d));
                    });
                }

                let name = field.name();
                let (ty, type_def) = type_for_field_schema(cont, field);

                if type_def.is_some() {
                    assert!(!schema_expr.definitions.is_empty());
                }

                let has_default = set_container_default.is_some() || !field.serde_attrs.default().is_none();
                let has_skip_serialize_if = field.serde_attrs.skip_serializing_if().is_some();
                let required_attr = field.attrs.validation.required;

                let is_optional = if has_skip_serialize_if && has_default {
                    quote!(true)
                } else if !has_skip_serialize_if && !has_default && !required_attr {
                    quote!(#GENERATOR.contract().is_deserialize() && <#ty as schemars::JsonSchema>::_schemars_private_is_option())
                } else {
                    quote!(if #GENERATOR.contract().is_deserialize() {
                        #has_default || (!#required_attr && <#ty as schemars::JsonSchema>::_schemars_private_is_option())
                    } else {
                        #has_skip_serialize_if
                    })
                };

                // Embed definitions outside of `#schema_expr`, because they may contain the
                // definition of the `#ty` type which is used in `#is_optional``
                let definitions = core::mem::take(&mut schema_expr.definitions);
                field.with_contract_check(quote!({
                    #(#definitions)*
                    schemars::_private::insert_object_property(&mut #SCHEMA, #name, #is_optional, #schema_expr);
                }))
            }
        })
        .collect();

    let set_additional_properties = if deny_unknown_fields {
        quote! {
            "additionalProperties": false,
        }
    } else {
        TokenStream::new()
    };

    SchemaExpr {
        definitions: set_container_default.into_iter().collect(),
        creator: quote!(schemars::json_schema!({
            "type": "object",
            #set_additional_properties
        })),
        mutators: properties,
    }
}

fn field_default_expr(field: &Field, container_has_default: bool) -> Option<TokenStream> {
    let field_default = field.serde_attrs.default();
    if field.serde_attrs.skip_serializing() || (field_default.is_none() && !container_has_default) {
        return None;
    }

    let ty = field.ty;
    let default_expr = match field_default {
        SerdeDefault::None => {
            let member = &field.member;
            quote!(#STRUCT_DEFAULT.#member)
        }
        SerdeDefault::Default => quote!(<#ty>::default()),
        SerdeDefault::Path(path) => quote!(#path()),
    };

    let default_expr = if let Some(skip_if) = field.serde_attrs.skip_serializing_if() {
        quote! {
            {
                let default = #default_expr;
                if #skip_if(&default) {
                    None
                } else {
                    Some(default)
                }
            }
        }
    } else {
        quote!(Some(#default_expr))
    };

    Some(if let Some(ser_with) = field.serde_attrs.serialize_with() {
        quote! {
            {
                struct _SchemarsDefaultSerialize<T>(T);

                impl serde::Serialize for _SchemarsDefaultSerialize<#ty>
                {
                    fn serialize<S>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error>
                    where
                        S: serde::Serializer
                    {
                        #ser_with(&self.0, serializer)
                    }
                }

                #default_expr.map(|d| _SchemarsDefaultSerialize(d))
            }
        }
    } else {
        default_expr
    })
}
