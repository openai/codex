use darling::{FromDeriveInput, FromMeta};
use syn::parse_quote;

#[derive(Debug, FromMeta, PartialEq, Eq)]
enum Dolor {
    Sit,
    #[darling(word)]
    Amet,
}

impl Default for Dolor {
    fn default() -> Self {
        Dolor::Sit
    }
}

#[derive(FromDeriveInput)]
#[darling(attributes(hello))]
struct Receiver {
    #[darling(default)]
    example: Dolor,
}

#[test]
fn missing_meta() {
    let di = Receiver::from_derive_input(&parse_quote! {
        #[hello]
        struct Example;
    })
    .unwrap();

    assert_eq!(Dolor::Sit, di.example);
}

#[test]
fn empty_meta() {
    let di = Receiver::from_derive_input(&parse_quote! {
        #[hello(example)]
        struct Example;
    })
    .unwrap();

    assert_eq!(Dolor::Amet, di.example);
}
