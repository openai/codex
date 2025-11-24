use darling::FromAttributes;
use syn::parse_quote;

#[derive(Default, darling::FromAttributes)]
#[darling(attributes(builder), forward_attrs)]
struct Params {
    default: Option<syn::Expr>,
    attrs: Vec<syn::Attribute>,
}

#[test]
fn forward_attrs_with_field() {
    let input: syn::DeriveInput = parse_quote! {
        #[doc = "Hello"]
        #[builder(default = 15)]
        struct Example;
    };

    let parsed = Params::from_attributes(&input.attrs).unwrap();
    assert!(parsed.default.is_some());
    assert_eq!(parsed.attrs.len(), 1);
}
