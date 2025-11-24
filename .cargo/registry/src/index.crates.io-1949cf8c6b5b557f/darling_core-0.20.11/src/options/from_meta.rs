use std::borrow::Cow;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::parse_quote;

use crate::ast::Data;
use crate::codegen::FromMetaImpl;
use crate::error::Accumulator;
use crate::options::{Core, ParseAttribute, ParseData};
use crate::util::Callable;
use crate::{Error, FromMeta, Result};

pub struct FromMetaOptions {
    base: Core,
    /// Override for the default [`FromMeta::from_word`] method.
    from_word: Option<Callable>,
    /// Override for the default [`FromMeta::from_none`] method.
    from_none: Option<Callable>,
}

impl FromMetaOptions {
    pub fn new(di: &syn::DeriveInput) -> Result<Self> {
        (FromMetaOptions {
            base: Core::start(di)?,
            from_word: None,
            from_none: None,
        })
        .parse_attributes(&di.attrs)?
        .parse_body(&di.data)
    }

    /// Get the `from_word` method body, if one exists. This can come from direct use of
    /// `#[darling(from_word = ...)]` on the container or from use of `#[darling(word)]` on
    /// a unit variant.
    #[allow(
        clippy::wrong_self_convention,
        // The reason is commented out due to MSRV issues.
        // reason = "This matches the name of the input option and output method"
    )]
    fn from_word(&self) -> Option<Cow<'_, Callable>> {
        self.from_word.as_ref().map(Cow::Borrowed).or_else(|| {
            if let Data::Enum(ref variants) = self.base.data {
                // The first variant which has `word` set to `true`.
                // This assumes that validation has prevented multiple variants
                // from claiming `word`.
                let variant = variants
                    .iter()
                    .find(|v| v.word.map(|x| *x).unwrap_or_default())?;
                let variant_ident = &variant.ident;
                let closure: syn::ExprClosure = parse_quote! {
                    || ::darling::export::Ok(Self::#variant_ident)
                };
                Some(Cow::Owned(Callable::from(closure)))
            } else {
                None
            }
        })
    }
}

impl ParseAttribute for FromMetaOptions {
    fn parse_nested(&mut self, mi: &syn::Meta) -> Result<()> {
        let path = mi.path();

        if path.is_ident("from_word") {
            if self.from_word.is_some() {
                return Err(Error::duplicate_field_path(path).with_span(path));
            }

            self.from_word = FromMeta::from_meta(mi).map(Some)?;
        } else if path.is_ident("from_none") {
            if self.from_none.is_some() {
                return Err(Error::duplicate_field_path(path).with_span(path));
            }

            self.from_none = FromMeta::from_meta(mi).map(Some)?;
        } else {
            self.base.parse_nested(mi)?;
        }

        Ok(())
    }
}

impl ParseData for FromMetaOptions {
    fn parse_variant(&mut self, variant: &syn::Variant) -> Result<()> {
        self.base.parse_variant(variant)
    }

    fn parse_field(&mut self, field: &syn::Field) -> Result<()> {
        self.base.parse_field(field)
    }

    fn validate_body(&self, errors: &mut Accumulator) {
        self.base.validate_body(errors);

        match self.base.data {
            Data::Struct(ref data) => {
                if let Some(from_word) = &self.from_word {
                    if data.is_unit() {
                        errors.push(Error::custom("`from_word` cannot be used on unit structs because it conflicts with the generated impl").with_span(from_word));
                    } else if data.is_newtype() {
                        errors.push(Error::custom("`from_word` cannot be used on newtype structs because the implementation is entirely delegated to the inner type").with_span(from_word));
                    }
                }
            }
            Data::Enum(ref data) => {
                let word_variants: Vec<_> = data
                    .iter()
                    .filter_map(|variant| variant.word.as_ref())
                    .collect();

                if !word_variants.is_empty() {
                    if let Some(from_word) = &self.from_word {
                        errors.push(
                            Error::custom(
                                "`from_word` cannot be used with an enum that also uses `word`",
                            )
                            .with_span(from_word),
                        )
                    }
                }

                // Adds errors for duplicate `#[darling(word)]` annotations across all variants.
                if word_variants.len() > 1 {
                    for word in word_variants {
                        errors.push(
                            Error::custom("`#[darling(word)]` can only be applied to one variant")
                                .with_span(&word.span()),
                        );
                    }
                }
            }
        }
    }
}

impl<'a> From<&'a FromMetaOptions> for FromMetaImpl<'a> {
    fn from(v: &'a FromMetaOptions) -> Self {
        FromMetaImpl {
            base: (&v.base).into(),
            from_word: v.from_word(),
            from_none: v.from_none.as_ref(),
        }
    }
}

impl ToTokens for FromMetaOptions {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        FromMetaImpl::from(self).to_tokens(tokens)
    }
}
