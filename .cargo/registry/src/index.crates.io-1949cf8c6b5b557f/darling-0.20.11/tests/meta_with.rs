use darling::{util::parse_expr, FromDeriveInput, FromMeta};
use syn::{parse_quote, Expr};

#[derive(FromDeriveInput)]
#[darling(attributes(demo))]
pub struct Receiver {
    #[darling(with = parse_expr::preserve_str_literal, map = Some)]
    example1: Option<Expr>,
    #[darling(
        with = |m| Ok(String::from_meta(m)?.to_uppercase()),
        map = Some
    )]
    example2: Option<String>,
    // This is deliberately strange - it keeps the field name, and ignores
    // the rest of the attribute. In normal operation, this is strongly discouraged.
    // It's used here to verify that the parameter type is known even if it can't be
    // inferred from usage within the closure.
    #[darling(with = |m| Ok(m.path().clone()))]
    example3: syn::Path,
}

#[test]
fn handles_all_cases() {
    let input = Receiver::from_derive_input(&parse_quote! {
        #[demo(example1 = test::path, example2 = "hello", example3)]
        struct Example;
    })
    .unwrap();

    assert_eq!(input.example1, Some(parse_quote!(test::path)));
    assert_eq!(input.example2, Some("HELLO".to_string()));
    assert_eq!(input.example3, parse_quote!(example3));
}
