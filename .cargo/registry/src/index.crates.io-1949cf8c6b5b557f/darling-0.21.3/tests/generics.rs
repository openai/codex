#![allow(dead_code)]

use darling::{FromDeriveInput, FromMeta};
use syn::parse_quote;

#[derive(Debug, Clone, FromMeta)]
struct Wrapper<T>(pub T);

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(hello))]
struct Foo<T> {
    lorem: Wrapper<T>,
}

#[test]
fn expansion() {
    let di = parse_quote! {
        #[hello(lorem = "Hello")]
        pub struct Foo;
    };

    Foo::<String>::from_derive_input(&di).unwrap();
}
