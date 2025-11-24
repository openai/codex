use crate::Result;

/// Creates an instance by parsing a specific `syn::GenericParam`.
/// This can be a type param, a lifetime, or a const param.
pub trait FromGenericParam: Sized {
    fn from_generic_param(param: &syn::GenericParam) -> Result<Self>;
}

impl FromGenericParam for () {
    fn from_generic_param(_param: &syn::GenericParam) -> Result<Self> {
        Ok(())
    }
}

impl FromGenericParam for syn::GenericParam {
    fn from_generic_param(param: &syn::GenericParam) -> Result<Self> {
        Ok(param.clone())
    }
}
