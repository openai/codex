//! When input is split across multiple attributes on one element,
//! darling should collapse that into one struct.

use darling::{Error, FromDeriveInput};
use syn::parse_quote;

#[derive(Debug, FromDeriveInput, PartialEq, Eq)]
#[darling(attributes(split))]
struct Lorem {
    foo: String,
    bar: bool,
}

#[test]
fn split_attributes_accrue_to_instance() {
    let di = parse_quote! {
        #[split(foo = "Hello")]
        #[split(bar)]
        pub struct Foo;
    };

    let parsed = Lorem::from_derive_input(&di).unwrap();
    assert_eq!(
        parsed,
        Lorem {
            foo: "Hello".to_string(),
            bar: true,
        }
    );
}

#[test]
fn duplicates_across_split_attrs_error() {
    let di = parse_quote! {
        #[split(foo = "Hello")]
        #[split(foo = "World", bar)]
        pub struct Foo;
    };

    let pr = Lorem::from_derive_input(&di).unwrap_err();
    assert!(pr.has_span());
    assert_eq!(pr.to_string(), Error::duplicate_field("foo").to_string());
}

#[test]
fn multiple_errors_accrue_to_instance() {
    let di = parse_quote! {
        #[split(foo = "Hello")]
        #[split(foo = "World")]
        pub struct Foo;
    };

    let pr = Lorem::from_derive_input(&di);
    let err: Error = pr.unwrap_err();
    assert_eq!(2, err.len());
    let mut errs = err.into_iter().peekable();
    assert_eq!(
        errs.peek().unwrap().to_string(),
        Error::duplicate_field("foo").to_string()
    );
    assert!(errs.next().unwrap().has_span());
    assert_eq!(
        errs.next().unwrap().to_string(),
        Error::missing_field("bar").to_string()
    );
    assert!(errs.next().is_none());
}
