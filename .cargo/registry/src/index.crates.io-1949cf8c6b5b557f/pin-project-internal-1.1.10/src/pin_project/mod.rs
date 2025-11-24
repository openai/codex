// SPDX-License-Identifier: Apache-2.0 OR MIT

mod args;
mod attribute;
mod derive;

use proc_macro2::TokenStream;
use syn::Error;

/// The annotation for pinned type.
const PIN: &str = "pin";

pub(crate) fn attribute(args: &TokenStream, input: TokenStream) -> TokenStream {
    attribute::parse_attribute(args, input).unwrap_or_else(Error::into_compile_error)
}

pub(crate) fn derive(input: TokenStream) -> TokenStream {
    derive::parse_derive(input).unwrap_or_else(Error::into_compile_error)
}
