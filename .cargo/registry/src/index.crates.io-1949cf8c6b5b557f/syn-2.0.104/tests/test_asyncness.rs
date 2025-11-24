#![allow(
    clippy::elidable_lifetime_names,
    clippy::needless_lifetimes,
    clippy::uninlined_format_args
)]

#[macro_use]
mod macros;

use syn::{Expr, Item};

#[test]
fn test_async_fn() {
    let input = "async fn process() {}";

    snapshot!(input as Item, @r#"
    Item::Fn {
        vis: Visibility::Inherited,
        sig: Signature {
            asyncness: Some,
            ident: "process",
            generics: Generics,
            output: ReturnType::Default,
        },
        block: Block {
            stmts: [],
        },
    }
    "#);
}

#[test]
fn test_async_closure() {
    let input = "async || {}";

    snapshot!(input as Expr, @r#"
    Expr::Closure {
        asyncness: Some,
        output: ReturnType::Default,
        body: Expr::Block {
            block: Block {
                stmts: [],
            },
        },
    }
    "#);
}
