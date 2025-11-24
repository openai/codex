use darling::{util::parse_expr, FromDeriveInput, FromMeta};
use syn::{parse_quote, Expr};

#[derive(FromDeriveInput)]
#[darling(attributes(demo))]
pub struct Receiver {
    #[darling(with = parse_expr::preserve_str_literal, map = Some)]
    example1: Option<Expr>,
    #[darling(
        // A closure can be used in lieu of a path.
        with = |m| Ok(String::from_meta(m)?.to_uppercase()),
        default
    )]
    example2: String,
}

fn main() {
    let input = Receiver::from_derive_input(&parse_quote! {
        #[demo(example1 = test::path, example2 = "hello")]
        struct Example;
    })
    .unwrap();

    assert_eq!(input.example1, Some(parse_quote!(test::path)));
    assert_eq!(input.example2, "HELLO".to_string());
}
