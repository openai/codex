#[macro_use]
mod macros;

use proc_macro2::{Delimiter, Group, Literal, Punct, Spacing, TokenStream, TokenTree};
use std::iter::FromIterator;
use syn::Expr;

#[test]
fn test_grouping() {
    let tokens: TokenStream = TokenStream::from_iter(vec![
        TokenTree::Literal(Literal::i32_suffixed(1)),
        TokenTree::Punct(Punct::new('+', Spacing::Alone)),
        TokenTree::Group(Group::new(
            Delimiter::None,
            TokenStream::from_iter(vec![
                TokenTree::Literal(Literal::i32_suffixed(2)),
                TokenTree::Punct(Punct::new('+', Spacing::Alone)),
                TokenTree::Literal(Literal::i32_suffixed(3)),
            ]),
        )),
        TokenTree::Punct(Punct::new('*', Spacing::Alone)),
        TokenTree::Literal(Literal::i32_suffixed(4)),
    ]);

    assert_eq!(tokens.to_string(), "1i32 + 2i32 + 3i32 * 4i32");

    snapshot!(tokens as Expr, @r###"
    Expr::Binary {
        left: Expr::Lit {
            lit: 1i32,
        },
        op: Add,
        right: Expr::Binary {
            left: Expr::Group {
                expr: Expr::Binary {
                    left: Expr::Lit {
                        lit: 2i32,
                    },
                    op: Add,
                    right: Expr::Lit {
                        lit: 3i32,
                    },
                },
            },
            op: Mul,
            right: Expr::Lit {
                lit: 4i32,
            },
        },
    }
    "###);
}
