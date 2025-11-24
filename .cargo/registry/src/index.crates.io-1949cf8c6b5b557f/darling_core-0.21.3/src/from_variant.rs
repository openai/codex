use syn::Variant;

use crate::Result;

/// Creates an instance from a specified `syn::Variant`.
pub trait FromVariant: Sized {
    /// Create an instance from `syn::Variant`, or return an error.
    fn from_variant(variant: &Variant) -> Result<Self>;
}

impl FromVariant for () {
    fn from_variant(_: &Variant) -> Result<Self> {
        Ok(())
    }
}

impl FromVariant for Variant {
    fn from_variant(variant: &Variant) -> Result<Self> {
        Ok(variant.clone())
    }
}

impl FromVariant for syn::Ident {
    fn from_variant(variant: &Variant) -> Result<Self> {
        Ok(variant.ident.clone())
    }
}

impl FromVariant for Vec<syn::Attribute> {
    fn from_variant(variant: &Variant) -> Result<Self> {
        Ok(variant.attrs.clone())
    }
}
