#![allow(dead_code)]
#![cfg(feature = "suggestions")]

use darling::{FromDeriveInput, FromMeta};
use syn::parse_quote;

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(suggest))]
struct Lorem {
    ipsum: String,
    dolor: Dolor,
    // This field is included to make sure that skipped fields aren't suggested.
    #[darling(skip)]
    amet: bool,
}

#[derive(Debug, FromMeta)]
struct Dolor {
    sit: bool,
}

#[test]
fn suggest_dolor() {
    let input: syn::DeriveInput = parse_quote! {
        #[suggest(ipsum = "Hello", dolorr(sit))]
        pub struct Foo;
    };

    let result = Lorem::from_derive_input(&input).unwrap_err();
    assert_eq!(2, result.len());
    assert!(format!("{}", result).contains("Did you mean"));
}

#[test]
fn dont_suggest_skipped_field() {
    let input: syn::DeriveInput = parse_quote! {
        #[suggest(ipsum = "Hello", dolor(sit), amt)]
        pub struct Foo;
    };

    let result = Lorem::from_derive_input(&input).unwrap_err();
    assert_eq!(1, result.len());
    assert!(!format!("{}", result).contains("amet"));
}
