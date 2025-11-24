#[macro_use]
mod macros;

use proc_macro2::TokenStream;
use quote::quote;
use syn::Lit;

#[test]
fn test_struct() {
    let input = "
        #[derive(Debug, Clone)]
        pub struct Item {
            pub ident: Ident,
            pub attrs: Vec<Attribute>,
        }
    ";

    snapshot!(input as TokenStream, @r###"
    TokenStream(
        `# [derive (Debug , Clone)] pub struct Item { pub ident : Ident , pub attrs : Vec < Attribute >, }`,
    )
    "###);
}

#[test]
fn test_literal_mangling() {
    let code = "0_4";
    let parsed: Lit = syn::parse_str(code).unwrap();
    assert_eq!(code, quote!(#parsed).to_string());
}
