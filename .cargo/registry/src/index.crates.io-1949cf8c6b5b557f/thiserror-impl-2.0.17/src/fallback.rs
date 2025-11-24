use crate::expand::call_site_ident;
use crate::private;
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

pub(crate) fn expand(input: &DeriveInput, error: syn::Error) -> TokenStream {
    let ty = call_site_ident(&input.ident);
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let error = error.to_compile_error();

    quote! {
        #error

        #[allow(unused_qualifications)]
        #[automatically_derived]
        impl #impl_generics ::thiserror::#private::Error for #ty #ty_generics #where_clause
        where
            // Work around trivial bounds being unstable.
            // https://github.com/rust-lang/rust/issues/48214
            for<'workaround> #ty #ty_generics: ::core::fmt::Debug,
        {}

        #[allow(unused_qualifications)]
        #[automatically_derived]
        impl #impl_generics ::core::fmt::Display for #ty #ty_generics #where_clause {
            fn fmt(&self, __formatter: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                ::core::unreachable!()
            }
        }
    }
}
