#![allow(
    clippy::elidable_lifetime_names,
    clippy::needless_lifetimes,
    clippy::uninlined_format_args
)]

#[macro_use]
mod macros;

use proc_macro2::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use syn::{parse_quote, Expr, Type, TypePath};

#[test]
fn parse_interpolated_leading_component() {
    // mimics the token stream corresponding to `$mod::rest`
    let tokens = TokenStream::from_iter([
        TokenTree::Group(Group::new(Delimiter::None, quote! { first })),
        TokenTree::Punct(Punct::new(':', Spacing::Joint)),
        TokenTree::Punct(Punct::new(':', Spacing::Alone)),
        TokenTree::Ident(Ident::new("rest", Span::call_site())),
    ]);

    snapshot!(tokens.clone() as Expr, @r#"
    Expr::Path {
        path: Path {
            segments: [
                PathSegment {
                    ident: "first",
                },
                Token![::],
                PathSegment {
                    ident: "rest",
                },
            ],
        },
    }
    "#);

    snapshot!(tokens as Type, @r#"
    Type::Path {
        path: Path {
            segments: [
                PathSegment {
                    ident: "first",
                },
                Token![::],
                PathSegment {
                    ident: "rest",
                },
            ],
        },
    }
    "#);
}

#[test]
fn print_incomplete_qpath() {
    // qpath with `as` token
    let mut ty: TypePath = parse_quote!(<Self as A>::Q);
    snapshot!(ty.to_token_stream(), @"TokenStream(`< Self as A > :: Q`)");
    assert!(ty.path.segments.pop().is_some());
    snapshot!(ty.to_token_stream(), @"TokenStream(`< Self as A > ::`)");
    assert!(ty.path.segments.pop().is_some());
    snapshot!(ty.to_token_stream(), @"TokenStream(`< Self >`)");
    assert!(ty.path.segments.pop().is_none());

    // qpath without `as` token
    let mut ty: TypePath = parse_quote!(<Self>::A::B);
    snapshot!(ty.to_token_stream(), @"TokenStream(`< Self > :: A :: B`)");
    assert!(ty.path.segments.pop().is_some());
    snapshot!(ty.to_token_stream(), @"TokenStream(`< Self > :: A ::`)");
    assert!(ty.path.segments.pop().is_some());
    snapshot!(ty.to_token_stream(), @"TokenStream(`< Self > ::`)");
    assert!(ty.path.segments.pop().is_none());

    // normal path
    let mut ty: TypePath = parse_quote!(Self::A::B);
    snapshot!(ty.to_token_stream(), @"TokenStream(`Self :: A :: B`)");
    assert!(ty.path.segments.pop().is_some());
    snapshot!(ty.to_token_stream(), @"TokenStream(`Self :: A ::`)");
    assert!(ty.path.segments.pop().is_some());
    snapshot!(ty.to_token_stream(), @"TokenStream(`Self ::`)");
    assert!(ty.path.segments.pop().is_some());
    snapshot!(ty.to_token_stream(), @"TokenStream(``)");
    assert!(ty.path.segments.pop().is_none());
}

#[test]
fn parse_parenthesized_path_arguments_with_disambiguator() {
    #[rustfmt::skip]
    let tokens = quote!(dyn FnOnce::() -> !);
    snapshot!(tokens as Type, @r#"
    Type::TraitObject {
        dyn_token: Some,
        bounds: [
            TypeParamBound::Trait(TraitBound {
                path: Path {
                    segments: [
                        PathSegment {
                            ident: "FnOnce",
                            arguments: PathArguments::Parenthesized {
                                output: ReturnType::Type(
                                    Type::Never,
                                ),
                            },
                        },
                    ],
                },
            }),
        ],
    }
    "#);
}
