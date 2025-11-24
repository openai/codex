use syn::TypeParam;

use crate::Result;

/// Creates an instance by parsing an individual type_param and its attributes.
pub trait FromTypeParam: Sized {
    fn from_type_param(type_param: &TypeParam) -> Result<Self>;
}

impl FromTypeParam for () {
    fn from_type_param(_: &TypeParam) -> Result<Self> {
        Ok(())
    }
}

impl FromTypeParam for TypeParam {
    fn from_type_param(type_param: &TypeParam) -> Result<Self> {
        Ok(type_param.clone())
    }
}

impl FromTypeParam for Vec<syn::Attribute> {
    fn from_type_param(type_param: &TypeParam) -> Result<Self> {
        Ok(type_param.attrs.clone())
    }
}

impl FromTypeParam for syn::Ident {
    fn from_type_param(type_param: &TypeParam) -> Result<Self> {
        Ok(type_param.ident.clone())
    }
}
