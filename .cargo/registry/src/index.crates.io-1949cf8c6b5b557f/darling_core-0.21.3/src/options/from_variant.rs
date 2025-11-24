use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{DeriveInput, Field, Ident, Meta};

use crate::codegen::FromVariantImpl;
use crate::options::{DataShape, OuterFrom, ParseAttribute, ParseData};
use crate::{FromMeta, Result};

#[derive(Debug, Clone)]
pub struct FromVariantOptions {
    pub base: OuterFrom,
    /// The field on the deriving struct into which the discriminant expression
    /// should be placed by the derived `FromVariant` impl.
    pub discriminant: Option<Ident>,
    pub fields: Option<Ident>,
    pub supports: Option<DataShape>,
}

impl FromVariantOptions {
    pub fn new(di: &DeriveInput) -> Result<Self> {
        (FromVariantOptions {
            base: OuterFrom::start(di)?,
            discriminant: Default::default(),
            fields: Default::default(),
            supports: Default::default(),
        })
        .parse_attributes(&di.attrs)?
        .parse_body(&di.data)
    }
}

impl<'a> From<&'a FromVariantOptions> for FromVariantImpl<'a> {
    fn from(v: &'a FromVariantOptions) -> Self {
        FromVariantImpl {
            base: (&v.base.container).into(),
            ident: v.base.ident.as_ref(),
            discriminant: v.discriminant.as_ref(),
            fields: v.fields.as_ref(),
            attr_names: &v.base.attr_names,
            forward_attrs: v.base.as_forward_attrs(),
            from_ident: v.base.from_ident,
            supports: v.supports.as_ref(),
        }
    }
}

impl ParseAttribute for FromVariantOptions {
    fn parse_nested(&mut self, mi: &Meta) -> Result<()> {
        if mi.path().is_ident("supports") {
            self.supports = FromMeta::from_meta(mi)?;
            Ok(())
        } else {
            self.base.parse_nested(mi)
        }
    }
}

impl ParseData for FromVariantOptions {
    fn parse_field(&mut self, field: &Field) -> Result<()> {
        match field.ident.as_ref().map(|v| v.to_string()).as_deref() {
            Some("discriminant") => {
                self.discriminant.clone_from(&field.ident);
                Ok(())
            }
            Some("fields") => {
                self.fields.clone_from(&field.ident);
                Ok(())
            }
            _ => self.base.parse_field(field),
        }
    }

    fn validate_body(&self, errors: &mut crate::error::Accumulator) {
        self.base.validate_body(errors);
    }
}

impl ToTokens for FromVariantOptions {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        FromVariantImpl::from(self).to_tokens(tokens)
    }
}
