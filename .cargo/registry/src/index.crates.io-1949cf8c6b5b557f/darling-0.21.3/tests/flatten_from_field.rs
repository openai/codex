use darling::{ast, util::Ignored, FromDeriveInput, FromField, FromMeta};
use proc_macro2::{Ident, Span};
use syn::parse_quote;

#[derive(FromMeta)]
struct Vis {
    #[darling(default)]
    public: bool,
    #[darling(default)]
    private: bool,
}

#[derive(FromField)]
#[darling(attributes(v))]
struct Field {
    ident: Option<Ident>,
    example: Option<String>,
    #[darling(flatten)]
    visibility: Vis,
}

#[derive(FromDeriveInput)]
#[darling(attributes(v))]
struct Input {
    data: ast::Data<Ignored, Field>,
}

#[test]
fn field_flattens() {
    let di = Input::from_derive_input(&parse_quote! {
        struct Demo {
            #[v(public, example = "world")]
            hello: String
        }
    })
    .unwrap();

    let fields = di.data.take_struct().unwrap();
    let first_field = fields.into_iter().next().unwrap();
    assert_eq!(
        first_field.ident,
        Some(Ident::new("hello", Span::call_site()))
    );
    assert!(first_field.visibility.public);
    assert!(!first_field.visibility.private);
    assert_eq!(first_field.example.unwrap(), "world");
}

#[test]
fn field_flattens_with_no_field_level_attributes() {
    let di = Input::from_derive_input(&parse_quote! {
        struct Demo {
            hello: String
        }
    })
    .unwrap();

    let fields = di.data.take_struct().unwrap();
    let first_field = fields.into_iter().next().unwrap();
    assert_eq!(
        first_field.ident,
        Some(Ident::new("hello", Span::call_site()))
    );
    assert!(!first_field.visibility.public);
    assert!(!first_field.visibility.private);
    assert_eq!(first_field.example, None);
}

#[test]
fn field_flattens_across_attributes() {
    let di = Input::from_derive_input(&parse_quote! {
        struct Demo {
            #[v(public)]
            #[v(private)]
            hello: String
        }
    })
    .unwrap();

    let fields = di.data.take_struct().unwrap();
    let first_field = fields.into_iter().next().unwrap();
    assert!(first_field.visibility.public);
    assert!(first_field.visibility.private);
}
