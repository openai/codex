use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::Ident;

use crate::codegen::FromDeriveInputImpl;
use crate::options::{DeriveInputShapeSet, OuterFrom, ParseAttribute, ParseData};
use crate::{FromField, FromMeta, Result};

use super::forwarded_field::ForwardedField;

#[derive(Debug)]
pub struct FdiOptions {
    pub base: OuterFrom,

    /// The field on the target struct which should receive the type visibility, if any.
    pub vis: Option<Ident>,

    /// The field on the target struct which should receive the type generics, if any.
    pub generics: Option<Ident>,

    /// The field on the target struct which should receive the derive input body, if any.
    pub data: Option<ForwardedField>,

    pub supports: Option<DeriveInputShapeSet>,
}

impl FdiOptions {
    pub fn new(di: &syn::DeriveInput) -> Result<Self> {
        (FdiOptions {
            base: OuterFrom::start(di)?,
            vis: Default::default(),
            generics: Default::default(),
            data: Default::default(),
            supports: Default::default(),
        })
        .parse_attributes(&di.attrs)?
        .parse_body(&di.data)
    }
}

impl ParseAttribute for FdiOptions {
    fn parse_nested(&mut self, mi: &syn::Meta) -> Result<()> {
        if mi.path().is_ident("supports") {
            self.supports = FromMeta::from_meta(mi)?;
            Ok(())
        } else {
            self.base.parse_nested(mi)
        }
    }
}

impl ParseData for FdiOptions {
    fn parse_variant(&mut self, variant: &syn::Variant) -> Result<()> {
        self.base.parse_variant(variant)
    }

    fn parse_field(&mut self, field: &syn::Field) -> Result<()> {
        match field.ident.as_ref().map(|v| v.to_string()).as_deref() {
            Some("vis") => {
                self.vis.clone_from(&field.ident);
                Ok(())
            }
            Some("data") => {
                self.data = ForwardedField::from_field(field).map(Some)?;
                Ok(())
            }
            Some("generics") => {
                self.generics.clone_from(&field.ident);
                Ok(())
            }
            _ => self.base.parse_field(field),
        }
    }

    fn validate_body(&self, errors: &mut crate::error::Accumulator) {
        self.base.validate_body(errors);
    }
}

impl<'a> From<&'a FdiOptions> for FromDeriveInputImpl<'a> {
    fn from(v: &'a FdiOptions) -> Self {
        FromDeriveInputImpl {
            base: (&v.base.container).into(),
            attr_names: &v.base.attr_names,
            from_ident: v.base.from_ident,
            ident: v.base.ident.as_ref(),
            vis: v.vis.as_ref(),
            data: v.data.as_ref(),
            generics: v.generics.as_ref(),
            forward_attrs: v.base.as_forward_attrs(),
            supports: v.supports.as_ref(),
        }
    }
}

impl ToTokens for FdiOptions {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        FromDeriveInputImpl::from(self).to_tokens(tokens)
    }
}
