use syn::spanned::Spanned;
use syn::{Field, Ident, Meta};

use crate::ast::Data;
use crate::codegen::ForwardAttrs;
use crate::options::{
    Core, DefaultExpression, ForwardAttrsFilter, ForwardedField, ParseAttribute, ParseData,
};
use crate::util::PathList;
use crate::{Error, FromField, FromMeta, Result};

/// Reusable base for `FromDeriveInput`, `FromVariant`, `FromField`, and other top-level
/// `From*` traits.
#[derive(Debug, Clone)]
pub struct OuterFrom {
    /// The field on the target struct which should receive the type identifier, if any.
    pub ident: Option<Ident>,

    /// The field on the target struct which should receive the type attributes, if any.
    pub attrs: Option<ForwardedField>,

    pub container: Core,

    /// The attribute names that should be searched.
    pub attr_names: PathList,

    /// The attribute names that should be forwarded. The presence of the word with no additional
    /// filtering will cause _all_ attributes to be cloned and exposed to the struct after parsing.
    pub forward_attrs: Option<ForwardAttrsFilter>,

    /// Whether or not the container can be made through conversion from the type `Ident`.
    pub from_ident: bool,
}

impl OuterFrom {
    pub fn start(di: &syn::DeriveInput) -> Result<Self> {
        Ok(OuterFrom {
            container: Core::start(di)?,
            attrs: Default::default(),
            ident: Default::default(),
            attr_names: Default::default(),
            forward_attrs: Default::default(),
            from_ident: Default::default(),
        })
    }

    pub fn as_forward_attrs(&self) -> ForwardAttrs<'_> {
        ForwardAttrs {
            field: self.attrs.as_ref(),
            filter: self.forward_attrs.as_ref(),
        }
    }
}

impl ParseAttribute for OuterFrom {
    fn parse_nested(&mut self, mi: &Meta) -> Result<()> {
        let path = mi.path();
        if path.is_ident("attributes") {
            self.attr_names = FromMeta::from_meta(mi)?;
        } else if path.is_ident("forward_attrs") {
            self.forward_attrs = FromMeta::from_meta(mi)?;
        } else if path.is_ident("from_ident") {
            // HACK: Declaring that a default is present will cause fields to
            // generate correct code, but control flow isn't that obvious.
            self.container.default = Some(DefaultExpression::Trait {
                // Use the span of the `from_ident` keyword so that errors in generated code
                // caused by this will point back to the correct location.
                span: path.span(),
            });
            self.from_ident = true;
        } else {
            return self.container.parse_nested(mi);
        }
        Ok(())
    }
}

impl ParseData for OuterFrom {
    fn parse_field(&mut self, field: &Field) -> Result<()> {
        match field.ident.as_ref().map(|v| v.to_string()).as_deref() {
            Some("ident") => {
                self.ident.clone_from(&field.ident);
                Ok(())
            }
            Some("attrs") => {
                self.attrs = ForwardedField::from_field(field).map(Some)?;
                Ok(())
            }
            _ => self.container.parse_field(field),
        }
    }

    fn validate_body(&self, errors: &mut crate::error::Accumulator) {
        self.container.validate_body(errors);
        if let Some(attrs) = &self.attrs {
            if self.forward_attrs.is_none() {
                let container_name = match &self.container.data {
                    Data::Enum(_) => "enum",
                    Data::Struct(_) => "struct",
                };
                errors.push(
                    Error::custom(format!(
                        "field will not be populated because `forward_attrs` is not set on the {}",
                        container_name
                    ))
                    .with_span(&attrs.ident),
                );
            }
        }
    }
}
