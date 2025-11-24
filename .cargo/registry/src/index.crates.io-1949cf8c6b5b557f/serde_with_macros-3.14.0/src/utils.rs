use crate::lazy_bool::LazyBool;
use darling::FromDeriveInput;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::ToTokens;
use std::ops::{BitAnd, BitOr, Not};
use syn::{
    parse::{Parse, ParseStream},
    parse_quote,
    punctuated::Punctuated,
    Attribute, DeriveInput, Generics, Meta, Path, PathSegment, Token, TypeGenerics, WhereClause,
};

/// Merge multiple [`syn::Error`] into one.
pub(crate) trait IteratorExt {
    fn collect_error(self) -> syn::Result<()>
    where
        Self: Iterator<Item = syn::Result<()>> + Sized,
    {
        let accu = Ok(());
        self.fold(accu, |accu, error| match (accu, error) {
            (Ok(()), error) => error,
            (accu, Ok(())) => accu,
            (Err(mut err), Err(error)) => {
                err.combine(error);
                Err(err)
            }
        })
    }
}
impl<I> IteratorExt for I where I: Iterator<Item = syn::Result<()>> + Sized {}

/// Attributes usable for derive macros
#[derive(FromDeriveInput)]
#[darling(attributes(serde_with))]
pub(crate) struct DeriveOptions {
    /// Path to the crate
    #[darling(rename = "crate", default)]
    pub(crate) alt_crate_path: Option<Path>,
}

impl DeriveOptions {
    pub(crate) fn from_derive_input(input: &DeriveInput) -> Result<Self, TokenStream> {
        match <Self as FromDeriveInput>::from_derive_input(input) {
            Ok(v) => Ok(v),
            Err(e) => Err(TokenStream::from(e.write_errors())),
        }
    }

    pub(crate) fn get_serde_with_path(&self) -> Path {
        self.alt_crate_path
            .clone()
            .unwrap_or_else(|| syn::parse_str("::serde_with").unwrap())
    }
}

// Inspired by https://github.com/serde-rs/serde/blob/fb2fe409c8f7ad6c95e3096e5e9ede865c8cfb49/serde_derive/src/de.rs#L3120
// Serde is also licensed Apache 2 + MIT
pub(crate) fn split_with_de_lifetime(
    generics: &Generics,
) -> (DeImplGenerics<'_>, TypeGenerics<'_>, Option<&WhereClause>) {
    let de_impl_generics = DeImplGenerics(generics);
    let (_, ty_generics, where_clause) = generics.split_for_impl();
    (de_impl_generics, ty_generics, where_clause)
}

pub(crate) struct DeImplGenerics<'a>(&'a Generics);

impl ToTokens for DeImplGenerics<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let mut generics = self.0.clone();
        generics.params = Some(parse_quote!('de))
            .into_iter()
            .chain(generics.params)
            .collect();
        let (impl_generics, _, _) = generics.split_for_impl();
        impl_generics.to_tokens(tokens);
    }
}

/// Represents the macro body of a `#[cfg_attr]` attribute.
///
/// ```text
/// #[cfg_attr(feature = "things", derive(Macro))]
///            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
/// ```
struct CfgAttr {
    condition: Meta,
    _comma: Token![,],
    metas: Punctuated<Meta, Token![,]>,
}

impl Parse for CfgAttr {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        Ok(Self {
            condition: input.parse()?,
            _comma: input.parse()?,
            metas: Punctuated::parse_terminated(input)?,
        })
    }
}

/// Determine if there is a `#[derive(JsonSchema)]` on this struct.
pub(crate) fn has_derive_jsonschema(input: TokenStream) -> syn::Result<SchemaFieldConfig> {
    fn parse_derive_args(input: ParseStream<'_>) -> syn::Result<Punctuated<Path, Token![,]>> {
        Punctuated::parse_terminated_with(input, Path::parse_mod_style)
    }

    fn eval_metas<'a>(metas: impl IntoIterator<Item = &'a Meta>) -> syn::Result<SchemaFieldConfig> {
        metas
            .into_iter()
            .map(eval_meta)
            .try_fold(
                SchemaFieldConfig::False,
                |state, result| Ok(state | result?),
            )
    }

    fn eval_meta(meta: &Meta) -> syn::Result<SchemaFieldConfig> {
        match meta.path() {
            path if path.is_ident("cfg_attr") => {
                let CfgAttr {
                    condition, metas, ..
                } = meta.require_list()?.parse_args()?;

                Ok(eval_metas(&metas)? & SchemaFieldConfig::Lazy(condition.into()))
            }
            path if path.is_ident("derive") => {
                let config = if meta
                    .require_list()?
                    .parse_args_with(parse_derive_args)?
                    .into_iter()
                    .any(|Path { segments, .. }| {
                        // This matches `JsonSchema`, `schemars::JsonSchema`
                        //   as well as any other path ending with `JsonSchema`.
                        // This will not match aliased `JsonSchema`s,
                        //   but might match other `JsonSchema` not `schemars::JsonSchema`!
                        match segments.last() {
                            Some(PathSegment { ident, .. }) => ident == "JsonSchema",
                            _ => false,
                        }
                    }) {
                    SchemaFieldConfig::True
                } else {
                    LazyBool::default()
                };
                Ok(config)
            }
            _ => Ok(SchemaFieldConfig::False),
        }
    }

    let DeriveInput { attrs, .. } = syn::parse(input)?;
    let metas = attrs.iter().map(|Attribute { meta, .. }| meta);
    eval_metas(metas)
}

/// Enum controlling when we should emit a `#[schemars]` field attribute.
pub(crate) type SchemaFieldConfig = LazyBool<SchemaFieldCondition>;

impl From<Meta> for SchemaFieldConfig {
    fn from(meta: Meta) -> Self {
        Self::Lazy(meta.into())
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SchemaFieldCondition(pub(crate) Meta);

impl BitAnd for SchemaFieldCondition {
    type Output = Self;

    fn bitand(self, Self(rhs): Self) -> Self::Output {
        let Self(lhs) = self;
        Self(parse_quote!(all(#lhs, #rhs)))
    }
}

impl BitAnd<&SchemaFieldCondition> for SchemaFieldCondition {
    type Output = Self;

    fn bitand(self, Self(rhs): &Self) -> Self::Output {
        let Self(lhs) = self;
        Self(parse_quote!(all(#lhs, #rhs)))
    }
}

impl BitOr for SchemaFieldCondition {
    type Output = Self;

    fn bitor(self, Self(rhs): Self) -> Self::Output {
        let Self(lhs) = self;
        Self(parse_quote!(any(#lhs, #rhs)))
    }
}

impl Not for SchemaFieldCondition {
    type Output = Self;

    fn not(self) -> Self::Output {
        let Self(condition) = self;
        Self(parse_quote!(not(#condition)))
    }
}

impl From<Meta> for SchemaFieldCondition {
    fn from(meta: Meta) -> Self {
        Self(meta)
    }
}

/// Get a `#[cfg]` expression under which this field has a `#[schemars]` attribute
/// with a `with = ...` argument.
pub(crate) fn schemars_with_attr_if(
    attrs: &[Attribute],
    filter: &[&str],
) -> syn::Result<SchemaFieldConfig> {
    fn eval_metas<'a>(
        filter: &[&str],
        metas: impl IntoIterator<Item = &'a Meta>,
    ) -> syn::Result<SchemaFieldConfig> {
        metas
            .into_iter()
            .map(|meta| eval_meta(filter, meta))
            .try_fold(
                SchemaFieldConfig::False,
                |state, result| Ok(state | result?),
            )
    }

    fn eval_meta(filter: &[&str], meta: &Meta) -> syn::Result<SchemaFieldConfig> {
        match meta.path() {
            path if path.is_ident("cfg_attr") => {
                let CfgAttr {
                    condition, metas, ..
                } = meta.require_list()?.parse_args()?;

                Ok(eval_metas(filter, &metas)? & SchemaFieldConfig::from(condition))
            }
            path if path.is_ident("schemars") => {
                let config = if meta
                    .require_list()?
                    .parse_args_with(<Punctuated<Meta, Token![,]>>::parse_terminated)?
                    .into_iter()
                    .any(|meta| match meta.path().get_ident() {
                        Some(ident) => filter.iter().any(|relevant| ident == relevant),
                        _ => false,
                    }) {
                    SchemaFieldConfig::True
                } else {
                    LazyBool::default()
                };
                Ok(config)
            }
            _ => Ok(SchemaFieldConfig::False),
        }
    }

    let metas = attrs.iter().map(|Attribute { meta, .. }| meta);
    eval_metas(filter, metas)
}
