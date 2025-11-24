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
    #[darling(with = "example4_parser")]
    example4: String,
}

// the custom parser function referred to by string
fn example4_parser(meta: &syn::Meta) -> darling::Result<String> {
    match meta {
        syn::Meta::NameValue(nv) => match &nv.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) => Ok(s.value()),
            other => Err(darling::Error::unexpected_expr_type(other)),
        },
        _ => Err(darling::Error::unexpected_type("name-value")),
    }
}

#[test]
fn handles_all_cases() {
    let input = Receiver::from_derive_input(&parse_quote! {
        #[demo(example1 = test::path, example2 = "hello", example3, example4 = "world")]
        struct Example;
    })
    .unwrap();

    assert_eq!(input.example1, Some(parse_quote!(test::path)));
    assert_eq!(input.example2, Some("HELLO".to_string()));
    assert_eq!(input.example3, parse_quote!(example3));
    assert_eq!(input.example4, "world".to_string());
}
