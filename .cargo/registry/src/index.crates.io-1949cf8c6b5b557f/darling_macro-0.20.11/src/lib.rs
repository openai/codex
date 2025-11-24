use darling_core::{derive, Error};
use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro_derive(FromMeta, attributes(darling))]
pub fn derive_from_meta(input: TokenStream) -> TokenStream {
    derive::from_meta(&parse_macro_input!(input)).into()
}

#[proc_macro_derive(FromMetaItem, attributes(darling))]
pub fn derive_from_meta_item(_input: TokenStream) -> TokenStream {
    Error::custom("darling::FromMetaItem has been replaced by darling::FromMeta")
        .write_errors()
        .into()
}

#[proc_macro_derive(FromAttributes, attributes(darling))]
pub fn derive_from_attributes(input: TokenStream) -> TokenStream {
    derive::from_attributes(&parse_macro_input!(input)).into()
}

#[proc_macro_derive(FromDeriveInput, attributes(darling))]
pub fn derive_from_input(input: TokenStream) -> TokenStream {
    derive::from_derive_input(&parse_macro_input!(input)).into()
}

#[proc_macro_derive(FromField, attributes(darling))]
pub fn derive_field(input: TokenStream) -> TokenStream {
    derive::from_field(&parse_macro_input!(input)).into()
}

#[proc_macro_derive(FromTypeParam, attributes(darling))]
pub fn derive_type_param(input: TokenStream) -> TokenStream {
    derive::from_type_param(&parse_macro_input!(input)).into()
}

#[proc_macro_derive(FromVariant, attributes(darling))]
pub fn derive_variant(input: TokenStream) -> TokenStream {
    derive::from_variant(&parse_macro_input!(input)).into()
}
