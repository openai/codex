//! Implementation of an [`Into`] derive macro.

use std::{
    any::{Any, TypeId},
    borrow::Cow,
    iter, slice,
};

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens as _};
use syn::{
    ext::IdentExt as _,
    parse::{discouraged::Speculative as _, Parse, ParseStream},
    punctuated::Punctuated,
    spanned::Spanned as _,
    token,
};

use crate::utils::{
    attr::{self, ParseMultiple as _},
    polyfill, Either, FieldsExt, Spanning,
};

/// Expands an [`Into`] derive macro.
pub fn expand(input: &syn::DeriveInput, _: &'static str) -> syn::Result<TokenStream> {
    let attr_name = format_ident!("into");

    let data = match &input.data {
        syn::Data::Struct(data) => Ok(data),
        syn::Data::Enum(e) => Err(syn::Error::new(
            e.enum_token.span(),
            "`Into` cannot be derived for enums",
        )),
        syn::Data::Union(u) => Err(syn::Error::new(
            u.union_token.span(),
            "`Into` cannot be derived for unions",
        )),
    }?;

    let struct_attr = StructAttribute::parse_attrs_with(
        &input.attrs,
        &attr_name,
        &ConsiderLegacySyntax {
            fields: &data.fields,
        },
    )?
    .map(Spanning::into_inner);

    let fields_data = data
        .fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let field_attr = FieldAttribute::parse_attrs_with(
                &f.attrs,
                &attr_name,
                &ConsiderLegacySyntax {
                    fields: slice::from_ref(f),
                },
            )?
            .map(Spanning::into_inner);

            let skip = field_attr
                .as_ref()
                .map(|attr| attr.skip.is_some())
                .unwrap_or(false);

            let convs = field_attr.and_then(|attr| attr.convs);

            Ok(((i, f, skip), convs))
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let (fields, fields_convs): (Vec<_>, Vec<_>) = fields_data.into_iter().unzip();

    let struct_attr = struct_attr.or_else(|| {
        fields_convs
            .iter()
            .all(Option::is_none)
            .then(ConversionsAttribute::default)
            .map(Either::Right)
    });

    let mut expansions: Vec<_> = fields
        .iter()
        .zip(fields_convs)
        .filter_map(|(&(i, field, _), convs)| {
            convs.map(|convs| Expansion {
                input_ident: &input.ident,
                input_generics: &input.generics,
                fields: vec![(i, field)],
                convs,
            })
        })
        .collect();
    if let Some(attr) = struct_attr {
        expansions.push(Expansion {
            input_ident: &input.ident,
            input_generics: &input.generics,
            fields: fields
                .into_iter()
                .filter_map(|(i, f, skip)| (!skip).then_some((i, f)))
                .collect(),
            convs: attr.into(),
        });
    }
    expansions.into_iter().map(Expansion::expand).collect()
}

/// Expansion of an [`Into`] derive macro, generating [`From`] implementations for a struct.
struct Expansion<'a> {
    /// [`syn::Ident`] of the struct.
    ///
    /// [`syn::Ident`]: struct@syn::Ident
    input_ident: &'a syn::Ident,

    /// [`syn::Generics`] of the struct.
    input_generics: &'a syn::Generics,

    /// Fields to convert from, along with their indices.
    fields: Vec<(usize, &'a syn::Field)>,

    /// Conversions to be generated.
    convs: ConversionsAttribute,
}

impl<'a> Expansion<'a> {
    fn expand(self) -> syn::Result<TokenStream> {
        let Self {
            input_ident,
            input_generics,
            fields,
            convs,
        } = self;

        let fields_idents: Vec<_> = fields
            .iter()
            .map(|(i, f)| {
                f.ident
                    .as_ref()
                    .map_or_else(|| Either::Left(syn::Index::from(*i)), Either::Right)
            })
            .collect();
        let fields_tys: Vec<_> = fields.iter().map(|(_, f)| &f.ty).collect();
        let fields_tuple = syn::Type::Tuple(syn::TypeTuple {
            paren_token: token::Paren::default(),
            elems: fields_tys.iter().cloned().cloned().collect(),
        });

        [
            (&convs.owned, false, false),
            (&convs.r#ref, true, false),
            (&convs.ref_mut, true, true),
        ]
        .into_iter()
        .filter(|(conv, _, _)| conv.consider_fields_ty || !conv.tys.is_empty())
        .map(|(conv, ref_, mut_)| {
            let lf = ref_.then(|| syn::Lifetime::new("'__derive_more_into", Span::call_site()));
            let r = ref_.then(token::And::default);
            let m = mut_.then(token::Mut::default);

            let gens = if let Some(lf) = lf.clone() {
                let mut gens = input_generics.clone();
                gens.params.push(syn::LifetimeParam::new(lf).into());
                Cow::Owned(gens)
            } else {
                Cow::Borrowed(input_generics)
            };
            let (impl_gens, _, where_clause) = gens.split_for_impl();
            let (_, ty_gens, _) = input_generics.split_for_impl();

            if conv.consider_fields_ty {
                Either::Left(iter::once(&fields_tuple))
            } else {
                Either::Right(iter::empty())
            }
            .chain(&conv.tys)
            .map(|out_ty| {
                let tys: Vec<_> = fields_tys.validate_type(out_ty)?.collect();

                Ok(quote! {
                    #[allow(clippy::unused_unit)]
                    #[automatically_derived]
                    impl #impl_gens derive_more::core::convert::From<#r #lf #m #input_ident #ty_gens>
                     for ( #( #r #lf #m #tys ),* ) #where_clause
                    {
                        #[inline]
                        fn from(value: #r #lf #m #input_ident #ty_gens) -> Self {
                            (#(
                                <#r #m #tys as derive_more::core::convert::From<_>>::from(
                                    #r #m value. #fields_idents
                                )
                            ),*)
                        }
                    }
                })
            })
            .collect::<syn::Result<TokenStream>>()
        })
        .collect()
    }
}

/// Representation of an [`Into`] derive macro struct container attribute.
///
/// ```rust,ignore
/// #[into]
/// #[into(<types>)]
/// #[into(owned(<types>), ref(<types>), ref_mut(<types>))]
/// ```
type StructAttribute = Either<attr::Empty, ConversionsAttribute>;

impl From<StructAttribute> for ConversionsAttribute {
    fn from(v: StructAttribute) -> Self {
        match v {
            Either::Left(_) => ConversionsAttribute::default(),
            Either::Right(c) => c,
        }
    }
}

type Untyped = Either<attr::Skip, Either<attr::Empty, ConversionsAttribute>>;
impl From<Untyped> for FieldAttribute {
    fn from(v: Untyped) -> Self {
        match v {
            Untyped::Left(skip) => Self {
                skip: Some(skip),
                convs: None,
            },
            Untyped::Right(c) => Self {
                skip: None,
                convs: Some(match c {
                    Either::Left(_empty) => ConversionsAttribute::default(),
                    Either::Right(convs) => convs,
                }),
            },
        }
    }
}

/// Representation of an [`Into`] derive macro field attribute.
///
/// ```rust,ignore
/// #[into]
/// #[into(<types>)]
/// #[into(owned(<types>), ref(<types>), ref_mut(<types>))]
/// #[into(skip)] #[into(ignore)]
/// ```
#[derive(Clone, Debug)]
struct FieldAttribute {
    skip: Option<attr::Skip>,
    convs: Option<ConversionsAttribute>,
}

impl Parse for FieldAttribute {
    fn parse(_: ParseStream<'_>) -> syn::Result<Self> {
        unreachable!("call `attr::ParseMultiple::parse_attr_with()` instead")
    }
}

impl attr::ParseMultiple for FieldAttribute {
    fn parse_attr_with<P: attr::Parser>(
        attr: &syn::Attribute,
        parser: &P,
    ) -> syn::Result<Self> {
        Untyped::parse_attr_with(attr, parser).map(Self::from)
    }

    fn merge_attrs(
        prev: Spanning<Self>,
        new: Spanning<Self>,
        name: &syn::Ident,
    ) -> syn::Result<Spanning<Self>> {
        let skip = attr::Skip::merge_opt_attrs(
            prev.clone().map(|v| v.skip).transpose(),
            new.clone().map(|v| v.skip).transpose(),
            name,
        )?
        .map(Spanning::into_inner);

        let convs = ConversionsAttribute::merge_opt_attrs(
            prev.clone().map(|v| v.convs).transpose(),
            new.clone().map(|v| v.convs).transpose(),
            name,
        )?
        .map(Spanning::into_inner);

        Ok(Spanning::new(
            Self { skip, convs },
            prev.span.join(new.span).unwrap_or(prev.span),
        ))
    }
}

/// [`Into`] conversions specified by a [`ConversionsAttribute`].
#[derive(Clone, Debug, Default)]
struct Conversions {
    /// Indicator whether these [`Conversions`] should contain a conversion into fields type.
    consider_fields_ty: bool,

    /// [`syn::Type`]s explicitly specified in a [`ConversionsAttribute`].
    tys: Punctuated<syn::Type, token::Comma>,
}

/// Representation of an [`Into`] derive macro attribute describing specified [`Into`] conversions.
///
/// ```rust,ignore
/// #[into(<types>)]
/// #[into(owned(<types>), ref(<types>), ref_mut(<types>))]
/// ```
#[derive(Clone, Debug)]
struct ConversionsAttribute {
    /// [`syn::Type`]s wrapped into `owned(...)` or simply `#[into(...)]`.
    owned: Conversions,

    /// [`syn::Type`]s wrapped into `ref(...)`.
    r#ref: Conversions,

    /// [`syn::Type`]s wrapped into `ref_mut(...)`.
    ref_mut: Conversions,
}

impl Default for ConversionsAttribute {
    fn default() -> Self {
        Self {
            owned: Conversions {
                consider_fields_ty: true,
                tys: Punctuated::new(),
            },
            r#ref: Conversions::default(),
            ref_mut: Conversions::default(),
        }
    }
}

impl Parse for ConversionsAttribute {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut out = Self {
            owned: Conversions::default(),
            r#ref: Conversions::default(),
            ref_mut: Conversions::default(),
        };

        let parse_inner = |ahead, convs: &mut Conversions| {
            input.advance_to(&ahead);

            if input.peek(token::Paren) {
                let inner;
                syn::parenthesized!(inner in input);

                convs.tys.extend(
                    inner
                        .parse_terminated(syn::Type::parse, token::Comma)?
                        .into_pairs(),
                );
            } else {
                convs.consider_fields_ty = true;
            }

            if input.peek(token::Comma) {
                let comma = input.parse::<token::Comma>()?;
                if !convs.tys.empty_or_trailing() {
                    convs.tys.push_punct(comma);
                }
            }

            Ok(())
        };

        let mut has_wrapped_type = false;
        let mut top_level_type = None;

        while !input.is_empty() {
            let ahead = input.fork();
            let res = if ahead.peek(syn::Ident::peek_any) {
                ahead.call(syn::Ident::parse_any).map(Into::into)
            } else {
                ahead.parse::<syn::Path>()
            };
            match res {
                Ok(p) if p.is_ident("owned") => {
                    has_wrapped_type = true;
                    parse_inner(ahead, &mut out.owned)?;
                }
                Ok(p) if p.is_ident("ref") => {
                    has_wrapped_type = true;
                    parse_inner(ahead, &mut out.r#ref)?;
                }
                Ok(p) if p.is_ident("ref_mut") => {
                    has_wrapped_type = true;
                    parse_inner(ahead, &mut out.ref_mut)?;
                }
                _ => {
                    let ty = input.parse::<syn::Type>()?;
                    let _ = top_level_type.get_or_insert_with(|| ty.clone());
                    out.owned.tys.push_value(ty);

                    if input.peek(token::Comma) {
                        out.owned.tys.push_punct(input.parse::<token::Comma>()?)
                    }
                }
            }
        }

        if let Some(ty) = top_level_type.filter(|_| has_wrapped_type) {
            Err(syn::Error::new(
                ty.span(),
                format!(
                    "mixing regular types with wrapped into `owned`/`ref`/`ref_mut` is not \
                     allowed, try wrapping this type into `owned({ty}), ref({ty}), ref_mut({ty})`",
                    ty = ty.into_token_stream(),
                ),
            ))
        } else {
            Ok(out)
        }
    }
}

impl attr::ParseMultiple for ConversionsAttribute {
    fn merge_attrs(
        prev: Spanning<Self>,
        new: Spanning<Self>,
        _: &syn::Ident,
    ) -> syn::Result<Spanning<Self>> {
        let Spanning {
            span: prev_span,
            item: mut prev,
        } = prev;
        let Spanning {
            span: new_span,
            item: new,
        } = new;

        prev.owned.tys.extend(new.owned.tys);
        prev.owned.consider_fields_ty |= new.owned.consider_fields_ty;
        prev.r#ref.tys.extend(new.r#ref.tys);
        prev.r#ref.consider_fields_ty |= new.r#ref.consider_fields_ty;
        prev.ref_mut.tys.extend(new.ref_mut.tys);
        prev.ref_mut.consider_fields_ty |= new.ref_mut.consider_fields_ty;

        Ok(Spanning::new(
            prev,
            prev_span.join(new_span).unwrap_or(prev_span),
        ))
    }
}

/// [`attr::Parser`] considering legacy syntax and performing [`check_legacy_syntax()`] for a
/// [`StructAttribute`] or a [`FieldAttribute`].
struct ConsiderLegacySyntax<F> {
    /// [`syn::Field`]s the [`StructAttribute`] or [`FieldAttribute`] is parsed for.
    fields: F,
}

impl<'a, F> attr::Parser for ConsiderLegacySyntax<&'a F>
where
    F: FieldsExt + ?Sized,
    &'a F: IntoIterator<Item = &'a syn::Field>,
{
    fn parse<T: Parse + Any>(&self, input: ParseStream<'_>) -> syn::Result<T> {
        if TypeId::of::<T>() == TypeId::of::<ConversionsAttribute>() {
            check_legacy_syntax(input, self.fields)?;
        }
        T::parse(input)
    }
}

/// [`Error`]ors for legacy syntax: `#[into(types(i32, "&str"))]`.
///
/// [`Error`]: syn::Error
fn check_legacy_syntax<'a, F>(tokens: ParseStream<'_>, fields: &'a F) -> syn::Result<()>
where
    F: FieldsExt + ?Sized,
    &'a F: IntoIterator<Item = &'a syn::Field>,
{
    let span = tokens.span();
    let tokens = tokens.fork();

    let map_ty = |s: String| {
        if fields.len() > 1 {
            format!(
                "({})",
                (0..fields.len())
                    .map(|_| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        } else {
            s
        }
    };
    let field = match fields.len() {
        0 => None,
        1 => Some(
            fields
                .into_iter()
                .next()
                .unwrap_or_else(|| unreachable!("fields.len() == 1"))
                .ty
                .to_token_stream()
                .to_string(),
        ),
        _ => Some(format!(
            "({})",
            fields
                .into_iter()
                .map(|f| f.ty.to_token_stream().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    };

    let Ok(metas) = tokens.parse_terminated(polyfill::Meta::parse, token::Comma) else {
        return Ok(());
    };

    let parse_list = |list: polyfill::MetaList, attrs: &mut Option<Vec<_>>| {
        if !list.path.is_ident("types") {
            return None;
        }
        for meta in list
            .parse_args_with(Punctuated::<_, token::Comma>::parse_terminated)
            .ok()?
        {
            attrs.get_or_insert_with(Vec::new).push(match meta {
                polyfill::NestedMeta::Lit(syn::Lit::Str(str)) => str.value(),
                polyfill::NestedMeta::Meta(polyfill::Meta::Path(path)) => {
                    path.into_token_stream().to_string()
                }
                _ => return None,
            })
        }
        Some(())
    };

    let Some((top_level, owned, ref_, ref_mut)) = metas
            .into_iter()
            .try_fold(
                (None, None, None, None),
                |(mut top_level, mut owned, mut ref_, mut ref_mut), meta| {
                    let is = |name| {
                        matches!(&meta, polyfill::Meta::Path(p) if p.is_ident(name))
                            || matches!(&meta, polyfill::Meta::List(list) if list.path.is_ident(name))
                    };
                    let parse_inner = |meta, attrs: &mut Option<_>| {
                        match meta {
                            polyfill::Meta::Path(_) => {
                                let _ = attrs.get_or_insert_with(Vec::new);
                                Some(())
                            }
                            polyfill::Meta::List(list) => {
                                if let polyfill::NestedMeta::Meta(polyfill::Meta::List(list)) = list
                                    .parse_args_with(Punctuated::<_, token::Comma>::parse_terminated)
                                    .ok()?
                                    .pop()?
                                    .into_value()
                                {
                                    parse_list(list, attrs)
                                } else {
                                    None
                                }
                            }
                        }
                    };

                    match meta {
                        meta if is("owned") => parse_inner(meta, &mut owned),
                        meta if is("ref") => parse_inner(meta, &mut ref_),
                        meta if is("ref_mut") => parse_inner(meta, &mut ref_mut),
                        polyfill::Meta::List(list) => parse_list(list, &mut top_level),
                        _ => None,
                    }
                    .map(|_| (top_level, owned, ref_, ref_mut))
                },
            )
            .filter(|(top_level, owned, ref_, ref_mut)| {
                [top_level, owned, ref_, ref_mut]
                    .into_iter()
                    .any(|l| l.as_ref().map_or(false, |l| !l.is_empty()))
            })
        else {
            return Ok(());
        };

    if [&owned, &ref_, &ref_mut].into_iter().any(Option::is_some) {
        let format = |list: Option<Vec<_>>, name: &str| match list {
            Some(l)
                if top_level.as_ref().map_or(true, Vec::is_empty) && l.is_empty() =>
            {
                Some(name.to_owned())
            }
            Some(l) => Some(format!(
                "{}({})",
                name,
                l.into_iter()
                    .chain(top_level.clone().into_iter().flatten())
                    .map(map_ty)
                    .chain(field.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
            )),
            None => None,
        };
        let format = [
            format(owned, "owned"),
            format(ref_, "ref"),
            format(ref_mut, "ref_mut"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");

        Err(syn::Error::new(
            span,
            format!("legacy syntax, use `{format}` instead"),
        ))
    } else {
        Err(syn::Error::new(
            span,
            format!(
                "legacy syntax, remove `types` and use `{}` instead",
                top_level.unwrap_or_else(|| unreachable!()).join(", "),
            ),
        ))
    }
}
