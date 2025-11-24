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
