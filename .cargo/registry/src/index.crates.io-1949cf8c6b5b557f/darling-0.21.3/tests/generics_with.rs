use std::collections::BTreeSet;

use darling::{Error, FromDeriveInput, Result};
use syn::{parse_quote, Ident};

fn check_ident(ident: &Ident) -> Result<String> {
    let s = ident.to_string();
    if s.len() < 2 {
        Err(Error::custom("generics must be at least 2 characters").with_span(ident))
    } else {
        Ok(s)
    }
}

fn long_generic_names(generics: &syn::Generics) -> Result<BTreeSet<String>> {
    let mut errors = Error::accumulator();
    let valid = generics
        .type_params()
        .filter_map(|c| errors.handle(check_ident(&c.ident)))
        .collect();
    errors.finish_with(valid)
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(a))]
struct Receiver {
    #[darling(with = long_generic_names)]
    generics: BTreeSet<String>,
    surname: String,
}

#[test]
fn succeeds_on_no_generics() {
    let di = Receiver::from_derive_input(&parse_quote! {
        #[a(surname = "Smith")]
        struct Demo;
    })
    .unwrap();

    assert!(di.generics.is_empty());
}

#[test]
fn succeeds_on_valid_generics() {
    let di = Receiver::from_derive_input(&parse_quote! {
        #[a(surname = "Smith")]
        struct Demo<Greeting> {
            hello: Greeting,
            world: String,
        }
    })
    .unwrap();

    assert_eq!(di.generics.len(), 1);
    assert!(di.generics.contains("Greeting"));
    assert_eq!(di.surname, "Smith");
}

#[test]
fn rejects_invalid_input() {
    let err = Receiver::from_derive_input(&parse_quote! {
        struct Demo<G, S> {
            hello: G,
            world: S,
        }
    })
    .unwrap_err();

    assert_eq!(
        err.len(),
        // 2 errors from short type param names
        2 +
        // error for missing field `surname`
        1,
        "errors should have accumulated, and body checking should have occurred"
    );
}
