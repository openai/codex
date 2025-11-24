//! In case of bad input, parsing should fail. The error should have locations set in derived implementations.

// The use of fields in debug print commands does not count as "used",
// which causes the fields to trigger an unwanted dead code warning.
#![allow(dead_code)]

use darling::{FromDeriveInput, FromMeta};
use syn::parse_quote;

#[derive(Debug, FromMeta)]
struct Dolor {
    #[darling(rename = "amet")]
    sit: bool,
    world: bool,
}

#[derive(Debug, FromDeriveInput)]
#[darling(from_ident, attributes(hello))]
struct Lorem {
    ident: syn::Ident,
    ipsum: Dolor,
}

impl From<syn::Ident> for Lorem {
    fn from(ident: syn::Ident) -> Self {
        Lorem {
            ident,
            ipsum: Dolor {
                sit: false,
                world: true,
            },
        }
    }
}

#[test]
fn parsing_fail() {
    let di = parse_quote! {
        #[hello(ipsum(amet = "yes", world = false))]
        pub struct Foo;
    };

    println!("{}", Lorem::from_derive_input(&di).unwrap_err());
}

#[test]
fn missing_field() {
    let di = parse_quote! {
        #[hello(ipsum(amet = true))]
        pub struct Foo;
    };

    println!("{}", Lorem::from_derive_input(&di).unwrap_err());
}
