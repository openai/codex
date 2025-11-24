#[macro_use]
mod macros;

use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::quote;
use std::iter::FromIterator;
use syn::{Expr, ExprRange};

#[test]
fn test_expr_parse() {
    let tokens = quote!(..100u32);
    snapshot!(tokens as Expr, @r###"
    Expr::Range {
        limits: HalfOpen,
        to: Some(Expr::Lit {
            lit: 100u32,
        }),
    }
    "###);

    let tokens = quote!(..100u32);
    snapshot!(tokens as ExprRange, @r###"
    ExprRange {
        limits: HalfOpen,
        to: Some(Expr::Lit {
            lit: 100u32,
        }),
    }
    "###);
}

#[test]
fn test_await() {
    // Must not parse as Expr::Field.
    let tokens = quote!(fut.await);

    snapshot!(tokens as Expr, @r###"
    Expr::Await {
        base: Expr::Path {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "fut",
                        arguments: None,
                    },
                ],
            },
        },
    }
    "###);
}

#[rustfmt::skip]
#[test]
fn test_tuple_multi_index() {
    let expected = snapshot!("tuple.0.0" as Expr, @r###"
    Expr::Field {
        base: Expr::Field {
            base: Expr::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "tuple",
                            arguments: None,
                        },
                    ],
                },
            },
            member: Unnamed(Index {
                index: 0,
            }),
        },
        member: Unnamed(Index {
            index: 0,
        }),
    }
    "###);

    for &input in &[
        "tuple .0.0",
        "tuple. 0.0",
        "tuple.0 .0",
        "tuple.0. 0",
        "tuple . 0 . 0",
    ] {
        assert_eq!(expected, syn::parse_str(input).unwrap());
    }

    for tokens in vec![
        quote!(tuple.0.0),
        quote!(tuple .0.0),
        quote!(tuple. 0.0),
        quote!(tuple.0 .0),
        quote!(tuple.0. 0),
        quote!(tuple . 0 . 0),
    ] {
        assert_eq!(expected, syn::parse2(tokens).unwrap());
    }
}

#[test]
fn test_macro_variable_func() {
    // mimics the token stream corresponding to `$fn()`
    let tokens = TokenStream::from_iter(vec![
        TokenTree::Group(Group::new(Delimiter::None, quote! { f })),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
    ]);

    snapshot!(tokens as Expr, @r###"
    Expr::Call {
        func: Expr::Group {
            expr: Expr::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "f",
                            arguments: None,
                        },
                    ],
                },
            },
        },
    }
    "###);

    let tokens = TokenStream::from_iter(vec![
        TokenTree::Punct(Punct::new('#', Spacing::Alone)),
        TokenTree::Group(Group::new(Delimiter::Bracket, quote! { outside })),
        TokenTree::Group(Group::new(Delimiter::None, quote! { #[inside] f })),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
    ]);

    snapshot!(tokens as Expr, @r###"
    Expr::Call {
        attrs: [
            Attribute {
                style: Outer,
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "outside",
                            arguments: None,
                        },
                    ],
                },
                tokens: TokenStream(``),
            },
        ],
        func: Expr::Group {
            expr: Expr::Path {
                attrs: [
                    Attribute {
                        style: Outer,
                        path: Path {
                            segments: [
                                PathSegment {
                                    ident: "inside",
                                    arguments: None,
                                },
                            ],
                        },
                        tokens: TokenStream(``),
                    },
                ],
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "f",
                            arguments: None,
                        },
                    ],
                },
            },
        },
    }
    "###);
}

#[test]
fn test_macro_variable_macro() {
    // mimics the token stream corresponding to `$macro!()`
    let tokens = TokenStream::from_iter(vec![
        TokenTree::Group(Group::new(Delimiter::None, quote! { m })),
        TokenTree::Punct(Punct::new('!', Spacing::Alone)),
        TokenTree::Group(Group::new(Delimiter::Parenthesis, TokenStream::new())),
    ]);

    snapshot!(tokens as Expr, @r###"
    Expr::Macro {
        mac: Macro {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "m",
                        arguments: None,
                    },
                ],
            },
            delimiter: Paren,
            tokens: TokenStream(``),
        },
    }
    "###);
}

#[test]
fn test_macro_variable_struct() {
    // mimics the token stream corresponding to `$struct {}`
    let tokens = TokenStream::from_iter(vec![
        TokenTree::Group(Group::new(Delimiter::None, quote! { S })),
        TokenTree::Group(Group::new(Delimiter::Brace, TokenStream::new())),
    ]);

    snapshot!(tokens as Expr, @r###"
    Expr::Struct {
        path: Path {
            segments: [
                PathSegment {
                    ident: "S",
                    arguments: None,
                },
            ],
        },
    }
    "###);
}

#[test]
fn test_macro_variable_match_arm() {
    // mimics the token stream corresponding to `match v { _ => $expr }`
    let tokens = TokenStream::from_iter(vec![
        TokenTree::Ident(Ident::new("match", Span::call_site())),
        TokenTree::Ident(Ident::new("v", Span::call_site())),
        TokenTree::Group(Group::new(
            Delimiter::Brace,
            TokenStream::from_iter(vec![
                TokenTree::Punct(Punct::new('_', Spacing::Alone)),
                TokenTree::Punct(Punct::new('=', Spacing::Joint)),
                TokenTree::Punct(Punct::new('>', Spacing::Alone)),
                TokenTree::Group(Group::new(Delimiter::None, quote! { #[a] () })),
            ]),
        )),
    ]);

    snapshot!(tokens as Expr, @r###"
    Expr::Match {
        expr: Expr::Path {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "v",
                        arguments: None,
                    },
                ],
            },
        },
        arms: [
            Arm {
                pat: Pat::Wild,
                body: Expr::Group {
                    expr: Expr::Tuple {
                        attrs: [
                            Attribute {
                                style: Outer,
                                path: Path {
                                    segments: [
                                        PathSegment {
                                            ident: "a",
                                            arguments: None,
                                        },
                                    ],
                                },
                                tokens: TokenStream(``),
                            },
                        ],
                    },
                },
            },
        ],
    }
    "###);
}

// https://github.com/dtolnay/syn/issues/1019
#[test]
fn test_closure_vs_rangefull() {
    #[rustfmt::skip] // rustfmt bug: https://github.com/rust-lang/rustfmt/issues/4808
    let tokens = quote!(|| .. .method());
    snapshot!(tokens as Expr, @r###"
    Expr::MethodCall {
        receiver: Expr::Closure {
            output: Default,
            body: Expr::Range {
                limits: HalfOpen,
            },
        },
        method: "method",
    }
    "###);
}

#[test]
fn test_postfix_operator_after_cast() {
    syn::parse_str::<Expr>("|| &x as T[0]").unwrap_err();
    syn::parse_str::<Expr>("|| () as ()()").unwrap_err();
}
