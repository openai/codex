#![allow(clippy::uninlined_format_args)]

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn corresponds(attr: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(attr as Ident);
    let item = parse_macro_input!(item as ItemFn);

    let function = function.to_string();
    let line = format!(
        "This corresponds to [`{0}`](https://www.openssl.org/docs/manmaster/man3/{0}.html).",
        function
    );

    let attrs = item.attrs;
    let vis = item.vis;
    let sig = item.sig;
    let block = item.block;

    let out = quote! {
        #(#attrs)*
        #[doc = ""]
        #[doc = #line]
        #[doc(alias = #function)]
        #vis #sig #block
    };
    out.into()
}
