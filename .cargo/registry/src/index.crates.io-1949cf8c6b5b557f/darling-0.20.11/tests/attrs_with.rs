use std::collections::BTreeSet;

use darling::{util, Error, FromDeriveInput, Result};
use syn::{parse_quote, Attribute};

fn unique_idents(attrs: Vec<Attribute>) -> Result<BTreeSet<String>> {
    let mut errors = Error::accumulator();
    let idents = attrs
        .into_iter()
        .filter_map(|attr| {
            let path = attr.path();
            errors.handle(
                path.get_ident()
                    .map(std::string::ToString::to_string)
                    .ok_or_else(|| {
                        Error::custom(format!("`{}` is not an ident", util::path_to_string(path)))
                            .with_span(path)
                    }),
            )
        })
        .collect();

    errors.finish_with(idents)
}

#[derive(FromDeriveInput)]
#[darling(attributes(a), forward_attrs)]
struct Receiver {
    #[darling(with = unique_idents)]
    attrs: BTreeSet<String>,
    other: Option<bool>,
}

#[test]
fn succeeds_on_no_attrs() {
    let di = Receiver::from_derive_input(&parse_quote! {
        struct Demo;
    })
    .unwrap();

    assert!(di.attrs.is_empty());
}

#[test]
fn succeeds_on_valid_input() {
    let di = Receiver::from_derive_input(&parse_quote! {
        #[allow(dead_code)]
        /// testing
        #[another]
        struct Demo;
    })
    .unwrap();

    assert_eq!(di.attrs.len(), 3);
    assert!(di.attrs.contains("allow"));
    assert!(di.attrs.contains("another"));
    assert!(di.attrs.contains("doc"));
    assert_eq!(di.other, None);
}

#[test]
fn errors_combined_with_others() {
    let e = Receiver::from_derive_input(&parse_quote! {
        #[path::to::attr(dead_code)]
        #[a(other = 5)]
        struct Demo;
    })
    .map(|_| "Should have failed")
    .unwrap_err();

    let error = e.to_string();

    assert_eq!(e.len(), 2);

    // Look for the error on the field `other`
    assert!(error.contains("at other"));

    // Look for the invalid path from attrs conversion
    assert!(error.contains("`path::to::attr`"));
}
