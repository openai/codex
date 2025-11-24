use proc_macro2::TokenStream;
use quote::quote;

pub fn wrap_in_const(serde_path: Option<&syn::Path>, code: TokenStream) -> TokenStream {
    let use_serde = match serde_path {
        Some(path) => quote! {
            use #path as _serde;
        },
        None => quote! {
            #[allow(unused_extern_crates, clippy::useless_attribute)]
            extern crate serde as _serde;
        },
    };

    quote! {
        #[doc(hidden)]
        #[allow(
            non_upper_case_globals,
            unused_attributes,
            unused_qualifications,
            clippy::absolute_paths,
        )]
        const _: () = {
            #use_serde

            _serde::__require_serde_not_serde_core!();

            #code
        };
    }
}
