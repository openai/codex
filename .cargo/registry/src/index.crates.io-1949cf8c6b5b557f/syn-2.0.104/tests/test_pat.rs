#![allow(
    clippy::elidable_lifetime_names,
    clippy::needless_lifetimes,
    clippy::uninlined_format_args
)]

#[macro_use]
mod macros;

use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
use quote::{quote, ToTokens as _};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{parse_quote, token, Item, Pat, PatTuple, Stmt, Token};

#[test]
fn test_pat_ident() {
    match Pat::parse_single.parse2(quote!(self)).unwrap() {
        Pat::Ident(_) => (),
        value => panic!("expected PatIdent, got {:?}", value),
    }
}

#[test]
fn test_pat_path() {
    match Pat::parse_single.parse2(quote!(self::CONST)).unwrap() {
        Pat::Path(_) => (),
        value => panic!("expected PatPath, got {:?}", value),
    }
}

#[test]
fn test_leading_vert() {
    // https://github.com/rust-lang/rust/blob/1.43.0/src/test/ui/or-patterns/remove-leading-vert.rs

    syn::parse_str::<Item>("fn f() {}").unwrap();
    syn::parse_str::<Item>("fn fun1(| A: E) {}").unwrap_err();
    syn::parse_str::<Item>("fn fun2(|| A: E) {}").unwrap_err();

    syn::parse_str::<Stmt>("let | () = ();").unwrap_err();
    syn::parse_str::<Stmt>("let (| A): E;").unwrap();
    syn::parse_str::<Stmt>("let (|| A): (E);").unwrap_err();
    syn::parse_str::<Stmt>("let (| A,): (E,);").unwrap();
    syn::parse_str::<Stmt>("let [| A]: [E; 1];").unwrap();
    syn::parse_str::<Stmt>("let [|| A]: [E; 1];").unwrap_err();
    syn::parse_str::<Stmt>("let TS(| A): TS;").unwrap();
    syn::parse_str::<Stmt>("let TS(|| A): TS;").unwrap_err();
    syn::parse_str::<Stmt>("let NS { f: | A }: NS;").unwrap();
    syn::parse_str::<Stmt>("let NS { f: || A }: NS;").unwrap_err();
}

#[test]
fn test_group() {
    let group = Group::new(Delimiter::None, quote!(Some(_)));
    let tokens = TokenStream::from_iter([TokenTree::Group(group)]);
    let pat = Pat::parse_single.parse2(tokens).unwrap();

    snapshot!(pat, @r#"
    Pat::TupleStruct {
        path: Path {
            segments: [
                PathSegment {
                    ident: "Some",
                },
            ],
        },
        elems: [
            Pat::Wild,
        ],
    }
    "#);
}

#[test]
fn test_ranges() {
    Pat::parse_single.parse_str("..").unwrap();
    Pat::parse_single.parse_str("..hi").unwrap();
    Pat::parse_single.parse_str("lo..").unwrap();
    Pat::parse_single.parse_str("lo..hi").unwrap();

    Pat::parse_single.parse_str("..=").unwrap_err();
    Pat::parse_single.parse_str("..=hi").unwrap();
    Pat::parse_single.parse_str("lo..=").unwrap_err();
    Pat::parse_single.parse_str("lo..=hi").unwrap();

    Pat::parse_single.parse_str("...").unwrap_err();
    Pat::parse_single.parse_str("...hi").unwrap_err();
    Pat::parse_single.parse_str("lo...").unwrap_err();
    Pat::parse_single.parse_str("lo...hi").unwrap();

    Pat::parse_single.parse_str("[lo..]").unwrap_err();
    Pat::parse_single.parse_str("[..=hi]").unwrap_err();
    Pat::parse_single.parse_str("[(lo..)]").unwrap();
    Pat::parse_single.parse_str("[(..=hi)]").unwrap();
    Pat::parse_single.parse_str("[lo..=hi]").unwrap();

    Pat::parse_single.parse_str("[_, lo.., _]").unwrap_err();
    Pat::parse_single.parse_str("[_, ..=hi, _]").unwrap_err();
    Pat::parse_single.parse_str("[_, (lo..), _]").unwrap();
    Pat::parse_single.parse_str("[_, (..=hi), _]").unwrap();
    Pat::parse_single.parse_str("[_, lo..=hi, _]").unwrap();
}

#[test]
fn test_tuple_comma() {
    let mut expr = PatTuple {
        attrs: Vec::new(),
        paren_token: token::Paren::default(),
        elems: Punctuated::new(),
    };
    snapshot!(expr.to_token_stream() as Pat, @"Pat::Tuple");

    expr.elems.push_value(parse_quote!(_));
    // Must not parse to Pat::Paren
    snapshot!(expr.to_token_stream() as Pat, @r#"
    Pat::Tuple {
        elems: [
            Pat::Wild,
            Token![,],
        ],
    }
    "#);

    expr.elems.push_punct(<Token![,]>::default());
    snapshot!(expr.to_token_stream() as Pat, @r#"
    Pat::Tuple {
        elems: [
            Pat::Wild,
            Token![,],
        ],
    }
    "#);

    expr.elems.push_value(parse_quote!(_));
    snapshot!(expr.to_token_stream() as Pat, @r#"
    Pat::Tuple {
        elems: [
            Pat::Wild,
            Token![,],
            Pat::Wild,
        ],
    }
    "#);

    expr.elems.push_punct(<Token![,]>::default());
    snapshot!(expr.to_token_stream() as Pat, @r#"
    Pat::Tuple {
        elems: [
            Pat::Wild,
            Token![,],
            Pat::Wild,
            Token![,],
        ],
    }
    "#);
}
