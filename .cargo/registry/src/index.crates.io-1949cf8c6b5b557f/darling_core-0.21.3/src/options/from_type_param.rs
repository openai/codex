use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::Ident;

use crate::codegen::FromTypeParamImpl;
use crate::options::{OuterFrom, ParseAttribute, ParseData};
use crate::Result;

#[derive(Debug)]
pub struct FromTypeParamOptions {
    pub base: OuterFrom,
    pub bounds: Option<Ident>,
    pub default: Option<Ident>,
}

impl FromTypeParamOptions {
    pub fn new(di: &syn::DeriveInput) -> Result<Self> {
        (FromTypeParamOptions {
            base: OuterFrom::start(di)?,
            bounds: None,
            default: None,
        })
        .parse_attributes(&di.attrs)?
        .parse_body(&di.data)
    }
}

impl ParseAttribute for FromTypeParamOptions {
    fn parse_nested(&mut self, mi: &syn::Meta) -> Result<()> {
        self.base.parse_nested(mi)
    }
}

impl ParseData for FromTypeParamOptions {
    fn parse_variant(&mut self, variant: &syn::Variant) -> Result<()> {
        self.base.parse_variant(variant)
    }

    fn parse_field(&mut self, field: &syn::Field) -> Result<()> {
        match field.ident.as_ref().map(|v| v.to_string()).as_deref() {
            Some("bounds") => {
                self.bounds.clone_from(&field.ident);
                Ok(())
            }
            Some("default") => {
                self.default.clone_from(&field.ident);
                Ok(())
            }
            _ => self.base.parse_field(field),
        }
    }

    fn validate_body(&self, errors: &mut crate::error::Accumulator) {
        self.base.validate_body(errors);
    }
}

impl<'a> From<&'a FromTypeParamOptions> for FromTypeParamImpl<'a> {
    fn from(v: &'a FromTypeParamOptions) -> Self {
        FromTypeParamImpl {
            base: (&v.base.container).into(),
            ident: v.base.ident.as_ref(),
            bounds: v.bounds.as_ref(),
            default: v.default.as_ref(),
            attr_names: &v.base.attr_names,
            forward_attrs: v.base.as_forward_attrs(),
            from_ident: v.base.from_ident,
        }
    }
}

impl ToTokens for FromTypeParamOptions {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        FromTypeParamImpl::from(self).to_tokens(tokens)
    }
}
