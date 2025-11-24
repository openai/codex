// The use of fields in debug print commands does not count as "used",
// which causes the fields to trigger an unwanted dead code warning.
#![allow(dead_code)]

use darling::{ast, util, FromDeriveInput, FromField};
use syn::{Ident, Type};

#[derive(Debug, FromField)]
#[darling(attributes(lorem))]
pub struct LoremField {
    ident: Option<Ident>,
    ty: Type,
    #[darling(default)]
    skip: bool,
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(lorem), supports(struct_named))]
pub struct Lorem {
    ident: Ident,
    data: ast::Data<util::Ignored, LoremField>,
}

fn main() {
    let good_input = r#"#[derive(Lorem)]
pub struct Foo {
    #[lorem(skip)]
    bar: bool,

    baz: i64,
}"#;

    let bad_input = r#"#[derive(Lorem)]
    pub struct BadFoo(String, u32);"#;

    let parsed = syn::parse_str(good_input).unwrap();
    let receiver = Lorem::from_derive_input(&parsed).unwrap();
    let wrong_shape_parsed = syn::parse_str(bad_input).unwrap();
    let wrong_shape = Lorem::from_derive_input(&wrong_shape_parsed).expect_err("Shape was wrong");

    println!(
        r#"
INPUT:

{}

PARSED AS:

{:?}

BAD INPUT:

{}

PRODUCED ERROR:

{}
    "#,
        good_input, receiver, bad_input, wrong_shape
    );
}
