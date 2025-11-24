#[macro_use]
mod macros;

use syn::{Expr, Item};

#[test]
fn test_async_fn() {
    let input = "async fn process() {}";

    snapshot!(input as Item, @r###"
    Item::Fn {
        vis: Inherited,
        sig: Signature {
            asyncness: Some,
            ident: "process",
            generics: Generics,
            output: Default,
        },
        block: Block,
    }
    "###);
}

#[test]
fn test_async_closure() {
    let input = "async || {}";

    snapshot!(input as Expr, @r###"
    Expr::Closure {
        asyncness: Some,
        output: Default,
        body: Expr::Block {
            block: Block,
        },
    }
    "###);
}
