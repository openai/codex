use darling::{FromDeriveInput, FromMeta};
use syn::parse_quote;

#[derive(FromDeriveInput)]
#[darling(attributes(hello))]
#[allow(dead_code)]
struct Lorem {
    ident: syn::Ident,
    ipsum: Ipsum,
}

#[derive(FromMeta)]
struct Ipsum {
    #[darling(multiple)]
    dolor: Vec<String>,
}

#[test]
fn expand_many() {
    let di = parse_quote! {
        #[hello(ipsum(dolor = "Hello", dolor = "World"))]
        pub struct Baz;
    };

    let lorem: Lorem = Lorem::from_derive_input(&di).unwrap();
    assert_eq!(
        lorem.ipsum.dolor,
        vec!["Hello".to_string(), "World".to_string()]
    );
}
