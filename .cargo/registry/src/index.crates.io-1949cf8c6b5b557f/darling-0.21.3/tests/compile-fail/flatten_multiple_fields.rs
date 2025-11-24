//! Test that multiple fields cannot be marked `flatten` at once.

use darling::{FromDeriveInput, FromMeta};

#[derive(FromMeta)]
struct Inner {
    left: String,
    right: String,
}

#[derive(FromMeta)]
pub struct Example {
    #[darling(flatten)]
    first: Inner,
    #[darling(flatten)]
    last: Inner,
}

#[derive(FromDeriveInput)]
pub struct FdiExample {
    ident: syn::Ident,
    #[darling(flatten)]
    first: Inner,
    #[darling(flatten)]
    last: Inner,
}

fn main() {}
