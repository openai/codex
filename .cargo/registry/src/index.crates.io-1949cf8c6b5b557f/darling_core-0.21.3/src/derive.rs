//! Functions to derive `darling`'s traits from well-formed input, without directly depending
//! on `proc_macro`.

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::DeriveInput;

use crate::options;

/// Run an expression which returns a `darling::Result`, then either return the tokenized
/// representation of the `Ok` value, or the tokens of the compiler errors in the `Err` case.
macro_rules! emit_impl_or_error {
    ($e:expr) => {
        match $e {
            Ok(val) => val.into_token_stream(),
            Err(err) => err.write_errors(),
        }
    };
}

/// Create tokens for a `darling::FromMeta` impl from a `DeriveInput`. If
/// the input cannot produce a valid impl, the returned tokens will contain
/// compile errors instead.
pub fn from_meta(input: &DeriveInput) -> TokenStream {
    emit_impl_or_error!(options::FromMetaOptions::new(input))
}

/// Create tokens for a `darling::FromAttributes` impl from a `DeriveInput`. If
/// the input cannot produce a valid impl, the returned tokens will contain
/// compile errors instead.
pub fn from_attributes(input: &DeriveInput) -> TokenStream {
    emit_impl_or_error!(options::FromAttributesOptions::new(input))
}

/// Create tokens for a `darling::FromDeriveInput` impl from a `DeriveInput`. If
/// the input cannot produce a valid impl, the returned tokens will contain
/// compile errors instead.
pub fn from_derive_input(input: &DeriveInput) -> TokenStream {
    emit_impl_or_error!(options::FdiOptions::new(input))
}

/// Create tokens for a `darling::FromField` impl from a `DeriveInput`. If
/// the input cannot produce a valid impl, the returned tokens will contain
/// compile errors instead.
pub fn from_field(input: &DeriveInput) -> TokenStream {
    emit_impl_or_error!(options::FromFieldOptions::new(input))
}

/// Create tokens for a `darling::FromTypeParam` impl from a `DeriveInput`. If
/// the input cannot produce a valid impl, the returned tokens will contain
/// compile errors instead.
pub fn from_type_param(input: &DeriveInput) -> TokenStream {
    emit_impl_or_error!(options::FromTypeParamOptions::new(input))
}

/// Create tokens for a `darling::FromVariant` impl from a `DeriveInput`. If
/// the input cannot produce a valid impl, the returned tokens will contain
/// compile errors instead.
pub fn from_variant(input: &DeriveInput) -> TokenStream {
    emit_impl_or_error!(options::FromVariantOptions::new(input))
}
