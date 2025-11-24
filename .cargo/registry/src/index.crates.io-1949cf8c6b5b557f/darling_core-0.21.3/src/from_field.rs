use syn::Field;

use crate::Result;

/// Creates an instance by parsing an individual field and its attributes.
pub trait FromField: Sized {
    fn from_field(field: &Field) -> Result<Self>;
}

impl FromField for () {
    fn from_field(_: &Field) -> Result<Self> {
        Ok(())
    }
}

impl FromField for Field {
    fn from_field(field: &Field) -> Result<Self> {
        Ok(field.clone())
    }
}

impl FromField for syn::Type {
    fn from_field(field: &Field) -> Result<Self> {
        Ok(field.ty.clone())
    }
}

impl FromField for syn::Visibility {
    fn from_field(field: &Field) -> Result<Self> {
        Ok(field.vis.clone())
    }
}

impl FromField for Vec<syn::Attribute> {
    fn from_field(field: &Field) -> Result<Self> {
        Ok(field.attrs.clone())
    }
}
