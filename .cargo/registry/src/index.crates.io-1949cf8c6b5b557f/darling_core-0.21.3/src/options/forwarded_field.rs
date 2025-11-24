use quote::{quote, ToTokens, TokenStreamExt};
use syn::{Ident, Path};

use crate::{Error, FromField, FromMeta};

use super::ParseAttribute;

/// A forwarded field and attributes that influence its behavior.
#[derive(Debug, Clone)]
pub struct ForwardedField {
    /// The ident of the field that will receive the forwarded value.
    pub ident: Ident,
    /// Path of the function that will be called to convert the forwarded value
    /// into the type expected by the field in `ident`.
    pub with: Option<Path>,
}

impl ForwardedField {
    /// Returns a field initializer for this forwarded field.
    pub fn as_initializer(&self) -> Initializer<'_> {
        Initializer(self)
    }
}

impl FromField for ForwardedField {
    fn from_field(field: &syn::Field) -> crate::Result<Self> {
        let result = Self {
            ident: field.ident.clone().ok_or_else(|| {
                Error::custom("forwarded field must be named field").with_span(field)
            })?,
            with: None,
        };

        result.parse_attributes(&field.attrs)
    }
}

impl ParseAttribute for ForwardedField {
    fn parse_nested(&mut self, mi: &syn::Meta) -> crate::Result<()> {
        if mi.path().is_ident("with") {
            if self.with.is_some() {
                return Err(Error::duplicate_field_path(mi.path()).with_span(mi));
            }

            self.with = FromMeta::from_meta(mi)?;
            Ok(())
        } else {
            Err(Error::unknown_field_path_with_alts(mi.path(), &["with"]).with_span(mi))
        }
    }
}

/// A field initializer that assumes:
///
/// 1. There is a local variable with the same ident as `self.ident`
/// 2. That local variable is an `Option`
/// 3. That any errors were already checked by an accumulator.
pub struct Initializer<'a>(&'a ForwardedField);

impl ToTokens for Initializer<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let ident = &self.0.ident;
        tokens.append_all(quote!(#ident: #ident.expect("Errors were already checked"),));
    }
}
