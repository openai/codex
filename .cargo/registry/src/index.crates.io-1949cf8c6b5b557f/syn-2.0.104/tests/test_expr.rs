#![cfg(not(miri))]
#![recursion_limit = "1024"]
#![feature(rustc_private)]
#![allow(
    clippy::elidable_lifetime_names,
    clippy::match_like_matches_macro,
    clippy::needless_lifetimes,
    clippy::single_element_loop,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unreadable_literal
)]

#[macro_use]
mod macros;

mod common;

use crate::common::visit::{AsIfPrinted, FlattenParens};
use proc_macro2::{Delimiter, Group, Ident, Span, TokenStream};
use quote::{quote, ToTokens as _};
use std::process::ExitCode;
use syn::punctuated::Punctuated;
use syn::visit_mut::VisitMut as _;
use syn::{
    parse_quote, token, AngleBracketedGenericArguments, Arm, BinOp, Block, Expr, ExprArray,
    ExprAssign, ExprAsync, ExprAwait, ExprBinary, ExprBlock, ExprBreak, ExprCall, ExprCast,
    ExprClosure, ExprConst, ExprContinue, ExprField, ExprForLoop, ExprIf, ExprIndex, ExprLet,
    ExprLit, ExprLoop, ExprMacro, ExprMatch, ExprMethodCall, ExprPath, ExprRange, ExprRawAddr,
    ExprReference, ExprReturn, ExprStruct, ExprTry, ExprTryBlock, ExprTuple, ExprUnary, ExprUnsafe,
    ExprWhile, ExprYield, GenericArgument, Label, Lifetime, Lit, LitInt, Macro, MacroDelimiter,
    Member, Pat, PatWild, Path, PathArguments, PathSegment, PointerMutability, QSelf, RangeLimits,
    ReturnType, Stmt, Token, Type, TypePath, UnOp,
};

#[test]
fn test_expr_parse() {
    let tokens = quote!(..100u32);
    snapshot!(tokens as Expr, @r#"
    Expr::Range {
        limits: RangeLimits::HalfOpen,
        end: Some(Expr::Lit {
            lit: 100u32,
        }),
    }
    "#);

    let tokens = quote!(..100u32);
    snapshot!(tokens as ExprRange, @r#"
    ExprRange {
        limits: RangeLimits::HalfOpen,
        end: Some(Expr::Lit {
            lit: 100u32,
        }),
    }
    "#);
}

#[test]
fn test_await() {
    // Must not parse as Expr::Field.
    let tokens = quote!(fut.await);

    snapshot!(tokens as Expr, @r#"
    Expr::Await {
        base: Expr::Path {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "fut",
                    },
                ],
            },
        },
    }
    "#);
}

#[rustfmt::skip]
#[test]
fn test_tuple_multi_index() {
    let expected = snapshot!("tuple.0.0" as Expr, @r#"
    Expr::Field {
        base: Expr::Field {
            base: Expr::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "tuple",
                        },
                    ],
                },
            },
            member: Member::Unnamed(Index {
                index: 0,
            }),
        },
        member: Member::Unnamed(Index {
            index: 0,
        }),
    }
    "#);

    for &input in &[
        "tuple .0.0",
        "tuple. 0.0",
        "tuple.0 .0",
        "tuple.0. 0",
        "tuple . 0 . 0",
    ] {
        assert_eq!(expected, syn::parse_str(input).unwrap());
    }

    for tokens in [
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
    let path = Group::new(Delimiter::None, quote!(f));
    let tokens = quote!(#path());

    snapshot!(tokens as Expr, @r#"
    Expr::Call {
        func: Expr::Group {
            expr: Expr::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "f",
                        },
                    ],
                },
            },
        },
    }
    "#);

    let path = Group::new(Delimiter::None, quote! { #[inside] f });
    let tokens = quote!(#[outside] #path());

    snapshot!(tokens as Expr, @r#"
    Expr::Call {
        attrs: [
            Attribute {
                style: AttrStyle::Outer,
                meta: Meta::Path {
                    segments: [
                        PathSegment {
                            ident: "outside",
                        },
                    ],
                },
            },
        ],
        func: Expr::Group {
            expr: Expr::Path {
                attrs: [
                    Attribute {
                        style: AttrStyle::Outer,
                        meta: Meta::Path {
                            segments: [
                                PathSegment {
                                    ident: "inside",
                                },
                            ],
                        },
                    },
                ],
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "f",
                        },
                    ],
                },
            },
        },
    }
    "#);
}

#[test]
fn test_macro_variable_macro() {
    // mimics the token stream corresponding to `$macro!()`
    let mac = Group::new(Delimiter::None, quote!(m));
    let tokens = quote!(#mac!());

    snapshot!(tokens as Expr, @r#"
    Expr::Macro {
        mac: Macro {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "m",
                    },
                ],
            },
            delimiter: MacroDelimiter::Paren,
            tokens: TokenStream(``),
        },
    }
    "#);
}

#[test]
fn test_macro_variable_struct() {
    // mimics the token stream corresponding to `$struct {}`
    let s = Group::new(Delimiter::None, quote! { S });
    let tokens = quote!(#s {});

    snapshot!(tokens as Expr, @r#"
    Expr::Struct {
        path: Path {
            segments: [
                PathSegment {
                    ident: "S",
                },
            ],
        },
    }
    "#);
}

#[test]
fn test_macro_variable_unary() {
    // mimics the token stream corresponding to `$expr.method()` where expr is `&self`
    let inner = Group::new(Delimiter::None, quote!(&self));
    let tokens = quote!(#inner.method());
    snapshot!(tokens as Expr, @r#"
    Expr::MethodCall {
        receiver: Expr::Group {
            expr: Expr::Reference {
                expr: Expr::Path {
                    path: Path {
                        segments: [
                            PathSegment {
                                ident: "self",
                            },
                        ],
                    },
                },
            },
        },
        method: "method",
    }
    "#);
}

#[test]
fn test_macro_variable_match_arm() {
    // mimics the token stream corresponding to `match v { _ => $expr }`
    let expr = Group::new(Delimiter::None, quote! { #[a] () });
    let tokens = quote!(match v { _ => #expr });
    snapshot!(tokens as Expr, @r#"
    Expr::Match {
        expr: Expr::Path {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "v",
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
                                style: AttrStyle::Outer,
                                meta: Meta::Path {
                                    segments: [
                                        PathSegment {
                                            ident: "a",
                                        },
                                    ],
                                },
                            },
                        ],
                    },
                },
            },
        ],
    }
    "#);

    let expr = Group::new(Delimiter::None, quote!(loop {} + 1));
    let tokens = quote!(match v { _ => #expr });
    snapshot!(tokens as Expr, @r#"
    Expr::Match {
        expr: Expr::Path {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "v",
                    },
                ],
            },
        },
        arms: [
            Arm {
                pat: Pat::Wild,
                body: Expr::Group {
                    expr: Expr::Binary {
                        left: Expr::Loop {
                            body: Block {
                                stmts: [],
                            },
                        },
                        op: BinOp::Add,
                        right: Expr::Lit {
                            lit: 1,
                        },
                    },
                },
            },
        ],
    }
    "#);
}

// https://github.com/dtolnay/syn/issues/1019
#[test]
fn test_closure_vs_rangefull() {
    #[rustfmt::skip] // rustfmt bug: https://github.com/rust-lang/rustfmt/issues/4808
    let tokens = quote!(|| .. .method());
    snapshot!(tokens as Expr, @r#"
    Expr::MethodCall {
        receiver: Expr::Closure {
            output: ReturnType::Default,
            body: Expr::Range {
                limits: RangeLimits::HalfOpen,
            },
        },
        method: "method",
    }
    "#);
}

#[test]
fn test_postfix_operator_after_cast() {
    syn::parse_str::<Expr>("|| &x as T[0]").unwrap_err();
    syn::parse_str::<Expr>("|| () as ()()").unwrap_err();
}

#[test]
fn test_range_kinds() {
    syn::parse_str::<Expr>("..").unwrap();
    syn::parse_str::<Expr>("..hi").unwrap();
    syn::parse_str::<Expr>("lo..").unwrap();
    syn::parse_str::<Expr>("lo..hi").unwrap();

    syn::parse_str::<Expr>("..=").unwrap_err();
    syn::parse_str::<Expr>("..=hi").unwrap();
    syn::parse_str::<Expr>("lo..=").unwrap_err();
    syn::parse_str::<Expr>("lo..=hi").unwrap();

    syn::parse_str::<Expr>("...").unwrap_err();
    syn::parse_str::<Expr>("...hi").unwrap_err();
    syn::parse_str::<Expr>("lo...").unwrap_err();
    syn::parse_str::<Expr>("lo...hi").unwrap_err();
}

#[test]
fn test_range_precedence() {
    snapshot!(".. .." as Expr, @r#"
    Expr::Range {
        limits: RangeLimits::HalfOpen,
        end: Some(Expr::Range {
            limits: RangeLimits::HalfOpen,
        }),
    }
    "#);

    snapshot!(".. .. ()" as Expr, @r#"
    Expr::Range {
        limits: RangeLimits::HalfOpen,
        end: Some(Expr::Range {
            limits: RangeLimits::HalfOpen,
            end: Some(Expr::Tuple),
        }),
    }
    "#);

    snapshot!("() .. .." as Expr, @r#"
    Expr::Range {
        start: Some(Expr::Tuple),
        limits: RangeLimits::HalfOpen,
        end: Some(Expr::Range {
            limits: RangeLimits::HalfOpen,
        }),
    }
    "#);

    snapshot!("() = .. + ()" as Expr, @r"
    Expr::Binary {
        left: Expr::Assign {
            left: Expr::Tuple,
            right: Expr::Range {
                limits: RangeLimits::HalfOpen,
            },
        },
        op: BinOp::Add,
        right: Expr::Tuple,
    }
    ");

    // A range with a lower bound cannot be the upper bound of another range,
    // and a range with an upper bound cannot be the lower bound of another
    // range.
    syn::parse_str::<Expr>(".. x ..").unwrap_err();
    syn::parse_str::<Expr>("x .. x ..").unwrap_err();
}

#[test]
fn test_range_attrs() {
    // Attributes are not allowed on range expressions starting with `..`
    syn::parse_str::<Expr>("#[allow()] ..").unwrap_err();
    syn::parse_str::<Expr>("#[allow()] .. hi").unwrap_err();

    snapshot!("#[allow()] lo .. hi" as Expr, @r#"
    Expr::Range {
        start: Some(Expr::Path {
            attrs: [
                Attribute {
                    style: AttrStyle::Outer,
                    meta: Meta::List {
                        path: Path {
                            segments: [
                                PathSegment {
                                    ident: "allow",
                                },
                            ],
                        },
                        delimiter: MacroDelimiter::Paren,
                        tokens: TokenStream(``),
                    },
                },
            ],
            path: Path {
                segments: [
                    PathSegment {
                        ident: "lo",
                    },
                ],
            },
        }),
        limits: RangeLimits::HalfOpen,
        end: Some(Expr::Path {
            path: Path {
                segments: [
                    PathSegment {
                        ident: "hi",
                    },
                ],
            },
        }),
    }
    "#);
}

#[test]
fn test_ranges_bailout() {
    syn::parse_str::<Expr>(".. ?").unwrap_err();
    syn::parse_str::<Expr>(".. .field").unwrap_err();

    snapshot!("return .. ?" as Expr, @r"
    Expr::Try {
        expr: Expr::Return {
            expr: Some(Expr::Range {
                limits: RangeLimits::HalfOpen,
            }),
        },
    }
    ");

    snapshot!("break .. ?" as Expr, @r"
    Expr::Try {
        expr: Expr::Break {
            expr: Some(Expr::Range {
                limits: RangeLimits::HalfOpen,
            }),
        },
    }
    ");

    snapshot!("|| .. ?" as Expr, @r"
    Expr::Try {
        expr: Expr::Closure {
            output: ReturnType::Default,
            body: Expr::Range {
                limits: RangeLimits::HalfOpen,
            },
        },
    }
    ");

    snapshot!("return .. .field" as Expr, @r#"
    Expr::Field {
        base: Expr::Return {
            expr: Some(Expr::Range {
                limits: RangeLimits::HalfOpen,
            }),
        },
        member: Member::Named("field"),
    }
    "#);

    snapshot!("break .. .field" as Expr, @r#"
    Expr::Field {
        base: Expr::Break {
            expr: Some(Expr::Range {
                limits: RangeLimits::HalfOpen,
            }),
        },
        member: Member::Named("field"),
    }
    "#);

    snapshot!("|| .. .field" as Expr, @r#"
    Expr::Field {
        base: Expr::Closure {
            output: ReturnType::Default,
            body: Expr::Range {
                limits: RangeLimits::HalfOpen,
            },
        },
        member: Member::Named("field"),
    }
    "#);

    snapshot!("return .. = ()" as Expr, @r"
    Expr::Assign {
        left: Expr::Return {
            expr: Some(Expr::Range {
                limits: RangeLimits::HalfOpen,
            }),
        },
        right: Expr::Tuple,
    }
    ");

    snapshot!("return .. += ()" as Expr, @r"
    Expr::Binary {
        left: Expr::Return {
            expr: Some(Expr::Range {
                limits: RangeLimits::HalfOpen,
            }),
        },
        op: BinOp::AddAssign,
        right: Expr::Tuple,
    }
    ");
}

#[test]
fn test_ambiguous_label() {
    for stmt in [
        quote! {
            return 'label: loop { break 'label 42; };
        },
        quote! {
            break ('label: loop { break 'label 42; });
        },
        quote! {
            break 1 + 'label: loop { break 'label 42; };
        },
        quote! {
            break 'outer 'inner: loop { break 'inner 42; };
        },
    ] {
        syn::parse2::<Stmt>(stmt).unwrap();
    }

    for stmt in [
        // Parentheses required. See https://github.com/rust-lang/rust/pull/87026.
        quote! {
            break 'label: loop { break 'label 42; };
        },
    ] {
        syn::parse2::<Stmt>(stmt).unwrap_err();
    }
}

#[test]
fn test_extended_interpolated_path() {
    let path = Group::new(Delimiter::None, quote!(a::b));

    let tokens = quote!(if #path {});
    snapshot!(tokens as Expr, @r#"
    Expr::If {
        cond: Expr::Group {
            expr: Expr::Path {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "a",
                        },
                        Token![::],
                        PathSegment {
                            ident: "b",
                        },
                    ],
                },
            },
        },
        then_branch: Block {
            stmts: [],
        },
    }
    "#);

    let tokens = quote!(#path {});
    snapshot!(tokens as Expr, @r#"
    Expr::Struct {
        path: Path {
            segments: [
                PathSegment {
                    ident: "a",
                },
                Token![::],
                PathSegment {
                    ident: "b",
                },
            ],
        },
    }
    "#);

    let tokens = quote!(#path :: c);
    snapshot!(tokens as Expr, @r#"
    Expr::Path {
        path: Path {
            segments: [
                PathSegment {
                    ident: "a",
                },
                Token![::],
                PathSegment {
                    ident: "b",
                },
                Token![::],
                PathSegment {
                    ident: "c",
                },
            ],
        },
    }
    "#);

    let nested = Group::new(Delimiter::None, quote!(a::b || true));
    let tokens = quote!(if #nested && false {});
    snapshot!(tokens as Expr, @r#"
    Expr::If {
        cond: Expr::Binary {
            left: Expr::Group {
                expr: Expr::Binary {
                    left: Expr::Path {
                        path: Path {
                            segments: [
                                PathSegment {
                                    ident: "a",
                                },
                                Token![::],
                                PathSegment {
                                    ident: "b",
                                },
                            ],
                        },
                    },
                    op: BinOp::Or,
                    right: Expr::Lit {
                        lit: Lit::Bool {
                            value: true,
                        },
                    },
                },
            },
            op: BinOp::And,
            right: Expr::Lit {
                lit: Lit::Bool {
                    value: false,
                },
            },
        },
        then_branch: Block {
            stmts: [],
        },
    }
    "#);
}

#[test]
fn test_tuple_comma() {
    let mut expr = ExprTuple {
        attrs: Vec::new(),
        paren_token: token::Paren::default(),
        elems: Punctuated::new(),
    };
    snapshot!(expr.to_token_stream() as Expr, @"Expr::Tuple");

    expr.elems.push_value(parse_quote!(continue));
    // Must not parse to Expr::Paren
    snapshot!(expr.to_token_stream() as Expr, @r#"
    Expr::Tuple {
        elems: [
            Expr::Continue,
            Token![,],
        ],
    }
    "#);

    expr.elems.push_punct(<Token![,]>::default());
    snapshot!(expr.to_token_stream() as Expr, @r#"
    Expr::Tuple {
        elems: [
            Expr::Continue,
            Token![,],
        ],
    }
    "#);

    expr.elems.push_value(parse_quote!(continue));
    snapshot!(expr.to_token_stream() as Expr, @r#"
    Expr::Tuple {
        elems: [
            Expr::Continue,
            Token![,],
            Expr::Continue,
        ],
    }
    "#);

    expr.elems.push_punct(<Token![,]>::default());
    snapshot!(expr.to_token_stream() as Expr, @r#"
    Expr::Tuple {
        elems: [
            Expr::Continue,
            Token![,],
            Expr::Continue,
            Token![,],
        ],
    }
    "#);
}

#[test]
fn test_binop_associativity() {
    // Left to right.
    snapshot!("() + () + ()" as Expr, @r#"
    Expr::Binary {
        left: Expr::Binary {
            left: Expr::Tuple,
            op: BinOp::Add,
            right: Expr::Tuple,
        },
        op: BinOp::Add,
        right: Expr::Tuple,
    }
    "#);

    // Right to left.
    snapshot!("() += () += ()" as Expr, @r#"
    Expr::Binary {
        left: Expr::Tuple,
        op: BinOp::AddAssign,
        right: Expr::Binary {
            left: Expr::Tuple,
            op: BinOp::AddAssign,
            right: Expr::Tuple,
        },
    }
    "#);

    // Parenthesization is required.
    syn::parse_str::<Expr>("() == () == ()").unwrap_err();
}

#[test]
fn test_assign_range_precedence() {
    // Range has higher precedence as the right-hand of an assignment, but
    // ambiguous precedence as the left-hand of an assignment.
    snapshot!("() = () .. ()" as Expr, @r#"
    Expr::Assign {
        left: Expr::Tuple,
        right: Expr::Range {
            start: Some(Expr::Tuple),
            limits: RangeLimits::HalfOpen,
            end: Some(Expr::Tuple),
        },
    }
    "#);

    snapshot!("() += () .. ()" as Expr, @r#"
    Expr::Binary {
        left: Expr::Tuple,
        op: BinOp::AddAssign,
        right: Expr::Range {
            start: Some(Expr::Tuple),
            limits: RangeLimits::HalfOpen,
            end: Some(Expr::Tuple),
        },
    }
    "#);

    syn::parse_str::<Expr>("() .. () = ()").unwrap_err();
    syn::parse_str::<Expr>("() .. () += ()").unwrap_err();
}

#[test]
fn test_chained_comparison() {
    // https://github.com/dtolnay/syn/issues/1738
    let _ = syn::parse_str::<Expr>("a = a < a <");
    let _ = syn::parse_str::<Expr>("a = a .. a ..");
    let _ = syn::parse_str::<Expr>("a = a .. a +=");

    let err = syn::parse_str::<Expr>("a < a < a").unwrap_err();
    assert_eq!("comparison operators cannot be chained", err.to_string());

    let err = syn::parse_str::<Expr>("a .. a .. a").unwrap_err();
    assert_eq!("unexpected token", err.to_string());

    let err = syn::parse_str::<Expr>("a .. a += a").unwrap_err();
    assert_eq!("unexpected token", err.to_string());
}

#[test]
fn test_fixup() {
    for tokens in [
        quote! { 2 * (1 + 1) },
        quote! { 0 + (0 + 0) },
        quote! { (a = b) = c },
        quote! { (x as i32) < 0 },
        quote! { 1 + (x as i32) < 0 },
        quote! { (1 + 1).abs() },
        quote! { (lo..hi)[..] },
        quote! { (a..b)..(c..d) },
        quote! { (x > ..) > x },
        quote! { (&mut fut).await },
        quote! { &mut (x as i32) },
        quote! { -(x as i32) },
        quote! { if (S {}) == 1 {} },
        quote! { { (m! {}) - 1 } },
        quote! { match m { _ => ({}) - 1 } },
        quote! { if let _ = (a && b) && c {} },
        quote! { if let _ = (S {}) {} },
        quote! { if (S {}) == 0 && let Some(_) = x {} },
        quote! { break ('a: loop { break 'a 1 } + 1) },
        quote! { a + (|| b) + c },
        quote! { if let _ = ((break) - 1 || true) {} },
        quote! { if let _ = (break + 1 || true) {} },
        quote! { if break (break) {} },
        quote! { if break break {} {} },
        quote! { if return (..) {} },
        quote! { if return .. {} {} },
        quote! { if || (Struct {}) {} },
        quote! { if || (Struct {}).await {} },
        quote! { if break || Struct {}.await {} },
        quote! { if break 'outer 'block: {} {} },
        quote! { if ..'block: {} {} },
        quote! { if break ({}).await {} },
        quote! { (break)() },
        quote! { (..) = () },
        quote! { (..) += () },
        quote! { (1 < 2) == (3 < 4) },
        quote! { { (let _ = ()) } },
        quote! { (#[attr] thing).field },
        quote! { #[attr] (1 + 1) },
        quote! { #[attr] (x = 1) },
        quote! { #[attr] (x += 1) },
        quote! { #[attr] (1 as T) },
        quote! { (return #[attr] (x + ..)).field },
        quote! { (self.f)() },
        quote! { (return)..=return },
        quote! { 1 + (return)..=1 + return },
        quote! { .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. .. },
    ] {
        let original: Expr = syn::parse2(tokens).unwrap();

        let mut flat = original.clone();
        FlattenParens::combine_attrs().visit_expr_mut(&mut flat);
        let reconstructed: Expr = match syn::parse2(flat.to_token_stream()) {
            Ok(reconstructed) => reconstructed,
            Err(err) => panic!("failed to parse `{}`: {}", flat.to_token_stream(), err),
        };

        assert!(
            original == reconstructed,
            "original: {}\n{:#?}\nreconstructed: {}\n{:#?}",
            original.to_token_stream(),
            crate::macros::debug::Lite(&original),
            reconstructed.to_token_stream(),
            crate::macros::debug::Lite(&reconstructed),
        );
    }
}

#[test]
fn test_permutations() -> ExitCode {
    fn iter(depth: usize, f: &mut dyn FnMut(Expr)) {
        let span = Span::call_site();

        // Expr::Path
        f(Expr::Path(ExprPath {
            // `x`
            attrs: Vec::new(),
            qself: None,
            path: Path::from(Ident::new("x", span)),
        }));
        if false {
            f(Expr::Path(ExprPath {
                // `x::<T>`
                attrs: Vec::new(),
                qself: None,
                path: Path {
                    leading_colon: None,
                    segments: Punctuated::from_iter([PathSegment {
                        ident: Ident::new("x", span),
                        arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                            colon2_token: Some(Token![::](span)),
                            lt_token: Token![<](span),
                            args: Punctuated::from_iter([GenericArgument::Type(Type::Path(
                                TypePath {
                                    qself: None,
                                    path: Path::from(Ident::new("T", span)),
                                },
                            ))]),
                            gt_token: Token![>](span),
                        }),
                    }]),
                },
            }));
            f(Expr::Path(ExprPath {
                // `<T as Trait>::CONST`
                attrs: Vec::new(),
                qself: Some(QSelf {
                    lt_token: Token![<](span),
                    ty: Box::new(Type::Path(TypePath {
                        qself: None,
                        path: Path::from(Ident::new("T", span)),
                    })),
                    position: 1,
                    as_token: Some(Token![as](span)),
                    gt_token: Token![>](span),
                }),
                path: Path {
                    leading_colon: None,
                    segments: Punctuated::from_iter([
                        PathSegment::from(Ident::new("Trait", span)),
                        PathSegment::from(Ident::new("CONST", span)),
                    ]),
                },
            }));
        }

        let Some(depth) = depth.checked_sub(1) else {
            return;
        };

        // Expr::Assign
        iter(depth, &mut |expr| {
            iter(0, &mut |simple| {
                f(Expr::Assign(ExprAssign {
                    // `x = $expr`
                    attrs: Vec::new(),
                    left: Box::new(simple.clone()),
                    eq_token: Token![=](span),
                    right: Box::new(expr.clone()),
                }));
                f(Expr::Assign(ExprAssign {
                    // `$expr = x`
                    attrs: Vec::new(),
                    left: Box::new(expr.clone()),
                    eq_token: Token![=](span),
                    right: Box::new(simple),
                }));
            });
        });

        // Expr::Binary
        iter(depth, &mut |expr| {
            iter(0, &mut |simple| {
                for op in [
                    BinOp::Add(Token![+](span)),
                    //BinOp::Sub(Token![-](span)),
                    //BinOp::Mul(Token![*](span)),
                    //BinOp::Div(Token![/](span)),
                    //BinOp::Rem(Token![%](span)),
                    //BinOp::And(Token![&&](span)),
                    //BinOp::Or(Token![||](span)),
                    //BinOp::BitXor(Token![^](span)),
                    //BinOp::BitAnd(Token![&](span)),
                    //BinOp::BitOr(Token![|](span)),
                    //BinOp::Shl(Token![<<](span)),
                    //BinOp::Shr(Token![>>](span)),
                    //BinOp::Eq(Token![==](span)),
                    BinOp::Lt(Token![<](span)),
                    //BinOp::Le(Token![<=](span)),
                    //BinOp::Ne(Token![!=](span)),
                    //BinOp::Ge(Token![>=](span)),
                    //BinOp::Gt(Token![>](span)),
                    BinOp::ShlAssign(Token![<<=](span)),
                ] {
                    f(Expr::Binary(ExprBinary {
                        // `x + $expr`
                        attrs: Vec::new(),
                        left: Box::new(simple.clone()),
                        op,
                        right: Box::new(expr.clone()),
                    }));
                    f(Expr::Binary(ExprBinary {
                        // `$expr + x`
                        attrs: Vec::new(),
                        left: Box::new(expr.clone()),
                        op,
                        right: Box::new(simple.clone()),
                    }));
                }
            });
        });

        // Expr::Block
        f(Expr::Block(ExprBlock {
            // `{}`
            attrs: Vec::new(),
            label: None,
            block: Block {
                brace_token: token::Brace(span),
                stmts: Vec::new(),
            },
        }));

        // Expr::Break
        f(Expr::Break(ExprBreak {
            // `break`
            attrs: Vec::new(),
            break_token: Token![break](span),
            label: None,
            expr: None,
        }));
        iter(depth, &mut |expr| {
            f(Expr::Break(ExprBreak {
                // `break $expr`
                attrs: Vec::new(),
                break_token: Token![break](span),
                label: None,
                expr: Some(Box::new(expr)),
            }));
        });

        // Expr::Call
        iter(depth, &mut |expr| {
            f(Expr::Call(ExprCall {
                // `$expr()`
                attrs: Vec::new(),
                func: Box::new(expr),
                paren_token: token::Paren(span),
                args: Punctuated::new(),
            }));
        });

        // Expr::Cast
        iter(depth, &mut |expr| {
            f(Expr::Cast(ExprCast {
                // `$expr as T`
                attrs: Vec::new(),
                expr: Box::new(expr),
                as_token: Token![as](span),
                ty: Box::new(Type::Path(TypePath {
                    qself: None,
                    path: Path::from(Ident::new("T", span)),
                })),
            }));
        });

        // Expr::Closure
        iter(depth, &mut |expr| {
            f(Expr::Closure(ExprClosure {
                // `|| $expr`
                attrs: Vec::new(),
                lifetimes: None,
                constness: None,
                movability: None,
                asyncness: None,
                capture: None,
                or1_token: Token![|](span),
                inputs: Punctuated::new(),
                or2_token: Token![|](span),
                output: ReturnType::Default,
                body: Box::new(expr),
            }));
        });

        // Expr::Field
        iter(depth, &mut |expr| {
            f(Expr::Field(ExprField {
                // `$expr.field`
                attrs: Vec::new(),
                base: Box::new(expr),
                dot_token: Token![.](span),
                member: Member::Named(Ident::new("field", span)),
            }));
        });

        // Expr::If
        iter(depth, &mut |expr| {
            f(Expr::If(ExprIf {
                // `if $expr {}`
                attrs: Vec::new(),
                if_token: Token![if](span),
                cond: Box::new(expr),
                then_branch: Block {
                    brace_token: token::Brace(span),
                    stmts: Vec::new(),
                },
                else_branch: None,
            }));
        });

        // Expr::Let
        iter(depth, &mut |expr| {
            f(Expr::Let(ExprLet {
                attrs: Vec::new(),
                let_token: Token![let](span),
                pat: Box::new(Pat::Wild(PatWild {
                    attrs: Vec::new(),
                    underscore_token: Token![_](span),
                })),
                eq_token: Token![=](span),
                expr: Box::new(expr),
            }));
        });

        // Expr::Range
        f(Expr::Range(ExprRange {
            // `..`
            attrs: Vec::new(),
            start: None,
            limits: RangeLimits::HalfOpen(Token![..](span)),
            end: None,
        }));
        iter(depth, &mut |expr| {
            f(Expr::Range(ExprRange {
                // `..$expr`
                attrs: Vec::new(),
                start: None,
                limits: RangeLimits::HalfOpen(Token![..](span)),
                end: Some(Box::new(expr.clone())),
            }));
            f(Expr::Range(ExprRange {
                // `$expr..`
                attrs: Vec::new(),
                start: Some(Box::new(expr)),
                limits: RangeLimits::HalfOpen(Token![..](span)),
                end: None,
            }));
        });

        // Expr::Reference
        iter(depth, &mut |expr| {
            f(Expr::Reference(ExprReference {
                // `&$expr`
                attrs: Vec::new(),
                and_token: Token![&](span),
                mutability: None,
                expr: Box::new(expr),
            }));
        });

        // Expr::Return
        f(Expr::Return(ExprReturn {
            // `return`
            attrs: Vec::new(),
            return_token: Token![return](span),
            expr: None,
        }));
        iter(depth, &mut |expr| {
            f(Expr::Return(ExprReturn {
                // `return $expr`
                attrs: Vec::new(),
                return_token: Token![return](span),
                expr: Some(Box::new(expr)),
            }));
        });

        // Expr::Try
        iter(depth, &mut |expr| {
            f(Expr::Try(ExprTry {
                // `$expr?`
                attrs: Vec::new(),
                expr: Box::new(expr),
                question_token: Token![?](span),
            }));
        });

        // Expr::Unary
        iter(depth, &mut |expr| {
            for op in [
                UnOp::Deref(Token![*](span)),
                //UnOp::Not(Token![!](span)),
                //UnOp::Neg(Token![-](span)),
            ] {
                f(Expr::Unary(ExprUnary {
                    // `*$expr`
                    attrs: Vec::new(),
                    op,
                    expr: Box::new(expr.clone()),
                }));
            }
        });

        if false {
            // Expr::Array
            f(Expr::Array(ExprArray {
                // `[]`
                attrs: Vec::new(),
                bracket_token: token::Bracket(span),
                elems: Punctuated::new(),
            }));

            // Expr::Async
            f(Expr::Async(ExprAsync {
                // `async {}`
                attrs: Vec::new(),
                async_token: Token![async](span),
                capture: None,
                block: Block {
                    brace_token: token::Brace(span),
                    stmts: Vec::new(),
                },
            }));

            // Expr::Await
            iter(depth, &mut |expr| {
                f(Expr::Await(ExprAwait {
                    // `$expr.await`
                    attrs: Vec::new(),
                    base: Box::new(expr),
                    dot_token: Token![.](span),
                    await_token: Token![await](span),
                }));
            });

            // Expr::Block
            f(Expr::Block(ExprBlock {
                // `'a: {}`
                attrs: Vec::new(),
                label: Some(Label {
                    name: Lifetime::new("'a", span),
                    colon_token: Token![:](span),
                }),
                block: Block {
                    brace_token: token::Brace(span),
                    stmts: Vec::new(),
                },
            }));
            iter(depth, &mut |expr| {
                f(Expr::Block(ExprBlock {
                    // `{ $expr }`
                    attrs: Vec::new(),
                    label: None,
                    block: Block {
                        brace_token: token::Brace(span),
                        stmts: Vec::from([Stmt::Expr(expr.clone(), None)]),
                    },
                }));
                f(Expr::Block(ExprBlock {
                    // `{ $expr; }`
                    attrs: Vec::new(),
                    label: None,
                    block: Block {
                        brace_token: token::Brace(span),
                        stmts: Vec::from([Stmt::Expr(expr, Some(Token![;](span)))]),
                    },
                }));
            });

            // Expr::Break
            f(Expr::Break(ExprBreak {
                // `break 'a`
                attrs: Vec::new(),
                break_token: Token![break](span),
                label: Some(Lifetime::new("'a", span)),
                expr: None,
            }));
            iter(depth, &mut |expr| {
                f(Expr::Break(ExprBreak {
                    // `break 'a $expr`
                    attrs: Vec::new(),
                    break_token: Token![break](span),
                    label: Some(Lifetime::new("'a", span)),
                    expr: Some(Box::new(expr)),
                }));
            });

            // Expr::Closure
            f(Expr::Closure(ExprClosure {
                // `|| -> T {}`
                attrs: Vec::new(),
                lifetimes: None,
                constness: None,
                movability: None,
                asyncness: None,
                capture: None,
                or1_token: Token![|](span),
                inputs: Punctuated::new(),
                or2_token: Token![|](span),
                output: ReturnType::Type(
                    Token![->](span),
                    Box::new(Type::Path(TypePath {
                        qself: None,
                        path: Path::from(Ident::new("T", span)),
                    })),
                ),
                body: Box::new(Expr::Block(ExprBlock {
                    attrs: Vec::new(),
                    label: None,
                    block: Block {
                        brace_token: token::Brace(span),
                        stmts: Vec::new(),
                    },
                })),
            }));

            // Expr::Const
            f(Expr::Const(ExprConst {
                // `const {}`
                attrs: Vec::new(),
                const_token: Token![const](span),
                block: Block {
                    brace_token: token::Brace(span),
                    stmts: Vec::new(),
                },
            }));

            // Expr::Continue
            f(Expr::Continue(ExprContinue {
                // `continue`
                attrs: Vec::new(),
                continue_token: Token![continue](span),
                label: None,
            }));
            f(Expr::Continue(ExprContinue {
                // `continue 'a`
                attrs: Vec::new(),
                continue_token: Token![continue](span),
                label: Some(Lifetime::new("'a", span)),
            }));

            // Expr::ForLoop
            iter(depth, &mut |expr| {
                f(Expr::ForLoop(ExprForLoop {
                    // `for _ in $expr {}`
                    attrs: Vec::new(),
                    label: None,
                    for_token: Token![for](span),
                    pat: Box::new(Pat::Wild(PatWild {
                        attrs: Vec::new(),
                        underscore_token: Token![_](span),
                    })),
                    in_token: Token![in](span),
                    expr: Box::new(expr.clone()),
                    body: Block {
                        brace_token: token::Brace(span),
                        stmts: Vec::new(),
                    },
                }));
                f(Expr::ForLoop(ExprForLoop {
                    // `'a: for _ in $expr {}`
                    attrs: Vec::new(),
                    label: Some(Label {
                        name: Lifetime::new("'a", span),
                        colon_token: Token![:](span),
                    }),
                    for_token: Token![for](span),
                    pat: Box::new(Pat::Wild(PatWild {
                        attrs: Vec::new(),
                        underscore_token: Token![_](span),
                    })),
                    in_token: Token![in](span),
                    expr: Box::new(expr),
                    body: Block {
                        brace_token: token::Brace(span),
                        stmts: Vec::new(),
                    },
                }));
            });

            // Expr::Index
            iter(depth, &mut |expr| {
                f(Expr::Index(ExprIndex {
                    // `$expr[0]`
                    attrs: Vec::new(),
                    expr: Box::new(expr),
                    bracket_token: token::Bracket(span),
                    index: Box::new(Expr::Lit(ExprLit {
                        attrs: Vec::new(),
                        lit: Lit::Int(LitInt::new("0", span)),
                    })),
                }));
            });

            // Expr::Loop
            f(Expr::Loop(ExprLoop {
                // `loop {}`
                attrs: Vec::new(),
                label: None,
                loop_token: Token![loop](span),
                body: Block {
                    brace_token: token::Brace(span),
                    stmts: Vec::new(),
                },
            }));
            f(Expr::Loop(ExprLoop {
                // `'a: loop {}`
                attrs: Vec::new(),
                label: Some(Label {
                    name: Lifetime::new("'a", span),
                    colon_token: Token![:](span),
                }),
                loop_token: Token![loop](span),
                body: Block {
                    brace_token: token::Brace(span),
                    stmts: Vec::new(),
                },
            }));

            // Expr::Macro
            f(Expr::Macro(ExprMacro {
                // `m!()`
                attrs: Vec::new(),
                mac: Macro {
                    path: Path::from(Ident::new("m", span)),
                    bang_token: Token![!](span),
                    delimiter: MacroDelimiter::Paren(token::Paren(span)),
                    tokens: TokenStream::new(),
                },
            }));
            f(Expr::Macro(ExprMacro {
                // `m! {}`
                attrs: Vec::new(),
                mac: Macro {
                    path: Path::from(Ident::new("m", span)),
                    bang_token: Token![!](span),
                    delimiter: MacroDelimiter::Brace(token::Brace(span)),
                    tokens: TokenStream::new(),
                },
            }));

            // Expr::Match
            iter(depth, &mut |expr| {
                f(Expr::Match(ExprMatch {
                    // `match $expr {}`
                    attrs: Vec::new(),
                    match_token: Token![match](span),
                    expr: Box::new(expr.clone()),
                    brace_token: token::Brace(span),
                    arms: Vec::new(),
                }));
                f(Expr::Match(ExprMatch {
                    // `match x { _ => $expr }`
                    attrs: Vec::new(),
                    match_token: Token![match](span),
                    expr: Box::new(Expr::Path(ExprPath {
                        attrs: Vec::new(),
                        qself: None,
                        path: Path::from(Ident::new("x", span)),
                    })),
                    brace_token: token::Brace(span),
                    arms: Vec::from([Arm {
                        attrs: Vec::new(),
                        pat: Pat::Wild(PatWild {
                            attrs: Vec::new(),
                            underscore_token: Token![_](span),
                        }),
                        guard: None,
                        fat_arrow_token: Token![=>](span),
                        body: Box::new(expr.clone()),
                        comma: None,
                    }]),
                }));
                f(Expr::Match(ExprMatch {
                    // `match x { _ if $expr => {} }`
                    attrs: Vec::new(),
                    match_token: Token![match](span),
                    expr: Box::new(Expr::Path(ExprPath {
                        attrs: Vec::new(),
                        qself: None,
                        path: Path::from(Ident::new("x", span)),
                    })),
                    brace_token: token::Brace(span),
                    arms: Vec::from([Arm {
                        attrs: Vec::new(),
                        pat: Pat::Wild(PatWild {
                            attrs: Vec::new(),
                            underscore_token: Token![_](span),
                        }),
                        guard: Some((Token![if](span), Box::new(expr))),
                        fat_arrow_token: Token![=>](span),
                        body: Box::new(Expr::Block(ExprBlock {
                            attrs: Vec::new(),
                            label: None,
                            block: Block {
                                brace_token: token::Brace(span),
                                stmts: Vec::new(),
                            },
                        })),
                        comma: None,
                    }]),
                }));
            });

            // Expr::MethodCall
            iter(depth, &mut |expr| {
                f(Expr::MethodCall(ExprMethodCall {
                    // `$expr.method()`
                    attrs: Vec::new(),
                    receiver: Box::new(expr.clone()),
                    dot_token: Token![.](span),
                    method: Ident::new("method", span),
                    turbofish: None,
                    paren_token: token::Paren(span),
                    args: Punctuated::new(),
                }));
                f(Expr::MethodCall(ExprMethodCall {
                    // `$expr.method::<T>()`
                    attrs: Vec::new(),
                    receiver: Box::new(expr),
                    dot_token: Token![.](span),
                    method: Ident::new("method", span),
                    turbofish: Some(AngleBracketedGenericArguments {
                        colon2_token: Some(Token![::](span)),
                        lt_token: Token![<](span),
                        args: Punctuated::from_iter([GenericArgument::Type(Type::Path(
                            TypePath {
                                qself: None,
                                path: Path::from(Ident::new("T", span)),
                            },
                        ))]),
                        gt_token: Token![>](span),
                    }),
                    paren_token: token::Paren(span),
                    args: Punctuated::new(),
                }));
            });

            // Expr::RawAddr
            iter(depth, &mut |expr| {
                f(Expr::RawAddr(ExprRawAddr {
                    // `&raw const $expr`
                    attrs: Vec::new(),
                    and_token: Token![&](span),
                    raw: Token![raw](span),
                    mutability: PointerMutability::Const(Token![const](span)),
                    expr: Box::new(expr),
                }));
            });

            // Expr::Struct
            f(Expr::Struct(ExprStruct {
                // `Struct {}`
                attrs: Vec::new(),
                qself: None,
                path: Path::from(Ident::new("Struct", span)),
                brace_token: token::Brace(span),
                fields: Punctuated::new(),
                dot2_token: None,
                rest: None,
            }));

            // Expr::TryBlock
            f(Expr::TryBlock(ExprTryBlock {
                // `try {}`
                attrs: Vec::new(),
                try_token: Token![try](span),
                block: Block {
                    brace_token: token::Brace(span),
                    stmts: Vec::new(),
                },
            }));

            // Expr::Unsafe
            f(Expr::Unsafe(ExprUnsafe {
                // `unsafe {}`
                attrs: Vec::new(),
                unsafe_token: Token![unsafe](span),
                block: Block {
                    brace_token: token::Brace(span),
                    stmts: Vec::new(),
                },
            }));

            // Expr::While
            iter(depth, &mut |expr| {
                f(Expr::While(ExprWhile {
                    // `while $expr {}`
                    attrs: Vec::new(),
                    label: None,
                    while_token: Token![while](span),
                    cond: Box::new(expr.clone()),
                    body: Block {
                        brace_token: token::Brace(span),
                        stmts: Vec::new(),
                    },
                }));
                f(Expr::While(ExprWhile {
                    // `'a: while $expr {}`
                    attrs: Vec::new(),
                    label: Some(Label {
                        name: Lifetime::new("'a", span),
                        colon_token: Token![:](span),
                    }),
                    while_token: Token![while](span),
                    cond: Box::new(expr),
                    body: Block {
                        brace_token: token::Brace(span),
                        stmts: Vec::new(),
                    },
                }));
            });

            // Expr::Yield
            f(Expr::Yield(ExprYield {
                // `yield`
                attrs: Vec::new(),
                yield_token: Token![yield](span),
                expr: None,
            }));
            iter(depth, &mut |expr| {
                f(Expr::Yield(ExprYield {
                    // `yield $expr`
                    attrs: Vec::new(),
                    yield_token: Token![yield](span),
                    expr: Some(Box::new(expr)),
                }));
            });
        }
    }

    let mut failures = 0;
    macro_rules! fail {
        ($($message:tt)*) => {{
            eprintln!($($message)*);
            failures += 1;
            return;
        }};
    }
    let mut assert = |mut original: Expr| {
        let tokens = original.to_token_stream();
        let Ok(mut parsed) = syn::parse2::<Expr>(tokens.clone()) else {
            fail!(
                "failed to parse: {}\n{:#?}",
                tokens,
                crate::macros::debug::Lite(&original),
            );
        };
        AsIfPrinted.visit_expr_mut(&mut original);
        FlattenParens::combine_attrs().visit_expr_mut(&mut parsed);
        if original != parsed {
            fail!(
                "before: {}\n{:#?}\nafter: {}\n{:#?}",
                tokens,
                crate::macros::debug::Lite(&original),
                parsed.to_token_stream(),
                crate::macros::debug::Lite(&parsed),
            );
        }
        let mut tokens_no_paren = tokens.clone();
        FlattenParens::visit_token_stream_mut(&mut tokens_no_paren);
        if tokens.to_string() != tokens_no_paren.to_string() {
            if let Ok(mut parsed2) = syn::parse2::<Expr>(tokens_no_paren) {
                FlattenParens::combine_attrs().visit_expr_mut(&mut parsed2);
                if original == parsed2 {
                    fail!("redundant parens: {}", tokens);
                }
            }
        }
    };

    iter(4, &mut assert);
    if failures > 0 {
        eprintln!("FAILURES: {failures}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
