use darling::{ast::Data, util::Flag, FromDeriveInput, FromMeta};
use darling_macro::FromField;

#[derive(Default, FromMeta)]
#[darling(from_word = || Ok(Default::default()), from_expr = |expr| Ok(ErrorPolicy::from(expr)))]
struct ErrorPolicy {
    warn: Flag,
    value: Option<syn::Expr>,
}

impl From<&'_ syn::Expr> for ErrorPolicy {
    fn from(expr: &'_ syn::Expr) -> Self {
        ErrorPolicy {
            warn: Flag::default(),
            value: Some(expr.clone()),
        }
    }
}

#[derive(FromField)]
#[darling(attributes(toml))]
struct Field {
    default: Option<ErrorPolicy>,
    recover: Option<ErrorPolicy>,
}

#[derive(FromDeriveInput)]
#[darling(attributes(toml))]
struct TomlConfig {
    data: Data<(), Field>,
}

fn main() {
    let input = TomlConfig::from_derive_input(&syn::parse_quote! {
        struct Config {
            #[toml(default, recover(warn))]
            field1: String,
            #[toml(default = String::new())]
            field2: String,
        }
    })
    .unwrap();

    assert!(input.data.is_struct());
    let fields = input.data.take_struct().expect("input is struct").fields;
    assert!(fields[0].default.is_some());
    assert!(fields[0]
        .recover
        .as_ref()
        .map(|r| r.warn.is_present())
        .unwrap_or(false));
    assert!(fields[1]
        .default
        .as_ref()
        .map(|d| d.value.is_some())
        .unwrap_or(false));
    assert!(fields[1].recover.is_none());
}
