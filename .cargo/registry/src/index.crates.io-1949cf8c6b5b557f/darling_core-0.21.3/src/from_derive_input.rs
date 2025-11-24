use syn::DeriveInput;

use crate::Result;

/// Creates an instance by parsing an entire proc-macro `derive` input,
/// including the, identity, generics, and visibility of the type.
///
/// This trait should either be derived or manually implemented by a type
/// in the proc macro crate which is directly using `darling`. It is unlikely
/// that these implementations will be reusable across crates.
pub trait FromDeriveInput: Sized {
    /// Create an instance from `syn::DeriveInput`, or return an error.
    fn from_derive_input(input: &DeriveInput) -> Result<Self>;
}

impl FromDeriveInput for () {
    fn from_derive_input(_: &DeriveInput) -> Result<Self> {
        Ok(())
    }
}

impl FromDeriveInput for DeriveInput {
    fn from_derive_input(input: &DeriveInput) -> Result<Self> {
        Ok(input.clone())
    }
}
