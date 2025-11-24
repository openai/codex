use std::collections::BTreeSet;

use darling::{Error, FromDeriveInput, Result};
use syn::parse_quote;

fn field_names(data: &syn::Data) -> Result<BTreeSet<String>> {
    let fields = match data {
        syn::Data::Struct(data) => data.fields.iter(),
        syn::Data::Enum(_) => return Err(Error::custom("Expected struct or union")),
        syn::Data::Union(data) => data.fields.named.iter(),
    };

    Ok(fields
        .filter_map(|f| f.ident.clone())
        .map(|i| i.to_string())
        .collect())
}

#[derive(FromDeriveInput)]
#[darling(attributes(a), forward_attrs)]
struct Receiver {
    #[darling(with = field_names)]
    data: BTreeSet<String>,
}

#[test]
fn succeeds_on_no_fields() {
    let di = Receiver::from_derive_input(&parse_quote! {
        struct Demo;
    })
    .unwrap();

    assert!(di.data.is_empty());
}

#[test]
fn succeeds_on_valid_input() {
    let di = Receiver::from_derive_input(&parse_quote! {
        struct Demo {
            hello: String,
            world: String,
        }
    })
    .unwrap();

    assert_eq!(di.data.len(), 2);
    assert!(di.data.contains("hello"));
    assert!(di.data.contains("world"));
}
