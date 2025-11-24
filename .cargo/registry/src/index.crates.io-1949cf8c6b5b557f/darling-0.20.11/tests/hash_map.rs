use std::collections::HashMap;

use darling::FromMeta;
use syn::{parse_quote, Attribute, Path};

#[derive(Debug, FromMeta, PartialEq, Eq)]
struct MapValue {
    name: String,
    #[darling(default)]
    option: bool,
}

#[test]
fn parse_map() {
    let attr: Attribute = parse_quote! {
        #[foo(first(name = "Hello", option), the::second(name = "Second"))]
    };

    let meta = attr.meta;
    let map: HashMap<Path, MapValue> = FromMeta::from_meta(&meta).unwrap();

    let comparison: HashMap<Path, MapValue> = vec![
        (
            parse_quote!(first),
            MapValue {
                name: "Hello".into(),
                option: true,
            },
        ),
        (
            parse_quote!(the::second),
            MapValue {
                name: "Second".into(),
                option: false,
            },
        ),
    ]
    .into_iter()
    .collect();

    assert_eq!(comparison, map);
}
