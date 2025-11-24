use darling::FromVariant;
use syn::{spanned::Spanned, Expr, ExprLit, LitInt};

#[derive(FromVariant)]
#[darling(from_ident, attributes(hello))]
#[allow(dead_code)]
pub struct Lorem {
    ident: syn::Ident,
    into: Option<bool>,
    skip: Option<bool>,
    discriminant: Option<syn::Expr>,
    fields: darling::ast::Fields<syn::Type>,
}

impl From<syn::Ident> for Lorem {
    fn from(ident: syn::Ident) -> Self {
        Lorem {
            ident,
            into: Default::default(),
            skip: Default::default(),
            discriminant: None,
            fields: darling::ast::Style::Unit.into(),
        }
    }
}

#[test]
fn discriminant() {
    let input: syn::DeriveInput = syn::parse_str(
        r#"
    pub enum Test {
        Works = 1,
        AlsoWorks = 2,
    }
    "#,
    )
    .unwrap();

    let span = input.span();
    if let syn::Data::Enum(enm) = input.data {
        let lorem = Lorem::from_variant(
            enm.variants
                .first()
                .expect("Hardcoded input has one variant"),
        )
        .expect("FromVariant can process the discriminant");
        assert_eq!(
            lorem.discriminant,
            Some(Expr::Lit(ExprLit {
                attrs: vec![],
                lit: LitInt::new("1", span).into(),
            }))
        )
    } else {
        panic!("Data should be enum");
    }
}
