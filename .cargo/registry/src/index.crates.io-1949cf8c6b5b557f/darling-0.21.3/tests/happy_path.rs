use darling::{FromDeriveInput, FromMeta};
use syn::parse_quote;

#[derive(Default, FromMeta, PartialEq, Debug)]
#[darling(default)]
struct Lorem {
    ipsum: bool,
    dolor: Option<String>,
}

#[derive(FromDeriveInput, PartialEq, Debug)]
#[darling(attributes(darling_demo))]
struct Core {
    ident: syn::Ident,
    vis: syn::Visibility,
    generics: syn::Generics,
    lorem: Lorem,
}

#[derive(FromDeriveInput, PartialEq, Debug)]
#[darling(attributes(darling_demo))]
struct TraitCore {
    ident: syn::Ident,
    generics: syn::Generics,
    lorem: Lorem,
}

#[test]
fn simple() {
    let di = parse_quote! {
        #[derive(Foo)]
        #[darling_demo(lorem(ipsum))]
        pub struct Bar;
    };

    assert_eq!(
        Core::from_derive_input(&di).unwrap(),
        Core {
            ident: parse_quote!(Bar),
            vis: parse_quote!(pub),
            generics: Default::default(),
            lorem: Lorem {
                ipsum: true,
                dolor: None,
            },
        }
    );
}

#[test]
fn trait_type() {
    let di = parse_quote! {
        #[derive(Foo)]
        #[darling_demo(lorem(dolor = "hello"))]
        pub struct Bar;
    };

    assert_eq!(
        TraitCore::from_derive_input(&di).unwrap(),
        TraitCore {
            ident: parse_quote!(Bar),
            generics: Default::default(),
            lorem: Lorem {
                ipsum: false,
                dolor: Some("hello".to_owned()),
            }
        }
    );
}
